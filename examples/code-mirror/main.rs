use axum::extract::ws::WebSocket;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use axum_ycrdt_websocket::broadcast::BroadcastGroup;
use axum_ycrdt_websocket::ws::{AxumSink, AxumStream};
use axum_ycrdt_websocket::AwarenessRef;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::services::ServeDir;
use yrs::sync::Awareness;
use yrs::{Doc, Text, Transact};

const STATIC_FILES_DIR: &str = "examples/code-mirror/frontend/dist";

#[tokio::main]
async fn main() {
    let awareness: AwarenessRef = {
        let doc = Doc::new();
        {
            let txt = doc.get_or_insert_text("codemirror");
            let mut txn = doc.transact_mut();
            txt.push(
                &mut txn,
                r#"function hello() {
  console.log('hello world');
}"#,
            );
        }
        Arc::new(RwLock::new(Awareness::new(doc)))
    };

    let bcast = Arc::new(BroadcastGroup::new(awareness.clone(), 32).await);

    let app: Router = Router::new()
        .route("/ws", get(ws_handler))
        .layer(axum::extract::Extension(bcast))
        .nest_service("/", ServeDir::new(STATIC_FILES_DIR));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, Extension(bcast): Extension<Arc<BroadcastGroup>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| peer(socket, bcast))
}

async fn peer(ws: WebSocket, bcast: Arc<BroadcastGroup>) {
    let (sink, stream) = ws.split();
    let sink = Arc::new(Mutex::new(AxumSink::from(sink)));
    let stream = AxumStream::from(stream);
    let sub = bcast.subscribe(sink, stream);
    match sub.completed().await {
        Ok(_) => println!("broadcasting for channel finished successfully"),
        Err(e) => eprintln!("broadcasting for channel finished abruptly: {}", e),
    }
}