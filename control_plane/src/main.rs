pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod collector {
        tonic::include_proto!("moxnotify.collector");
    }
}

use crate::moxnotify::collector::{collector_message, collector_response};
use crate::moxnotify::types::{ActionInvoked, NotificationClosed};
use moxnotify::collector::collector_service_server::{CollectorService, CollectorServiceServer};
use moxnotify::collector::{CollectorMessage, CollectorResponse};
use redis::TypedCommands;
use redis::streams::StreamReadOptions;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, transport::Server};

#[derive(Clone)]
pub struct ControlPlaneService {
    con: Arc<Mutex<redis::Connection>>,
    redis_client: redis::Client,
}

impl ControlPlaneService {
    fn try_new(
        mut redis_con: redis::Connection,
        redis_client: redis::Client,
    ) -> anyhow::Result<Self> {
        // If any of these errors it's likely because group already exists
        _ = redis_con.xgroup_create_mkstream("moxnotify:notify", "indexer-group", "$");
        _ = redis_con.xgroup_create_mkstream("moxnotify:notify", "scheduler-group", "$");
        _ = redis_con.xgroup_create_mkstream(
            "moxnotify:notification_closed",
            "control-plane-group",
            "$",
        );
        _ = redis_con.xgroup_create_mkstream(
            "moxnotify:action_invoked",
            "control-plane-group",
            "$",
        );
        _ = redis_con.xgroup_create_mkstream(
            "moxnotify:close_notification",
            "scheduler-group",
            "$",
        );

        Ok(Self {
            con: Arc::new(Mutex::new(redis_con)),
            redis_client,
        })
    }
}

#[tonic::async_trait]
impl CollectorService for ControlPlaneService {
    type NotificationsStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<CollectorResponse, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn notifications(
        &self,
        request: Request<tonic::Streaming<CollectorMessage>>,
    ) -> Result<Response<Self::NotificationsStream>, Status> {
        let remote_addr = request.remote_addr().unwrap();

        log::info!("New connection from: {:?}", remote_addr);

        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);
        let response_tx = tx.clone();

        let con = Arc::clone(&self.con);

        // Create Redis Pub/Sub subscriptions
        let notification_closed_sub_client = self.redis_client.clone();
        let action_invoked_sub_client = self.redis_client.clone();
        let (notification_closed_tx, mut notification_closed_rx) = mpsc::channel(128);
        let (action_invoked_tx, mut action_invoked_rx) = mpsc::channel(128);

