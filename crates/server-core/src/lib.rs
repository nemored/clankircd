use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use clankircd_protocol::Message;

pub type ConnectionId = usize;

#[derive(Debug, Default)]
pub struct ServerMetrics {
    active_connections: AtomicUsize,
    total_connections: AtomicUsize,
    total_lines_received: AtomicUsize,
}

impl ServerMetrics {
    pub fn record_connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_line_received(&self) {
        self.total_lines_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            total_lines_received: self.total_lines_received.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub active_connections: usize,
    pub total_connections: usize,
    pub total_lines_received: usize,
}

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub target: ConnectionId,
    pub line: String,
}

#[derive(Clone)]
pub struct ServerCore {
    metrics: Arc<ServerMetrics>,
    state: Arc<Mutex<ServerState>>,
}

#[derive(Debug)]
struct ServerState {
    server_name: String,
    next_connection_id: ConnectionId,
    clients: HashMap<ConnectionId, ClientSession>,
    channels: HashMap<String, HashSet<ConnectionId>>,
}

#[derive(Debug, Default)]
struct ClientSession {
    nick: Option<String>,
    user: Option<String>,
    real_name: Option<String>,
    cap_active: bool,
    registered: bool,
    joined_channels: HashSet<String>,
}

impl Default for ServerCore {
    fn default() -> Self {
        Self {
            metrics: Arc::new(ServerMetrics::default()),
            state: Arc::new(Mutex::new(ServerState {
                server_name: "clankircd.local".to_string(),
                next_connection_id: 1,
                clients: HashMap::new(),
                channels: HashMap::new(),
            })),
        }
    }
}

impl ServerCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metrics(&self) -> Arc<ServerMetrics> {
        Arc::clone(&self.metrics)
    }

    pub fn on_connection_opened(&self) -> ConnectionId {
        self.metrics.record_connection_opened();
        let mut state = self.state.lock().expect("state lock");
        let connection_id = state.next_connection_id;
        state.next_connection_id += 1;
        state
            .clients
            .insert(connection_id, ClientSession::default());
        connection_id
    }

    pub fn on_connection_closed(&self, connection_id: ConnectionId) -> Vec<OutboundMessage> {
        self.metrics.record_connection_closed();
        self.handle_quit(connection_id, "Client disconnected")
    }

    pub fn on_line(&self, connection_id: ConnectionId, line: &str) -> Vec<OutboundMessage> {
        self.metrics.record_line_received();
        let msg = match Message::parse(line) {
            Ok(msg) => msg,
            Err(err) => {
                tracing::debug!(error = %err, line = %line, "failed to parse message");
                return vec![];
            }
        };

        tracing::debug!(command = %msg.command, params = ?msg.params, "received message");

        let mut state = self.state.lock().expect("state lock");
        match msg.command.as_str() {
            "CAP" => handle_cap(&mut state, connection_id, &msg.params),
            "PASS" => vec![],
            "NICK" => handle_nick(&mut state, connection_id, &msg.params),
            "USER" => handle_user(&mut state, connection_id, &msg.params),
            "JOIN" => handle_join(&mut state, connection_id, &msg.params),
            "PART" => handle_part(&mut state, connection_id, &msg.params),
            "PRIVMSG" => handle_privmsg(&mut state, connection_id, &msg.params, false),
            "NOTICE" => handle_privmsg(&mut state, connection_id, &msg.params, true),
            "QUIT" => {
                let reason = msg
                    .params
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("Client quit");
                drop(state);
                self.handle_quit(connection_id, reason)
            }
            "PING" => vec![reply(
                connection_id,
                Message {
                    prefix: None,
                    command: "PONG".to_string(),
                    params: msg.params,
                },
            )],
            _ => vec![numeric(
                &state.server_name,
                connection_id,
                &client_nick(&state, connection_id),
                "421",
                vec![msg.command, "Unknown command".to_string()],
            )],
        }
    }

    fn handle_quit(&self, connection_id: ConnectionId, reason: &str) -> Vec<OutboundMessage> {
        let mut state = self.state.lock().expect("state lock");
        let Some(client) = state.clients.remove(&connection_id) else {
            return vec![];
        };

        let mut outbound = Vec::new();
        let quit_prefix = client_prefix(&client);
        let quit_message = Message {
            prefix: Some(quit_prefix),
            command: "QUIT".to_string(),
            params: vec![reason.to_string()],
        }
        .serialize();

        for channel in client.joined_channels {
            if let Some(members) = state.channels.get_mut(&channel) {
                members.remove(&connection_id);
                for member_id in members.iter().copied() {
                    outbound.push(OutboundMessage {
                        target: member_id,
                        line: quit_message.clone(),
                    });
                }
                if members.is_empty() {
                    state.channels.remove(&channel);
                }
            }
        }

        outbound
    }
}

