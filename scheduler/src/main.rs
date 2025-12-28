pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod client {
        tonic::include_proto!("moxnotify.client");
    }
}

mod timeout_scheduler;
mod view_range;

use crate::moxnotify::client::notification_message;
use crate::timeout_scheduler::TimeoutScheduler;
use moxnotify::client::client_service_server::{ClientService, ClientServiceServer};
use moxnotify::client::viewport_navigation_request::Direction;
use moxnotify::client::{
    ClientActionInvokedRequest, ClientActionInvokedResponse, ClientNotificationClosedRequest,
    ClientNotificationClosedResponse, ClientNotifyRequest, GetViewportRequest, NotificationMessage,
    StartTimersRequest, StartTimersResponse, StopTimersRequest, StopTimersResponse,
    ViewportNavigationRequest, ViewportNavigationResponse,
};
use moxnotify::types::{CloseNotification, CloseReason, NewNotification, NotificationClosed};
use redis::TypedCommands;
use redis::streams::StreamReadOptions;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};
use view_range::ViewRange;

#[derive(Clone)]
struct Scheduler {
    timeouts: Arc<TimeoutScheduler>,
    notification_broadcast: Arc<broadcast::Sender<NewNotification>>,
    close_notification_broadcast: Arc<broadcast::Sender<CloseNotification>>,
    redis_con: Arc<Mutex<redis::Connection>>,
    selected_id: Arc<Mutex<Option<u32>>>,
    max_visible: Arc<AtomicUsize>,
    range: Arc<Mutex<ViewRange>>,
    prev_visible_ids: Arc<Mutex<Vec<u32>>>,
}

