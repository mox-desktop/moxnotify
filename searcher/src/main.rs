use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use chrono::DateTime as ChronoDateTime;
use serde::Deserialize;
use std::ops::Bound as StdBound;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::{BooleanQuery, Occur, QueryParser, RangeQuery};
use tantivy::{
    DateTime, DocAddress, Index, IndexReader, Order, ReloadPolicy, Term, doc, schema::*,
};
use tower_http::cors::CorsLayer;

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

#[derive(Clone)]
struct GlobalState {
    reader: IndexReader,
    parser: QueryParser,
    schema: Schema,
    timestamp_field: Field,
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let config = config::Config::load(None);

    env_logger::Builder::new()
        .filter(Some("searcher"), config.searcher.log_level.into())
        .init();

    let index_path = path();
    log::info!("Opening index from: {:?}", index_path);

    let index = Index::open(MmapDirectory::open(&index_path).unwrap()).unwrap();
    log::info!("Index opened successfully");

    let schema = index.schema();
    let summary = schema.get_field("summary").unwrap();
    let body = schema.get_field("body").unwrap();
    let app_name = schema.get_field("app_name").unwrap();
    let timestamp_field = schema.get_field("timestamp").unwrap();

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    log::info!("Index reader created");

    let mut query_parser = QueryParser::for_index(&index, vec![summary, body, app_name]);
    query_parser.set_field_boost(summary, 2.);

    let state = GlobalState {
        reader,
        schema,
        parser: query_parser,
        timestamp_field,
    };

    let app = Router::new()
        .route("/api/search", post(search))
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
                .allow_credentials(false)
        )
        .with_state(state);

    log::info!("Starting searcher server on {}", config.searcher.address);
    let listener = tokio::net::TcpListener::bind(&config.searcher.address)
        .await
        .unwrap();
    log::info!("Searcher server listening on {}", config.searcher.address);
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn search(
    State(state): State<GlobalState>,
    Json(payload): Json<Query>,
) -> Json<Vec<serde_json::Value>> {
    log::info!(
        "Received search request: query='{}', max_hits={:?}, sort_by={:?}, sort_order={:?}",
        payload.query,
        payload.max_hits,
        payload.sort_by,
        payload.sort_order
    );

    log::debug!(
        "Search request details: start_timestamp={:?}, end_timestamp={:?}",
        payload.start_timestamp,
        payload.end_timestamp
    );

    state.reader.reload().unwrap();
    log::debug!("Index reader reloaded");

    let searcher = state.reader.searcher();
    let text_query = match state.parser.parse_query(&payload.query) {
        Ok(q) => q,
        Err(e) => {
            log::error!("Failed to parse query '{}': {}", payload.query, e);
            return Json(vec![]);
        }
    };

    let query = if payload.start_timestamp.is_some() || payload.end_timestamp.is_some() {
        log::debug!("Building query with timestamp range");
        let lower_bound = payload
            .start_timestamp
            .as_ref()
            .and_then(|ts_str| {
                ChronoDateTime::parse_from_rfc3339(ts_str).ok().map(|dt| {
                    let timestamp_ms = dt.timestamp_millis();
                    DateTime::from_timestamp_millis(timestamp_ms)
                })
            })
            .map(|date_time| {
                let term = Term::from_field_date(state.timestamp_field, date_time);
                StdBound::Included(term)
            })
            .unwrap_or(StdBound::Unbounded);

        let upper_bound = payload
            .end_timestamp
            .as_ref()
            .and_then(|ts_str| {
                ChronoDateTime::parse_from_rfc3339(ts_str).ok().map(|dt| {
                    let timestamp_ms = dt.timestamp_millis();
                    DateTime::from_timestamp_millis(timestamp_ms)
                })
            })
            .map(|date_time| {
                let term = Term::from_field_date(state.timestamp_field, date_time);
                StdBound::Included(term)
            })
            .unwrap_or(StdBound::Unbounded);

        let range_query: Box<dyn tantivy::query::Query> =
            Box::new(RangeQuery::new(lower_bound, upper_bound));

        Box::new(BooleanQuery::new(vec![
            (Occur::Must, text_query),
            (Occur::Must, range_query),
        ])) as Box<dyn tantivy::query::Query>
    } else {
        log::debug!("Building query without timestamp range");
        text_query
    };

    let limit = payload.max_hits.unwrap_or(20) as usize;
    log::debug!("Search limit: {}", limit);

    let top_docs: Vec<DocAddress> = if let Some(sort_by) = payload.sort_by {
        let sort_order = match payload.sort_order {
            Some(SortOrder::Asc) => Order::Asc,
            _ => Order::Desc,
        };
        log::debug!(
            "Searching with sort: field={}, order={:?}",
            sort_by,
            sort_order
        );

        let res = searcher.search(
            &query,
            &TopDocs::with_limit(limit).order_by_u64_field(sort_by, sort_order),
        );

        match res {
            Ok(results) => results.into_iter().map(|(_, addr)| addr).collect(),
            Err(e) => {
                log::error!("Search failed: {}", e);
                return Json(vec![]);
            }
        }
    } else {
        log::debug!("Searching without sort");
        match searcher.search(&query, &TopDocs::with_limit(limit)) {
            Ok(results) => results.into_iter().map(|(_, addr)| addr).collect(),
            Err(e) => {
                log::error!("Search failed: {}", e);
                return Json(vec![]);
            }
        }
    };

    log::info!("Search found {} documents", top_docs.len());

    let docs: Vec<serde_json::Value> = top_docs
        .into_iter()
        .filter_map(|doc_addr| {
            let doc = searcher.doc::<TantivyDocument>(doc_addr).unwrap();
            serde_json::from_str::<serde_json::Value>(&doc.to_json(&state.schema)).ok()
        })
        .collect();

    log::debug!("Returning {} documents", docs.len());
    Json(docs)
}

#[derive(Deserialize)]
struct Query {
    query: String,
    start_timestamp: Option<String>,
    end_timestamp: Option<String>,
    max_hits: Option<u32>,
    sort_by: Option<String>,
    sort_order: Option<SortOrder>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum SortOrder {
    Asc,
    Desc,
}
