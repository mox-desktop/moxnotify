pub mod loader;
pub mod types;

use loader::load_config;
use serde::Deserialize;
use types::{LogLevel, Timeout};

#[derive(Deserialize, Default)]
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
    #[serde(default = "default_redis_address")]
    pub redis_address: Box<str>,
}

fn default_redis_address() -> Box<str> {
    "redis://127.0.0.1/".into()
}

#[derive(Deserialize, Default)]
pub struct CollectorConfig {
    #[serde(default)]
    pub default_timeout: Timeout,
    #[serde(default = "default_control_plane_address")]
    pub control_plane_address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

fn default_control_plane_address() -> String {
    "http://[::1]:64201".to_string()
}

#[derive(Deserialize, Default)]
pub struct SchedulerConfig {
    #[serde(default = "default_scheduler_addr")]
    pub address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

fn default_scheduler_addr() -> String {
    "[::1]:64202".to_string()
}

#[derive(Deserialize, Default)]
pub struct ControlPlaneConfig {
    #[serde(default = "default_control_plane_addr")]
    pub address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

#[derive(Deserialize, Default)]
pub struct IndexerConfig {
    #[serde(default = "default_control_plane_address")]
    pub control_plane_address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

#[derive(Deserialize, Default)]
pub struct SearcherConfig {
    #[serde(default = "default_searcher_addr")]
    pub address: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
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