impl Scheduler {
    fn new(redis_con: redis::Connection) -> Self {
        let (tx, _) = broadcast::channel(128);
        let (close_tx, _) = broadcast::channel(128);

        Self {
            timeouts: Arc::new(TimeoutScheduler::new()),
            notification_broadcast: Arc::new(tx),
            close_notification_broadcast: Arc::new(close_tx),
            redis_con: Arc::new(Mutex::new(redis_con)),
            selected_id: Arc::new(Mutex::new(None)),
            max_visible: Arc::new(AtomicUsize::new(0)),
            range: Arc::new(Mutex::new(ViewRange::default())),
            prev_visible_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_active_notifications(&self) -> HashMap<u32, NewNotification> {
        let mut con = self.redis_con.lock().await;

        let hash_data: HashMap<String, String> = con.hgetall("moxnotify:active").unwrap();

        let mut active_notifications = HashMap::new();
        for (id_str, json) in hash_data {
            if let Ok(id) = id_str.parse::<u32>() {
                if let Ok(notification) = serde_json::from_str::<NewNotification>(&json) {
                    active_notifications.insert(id, notification);
                } else {
                    log::warn!(
                        "Failed to parse notification JSON for id {}: {}",
                        id_str,
                        json
                    );
                }
            } else {
                log::warn!("Failed to parse notification ID: {}", id_str);
            }
        }

        active_notifications
    }

    async fn start_timers_for_newly_visible(
        &self,
        notifications: &[&NewNotification],
        current_visible_ids: &[u32],
    ) {
        let prev_visible = self.prev_visible_ids.lock().await;
        let newly_visible: Vec<u32> = current_visible_ids
            .iter()
            .filter(|id| !prev_visible.contains(id))
            .copied()
            .collect();
        drop(prev_visible);

        if newly_visible.is_empty() {
            return;
        }

        let timeouts = Arc::clone(&self.timeouts);
        for notification in notifications.iter() {
            if !newly_visible.contains(&notification.id) {
                continue;
            }

            let timeout_ms = notification.timeout;
            // Timeout == 0 means that notification never expires
            // Timeout == -1 means that timeout should be chosen by notifications server
            // but we handle it in collectors
            if timeout_ms > 0 {
                let duration = std::time::Duration::from_millis(timeout_ms as u64);
                timeouts
                    .start_timer(notification.id, notification.uuid.clone(), duration)
                    .await;
            }
        }

        let mut prev_visible = self.prev_visible_ids.lock().await;
        *prev_visible = current_visible_ids.to_vec();
    }
}

#[tonic::async_trait]
impl ClientService for Scheduler {
    type NotifyStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<NotificationMessage, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn notify(
        &self,
        request: Request<ClientNotifyRequest>,
    ) -> Result<Response<Self::NotifyStream>, Status> {
        let remote_addr = request.remote_addr().unwrap();
        let req = request.into_inner();

        log::info!("New client connection from: {:?}", remote_addr);

        let mut notification_rx = self.notification_broadcast.subscribe();
        let mut close_notification_rx = self.close_notification_broadcast.subscribe();
        let (tx, stream_rx) = mpsc::channel(128);

        self.max_visible
            .store(req.max_visible as usize, Ordering::Relaxed);
        {
            let mut range = self.range.lock().await;
            *range = ViewRange::new(req.max_visible as usize);
        }

        let active_notifications = self.get_active_notifications().await;

        let notifications = {
            let mut notifications: Vec<NewNotification> =
                active_notifications.into_values().collect::<Vec<_>>();
            notifications.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            Arc::new(notifications)
        };

        {
            let tx = tx.clone();
            let range = Arc::clone(&self.range);
            let scheduler = self.clone();

            let redis_con = Arc::clone(&self.redis_con);
            let timeouts = Arc::clone(&self.timeouts);
            tokio::spawn(async move {
                let mut receiver = timeouts.receiver();
                let redis_con = redis_con;

                loop {
                    tokio::select! {
                        notification = notification_rx.recv() => {
                            match notification {
                                Ok(notification) => {
                                    let active_count = {
                                        let mut redis_con = redis_con.lock().await;
                                        let hash_data: HashMap<String, String> = redis_con.hgetall("moxnotify:active").unwrap_or_default();
                                        hash_data.len()
                                    };

                                    let mut range = range.lock().await;
                                    range.show_tail(active_count);
                                    log::debug!("notify, range: {}", range);

                                    let active_notifications = scheduler.get_active_notifications().await;
                                    let mut notifications_vec: Vec<&NewNotification> = active_notifications.values().collect();
                                    notifications_vec.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

                                    let focused_ids: Vec<u32> = notifications_vec
                                        .iter()
                                        .skip(range.start())
                                        .take(range.width())
                                        .map(|n| n.id)
                                        .collect();

                                    drop(range);
                                    scheduler.start_timers_for_newly_visible(
                                        &notifications_vec,
                                        &focused_ids,
                                    ).await;

                                    let message = NotificationMessage {
                                        message: Some(notification_message::Message::Notification(notification))
                                    };

                                    if tx.send(Ok(message)).await.is_err() {
                                        log::info!("Client disconnected: {:?}", remote_addr);
                                        break;
                                    }
                                }
                                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                    log::warn!(
                                        "Client {:?} lagged, skipped {} notification messages",
                                        remote_addr,
                                        skipped
                                    );
                                }
                                Err(broadcast::error::RecvError::Closed) => {
                                    log::error!(
                                        "Notification broadcast channel closed for client: {:?}",
                                        remote_addr
                                    );
                                    break;
                                }
                            }
                        }
                        close_notification = close_notification_rx.recv() => {
                            match close_notification {
                                Ok(close_notification) => {
                                    let message = NotificationMessage {
                                        message: Some(notification_message::Message::CloseNotification(close_notification))
                                    };

                                    if tx.send(Ok(message)).await.is_err() {
                                        log::info!("Client disconnected: {:?}", remote_addr);
                                        break;
                                    }

                                    let mut redis_con = redis_con.lock().await;
                                    let hash_data: HashMap<String, String> = redis_con.hgetall("moxnotify:active").unwrap_or_default();
                                    let remaining_count = hash_data.len();

                                    let mut range = range.lock().await;
                                    range.show_tail(remaining_count);
                                    drop(redis_con);

                                    let active_notifications = scheduler.get_active_notifications().await;
                                    let mut notifications_vec: Vec<&NewNotification> = active_notifications.values().collect();
                                    notifications_vec.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

                                    let focused_ids: Vec<u32> = notifications_vec
                                        .iter()
                                        .skip(range.start())
                                        .take(range.width())
                                        .map(|n| n.id)
                                        .collect();

                                    drop(range);
                                    scheduler.start_timers_for_newly_visible(
                                        &notifications_vec,
                                        &focused_ids,
                                    ).await;
                                }
                                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                    log::warn!(
                                        "Client {:?} lagged, skipped {} close_notification messages",
                                        remote_addr,
                                        skipped
                                    );
                                }
                                Err(broadcast::error::RecvError::Closed) => {
                                    log::error!(
                                        "CloseNotification broadcast channel closed for client: {:?}",
                                        remote_addr
                                    );
                                    break;
                                }
                            }
                        }
                        Ok((id, uuid)) = receiver.recv() => {
                            let message = NotificationMessage {
                                message: Some(notification_message::Message::CloseNotification(CloseNotification { id }))
                            };

                            log::debug!("Notification {id} expired");

                            if tx.send(Ok(message)).await.is_err() {
                                log::info!("Client disconnected: {:?}", remote_addr);
                                break;
                            }

                            let mut redis_con = redis_con.lock().await;

                            let closed = NotificationClosed {
                                id,
                                reason: CloseReason::ReasonExpired as i32,
                                uuid,
                            };

                            let json = serde_json::to_string(&closed).unwrap();
                            if let Err(e) = redis_con.xadd(
                                "moxnotify:notification_closed",
                                "*",
                                &[("notification", json.as_str())],
                            ) {
                                log::error!("Failed to write notification_closed to Redis: {}", e);
                            }

                            if let Err(e) = redis_con.hdel("moxnotify:active", id.to_string().as_str()) {
                                log::warn!("Failed to remove notification from active HASH: {}", e);
                            }

                            let hash_data: HashMap<String, String> = redis_con.hgetall("moxnotify:active").unwrap_or_default();
                            let remaining_count = hash_data.len();

                            let mut range = range.lock().await;
                            range.show_tail(remaining_count);
                            log::debug!("Notification {id} expired, range: {}", range);
                            drop(redis_con);

                            let active_notifications = scheduler.get_active_notifications().await;
                            let mut notifications_vec: Vec<&NewNotification> = active_notifications.values().collect();
                            notifications_vec.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

                            let focused_ids: Vec<u32> = notifications_vec
                                .iter()
                                .skip(range.start())
                                .take(range.width())
                                .map(|n| n.id)
                                .collect();

                            drop(range);
                            scheduler.start_timers_for_newly_visible(
                                &notifications_vec,
                                &focused_ids,
                            ).await;
                        }
                    }
                }
            });
        }

        {
            let mut range = self.range.lock().await;
            range.show_tail(notifications.len());
        }

        for notification in notifications.iter().rev() {
            let message = NotificationMessage {
                message: Some(notification_message::Message::Notification(
                    notification.to_owned(),
                )),
            };

            if tx.send(Ok(message)).await.is_err() {
                log::info!("Client disconnected during initial sync: {:?}", remote_addr);
                break;
            }
        }

        let output_stream: Self::NotifyStream = Box::pin(ReceiverStream::new(stream_rx));
        Ok(Response::new(output_stream))
    }

