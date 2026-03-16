use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use clankircd_config::Config;
use clankircd_server_core::ServerCore;
use clankircd_transport::TcpTransport;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "clankircd", about = "ClankyIRCd server")]
struct Cli {
    #[arg(long, default_value = "config/clankircd.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let config = Config::load(&cli.config)
        .with_context(|| format!("unable to load configuration from {}", cli.config.display()))?;

    let core = ServerCore::new();
    start_metrics_logger(
        core.clone(),
        Duration::from_secs(config.metrics_log_interval_secs),
    );

    let transport = TcpTransport::bind(config.bind_addr, core).await?;
    let bound_addr = transport.local_addr()?;
    tracing::info!(%bound_addr, "clankircd started and listening for TCP connections");

    transport.run().await
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "clankircd=info,clankircd_transport=info".into());

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
}

fn start_metrics_logger(core: ServerCore, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            let metrics = core.metrics().snapshot();
            tracing::info!(
                active_connections = metrics.active_connections,
                total_connections = metrics.total_connections,
                total_lines_received = metrics.total_lines_received,
                "server metrics snapshot"
            );
        }
    });
}
