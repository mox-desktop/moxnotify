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
use clap::Parser;
use moxnotify::collector::collector_service_server::{CollectorService, CollectorServiceServer};
use moxnotify::collector::{CollectorMessage, CollectorResponse};
use redis::AsyncTypedCommands;
use redis::streams::StreamReadOptions;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ControlPlaneService {
    con: Arc<Mutex<redis::aio::MultiplexedConnection>>,
    redis_client: redis::Client,
}

impl ControlPlaneService {
    async fn try_new(
        mut redis_con: redis::aio::MultiplexedConnection,
        redis_client: redis::Client,
    ) -> anyhow::Result<Self> {
        // If any of these errors it's likely because group already exists
        _ = AsyncTypedCommands::xgroup_create_mkstream(
            &mut redis_con,
            "moxnotify:notify",
            "indexer-group",
            "$",
        )
        .await;
        _ = AsyncTypedCommands::xgroup_create_mkstream(
            &mut redis_con,
            "moxnotify:notify",
            "scheduler-group",
            "$",
        )
        .await;
        _ = AsyncTypedCommands::xgroup_create_mkstream(
            &mut redis_con,
            "moxnotify:notification_closed",
            "control-plane-group",
            "$",
        )
        .await;
        _ = AsyncTypedCommands::xgroup_create_mkstream(
            &mut redis_con,
            "moxnotify:action_invoked",
            "control-plane-group",
            "$",
        )
        .await;
        _ = AsyncTypedCommands::xgroup_create_mkstream(
            &mut redis_con,
            "moxnotify:close_notification",
            "scheduler-group",
            "$",
        )
        .await;

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

        let con = Arc::clone(&self.con);

        let notification_closed_sub_client = self.redis_client.clone();
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let (notification_closed_tx, mut notification_closed_rx) = mpsc::channel(128);
            let (action_invoked_tx, mut action_invoked_rx) = mpsc::channel(128);

            let mut pubsub = notification_closed_sub_client
                .get_async_pubsub()
                .await
                .unwrap();

            let _ = pubsub
                .subscribe("moxnotify:pubsub:notification_closed")
                .await;
            let _ = pubsub.subscribe("moxnotify:pubsub:action_invoked").await;

            let mut pubsub_stream = pubsub.on_message();

            loop {
                tokio::select! {
                    Some(msg) = pubsub_stream.next() => {
                        let payload = msg.get_payload::<String>().unwrap();
                        let channel = msg.get_channel_name();

                        match channel {
                            "moxnotify:pubsub:notification_closed" => {
                                if let Ok(closed) = serde_json::from_str::<NotificationClosed>(&payload) {
                                    let _ = notification_closed_tx.send(closed).await;
                                }
                            }
                            "moxnotify:pubsub:action_invoked" => {
                                if let Ok(action) = serde_json::from_str::<ActionInvoked>(&payload) {
                                    let _ = action_invoked_tx.send(action).await;
                                }
                            }
                            _ => {}
                        }
                    }
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
                                    if let Err(e) = AsyncTypedCommands::xadd(&mut *con, "moxnotify:notify", "*", &[("notification", json.as_str())]).await {
                                        log::error!("Failed to add notification to Redis stream: {}", e);
                                        drop(con);
                                        continue;
                                    }

                                    let id_str = notification.id.to_string();
                                    if let Err(e) =
                                        AsyncTypedCommands::hset(&mut *con, "moxnotify:active", id_str.as_str(), json.as_str()).await
                                    {
                                        log::warn!("Failed to add notification to active HASH: {}", e);
                                    }

                                    // Publish to Redis Pub/Sub
                                    if let Err(e) = redis::AsyncCommands::publish::<&str, &str, usize>(&mut *con, "moxnotify:pubsub:notification", &json).await {
                                        log::error!("Failed to publish notification to Redis Pub/Sub: {}", e);
                                    }
                                }
                                Some(collector_message::Message::CloseNotification(close)) => {
                                    log::info!("Received close notification request: id={}", close.id);

                                    let mut con = con.lock().await;
                                    let json = serde_json::to_string(&close).unwrap();
                                    if let Err(e) = AsyncTypedCommands::xadd(
                                        &mut *con,
                                        "moxnotify:close_notification",
                                        "*",
                                        &[("close_notification", json.as_str())],
                                    ).await {
                                        log::error!("Failed to add close_notification to Redis stream: {}", e);
                                        drop(con);
                                        continue;
                                    }

                                    let id_str = close.id.to_string();
                                    if let Err(e) = AsyncTypedCommands::hdel(&mut *con, "moxnotify:active", id_str.as_str()).await {
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
                                if tx.send(Ok(response)).await.is_err() {
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
                                if tx.send(Ok(response)).await.is_err() {
                                    break;
                                }
                            }
                            None => {
                                log::info!("ActionInvoked Pub/Sub channel closed for collector: {:?}", remote_addr);
                                break;
                            }
                        }
                    }
                    else => {}
                }
            }
        });

        let output_stream: Self::NotificationsStream = Box::pin(ReceiverStream::new(rx));
        Ok(Response::new(output_stream))
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE", help = "Path to the config file")]
    config: Option<Box<Path>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config =
        config::Config::load(cli.config.as_ref().map(|p| p.as_ref())).unwrap_or_else(|err| {
            log::warn!("{err}");
            config::Config::default()
        });

    env_logger::Builder::new()
        .filter(Some("control_plane"), config.control_plane.log_level.into())
        .init();

    let client = redis::Client::open(&*config.redis.address).unwrap();
    let write_con = client.get_multiplexed_async_connection().await?;
    let read_con = client.get_multiplexed_async_connection().await?;
    let pub_con = client.get_multiplexed_async_connection().await?;

    let service = ControlPlaneService::try_new(write_con, client.clone()).await?;

    tokio::spawn(async move {
        Server::builder()
            .add_service(CollectorServiceServer::new(service.clone()))
            .serve(config.control_plane.address.parse().unwrap())
            .await
            .unwrap();

        log::info!(
            "Control plane server listening on {}",
            config.control_plane.address
        );
    });

    let mut read_con_mut = read_con;
    let mut pub_con_mut = pub_con;
    let mut read_pending = false;

    loop {
        // Alternate between reading pending messages ("0") and new messages (">")
        // This ensures we don't miss messages that were delivered but not ACKed
        let stream_ids = if read_pending { ["0", "0"] } else { [">", ">"] };
        read_pending = !read_pending;

        if let Ok(Some(streams)) = AsyncTypedCommands::xread_options(
            &mut read_con_mut,
            &["moxnotify:action_invoked", "moxnotify:notification_closed"],
            &stream_ids,
            &StreamReadOptions::default()
                .group("control-plane-group", "control-plane")
                .block(if stream_ids[0] == ">" { 100 } else { 0 }),
        )
        .await
        {
            for stream_key in streams.keys.iter() {
                for stream_id in &stream_key.ids {
                    match stream_key.key.as_str() {
                        "moxnotify:action_invoked" => {
                            if let Some(redis::Value::BulkString(json)) =
                                stream_id.map.get("action")
                            {
                                let json = std::str::from_utf8(json).unwrap();
                                let action = serde_json::from_str::<ActionInvoked>(json).unwrap();

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

                                if let Err(e) = redis::AsyncCommands::publish::<&str, &str, usize>(
                                    &mut pub_con_mut,
                                    "moxnotify:pubsub:action_invoked",
                                    json,
                                )
                                .await
                                {
                                    log::error!(
                                        "Failed to publish action_invoked to Redis Pub/Sub: {}",
                                        e
                                    );
                                    // Don't ACK if publishing failed
                                    continue;
                                }

                                log::info!("Finished publishing for id={}", action.id);
                            }
                        }
                        "moxnotify:notification_closed" => {
                            if let Some(redis::Value::BulkString(json)) =
                                stream_id.map.get("notification")
                            {
                                let json = std::str::from_utf8(json).unwrap();
                                let closed =
                                    serde_json::from_str::<NotificationClosed>(json).unwrap();

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

                                if let Err(e) = redis::AsyncCommands::publish::<&str, &str, usize>(
                                    &mut pub_con_mut,
                                    "moxnotify:pubsub:notification_closed",
                                    json,
                                )
                                .await
                                {
                                    log::error!(
                                        "Failed to publish notification_closed to Redis Pub/Sub: {}",
                                        e
                                    );
                                    // Don't ACK if publishing failed
                                    continue;
                                }

                                log::debug!("Published notification_closed to Redis Pub/Sub");
                                log::info!("Finished publishing for id={}", closed.id);
                            }
                        }
                        _ => unreachable!(),
                    }

                    if let Err(ack_err) = AsyncTypedCommands::xack(
                        &mut read_con_mut,
                        stream_key.key.as_str(),
                        "control-plane-group",
                        &[stream_id.id.as_str()],
                    )
                    .await
                    {
                        log::error!("Failed to ACK message: {}", ack_err);
                    }
                }
            }
        }
    }
}