    async fn notification_closed(
        &self,
        request: Request<ClientNotificationClosedRequest>,
    ) -> Result<Response<ClientNotificationClosedResponse>, Status> {
        let closed = request.into_inner().notification_closed.unwrap();
        log::info!(
            "Received notification_closed request: id: {}, reason: {:?}",
            closed.id,
            closed.reason()
        );

        let active_notifications = self.get_active_notifications().await;

        let mut notifications: Vec<&NewNotification> = active_notifications.values().collect();
        notifications.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        {
            let mut selected_id = self.selected_id.lock().await;

            if let Some(selected) = *selected_id
                && selected == closed.id
                && let Some(pos) = notifications.iter().position(|n| n.id == selected)
            {
                let new_selected = pos
                    .checked_sub(1)
                    .or_else(|| pos.checked_add(1).filter(|&i| i < notifications.len()))
                    .and_then(|idx| notifications.get(idx).map(|n| n.id));

                *selected_id = new_selected;
            }
        }

        let mut con = self.redis_con.lock().await;
        let json = serde_json::to_string(&closed).unwrap();
        if let Err(e) = con.xadd(
            "moxnotify:notification_closed",
            "*",
            &[("notification", json.as_str())],
        ) {
            log::error!("Failed to write notification_closed to Redis: {}", e);
        }

        let id_str = closed.id.to_string();
        if let Err(e) = con.hdel("moxnotify:active", id_str.as_str()) {
            log::warn!("Failed to remove notification from active HASH: {}", e);
        }

        let mut range = self.range.lock().await;
        range.scroll_down_clamped(notifications.len());

        let focused_ids: Vec<u32> = notifications
            .iter()
            .skip(range.start())
            .take(range.width())
            .map(|n| n.id)
            .collect();

        log::debug!("notification_closed, range: {}", range);

        drop(range);
        self.start_timers_for_newly_visible(&notifications, &focused_ids)
            .await;

        Ok(Response::new(ClientNotificationClosedResponse {}))
    }

