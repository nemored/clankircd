use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use clankircd_protocol::Message;

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

#[derive(Clone, Default)]
pub struct ServerCore {
    metrics: Arc<ServerMetrics>,
}

impl ServerCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metrics(&self) -> Arc<ServerMetrics> {
        Arc::clone(&self.metrics)
    }

    pub fn on_connection_opened(&self) {
        self.metrics.record_connection_opened();
    }

    pub fn on_connection_closed(&self) {
        self.metrics.record_connection_closed();
    }

    pub fn on_line(&self, line: &str) {
        self.metrics.record_line_received();
        match Message::parse(line) {
            Ok(msg) => {
                tracing::debug!(command = %msg.command, params = ?msg.params, "received message")
            }
            Err(err) => tracing::debug!(error = %err, line = %line, "failed to parse message"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ServerCore;

    #[test]
    fn updates_metrics() {
        let core = ServerCore::new();
        core.on_connection_opened();
        core.on_line("PING 123\r\n");
        core.on_connection_closed();

        let metrics = core.metrics().snapshot();
        assert_eq!(metrics.total_connections, 1);
        assert_eq!(metrics.active_connections, 0);
        assert_eq!(metrics.total_lines_received, 1);
    }
}
