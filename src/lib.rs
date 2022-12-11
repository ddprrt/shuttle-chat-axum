use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service},
    Extension, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use sync_wrapper::SyncWrapper;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tower_http::services::ServeDir;

#[shuttle_service::main]
async fn axum() -> shuttle_service::ShuttleAxum {
    let router = router();
    let sync_wrapper = SyncWrapper::new(router);

    Ok(sync_wrapper)
}

static NEXT_USERID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

type Users = Arc<RwLock<HashMap<usize, UnboundedSender<Message>>>>;

fn router() -> Router {
    let directory = get_service(ServeDir::new("static")).handle_error(handle_error);
    let users = Users::default();
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/", directory.clone())
        .layer(Extension(users))
        .fallback(directory)
}

async fn ws_handler(ws: WebSocketUpgrade, Extension(state): Extension<Users>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(ws: WebSocket, state: Users) {
    println!("Hello {:?}", state);
    let my_id = NEXT_USERID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (mut sender, mut receiver) = ws.split();

    let (tx, mut rx): (UnboundedSender<Message>, UnboundedReceiver<Message>) =
        mpsc::unbounded_channel();

    {
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                sender.send(msg).await.expect("Error!");
            }
        });
    }

    if let Ok(mut state) = state.write() {
        state.insert(my_id, tx);
    }

    while let Some(Ok(result)) = receiver.next().await {
        println!("{:?}", result);
        broadcast_msg(result, &state).await;
    }

    disconnect(my_id, &state).await;
}

async fn broadcast_msg(msg: Message, users: &Users) {
    if let Ok(state) = users.read() {
        for (&_uid, tx) in state.iter() {
            tx.send(msg.clone()).expect("Failed to send Message")
        }
    }
}

async fn disconnect(my_id: usize, users: &Users) {
    println!("Good bye user {}", my_id);

    if let Ok(mut state) = users.write() {
        state.remove(&my_id);
    }
}

async fn handle_error(err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err))
}
