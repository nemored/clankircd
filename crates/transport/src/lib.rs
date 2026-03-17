use std::net::SocketAddr;

use anyhow::Context;
use clankircd_server_core::ServerCore;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
};

pub struct TcpTransport {
    listener: TcpListener,
    core: ServerCore,
}

impl TcpTransport {
    pub async fn bind(addr: SocketAddr, core: ServerCore) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind TCP listener on {addr}"))?;
        Ok(Self { listener, core })
    }

    pub fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            let (stream, peer_addr) = self.listener.accept().await?;
            let core = self.core.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_connection(stream, peer_addr, core).await {
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
) -> anyhow::Result<()> {
    tracing::info!(%peer_addr, "accepted TCP client");
    core.on_connection_opened();

    let result: anyhow::Result<()> = async {
        let mut lines = BufReader::new(stream).lines();
        while let Some(line) = lines.next_line().await? {
            core.on_line(&line);
        }
        Ok(())
    }
    .await;

    core.on_connection_closed();
    tracing::info!(%peer_addr, "client disconnected");
    result
}