fn handle_cap(
    state: &mut ServerState,
    connection_id: ConnectionId,
    params: &[String],
) -> Vec<OutboundMessage> {
    let Some(subcommand) = params.first() else {
        return vec![];
    };

    match subcommand.to_ascii_uppercase().as_str() {
        "LS" => vec![reply(
            connection_id,
            Message {
                prefix: Some(state.server_name.clone()),
                command: "CAP".to_string(),
                params: vec![
                    client_nick(state, connection_id),
                    "LS".to_string(),
                    "multi-prefix".to_string(),
                ],
            },
        )],
        "END" => {
            if let Some(client) = state.clients.get_mut(&connection_id) {
                client.cap_active = false;
            }
            maybe_complete_registration(state, connection_id)
        }
        _ => vec![],
    }
}

fn handle_nick(
    state: &mut ServerState,
    connection_id: ConnectionId,
    params: &[String],
) -> Vec<OutboundMessage> {
    let Some(nick) = params.first() else {
        return vec![numeric(
            &state.server_name,
            connection_id,
            &client_nick(state, connection_id),
            "431",
            vec!["No nickname given".to_string()],
        )];
    };

    if state
        .clients
        .iter()
        .any(|(id, c)| *id != connection_id && c.nick.as_deref() == Some(nick.as_str()))
    {
        return vec![numeric(
            &state.server_name,
            connection_id,
            &client_nick(state, connection_id),
            "433",
            vec![nick.clone(), "Nickname is already in use".to_string()],
        )];
    }

    let mut outbound = Vec::new();
    let old_prefix = state
        .clients
        .get(&connection_id)
        .and_then(|c| c.nick.as_ref())
        .map(|_| {
            let c = state.clients.get(&connection_id).expect("client exists");
            client_prefix(c)
        });

    if let Some(client) = state.clients.get_mut(&connection_id) {
        client.nick = Some(nick.clone());
    }

    if let Some(old_prefix) = old_prefix {
        let new_nick_msg = Message {
            prefix: Some(old_prefix),
            command: "NICK".to_string(),
            params: vec![nick.clone()],
        }
        .serialize();
        for target in collect_visible_peers(state, connection_id) {
            outbound.push(OutboundMessage {
                target,
                line: new_nick_msg.clone(),
            });
        }
    }

    outbound.extend(maybe_complete_registration(state, connection_id));
    outbound
}

fn handle_user(
    state: &mut ServerState,
    connection_id: ConnectionId,
    params: &[String],
) -> Vec<OutboundMessage> {
    if params.len() < 4 {
        return vec![numeric(
            &state.server_name,
            connection_id,
            &client_nick(state, connection_id),
            "461",
            vec!["USER".to_string(), "Not enough parameters".to_string()],
        )];
    }

    if let Some(client) = state.clients.get_mut(&connection_id) {
        client.user = Some(params[0].clone());
        client.real_name = Some(params[3].clone());
    }

    maybe_complete_registration(state, connection_id)
}

