use axum::Json;
use axum::extract::State;
use axum::routing::post;
use axum::{Router, routing::get};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::tokenizer::{NgramTokenizer, TextAnalyzer};
use tantivy::{
    DocAddress, Index, IndexReader, IndexWriter, Order, ReloadPolicy, Searcher, doc, schema::*,
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

#[derive(Clone)]
struct GlobalState {
    reader: IndexReader,
    parser: QueryParser,
    schema: Schema,
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let index = Index::open(MmapDirectory::open(path()).unwrap()).unwrap();

    let schema = index.schema();
    let summary = schema.get_field("summary").unwrap();
    let body = schema.get_field("body").unwrap();
    let app_name = schema.get_field("app_name").unwrap();
    let hint_category = schema.get_field("hint_category").unwrap();

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;

    let mut query_parser =
        QueryParser::for_index(&index, vec![summary, body, app_name, hint_category]);
    query_parser.set_field_boost(summary, 2.);

    let state = GlobalState {
        reader,
        schema,
        parser: query_parser,
    };

    let app = Router::new()
        .route("/api/search", post(search))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3029").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn search(
    State(state): State<GlobalState>,
    Json(payload): Json<Query>,
) -> Json<Vec<serde_json::Value>> {
    state.reader.reload().unwrap();

    let searcher = state.reader.searcher();
    let query = state.parser.parse_query(&payload.query).unwrap();

    let limit = payload.max_hits.unwrap_or(20) as usize;
    let top_docs: Vec<DocAddress> = if let Some(sort_by) = payload.sort_by {
        let sort_order = match payload.sort_order {
            Some(SortOrder::Asc) => Order::Asc,
            _ => Order::Desc,
        };
        searcher
            .search(
                &query,
                &TopDocs::with_limit(limit).order_by_u64_field(sort_by, sort_order),
            )
            .unwrap()
            .into_iter()
            .map(|(_, addr)| addr)
            .collect()
    } else {
        searcher
            .search(&query, &TopDocs::with_limit(limit))
            .unwrap()
            .into_iter()
            .map(|(_, addr)| addr)
            .collect()
    };

    let docs = top_docs
        .into_iter()
        .map(|doc| {
            let doc: TantivyDocument = searcher.doc(doc).unwrap();
            serde_json::from_str(&doc.to_json(&state.schema)).unwrap()
        })
        .collect();

    Json(docs)
}

#[derive(Deserialize)]
struct Query {
    query: String,
    max_hits: Option<u32>,
    sort_by: Option<String>,
    sort_order: Option<SortOrder>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum SortOrder {
    Asc,
    Desc,
}
