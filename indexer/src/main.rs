pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
}

use moxnotify::types::NewNotification;
use redis::AsyncTypedCommands;
use redis::streams::StreamReadOptions;
use std::path::PathBuf;
use std::sync::Arc;
use tantivy::directory::MmapDirectory;
use tantivy::{DateTime, Index, IndexWriter, schema::*};

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
async fn main() -> anyhow::Result<()> {
    let config = Arc::new(config::Config::load(None));

    env_logger::Builder::new()
        .filter(Some("indexer"), config.indexer.log_level.into())
        .init();

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

    let client = redis::Client::open(&*config.redis.address)?;
    let mut con = client.get_multiplexed_async_connection().await?;
    let mut read_pending = false;

    loop {
        // Alternate between reading pending messages ("0") and new messages (">")
        // This ensures we don't miss messages that were delivered but not ACKed
        let stream_id = if read_pending { "0" } else { ">" };
        read_pending = !read_pending;

        if let Ok(Some(streams)) = AsyncTypedCommands::xread_options(
            &mut con,
            &["moxnotify:notify"],
            &[stream_id],
            &StreamReadOptions::default()
                .group("indexer-group", "indexer-1")
                .block(if stream_id == ">" { 100 } else { 0 }), // Block only when reading new messages
        )
        .await
        {
            if let Some(stream_key) = streams.keys.iter().find(|sk| sk.key == "moxnotify:notify") {
                for stream_id in stream_key.ids.iter() {
                    if let Some(redis::Value::BulkString(json)) = stream_id.map.get("notification")
                    {
                        let notification =
                            serde_json::from_str::<NewNotification>(str::from_utf8(json).unwrap())
                                .unwrap();

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

                        index_writer.add_document(doc).unwrap();

                        index_writer.commit().unwrap();
                    }

                    if let Err(e) = AsyncTypedCommands::xack(
                        &mut con,
                        "moxnotify:notify",
                        "indexer-group",
                        &[stream_id.id.as_str()],
                    )
                    .await
                    {
                        log::error!("Failed to ACK message: {}", e);
                    }
                }
            }
        } else if stream_id == ">" {
            // No new messages available, yield to avoid busy-waiting
            tokio::task::yield_now().await;
        }
    }
}