    async fn action_invoked(
        &self,
        request: Request<ClientActionInvokedRequest>,
    ) -> Result<Response<ClientActionInvokedResponse>, Status> {
        let invoked = request.into_inner().action_invoked.unwrap();
        log::info!(
            "Received action_invoked request: id: {}, key: {}",
            invoked.id,
            invoked.action_key
        );

        let mut con = self.redis_con.lock().await;
        let json = serde_json::to_string(&invoked).unwrap();
        if let Err(e) = con.xadd(
            "moxnotify:action_invoked",
            "*",
            &[("action", json.as_str())],
        ) {
            log::error!("Failed to write action_invoked to Redis: {}", e);
        }

        Ok(Response::new(ClientActionInvokedResponse {}))
    }

    async fn navigate_viewport(
        &self,
        request: Request<ViewportNavigationRequest>,
    ) -> Result<Response<ViewportNavigationResponse>, Status> {
        let req = request.into_inner();
        let active_notifications = self.get_active_notifications().await;

        let mut notifications: Vec<&NewNotification> = active_notifications.values().collect();
        notifications.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let mut range = self.range.lock().await;
        let mut selected_id = self.selected_id.lock().await;
        match Direction::try_from(req.direction).unwrap() {
            Direction::Prev => {
                if let Some(selected) = *selected_id
                    && let Some(pos) = notifications.iter().position(|n| n.id == selected)
                {
                    let idx = pos
                        .checked_add(1)
                        .filter(|&i| i < notifications.len())
                        .unwrap_or(0);

                    *selected_id = notifications.get(idx).map(|n| n.id);

                    range.ensure_visible_down(idx);
                } else if let Some(first) = notifications.first() {
                    *selected_id = Some(first.id);

                    range.show_tail(notifications.len());
                }
                log::debug!("Direction::Prev, range: {}", range);
            }
            Direction::Next => {
                if let Some(selected) = *selected_id
                    && let Some(pos) = notifications.iter().position(|n| n.id == selected)
                {
                    let idx = pos.checked_sub(1).unwrap_or(notifications.len() - 1);

                    *selected_id = notifications.get(idx).map(|n| n.id);

                    range.ensure_visible_up(idx, notifications.len());
                } else if let Some(last) = notifications.last() {
                    *selected_id = Some(last.id);

                    range.show_head();
                }
                log::debug!("Direction::Next, range: {}", range);
            }
            Direction::First => {
                *selected_id = notifications.last().map(|n| n.id);
                range.show_tail(notifications.len());
                log::debug!("Direction::First, range: {}", range);
            }
            Direction::Last => {
                *selected_id = notifications.first().map(|n| n.id);
                range.show_head();
                log::debug!("Direction::Last, range: {}", range);
            }
        }

        let focused_ids: Vec<u32> = notifications
            .iter()
            .skip(range.start())
            .take(range.width())
            .map(|n| n.id)
            .collect();

        let selected_id_val = *selected_id;
        drop(range);
        drop(selected_id);
        self.start_timers_for_newly_visible(&notifications, &focused_ids)
            .await;

        let range = self.range.lock().await;
        Ok(Response::new(ViewportNavigationResponse {
            focused_ids,
            before_count: notifications.len().saturating_sub(range.end()) as u32,
            after_count: range.start() as u32,
            selected_id: selected_id_val,
        }))
    }

    async fn get_viewport(
        &self,
        _: Request<GetViewportRequest>,
    ) -> Result<Response<ViewportNavigationResponse>, Status> {
        let active_notifications = self.get_active_notifications().await;

        let mut notifications: Vec<&NewNotification> = active_notifications.values().collect();
        notifications.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let selected_id = {
            let selected = self.selected_id.lock().await;
            *selected
        };

        let range = self.range.lock().await;
        let focused_ids = notifications
            .iter()
            .skip(range.start())
            .take(range.width())
            .map(|n| n.id)
            .collect();

        Ok(Response::new(ViewportNavigationResponse {
            focused_ids,
            before_count: notifications.len().saturating_sub(range.end()) as u32,
            after_count: range.start() as u32,
            selected_id,
        }))
    }

    async fn start_timers(
        &self,
        _: Request<StartTimersRequest>,
    ) -> Result<Response<StartTimersResponse>, Status> {
        let active_notifications = self.get_active_notifications().await;

        let mut notifications: Vec<&NewNotification> = active_notifications.values().collect();
        notifications.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let range = self.range.lock().await;
        let visible_notifications: Vec<&NewNotification> = notifications
            .iter()
            .skip(range.start())
            .take(range.width())
            .copied()
            .collect();

        let timeouts = Arc::clone(&self.timeouts);
        for notification in visible_notifications {
            let timeout_ms = notification.timeout;
            // Timeout == 0 means that notification never expires
            // Timeout == -1 means that timeout should be chosen by notifications server
            // but we handle it in collectors
            if timeout_ms > 0 {
                let duration = std::time::Duration::from_millis(timeout_ms as u64);
                timeouts
                    .start_timer(notification.id, notification.uuid.clone(), duration)
                    .await;
            }
        }

        Ok(Response::new(StartTimersResponse {}))
    }

