use redis::AsyncTypedCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientState {
    pub selected_id: Option<u32>,
    pub range_start: usize,
    pub range_end: usize,
    pub max_visible: usize,
    pub prev_visible_ids: Vec<u32>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            selected_id: None,
            range_start: 0,
            range_end: 0,
            max_visible: 0,
            prev_visible_ids: Vec::new(),
        }
    }
}

pub struct ClientStateManager {
    redis_con: Arc<Mutex<redis::aio::MultiplexedConnection>>,
}

impl ClientStateManager {
    pub fn new(redis_con: redis::aio::MultiplexedConnection) -> Self {
        Self {
            redis_con: Arc::new(Mutex::new(redis_con)),
        }
    }

    fn client_key(client_id: &str) -> String {
        format!("moxnotify:client:{}:state", client_id)
    }

    pub async fn load_state(&self, client_id: &str) -> ClientState {
        let mut con = self.redis_con.lock().await;
        let key = Self::client_key(client_id);

        match con.hgetall::<&str>(&key).await {
            Ok(hash_data) => {
                if hash_data.is_empty() {
                    log::debug!(
                        "No existing state found for client {}, using defaults",
                        client_id
                    );
                    return ClientState::default();
                }

                let selected_id = hash_data
                    .get("selected_id")
                    .and_then(|s| s.parse::<u32>().ok());

                let range_start = hash_data
                    .get("range_start")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                let range_end = hash_data
                    .get("range_end")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                let max_visible = hash_data
                    .get("max_visible")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                let prev_visible_ids = hash_data
                    .get("prev_visible_ids")
                    .and_then(|s| serde_json::from_str::<Vec<u32>>(s).ok())
                    .unwrap_or_default();

                log::debug!(
                    "Loaded state for client {}: selected_id={:?}, range={}..{}, max_visible={}",
                    client_id,
                    selected_id,
                    range_start,
                    range_end,
                    max_visible
                );

                ClientState {
                    selected_id,
                    range_start,
                    range_end,
                    max_visible,
                    prev_visible_ids,
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to load state for client {}: {}, using defaults",
                    client_id,
                    e
                );
                ClientState::default()
            }
        }
    }

    pub async fn save_state(&self, client_id: &str, state: &ClientState) {
        let mut con = self.redis_con.lock().await;
        let key = Self::client_key(client_id);

        // Use individual hset calls for simplicity
        let mut success = true;

        if let Some(selected_id) = state.selected_id {
            if let Err(e) = con
                .hset::<&str, &str, &str>(&key, "selected_id", &selected_id.to_string())
                .await
            {
                log::warn!("Failed to save selected_id for client {}: {}", client_id, e);
                success = false;
            }
        } else {
            // Remove selected_id if None
            let _ = con.hdel::<&str, &str>(&key, "selected_id").await;
        }

        if let Err(e) = con
            .hset::<&str, &str, &str>(&key, "range_start", &state.range_start.to_string())
            .await
        {
            log::warn!("Failed to save range_start for client {}: {}", client_id, e);
            success = false;
        }
        if let Err(e) = con
            .hset::<&str, &str, &str>(&key, "range_end", &state.range_end.to_string())
            .await
        {
            log::warn!("Failed to save range_end for client {}: {}", client_id, e);
            success = false;
        }
        if let Err(e) = con
            .hset::<&str, &str, &str>(&key, "max_visible", &state.max_visible.to_string())
            .await
        {
            log::warn!("Failed to save max_visible for client {}: {}", client_id, e);
            success = false;
        }

        let prev_visible_ids_json =
            serde_json::to_string(&state.prev_visible_ids).unwrap_or_else(|_| "[]".to_string());
        if let Err(e) = con
            .hset::<&str, &str, &str>(&key, "prev_visible_ids", &prev_visible_ids_json)
            .await
        {
            log::warn!(
                "Failed to save prev_visible_ids for client {}: {}",
                client_id,
                e
            );
            success = false;
        }

        if success {
            // Set TTL of 1 hour for client state (auto-cleanup if client disconnects)
            let _ = con.expire::<&str>(&key, 3600).await;
            log::debug!("Saved state for client {}", client_id);
        }
    }

    pub async fn delete_state(&self, client_id: &str) {
        let mut con = self.redis_con.lock().await;
        let key = Self::client_key(client_id);

        match con.del::<&str>(&key).await {
            Ok(_) => {
                log::debug!("Deleted state for client {}", client_id);
            }
            Err(e) => {
                log::warn!("Failed to delete state for client {}: {}", client_id, e);
            }
        }
    }
}
