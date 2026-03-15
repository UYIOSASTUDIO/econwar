//! WebSocket handler for real-time updates.
//!
//! Clients connect to /ws and receive a stream of `ServerEvent`s.
//! They can also send commands over the WebSocket as an alternative
//! to the REST API.

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};

use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new().route("/ws", get(ws_handler))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: SharedState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to the broadcast channel.
    let mut rx = state.subscribe();

    // Spawn a task to forward broadcast events to this client.
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if sender.send(Message::Text(json.into())).await.is_err() {
                break; // Client disconnected.
            }
        }
    });

    // Read incoming messages from client (commands, pings).
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                tracing::debug!("WS received: {}", text);
                // Future: parse as GameCommand and execute.
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Clean up when client disconnects.
    send_task.abort();
    tracing::debug!("WebSocket client disconnected");
}