    async fn stop_timers(
        &self,
        _: Request<StopTimersRequest>,
    ) -> Result<Response<StopTimersResponse>, Status> {
        Ok(Response::new(StopTimersResponse {}))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::new().filter("MOXNOTIFY_LOG"))
        .filter_level(log::LevelFilter::Off)
        .filter_module("scheduler", log::LevelFilter::max())
        .init();

    let scheduler_addr =
        std::env::var("MOXNOTIFY_SCHEDULER_ADDR").unwrap_or_else(|_| "[::1]:50052".to_string());

    log::info!("Connecting to Redis and subscribing to notifications...");

    let client = redis::Client::open("redis://127.0.0.1/")?;
    let write_con = client.get_connection()?;
    let read_con = client.get_connection()?;
    let scheduler = Scheduler::new(write_con);
    let notification_broadcast = Arc::clone(&scheduler.notification_broadcast);
    let close_notification_broadcast = Arc::clone(&scheduler.close_notification_broadcast);

    let server_addr = scheduler_addr.parse()?;
    tokio::spawn(async move {
        log::info!("Scheduler server listening on {}", server_addr);
        Server::builder()
            .add_service(ClientServiceServer::new(scheduler))
            .serve(server_addr)
            .await
            .expect("Server failed to start");
    });

    log::info!("Subscribed to notifications from Redis stream");

    let mut con = read_con;
    loop {
        if let Some(streams) = con.xread_options(
            &["moxnotify:notify", "moxnotify:close_notification"],
            &[">", ">"],
            &StreamReadOptions::default()
                .group("scheduler-group", "scheduler-1")
                .block(0),
        )? {
            for stream_key in &streams.keys {
                match stream_key.key.as_str() {
                    "moxnotify:notify" => {
                        for stream_id in &stream_key.ids {
                            if let Some(redis::Value::BulkString(json)) =
                                stream_id.map.get("notification")
                            {
                                let json = std::str::from_utf8(json).unwrap();
                                let notification: NewNotification =
                                    serde_json::from_str(json).unwrap();

                                log::info!(
                                    "Scheduling notification: id={}, app_name='{}', summary='{}'",
                                    notification.id,
                                    notification.app_name,
                                    notification.summary
                                );

                                match notification_broadcast.send(notification) {
                                    Ok(receiver_count) => {
                                        log::info!(
                                            "Broadcast notification to {} receivers",
                                            receiver_count
                                        );
                                    }
                                    Err(e) => {
                                        log::error!("Failed to broadcast notification: {}", e);
                                    }
                                }

                                if let Err(e) = con.xack(
                                    "moxnotify:notify",
                                    "scheduler-group",
                                    &[stream_id.id.as_str()],
                                ) {
                                    log::error!("Failed to ACK message: {}", e);
                                }
                            }
                        }
                    }
                    "moxnotify:close_notification" => {
                        for stream_id in &stream_key.ids {
                            if let Some(redis::Value::BulkString(json)) =
                                stream_id.map.get("close_notification")
                            {
                                let json = std::str::from_utf8(json).unwrap();
                                let close_notification: CloseNotification =
                                    serde_json::from_str(json).unwrap();

                                log::info!(
                                    "Broadcasting close_notification to clients: id={}",
                                    close_notification.id
                                );

                                let id_str = close_notification.id.to_string();
                                if let Err(e) = con.hdel("moxnotify:active", id_str.as_str()) {
                                    log::warn!(
                                        "Failed to remove notification from active HASH: {}",
                                        e
                                    );
                                }

                                if let Err(e) =
                                    close_notification_broadcast.send(close_notification)
                                {
                                    log::error!("{e}");
                                }

                                if let Err(e) = con.xack(
                                    "moxnotify:close_notification",
                                    "scheduler-group",
                                    &[stream_id.id.as_str()],
                                ) {
                                    log::error!("Failed to ACK message: {e}");
                                }
                            }
                        }
                    }
                    _ => {
                        log::warn!("Received message from unknown stream: {}", stream_key.key);
                    }
                }
            }
        }
    }
}