fn maybe_complete_registration(
    state: &mut ServerState,
    connection_id: ConnectionId,
) -> Vec<OutboundMessage> {
    let Some(client) = state.clients.get_mut(&connection_id) else {
        return vec![];
    };
    if client.registered || client.nick.is_none() || client.user.is_none() || client.cap_active {
        return vec![];
    }

    client.registered = true;
    let nick = client.nick.clone().unwrap_or_else(|| "*".to_string());
    vec![
        numeric(
            &state.server_name,
            connection_id,
            &nick,
            "001",
            vec![format!("Welcome to ClankyIRCd, {nick}")],
        ),
        numeric(
            &state.server_name,
            connection_id,
            &nick,
            "002",
            vec![format!("Your host is {}", state.server_name)],
        ),
        numeric(
            &state.server_name,
            connection_id,
            &nick,
            "003",
            vec!["This server was created today".to_string()],
        ),
        numeric(
            &state.server_name,
            connection_id,
            &nick,
            "004",
            vec![
                state.server_name.clone(),
                "clankircd-0.1".to_string(),
                "ao".to_string(),
                "mtov".to_string(),
            ],
        ),
    ]
}

fn handle_join(
    state: &mut ServerState,
    connection_id: ConnectionId,
    params: &[String],
) -> Vec<OutboundMessage> {
    if !is_registered(state, connection_id) {
        return vec![numeric(
            &state.server_name,
            connection_id,
            &client_nick(state, connection_id),
            "451",
            vec!["You have not registered".to_string()],
        )];
    }

    let Some(channels_raw) = params.first() else {
        return vec![];
    };

    let mut outbound = Vec::new();
    for channel in channels_raw.split(',').filter(|c| c.starts_with('#')) {
        let prefix = {
            let client = state.clients.get(&connection_id).expect("client exists");
            client_prefix(client)
        };

        let members = state.channels.entry(channel.to_string()).or_default();
        members.insert(connection_id);

        if let Some(client) = state.clients.get_mut(&connection_id) {
            client.joined_channels.insert(channel.to_string());
        }

        let join_line = Message {
            prefix: Some(prefix),
            command: "JOIN".to_string(),
            params: vec![channel.to_string()],
        }
        .serialize();

        for member_id in members.iter().copied() {
            outbound.push(OutboundMessage {
                target: member_id,
                line: join_line.clone(),
            });
        }

        let names = members
            .iter()
            .filter_map(|member_id| state.clients.get(member_id).and_then(|c| c.nick.clone()))
            .collect::<Vec<_>>()
            .join(" ");

        outbound.push(numeric(
            &state.server_name,
            connection_id,
            &client_nick(state, connection_id),
            "353",
            vec!["=".to_string(), channel.to_string(), names],
        ));
        outbound.push(numeric(
            &state.server_name,
            connection_id,
            &client_nick(state, connection_id),
            "366",
            vec![channel.to_string(), "End of /NAMES list".to_string()],
        ));
    }

    outbound
}

fn handle_part(
    state: &mut ServerState,
    connection_id: ConnectionId,
    params: &[String],
) -> Vec<OutboundMessage> {
    let Some(channel) = params.first() else {
        return vec![];
    };
    let mut outbound = Vec::new();

    if let Some(members) = state.channels.get_mut(channel) {
        let prefix = {
            let client = state.clients.get(&connection_id).expect("client exists");
            client_prefix(client)
        };
        let part_line = Message {
            prefix: Some(prefix),
            command: "PART".to_string(),
            params: vec![channel.clone()],
        }
        .serialize();

        for member_id in members.iter().copied() {
            outbound.push(OutboundMessage {
                target: member_id,
                line: part_line.clone(),
            });
        }

        members.remove(&connection_id);
        if members.is_empty() {
            state.channels.remove(channel);
        }
    }

    if let Some(client) = state.clients.get_mut(&connection_id) {
        client.joined_channels.remove(channel);
    }

    outbound
}

