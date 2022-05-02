use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    response::{Html, IntoResponse},
    routing::get,
    Extension, Router,
};
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use hyper::{Client, Uri};
use hyper_tls::HttpsConnector;
use serde::Serialize;
use tokio::{
    sync::{watch, Mutex},
    time::sleep,
};

struct State {
    count: usize,
    rx: watch::Receiver<Message>,
}

#[derive(Serialize)]
struct Response {
    count: usize,
    datetime: DateTime<Utc>,
    is_up: bool,
}

#[tokio::main]
async fn main() {
    let (tx, rx) = watch::channel(Message::Text("{}".to_string()));

    let state = Arc::new(Mutex::new(State { count: 0, rx }));

    // Spawn a thread to continually chech the status of the api
    let state_send = state.clone();
    tokio::spawn(async move {
        let duration = Duration::from_secs(5);
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);
        let uri: Uri = "https://api.shuttle.rs/status".parse().unwrap();

        loop {
            let is_up = client.get(uri.clone()).await;
            let is_up = is_up.is_ok();

            let response = Response {
                count: state_send.lock().await.count,
                datetime: Utc::now(),
                is_up,
            };
            let msg = serde_json::to_string(&response).unwrap();

            if tx.send(Message::Text(msg)).is_err() {
                break;
            }

            sleep(duration).await;
        }
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/websocket", get(websocket_handler))
        .layer(Extension(state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<Mutex<State>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: Arc<Mutex<State>>) {
    // By splitting we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    let mut rx = {
        let mut state = state.lock().await;
        state.count += 1;
        state.rx.clone()
    };

    // This task will receive watch messages and forward it to this connected client.
    let mut send_task = tokio::spawn(async move {
        let duration = Duration::from_secs(5);
        while let Ok(()) = rx.changed().await {
            let msg = rx.borrow().clone();

            if sender.send(msg).await.is_err() {
                break;
            }

            sleep(duration).await;
        }
    });

    // This task will receive messages from this client.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            println!("this example does not read any messages, but got: {msg}");
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // This client disconnected
    state.lock().await.count -= 1;
}

async fn index() -> Html<&'static str> {
    Html("<html></html>")
}
