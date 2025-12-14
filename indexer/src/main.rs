mod indexer {
    tonic::include_proto!("indexer");
}

use base64::engine::{Engine, general_purpose};
use env_logger::Builder;
use indexer::IndexerSubscribeRequest;
use indexer::control_plane_indexer_client::ControlPlaneIndexerClient;
use log::LevelFilter;
use prost::Message;
use std::path::PathBuf;
use tantivy::directory::MmapDirectory;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, doc};
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

    // Connect to control plane
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

    // Subscribe to notifications
    let request = Request::new(IndexerSubscribeRequest {});
    let mut stream = client
        .stream_notifications(request)
        .await
        .map_err(|e| tantivy::TantivyError::InvalidArgument(format!("Failed to subscribe: {}", e)))?
        .into_inner();

    log::info!("Subscribed to notifications");

    let mut schema_builder = Schema::builder();

    schema_builder.add_u64_field("id", INDEXED | STORED);

    schema_builder.add_text_field("summary", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    schema_builder.add_text_field("app_name", TEXT | STORED);
    schema_builder.add_text_field("app_icon", STORED);

    // searchable + stored hints
    schema_builder.add_text_field("hint_category", TEXT | STORED);
    schema_builder.add_text_field("hint_desktop_entry", TEXT | STORED);
    schema_builder.add_i64_field("hint_value", INDEXED | STORED);
    schema_builder.add_i64_field("hint_urgency", INDEXED | STORED);

    // stored-only hints
    schema_builder.add_bool_field("hint_action_icons", STORED);
    schema_builder.add_bool_field("hint_resident", STORED);
    schema_builder.add_bool_field("hint_suppress_sound", STORED);
    schema_builder.add_bool_field("hint_transient", STORED);

    schema_builder.add_text_field("hint_sound_file", STORED);
    schema_builder.add_text_field("hint_sound_name", STORED);

    schema_builder.add_i64_field("hint_x", STORED);
    schema_builder.add_i64_field("hint_y", STORED);

    schema_builder.add_text_field("hint_image", STORED);
    let schema = schema_builder.build();

    let index =
        Index::open_or_create(MmapDirectory::open(path()).unwrap(), schema.clone()).unwrap();
    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    let id = schema.get_field("id").unwrap();
    let summary = schema.get_field("summary").unwrap();
    let body = schema.get_field("body").unwrap();
    let app_name = schema.get_field("app_name").unwrap();
    let app_icon = schema.get_field("app_icon").unwrap();

    let hint_category = schema.get_field("hint_category").unwrap();
    let hint_desktop_entry = schema.get_field("hint_desktop_entry").unwrap();
    let hint_value = schema.get_field("hint_value").unwrap();
    let hint_urgency = schema.get_field("hint_urgency").unwrap();

    let hint_action_icons = schema.get_field("hint_action_icons").unwrap();
    let hint_resident = schema.get_field("hint_resident").unwrap();
    let hint_suppress_sound = schema.get_field("hint_suppress_sound").unwrap();
    let hint_transient = schema.get_field("hint_transient").unwrap();
    let hint_sound_file = schema.get_field("hint_sound_file").unwrap();
    let hint_sound_name = schema.get_field("hint_sound_name").unwrap();
    let hint_x = schema.get_field("hint_x").unwrap();
    let hint_y = schema.get_field("hint_y").unwrap();
    let hint_image = schema.get_field("hint_image").unwrap();

    while let Some(msg_result) = stream.next().await {
        if let Ok(msg) = msg_result
            && let Some(notification) = msg.notification
        {
            log::info!(
                "Received notification: id={}, app_name='{}', summary='{}', body='{}'",
                notification.id,
                notification.app_name,
                notification.summary,
                notification.body,
            );

            let mut doc = TantivyDocument::default();

            doc.add_u64(id, notification.id as u64);
            doc.add_text(summary, notification.summary);
            doc.add_text(body, notification.body);
            doc.add_text(app_name, notification.app_name);

            if let Some(icon) = notification.app_icon {
                doc.add_text(app_icon, icon);
            }

            if let Some(h) = notification.hints {
                doc.add_bool(hint_action_icons, h.action_icons);
                doc.add_bool(hint_resident, h.resident);
                doc.add_bool(hint_suppress_sound, h.suppress_sound);
                doc.add_bool(hint_transient, h.transient);

                doc.add_i64(hint_x, h.x as i64);
                if let Some(y) = h.y {
                    doc.add_i64(hint_y, y as i64);
                }

                doc.add_i64(hint_urgency, h.urgency as i64);

                if let Some(v) = h.value {
                    doc.add_i64(hint_value, v as i64);
                }

                if let Some(cat) = h.category {
                    doc.add_text(hint_category, cat);
                }

                if let Some(entry) = h.desktop_entry {
                    doc.add_text(hint_desktop_entry, entry);
                }

                if let Some(sf) = h.sound_file {
                    doc.add_text(hint_sound_file, sf);
                }

                if let Some(sn) = h.sound_name {
                    doc.add_text(hint_sound_name, sn);
                }

                if let Some(img) = h.image {
                    let mut buf = Vec::new();
                    img.encode(&mut buf);
                    let encoded = general_purpose::STANDARD.encode(buf);
                    doc.add_text(hint_image, encoded);
                }
            }

            index_writer.add_document(doc);
        }
    }

    Ok(())
}
