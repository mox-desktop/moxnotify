use redis::AsyncTypedCommands;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::{
    sync::{Mutex, broadcast, watch},
    time,
};

const POP_EXPIRED_TIMERS_SCRIPT: &str = r#"
    local now = tonumber(ARGV[1])
    local timers = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', now)
    if #timers > 0 then
        redis.call('ZREM', KEYS[1], unpack(timers))
    end
    return timers
"#;

pub struct TimeoutScheduler {
    sender: broadcast::Sender<(u32, String)>,
    redis_con: Arc<Mutex<redis::aio::MultiplexedConnection>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TimeoutScheduler {
    pub fn new(redis_con: redis::aio::MultiplexedConnection) -> Self {
        let (sender, _) = broadcast::channel(32);
        let (global_pause, _) = watch::channel(false);
        let redis_con = Arc::new(Mutex::new(redis_con));
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let pop_script = redis::Script::new(POP_EXPIRED_TIMERS_SCRIPT);

        let timer_redis_con = Arc::clone(&redis_con);
        let timer_sender = sender.clone();
        let mut timer_pause = global_pause.subscribe();
        let timer_pop_script = pop_script.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(100));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            let mut shutdown_rx = shutdown_rx;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !*timer_pause.borrow() {
                            Self::process_expired_timers(
                                &timer_redis_con,
                                &timer_sender,
                                &timer_pop_script,
                            ).await;
                        }
                    }
                    _ = &mut shutdown_rx => {
                        log::debug!("Timer background task shutting down");
                        break;
                    }
                    _ = timer_pause.changed() => {}
                }
            }
        });

        Self {
            sender,
            redis_con,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    async fn process_expired_timers(
        redis_con: &Arc<Mutex<redis::aio::MultiplexedConnection>>,
        sender: &broadcast::Sender<(u32, String)>,
        pop_script: &redis::Script,
    ) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let mut con = redis_con.lock().await;

        let expired_timers: Vec<String> = pop_script
            .key("moxnotify:timers")
            .arg(now_ms)
            .invoke_async::<Vec<String>>(&mut *con)
            .await
            .unwrap();

        if expired_timers.is_empty() {
            return;
        }

        log::debug!("Processing {} expired timer(s)", expired_timers.len());

        for timer_id_str in expired_timers {
            if let Ok(id) = timer_id_str.parse::<u32>() {
                let timer_key = format!("moxnotify:timer:{}", id);
                let uuid: Option<String> =
                    match AsyncTypedCommands::hget(&mut *con, &timer_key, "uuid").await {
                        Ok(uuid) => uuid,
                        Err(e) => {
                            log::warn!("Failed to get UUID for timer {}: {}", id, e);
                            let _ = AsyncTypedCommands::del::<&str>(&mut *con, &timer_key).await;
                            continue;
                        }
                    };

                if let Some(uuid) = uuid {
                    let _ = AsyncTypedCommands::del::<&str>(&mut *con, &timer_key).await;

                    if let Err(e) = sender.send((id, uuid)) {
                        log::warn!("Failed to send timer expiration event: {}", e);
                    }
                } else {
                    log::warn!("Timer {} metadata missing, skipping", id);
                }
            } else {
                log::warn!("Invalid timer ID in Redis: {}", timer_id_str);
            }
        }
    }

    pub async fn start_timer(&self, id: u32, uuid: String, duration: Duration) {
        let expiration_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + duration.as_millis() as i64;

        let mut con = self.redis_con.lock().await;
        let timer_id_str = id.to_string();
        let timer_key = format!("moxnotify:timer:{}", id);

        let _: Result<usize, _> =
            AsyncTypedCommands::zrem(&mut *con, "moxnotify:timers", &timer_id_str).await;
        let _ = AsyncTypedCommands::del::<&str>(&mut *con, &timer_key).await;

        let zadd_result: Result<usize, _> =
            AsyncTypedCommands::zadd(&mut *con, "moxnotify:timers", &timer_id_str, expiration_ms)
                .await;

        if let Err(e) = zadd_result {
            log::error!("Failed to add timer {} to Redis: {}", id, e);
            return;
        }

        if let Err(e) = AsyncTypedCommands::hset_multiple::<&str, &str, String>(
            &mut *con,
            &timer_key,
            &[("id", id.to_string()), ("uuid", uuid.clone())],
        )
        .await
        {
            log::error!("Failed to store timer metadata for {}: {}", id, e);
            let _: Result<usize, _> =
                AsyncTypedCommands::zrem(&mut *con, "moxnotify:timers", &timer_id_str).await;
            return;
        }

        log::debug!(
            "Started timer for notification {} (expires at {} ms)",
            id,
            expiration_ms
        );
    }

    pub fn receiver(&self) -> broadcast::Receiver<(u32, String)> {
        self.sender.subscribe()
    }

    pub async fn stop(&self, id: u32) {
        let mut con = self.redis_con.lock().await;
        let timer_id_str = id.to_string();
        let timer_key = format!("moxnotify:timer:{}", id);

        let _: Result<usize, _> =
            AsyncTypedCommands::zrem(&mut *con, "moxnotify:timers", &timer_id_str).await;
        let _ = AsyncTypedCommands::del::<&str>(&mut *con, &timer_key).await;

        log::debug!("Stopped timer for notification {}", id);
    }
}

impl Drop for TimeoutScheduler {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}
