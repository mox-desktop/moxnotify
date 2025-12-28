pub mod loader;
pub mod types;

use loader::load_config;
use serde::Deserialize;
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

fn default_log_level() -> LogLevel {
    LogLevel::default()
}

impl Config {
    pub fn load(path: Option<&std::path::Path>) -> Self {
        load_config(path)
    }
}
