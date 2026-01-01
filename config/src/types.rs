use log::LevelFilter;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize, Clone, Copy)]
pub struct Timeout {
    #[serde(default = "default_urgency_low")]
    pub urgency_low: i32,
    #[serde(default = "default_urgency_normal")]
    pub urgency_normal: i32,
    #[serde(default = "default_urgency_critical")]
    pub urgency_critical: i32,
}

impl Default for Timeout {
    fn default() -> Self {
        Self {
            urgency_low: default_urgency_low(),
            urgency_normal: default_urgency_normal(),
            urgency_critical: default_urgency_critical(),
        }
    }
}

fn default_urgency_low() -> i32 {
    5
}

fn default_urgency_normal() -> i32 {
    10
}

fn default_urgency_critical() -> i32 {
    0
}

#[derive(Clone, Copy)]
pub struct LogLevel(pub LevelFilter);

impl Default for LogLevel {
    fn default() -> Self {
        Self(LevelFilter::Info)
    }
}

impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let level = match s.to_lowercase().as_str() {
            "off" => LevelFilter::Off,
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "invalid log level: {}. Valid values are: off, error, warn, info, debug, trace",
                    s
                )));
            }
        };
        Ok(LogLevel(level))
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        level.0
    }
}
