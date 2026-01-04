pub mod client;
pub mod types;

use client::ClientConfig;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tvix_serde::from_str;
use types::{LogLevel, Timeout};

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub collector: CollectorConfig,
    #[serde(default)]
    pub control_plane: ControlPlaneConfig,
    #[serde(default)]
    pub indexer: IndexerConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub searcher: SearcherConfig,
    #[serde(default)]
    pub janitor: JanitorConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub redis: Redis,
}

fn default_redis_address() -> Box<str> {
    "redis://127.0.0.1/".into()
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Redis {
    #[serde(default = "default_redis_address")]
    pub address: Box<str>,
}

impl Default for Redis {
    fn default() -> Self {
        Self {
            address: default_redis_address(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct CollectorConfig {
    #[serde(default)]
    pub default_timeout: Timeout,
    #[serde(default = "default_control_plane_address")]
    pub control_plane_address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            default_timeout: Timeout::default(),
            control_plane_address: default_control_plane_address(),
            log_level: default_log_level(),
        }
    }
}

fn default_control_plane_address() -> String {
    "http://[::1]:64201".to_string()
}

#[derive(Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    #[serde(default = "default_scheduler_addr")]
    pub address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            address: default_scheduler_addr(),
            log_level: default_log_level(),
        }
    }
}

fn default_scheduler_addr() -> String {
    "[::1]:64202".to_string()
}

#[derive(Deserialize)]
#[serde(default)]
pub struct ControlPlaneConfig {
    #[serde(default = "default_control_plane_addr")]
    pub address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

impl Default for ControlPlaneConfig {
    fn default() -> Self {
        Self {
            address: default_control_plane_addr(),
            log_level: default_log_level(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct IndexerConfig {
    #[serde(default = "default_control_plane_address")]
    pub control_plane_address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            control_plane_address: default_control_plane_address(),
            log_level: default_log_level(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct SearcherConfig {
    #[serde(default = "default_searcher_addr")]
    pub address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

impl Default for SearcherConfig {
    fn default() -> Self {
        Self {
            address: default_searcher_addr(),
            log_level: default_log_level(),
        }
    }
}

fn default_searcher_addr() -> String {
    "0.0.0.0:64203".to_string()
}

fn default_control_plane_addr() -> String {
    "[::1]:64201".to_string()
}

#[derive(Deserialize)]
#[serde(default)]
pub struct JanitorConfig {
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub retention: Retention,
}

impl Default for JanitorConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            retention: Retention::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Retention {
    #[serde(
        default = "default_retention_period",
        deserialize_with = "deserialize_duration"
    )]
    pub period: Duration,
    #[serde(
        default = "default_retention_schedule",
        deserialize_with = "deserialize_duration"
    )]
    pub schedule: Duration,
}

fn default_retention_period() -> Duration {
    Duration::from_secs(90 * 86400) // 90 days
}

fn default_retention_schedule() -> Duration {
    Duration::from_secs(86400) // daily
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    let s = s.trim().to_lowercase();

    // Support common aliases
    let duration = match s.as_str() {
        "hourly" => Duration::from_secs(3600),
        "daily" => Duration::from_secs(86400),
        "weekly" => Duration::from_secs(604800),
        "monthly" => Duration::from_secs(2592000), // 30 days
        _ => humantime::parse_duration(&s)
            .map_err(|e| serde::de::Error::custom(format!("invalid duration '{}': {}", s, e)))?,
    };
    Ok(duration)
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            period: default_retention_period(),
            schedule: default_retention_schedule(),
        }
    }
}

fn default_log_level() -> LogLevel {
    LogLevel::default()
}

pub fn xdg_config_dir() -> anyhow::Result<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map_err(Into::into)
}

impl Config {
    pub fn load(path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        let nix_code = if let Some(p) = path {
            std::fs::read_to_string(p)?
        } else {
            let xdg = xdg_config_dir()?;
            let candidates = [
                xdg.join("mox/moxnotify/default.nix"),
                xdg.join("mox/moxnotify.nix"),
            ];
            match candidates
                .iter()
                .find_map(|p| std::fs::read_to_string(p).ok())
            {
                Some(content) => content,
                None => {
                    log::warn!("Config file not found");
                    return Ok(Self::default());
                }
            }
        };

        from_str(&nix_code).map_err(|e| anyhow::anyhow!("{e}"))
    }
}
