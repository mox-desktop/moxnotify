use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tvix_serde::from_str;

/// Get the XDG config directory
pub fn xdg_config_dir() -> Result<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map_err(Into::into)
}

/// Load configuration from a Nix file
///
/// This function reads a Nix configuration file and deserializes it into the specified type.
/// If no path is provided, it will look for config files in standard locations:
/// - `$XDG_CONFIG_HOME/mox/moxnotify/default.nix`
/// - `$XDG_CONFIG_HOME/mox/moxnotify.nix`
pub fn load_config<T>(path: Option<&Path>) -> T
where
    T: for<'de> Deserialize<'de> + Default,
{
    let nix_code = if let Some(p) = path {
        match std::fs::read_to_string(p) {
            Ok(content) => content,
            Err(e) => {
                log::error!("Failed to read config file: {e}");
                return T::default();
            }
        }
    } else {
        match xdg_config_dir() {
            Ok(base) => {
                let candidates = [
                    base.join("mox/moxnotify/default.nix"),
                    base.join("mox/moxnotify.nix"),
                ];
                match candidates
                    .iter()
                    .find_map(|p| std::fs::read_to_string(p).ok())
                {
                    Some(content) => content,
                    None => {
                        log::warn!("Config file not found");
                        return T::default();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to determine config directory: {e}");
                return T::default();
            }
        }
    };

    match from_str(&nix_code) {
        Ok(config) => config,
        Err(e) => {
            log::error!("{e}");
            T::default()
        }
    }
}
