//! WebSocket handler and Redis Pub/Sub integration for real-time updates.

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use redis::AsyncCommands;
use tokio::sync::broadcast;

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

    // Subscribe to the local broadcast channel (populated by the Redis subscriber).
    let mut rx = state.subscribe_local();

    // Spawn a task to forward broadcast events to this client.
    let send_task = tokio::spawn(async move {
        while let Ok(json_text) = rx.recv().await {
            // Die Nachricht ist bereits serialisiert, direkter Pass-Through.
            if sender.send(Message::Text(json_text.into())).await.is_err() {
                break; // Client disconnected.
            }
        }
    });

    // Read incoming messages from client (commands, pings).
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                tracing::debug!("WS received: {}", text);
                // TODO: Parse as GameCommand and push to GameLoopActor's cmd_rx
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
    tracing::debug!("WebSocket client disconnected");
}

/// Hintergrund-Task, der genau einmal pro Server-Instanz gestartet wird.
/// Er verbindet sich mit Redis, lauscht auf den Broadcast-Channel und leitet
/// alles an den lokalen Tokio-Channel weiter.
pub async fn run_redis_subscriber(redis_url: String, local_ws_tx: broadcast::Sender<String>) {
    tracing::info!("Connecting Redis Pub/Sub subscriber...");

    let client = match redis::Client::open(redis_url.clone()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("CRITICAL: Invalid Redis URL: {}", e);
            return;
        }
    };

    let mut con = match client.get_async_pubsub().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("CRITICAL: Failed to connect to Redis Pub/Sub: {}", e);
            return;
        }
    };

    if let Err(e) = con.subscribe("econwar:ws:broadcast").await {
        tracing::error!("CRITICAL: Failed to subscribe to channel: {}", e);
        return;
    }

    tracing::info!("Redis subscriber listening on 'econwar:ws:broadcast'");
    let mut stream = con.on_message();

    while let Some(msg) = stream.next().await {
        if let Ok(payload) = msg.get_payload::<String>() {
            // Fan-out an alle lokal verbundenen Sockets
            let _ = local_ws_tx.send(payload);
        } else {
            tracing::warn!("Received malformed payload from Redis Pub/Sub");
        }
    }

    tracing::error!("Redis Pub/Sub stream closed unexpectedly.");
}