        // Spawn task to subscribe to notification_closed channel (using blocking connection)
        tokio::spawn(async move {
            let notification_closed_tx = notification_closed_tx;
            tokio::task::spawn_blocking(move || {
                if let Ok(mut con) = notification_closed_sub_client.get_connection() {
                    let mut pubsub = con.as_pubsub();
                    if pubsub
                        .subscribe("moxnotify:pubsub:notification_closed")
                        .is_ok()
                    {
                        loop {
                            match pubsub.get_message() {
                                Ok(msg) => {
                                    if let Ok(payload) = msg.get_payload::<String>() {
                                        if let Ok(notification_closed) =
                                            serde_json::from_str::<NotificationClosed>(&payload)
                                        {
                                            // Use blocking send since we're in a blocking context
                                            if notification_closed_tx
                                                .blocking_send(notification_closed)
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                }
            })
            .await
            .ok();
        });

        // Spawn task to subscribe to action_invoked channel (using blocking connection)
        tokio::spawn(async move {
            let action_invoked_tx = action_invoked_tx;
            tokio::task::spawn_blocking(move || {
                if let Ok(mut con) = action_invoked_sub_client.get_connection() {
                    let mut pubsub = con.as_pubsub();
                    if pubsub.subscribe("moxnotify:pubsub:action_invoked").is_ok() {
                        loop {
                            match pubsub.get_message() {
                                Ok(msg) => {
                                    if let Ok(payload) = msg.get_payload::<String>() {
                                        if let Ok(action_invoked) =
                                            serde_json::from_str::<ActionInvoked>(&payload)
                                        {
                                            // Use blocking send since we're in a blocking context
                                            if action_invoked_tx
                                                .blocking_send(action_invoked)
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                }
            })
            .await
            .ok();
        });

        log::info!(
            "Subscribed collector {:?} to notification_closed Pub/Sub",
            remote_addr
        );
        log::info!(
            "Started forward task for collector {:?}, receiver is ready",
            remote_addr
        );

        let response_tx_action = tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(msg)) => match msg.message {
                                Some(collector_message::Message::NewNotification(notification)) => {
                                    log::info!(
                                        "Received notification: id={}, app_name='{}', summary='{}', body='{}', urgency='{}'",
                                        notification.id,
                                        notification.app_name,
                                        notification.summary,
                                        notification.body,
                                        notification.hints.as_ref().unwrap().urgency
                                    );

                                    let mut con = con.lock().await;
                                    let json = serde_json::to_string(&notification).unwrap();
                                    con.xadd("moxnotify:notify", "*", &[("notification", json.as_str())])
                                        .unwrap();

                                    let id_str = notification.id.to_string();
                                    if let Err(e) =
                                        con.hset("moxnotify:active", id_str.as_str(), json.as_str())
                                    {
                                        log::warn!("Failed to add notification to active HASH: {}", e);
                                    }

                                    // Publish to Redis Pub/Sub
                                    if let Err(e) = con.publish::<&str, &str>("moxnotify:pubsub:notification", &json) {
                                        log::error!("Failed to publish notification to Redis Pub/Sub: {}", e);
                                    }
                                }
                                Some(collector_message::Message::CloseNotification(close)) => {
                                    log::info!("Received close notification request: id={}", close.id);

                                    let mut con = con.lock().await;
                                    let json = serde_json::to_string(&close).unwrap();
                                    con.xadd(
                                        "moxnotify:close_notification",
                                        "*",
                                        &[("close_notification", json.as_str())],
                                    )
                                    .unwrap();

                                    let id_str = close.id.to_string();
                                    if let Err(e) = con.hdel("moxnotify:active", id_str.as_str()) {
                                        log::warn!("Failed to remove notification from active HASH: {}", e);
                                    }
                                }
                                None => {
                                    log::warn!("Received empty CollectorMessage");
                                }
                            },
                            Some(Err(e)) => {
                                log::error!("Error receiving message from collector: {}", e);
                                break;
                            }
                            None => {
                                break;
                            }
                        }
                    }
                    closed = notification_closed_rx.recv() => {
                        match closed {
                            Some(closed) => {
                                log::info!(
                                    "Forwarding notification_closed to collector {:?}: id={}, reason={:?}",
                                    remote_addr,
                                    closed.id,
                                    closed.reason()
                                );
                                let response = CollectorResponse {
                                    message: Some(collector_response::Message::NotificationClosed(closed)),
                                };
                                if response_tx.send(Ok(response)).await.is_err() {
                                    log::info!(
                                        "Collector disconnected, stopping forward task: {:?}",
                                        remote_addr
                                    );
                                    break;
                                }
                                log::info!(
                                    "Successfully sent notification_closed to collector {:?}",
                                    remote_addr
                                );
                            }
                            None => {
                                log::info!("NotificationClosed Pub/Sub channel closed for collector: {:?}", remote_addr);
                                break;
                            }
                        }
                    }
                    action = action_invoked_rx.recv() => {
                        match action {
                            Some(action) => {
                                let response = CollectorResponse {
                                    message: Some(
                                        moxnotify::collector::collector_response::Message::ActionInvoked(
                                            action,
                                        ),
                                    ),
                                };
                                if response_tx_action.send(Ok(response)).await.is_err() {
                                    break;
                                }
                            }
                            None => {
                                log::info!("ActionInvoked Pub/Sub channel closed for collector: {:?}", remote_addr);
                                break;
                            }
                        }
                    }
                }
            }
        });

        let output_stream: Self::NotificationsStream = Box::pin(ReceiverStream::new(rx));
        Ok(Response::new(output_stream))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::load(None);

    env_logger::Builder::new()
        .filter(Some("control_plane"), config.control_plane.log_level.into())
        .init();

    let client = redis::Client::open(&*config.redis.address).unwrap();

    let service = ControlPlaneService::try_new(client.get_connection()?, client.clone())?;

    let con = client.get_connection().unwrap();
    let pubsub_client = client.clone();
    tokio::spawn(async move {
        let mut con = con;
        loop {
            if let Some(streams) = con
                .xread_options(
                    &["moxnotify:notification_closed"],
                    &[">"],
                    &StreamReadOptions::default()
                        .group("control-plane-group", "control-plane")
                        .block(0),
                )
                .unwrap()
                && let Some(stream_key) = streams
                    .keys
                    .iter()
                    .find(|sk| sk.key == "moxnotify:notification_closed")
            {
                for stream_id in &stream_key.ids {
                    if let Some(redis::Value::BulkString(json)) = stream_id.map.get("notification")
                    {
                        match std::str::from_utf8(json) {
                            Ok(json_str) => {
                                match serde_json::from_str::<NotificationClosed>(json_str) {
                                    Ok(closed) => {
                                        log::info!(
                                            "Received notification_closed from Redis: id: {}, reason: {:?}",
                                            closed.id,
                                            closed.reason()
                                        );

                                        log::info!(
                                            "Publishing notification_closed to Redis Pub/Sub: id={}, reason={:?}",
                                            closed.id,
                                            closed.reason()
                                        );
                                        let json = serde_json::to_string(&closed).unwrap();
                                        let pubsub_client = pubsub_client.clone();
                                        tokio::spawn(async move {
                                            if let Ok(mut pub_con) = pubsub_client.get_connection()
                                            {
                                                if let Err(e) = pub_con.publish::<&str, &str>(
                                                    "moxnotify:pubsub:notification_closed",
                                                    &json,
                                                ) {
                                                    log::error!(
                                                        "Failed to publish notification_closed to Redis Pub/Sub: {}",
                                                        e
                                                    );
                                                } else {
                                                    log::debug!(
                                                        "Published notification_closed to Redis Pub/Sub"
                                                    );
                                                }
                                            }
                                        });
                                        tokio::task::yield_now().await;
                                        log::info!("Finished publishing for id={}", closed.id);

                                        if let Err(e) = con.xack(
                                            "moxnotify:notification_closed",
                                            "control-plane-group",
                                            &[stream_id.id.as_str()],
                                        ) {
                                            log::error!("Failed to ACK message: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Failed to parse notification_closed from Redis: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to convert notification_closed bytes to string: {}",
                                    e
                                );
                            }
                        }
                    } else {
                        log::warn!(
                            "Received notification_closed message from Redis but 'notification' field is missing or has unexpected type: stream_id={}",
                            stream_id.id
                        );
                    }
                }
            }
        }
    });

    let con_action = client.get_connection().unwrap();
    let pubsub_client_action = client.clone();
    tokio::spawn(async move {
        let mut con = con_action;
        loop {
            if let Some(streams) = con
                .xread_options(
                    &["moxnotify:action_invoked"],
                    &[">"],
                    &StreamReadOptions::default()
                        .group("control-plane-group", "control-plane")
                        .block(0),
                )
                .unwrap()
                && let Some(stream_key) = streams
                    .keys
                    .iter()
                    .find(|sk| sk.key == "moxnotify:action_invoked")
            {
                for stream_id in &stream_key.ids {
                    if let Some(redis::Value::BulkString(json)) = stream_id.map.get("action")
                        && let Ok(json_str) = std::str::from_utf8(json)
                        && let Ok(action) = serde_json::from_str::<ActionInvoked>(json_str)
                    {
                        log::info!(
                            "Received action_invoked from Redis: id: {}, action_key: {}",
                            action.id,
                            action.action_key
                        );
                        log::info!(
                            "Publishing action_invoked to Redis Pub/Sub: id={}, action_key={}",
                            action.id,
                            action.action_key
                        );
                        let json = serde_json::to_string(&action).unwrap();
                        let pubsub_client_action = pubsub_client_action.clone();
                        tokio::spawn(async move {
                            if let Ok(mut pub_con) = pubsub_client_action.get_connection() {
                                if let Err(e) = pub_con
                                    .publish::<&str, &str>("moxnotify:pubsub:action_invoked", &json)
                                {
                                    log::error!(
                                        "Failed to publish action_invoked to Redis Pub/Sub: {}",
                                        e
                                    );
                                } else {
                                    log::debug!("Published action_invoked to Redis Pub/Sub");
                                }
                            }
                        });
                        tokio::task::yield_now().await;
                        log::info!("Finished publishing for id={}", action.id);
                        let _ = con.xack(
                            "moxnotify:action_invoked",
                            "control-plane-group",
                            &[stream_id.id.as_str()],
                        );
                    }
                }
            }
        }
    });

    Server::builder()
        .add_service(CollectorServiceServer::new(service.clone()))
        .serve(config.control_plane.address.parse()?)
        .await?;

    log::info!(
        "Control plane server listening on {}",
        config.control_plane.address
    );

    Ok(())
}
