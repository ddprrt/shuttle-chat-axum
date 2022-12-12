use std::{collections::HashMap, fs, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
    Extension, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use shuttle_secrets::SecretStore;
use sync_wrapper::SyncWrapper;
use tokio::sync::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    RwLock,
};
use tower_http::{auth::RequireAuthorizationLayer, services::ServeDir};

#[shuttle_service::main]
async fn axum(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_service::ShuttleAxum {
    let secret = secret_store.get("BEARER").unwrap();
    let router = router(secret);
    let sync_wrapper = SyncWrapper::new(router);

    Ok(sync_wrapper)
}

static NEXT_USERID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

static INDEX: &'static str = include_str!("../static/index.html");
static CSS: &'static str = include_str!("../static/main.css");
static JS: &'static str = include_str!("../static/main.js");

type Users = Arc<RwLock<HashMap<usize, UnboundedSender<Message>>>>;

fn router(secret: String) -> Router {
    let directory = get_service(ServeDir::new("static")).handle_error(handle_error);
    let users = Users::default();
    let admin = Router::new()
        .route("/disconnect/:user_id", get(disconnect_user))
        .layer(RequireAuthorizationLayer::bearer(&secret));

    Router::new()
        .route("/ws", get(ws_handler))
        .route("/", get(index))
        .route("/main.css", get(css))
        .route("/main.js", get(js))
        .route("/dbg", get(list))
        .route("/prepare", get(prepare))
        .route("/num", get(num_cpu))
        .nest("/admin", admin)
        .layer(Extension(users))
        .fallback(directory)
}

async fn index() -> impl IntoResponse {
    Html(INDEX)
}

async fn css() -> impl IntoResponse {
    CSS
}

async fn js() -> impl IntoResponse {
    JS
}

async fn list() -> impl IntoResponse {
    let paths = fs::read_dir(".").unwrap();
    let mut s = "".to_string();
    for path in paths {
        s = format!("{}\n{:?}", s, path.unwrap());
    }
    let paths = fs::read_dir("/tmp").unwrap();
    for path in paths {
        s = format!("{}\n{:?}", s, path.unwrap());
    }
    let paths = fs::read_dir("/usr/local/cargo").unwrap();
    for path in paths {
        s = format!("{}\n{:?}", s, path.unwrap());
    }
    for (key, value) in std::env::vars() {
        s = format!("{}\n{}: {}", s, key, value);
    }
    s
}

async fn prepare() -> impl IntoResponse {
    let file = std::path::Path::new("/prepare.sh");
    fs::read_to_string(file).unwrap()
}

async fn num_cpu() -> impl IntoResponse {
    num_cpus::get().to_string()
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

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            sender.send(msg).await.expect("Error!");
        }
        sender.close().await.unwrap();
    });

    state.write().await.insert(my_id, tx);

    while let Some(Ok(result)) = receiver.next().await {
        println!("{:?}", result);
        broadcast_msg(result, &state).await;
    }

    disconnect(my_id, &state).await;
}

async fn broadcast_msg(msg: Message, users: &Users) {
    if let Message::Text(msg) = msg {
        for (&_uid, tx) in users.read().await.iter() {
            tx.send(Message::Text(msg.clone()))
                .expect("Failed to send Message")
        }
    }
}

async fn disconnect_user(
    Path(user_id): Path<usize>,
    Extension(users): Extension<Users>,
) -> impl IntoResponse {
    disconnect(user_id, &users).await;
    "Done"
}

async fn disconnect(my_id: usize, users: &Users) {
    println!("Good bye user {}", my_id);
    users.write().await.remove(&my_id);
    println!("Disconnected {my_id}");
}

async fn handle_error(err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err))
}
