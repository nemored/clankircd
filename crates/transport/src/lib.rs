use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::Context;
use clankircd_server_core::{ConnectionId, OutboundMessage, ServerCore};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};

pub struct TcpTransport {
    listener: TcpListener,
    core: ServerCore,
    sessions: Arc<Mutex<HashMap<ConnectionId, mpsc::Sender<String>>>>,
}

impl TcpTransport {
    pub async fn bind(addr: SocketAddr, core: ServerCore) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind TCP listener on {addr}"))?;
        Ok(Self {
            listener,
            core,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let core = self.core.clone();
            let sessions = Arc::clone(&self.sessions);
            tokio::spawn(async move {
                if let Err(err) = handle_connection(stream, peer_addr, core, sessions).await {
                    tracing::warn!(%peer_addr, error = %err, "connection ended with error");
                }
            });
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    core: ServerCore,
    sessions: Arc<Mutex<HashMap<ConnectionId, mpsc::Sender<String>>>>,
) -> anyhow::Result<()> {
    tracing::info!(%peer_addr, "accepted TCP client");
    let connection_id = core.on_connection_opened();

    let (read_half, mut write_half) = stream.into_split();
    let (tx, mut rx) = mpsc::channel::<String>(128);

    sessions.lock().await.insert(connection_id, tx);

    let writer = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if write_half.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    let result: anyhow::Result<()> = async {
        let mut lines = BufReader::new(read_half).lines();
        while let Some(line) = lines.next_line().await? {
            let outbound = core.on_line(connection_id, &line);
            dispatch_outbound(&sessions, outbound).await;
        }
        Ok(())
    }
    .await;

    let quit_messages = core.on_connection_closed(connection_id);
    dispatch_outbound(&sessions, quit_messages).await;
    sessions.lock().await.remove(&connection_id);

    writer.abort();
    tracing::info!(%peer_addr, "client disconnected");
    result
}

async fn dispatch_outbound(
    sessions: &Arc<Mutex<HashMap<ConnectionId, mpsc::Sender<String>>>>,
    messages: Vec<OutboundMessage>,
) {
    for outbound in messages {
        let sender = {
            let guard = sessions.lock().await;
            guard.get(&outbound.target).cloned()
        };

        if let Some(sender) = sender {
            let _ = sender.send(outbound.line).await;
        }
    }
}
