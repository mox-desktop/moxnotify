use std::ops::Bound as StdBound;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::RangeQuery;
use tantivy::{
    DateTime, DocAddress, Index, IndexReader, IndexWriter, ReloadPolicy, Term, schema::*,
};

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

async fn cleanup_old_documents(
    index: &Index,
    reader: &IndexReader,
    retention_days: u64,
) -> anyhow::Result<u64> {
    let schema = index.schema();
    let timestamp_field = schema.get_field("timestamp").unwrap();
    let id_field = schema.get_field("id").unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let retention_ms = (retention_days as i64) * 24 * 60 * 60 * 1000;
    let cutoff_timestamp_ms = now - retention_ms;
    let cutoff_datetime = DateTime::from_timestamp_millis(cutoff_timestamp_ms);

    log::info!(
        "Cleaning up documents older than {} days (cutoff: {} ms, now: {} ms)",
        retention_days,
        cutoff_timestamp_ms,
        now
    );

    reader.reload()?;
    let searcher = reader.searcher();

    let lower_bound = StdBound::Unbounded;
    let upper_bound = StdBound::Included(Term::from_field_date(timestamp_field, cutoff_datetime));

    let range_query: Box<dyn tantivy::query::Query> =
        Box::new(RangeQuery::new(lower_bound, upper_bound));

    let top_docs: Vec<DocAddress> =
        match searcher.search(&range_query, &TopDocs::with_limit(1_000_000)) {
            Ok(results) => results.into_iter().map(|(_, addr)| addr).collect(),
            Err(e) => {
                log::error!("Failed to search for old documents: {}", e);
                return Err(anyhow::anyhow!("Search failed: {}", e));
            }
        };

    let count = top_docs.len();
    log::info!("Found {} documents to delete", count);

    if count == 0 {
        return Ok(0);
    }

    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    let mut deleted_count = 0u64;
    for doc_addr in top_docs {
        if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_addr) {
            if let Some(id_value) = doc.get_first(id_field) {
                if let Some(id_u64) = id_value.as_u64() {
                    let term = Term::from_field_u64(id_field, id_u64);
                    index_writer.delete_term(term);
                    deleted_count += 1;
                }
            }
        }
    }

    index_writer.commit()?;
    log::info!("Deleted {} documents", deleted_count);

    Ok(deleted_count)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::load(None);

    env_logger::Builder::new()
        .filter(Some("janitor"), config.janitor.log_level.into())
        .init();

    let retention_days = config.janitor.retention.period.as_secs() / 86400
        + if config.janitor.retention.period.as_secs() % 86400 > 0 {
            1
        } else {
            0
        };
    let interval_seconds = config.janitor.retention.schedule.as_secs();

    log::info!(
        "Starting janitor service: retention={} days, schedule={} seconds",
        retention_days,
        interval_seconds
    );

    let index_path = path();
    log::info!("Using index path: {:?}", index_path);

    let index = Index::open(MmapDirectory::open(&index_path).unwrap())?;
    log::info!("Index opened successfully");

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;

    log::info!("Running initial cleanup...");
    match cleanup_old_documents(&index, &reader, retention_days).await {
        Ok(count) => log::info!("Initial cleanup completed: {} documents deleted", count),
        Err(e) => log::error!("Initial cleanup failed: {}", e),
    }

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_seconds));
    interval.tick().await;

    loop {
        interval.tick().await;
        log::info!("Running scheduled cleanup...");
        match cleanup_old_documents(&index, &reader, retention_days).await {
            Ok(count) => log::info!("Scheduled cleanup completed: {} documents deleted", count),
            Err(e) => log::error!("Scheduled cleanup failed: {}", e),
        }
    }
}
