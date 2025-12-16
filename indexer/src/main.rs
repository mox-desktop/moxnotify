pub mod moxnotify {
    pub mod common {
        tonic::include_proto!("moxnotify.common");
    }
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
}
mod indexer {
    tonic::include_proto!("indexer");
}

use env_logger::Builder;
use indexer::IndexerSubscribeRequest;
use indexer::control_plane_indexer_client::ControlPlaneIndexerClient;
use log::LevelFilter;
use std::path::PathBuf;
use tantivy::directory::MmapDirectory;
use tantivy::{DateTime, schema::*};
use tantivy::{Index, IndexWriter};
use tokio_stream::StreamExt;
use tonic::Request;

fn path() -> PathBuf {
    let path = std::env::var("XDG_DATA_HOME")
        .map(|data_home| PathBuf::from(data_home).join("moxnotify"))
        .or_else(|_| {
            std::env::var("HOME").map(|home| PathBuf::from(home).join(".local/share/moxnotify"))
        })
        .unwrap_or_else(|_| PathBuf::from(""));

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }

    path
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new().filter(Some("indexer"), log_level).init();

    let control_plane_addr = std::env::var("MOXNOTIFY_CONTROL_PLANE_ADDR")
        .unwrap_or_else(|_| "http://[::1]:50051".to_string());

    log::info!("Connecting to control plane at: {}", control_plane_addr);

    let mut client = ControlPlaneIndexerClient::connect(control_plane_addr)
        .await
        .map_err(|e| {
            tantivy::TantivyError::InvalidArgument(format!(
                "Failed to connect to control plane: {}",
                e
            ))
        })?;

    log::info!("Connected to control plane, subscribing to notifications...");

    let request = Request::new(IndexerSubscribeRequest {});
    let mut stream = client
        .stream_notifications(request)
        .await
        .map_err(|e| tantivy::TantivyError::InvalidArgument(format!("Failed to subscribe: {}", e)))?
        .into_inner();

    log::info!("Subscribed to notifications");

    let mut schema_builder = Schema::builder();

    schema_builder.add_u64_field("id", INDEXED | STORED | FAST);
    schema_builder.add_i64_field("timeout", STORED);
    schema_builder.add_date_field(
        "timestamp",
        DateOptions::default()
            .set_indexed()
            .set_fast()
            .set_stored()
            .set_precision(DateTimePrecision::Milliseconds),
    );

    schema_builder.add_text_field("summary", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    schema_builder.add_text_field("app_name", STRING | STORED | FAST);
    schema_builder.add_text_field("app_icon", STORED);

    schema_builder.add_json_field("hints", STORED);
    let schema = schema_builder.build();

    let index =
        Index::open_or_create(MmapDirectory::open(path()).unwrap(), schema.clone()).unwrap();
    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    let id = schema.get_field("id").unwrap();
    let summary = schema.get_field("summary").unwrap();
    let timestamp = schema.get_field("timestamp").unwrap();
    let body = schema.get_field("body").unwrap();
    let app_name = schema.get_field("app_name").unwrap();
    let app_icon = schema.get_field("app_icon").unwrap();
    let timeout = schema.get_field("timeout").unwrap();

    let hints = schema.get_field("hints").unwrap();

    while let Some(msg_result) = stream.next().await {
        if let Ok(msg) = msg_result
            && let Some(notification) = msg.notification
        {
            log::info!(
                "Indexing notification: id={}, app_name='{}', summary='{}', body='{}', urgency='{}'",
                notification.id,
                notification.app_name,
                notification.summary,
                notification.body,
                notification.hints.as_ref().unwrap().urgency
            );

            let mut doc = TantivyDocument::default();

            doc.add_u64(id, notification.id as u64);
            doc.add_date(
                timestamp,
                DateTime::from_timestamp_millis(notification.timestamp),
            );
            doc.add_text(summary, notification.summary);
            doc.add_text(body, notification.body);
            doc.add_text(app_name, notification.app_name);
            doc.add_i64(timeout, notification.timeout as i64);

            if let Some(icon) = notification.app_icon {
                doc.add_text(app_icon, icon);
            }

            if let Some(h) = notification.hints {
                doc.add_text(hints, serde_json::to_string(&h).unwrap());
            }

            index_writer.add_document(doc)?;
        }

        index_writer.commit()?;
    }

    Ok(())
}
