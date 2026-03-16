use std::{net::SocketAddr, path::Path};

use anyhow::{bail, Context};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind_addr")]
    pub bind_addr: SocketAddr,
    #[serde(default = "default_metrics_log_interval_secs")]
    pub metrics_log_interval_secs: u64,
}

fn default_bind_addr() -> SocketAddr {
    "127.0.0.1:6667"
        .parse()
        .expect("hardcoded default bind addr must parse")
}

fn default_metrics_log_interval_secs() -> u64 {
    30
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path_ref = path.as_ref();
        let raw = std::fs::read_to_string(path_ref)
            .with_context(|| format!("failed to read config file {}", path_ref.display()))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config file {}", path_ref.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.metrics_log_interval_secs == 0 {
            bail!("metrics_log_interval_secs must be >= 1");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parses_defaults() {
        let cfg: Config = toml::from_str("").expect("default config should parse");
        assert_eq!(cfg.bind_addr.to_string(), "127.0.0.1:6667");
        assert_eq!(cfg.metrics_log_interval_secs, 30);
    }

    #[test]
    fn rejects_zero_metrics_interval() {
        let cfg: Config = toml::from_str("metrics_log_interval_secs = 0").expect("parses");
        assert!(cfg.validate().is_err());
    }
}