fn handle_privmsg(
    state: &mut ServerState,
    connection_id: ConnectionId,
    params: &[String],
    is_notice: bool,
) -> Vec<OutboundMessage> {
    if params.len() < 2 {
        return vec![];
    }

    let target = &params[0];
    let body = params[1].clone();
    let command = if is_notice { "NOTICE" } else { "PRIVMSG" };
    let prefix = {
        let client = state.clients.get(&connection_id).expect("client exists");
        client_prefix(client)
    };

    let line = Message {
        prefix: Some(prefix),
        command: command.to_string(),
        params: vec![target.clone(), body],
    }
    .serialize();

    if target.starts_with('#') {
        let recipients = state
            .channels
            .get(target)
            .into_iter()
            .flat_map(|members| members.iter().copied())
            .filter(|id| *id != connection_id)
            .collect::<Vec<_>>();

        recipients
            .into_iter()
            .map(|target| OutboundMessage {
                target,
                line: line.clone(),
            })
            .collect()
    } else {
        state
            .clients
            .iter()
            .find(|(_, client)| client.nick.as_deref() == Some(target.as_str()))
            .map(|(target_id, _)| {
                vec![OutboundMessage {
                    target: *target_id,
                    line,
                }]
            })
            .unwrap_or_default()
    }
}

fn reply(target: ConnectionId, message: Message) -> OutboundMessage {
    OutboundMessage {
        target,
        line: message.serialize(),
    }
}

fn numeric(
    server_name: &str,
    target: ConnectionId,
    nick: &str,
    code: &str,
    mut params: Vec<String>,
) -> OutboundMessage {
    let mut full_params = vec![nick.to_string()];
    full_params.append(&mut params);
    reply(
        target,
        Message {
            prefix: Some(server_name.to_string()),
            command: code.to_string(),
            params: full_params,
        },
    )
}

fn client_nick(state: &ServerState, connection_id: ConnectionId) -> String {
    state
        .clients
        .get(&connection_id)
        .and_then(|client| client.nick.clone())
        .unwrap_or_else(|| "*".to_string())
}

fn client_prefix(client: &ClientSession) -> String {
    let nick = client.nick.clone().unwrap_or_else(|| "*".to_string());
    let user = client.user.clone().unwrap_or_else(|| "unknown".to_string());
    format!("{nick}!{user}@localhost")
}

fn collect_visible_peers(
    state: &ServerState,
    connection_id: ConnectionId,
) -> HashSet<ConnectionId> {
    let mut peers = HashSet::new();
    let Some(client) = state.clients.get(&connection_id) else {
        return peers;
    };

    for channel in &client.joined_channels {
        if let Some(members) = state.channels.get(channel) {
            peers.extend(members.iter().copied());
        }
    }

    peers.remove(&connection_id);
    peers
}

fn is_registered(state: &ServerState, connection_id: ConnectionId) -> bool {
    state
        .clients
        .get(&connection_id)
        .map(|client| client.registered)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::ServerCore;

    #[test]
    fn updates_metrics() {
        let core = ServerCore::new();
        let id = core.on_connection_opened();
        core.on_line(id, "PING 123\r\n");
        core.on_connection_closed(id);

        let metrics = core.metrics().snapshot();
        assert_eq!(metrics.total_connections, 1);
        assert_eq!(metrics.active_connections, 0);
        assert_eq!(metrics.total_lines_received, 1);
    }

    #[test]
    fn completes_registration_after_nick_and_user() {
        let core = ServerCore::new();
        let id = core.on_connection_opened();
        assert!(core.on_line(id, "NICK test").is_empty());
        let messages = core.on_line(id, "USER u 0 * :Real Name");
        assert!(messages.iter().any(|m| m.line.contains(" 001 test ")));
    }

    #[test]
    fn relays_privmsg_to_channel_members() {
        let core = ServerCore::new();
        let a = core.on_connection_opened();
        let b = core.on_connection_opened();

        core.on_line(a, "NICK a");
        core.on_line(a, "USER a 0 * :a");
        core.on_line(b, "NICK b");
        core.on_line(b, "USER b 0 * :b");
        core.on_line(a, "JOIN #test");
        core.on_line(b, "JOIN #test");

        let relay = core.on_line(a, "PRIVMSG #test :hello");
        assert!(relay
            .iter()
            .any(|m| m.target == b && m.line.contains("PRIVMSG #test hello")));
    }
}
