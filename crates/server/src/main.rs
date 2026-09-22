mod room;
mod state;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage, PROTOCOL_VERSION};
use state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "astrowar_server=info,tower_http=info".into()),
        )
        .init();

    let host = std::env::var("ASTROWAR_SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("ASTROWAR_SERVER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    let room_code_length: usize = std::env::var("ASTROWAR_ROOM_CODE_LENGTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    let room_ttl_secs: u64 = std::env::var("ASTROWAR_ROOM_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600);

    let state = AppState::new(room_code_length, room_ttl_secs);
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("invalid ASTROWAR_SERVER_HOST/PORT");
    info!(%addr, "AstroWar server listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let player_id = Uuid::new_v4().to_string();
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let welcome = ServerMessage::Welcome {
        protocol_version: PROTOCOL_VERSION,
        player_id: player_id.clone(),
    };
    if sink
        .send(Message::Text(
            serde_json::to_string(&welcome).expect("serialize").into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let Ok(text) = serde_json::to_string(&msg) else {
                continue;
            };
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    let mut current_room: Option<String> = None;
    let connections = Arc::clone(&state.connections);
    connections
        .lock()
        .await
        .insert(player_id.clone(), tx.clone());

    while let Some(Ok(message)) = stream.next().await {
        let Message::Text(text) = message else {
            continue;
        };

        let parsed: Result<ClientMessage, _> = serde_json::from_str(&text);
        let client_msg = match parsed {
            Ok(msg) => msg,
            Err(err) => {
                let _ = tx.send(ServerMessage::Error {
                    message: format!("invalid message: {err}"),
                });
                continue;
            }
        };

        match client_msg {
            ClientMessage::Hello { protocol_version } => {
                if protocol_version != PROTOCOL_VERSION {
                    let _ = tx.send(ServerMessage::Error {
                        message: format!(
                            "protocol mismatch: server={PROTOCOL_VERSION} client={protocol_version}"
                        ),
                    });
                }
            }
            ClientMessage::CreateRoom {
                nickname,
                mode,
                duration_minutes,
            } => {
                match state
                    .create_room(player_id.clone(), nickname, mode, duration_minutes)
                    .await
                {
                    Ok(room) => {
                        current_room = Some(room.code.clone());
                        let _ = tx.send(ServerMessage::RoomUpdated { room });
                    }
                    Err(err) => {
                        let _ = tx.send(ServerMessage::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
            ClientMessage::JoinRoom { code, nickname } => {
                match state
                    .join_room(code.clone(), player_id.clone(), nickname)
                    .await
                {
                    Ok(room) => {
                        current_room = Some(room.code.clone());
                        broadcast_room(&state, &room).await;
                    }
                    Err(err) => {
                        let _ = tx.send(ServerMessage::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
            ClientMessage::StartGame => {
                let Some(code) = current_room.clone() else {
                    let _ = tx.send(ServerMessage::Error {
                        message: "not in a room".into(),
                    });
                    continue;
                };
                match state.start_game(&code, &player_id).await {
                    Ok(room) => {
                        broadcast_started(&state, &room).await;
                    }
                    Err(err) => {
                        let _ = tx.send(ServerMessage::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
            ClientMessage::Leave => {
                if let Some(code) = current_room.take() {
                    handle_leave(&state, &code, &player_id).await;
                }
            }
            ClientMessage::Relay { payload } => {
                let Some(code) = current_room.clone() else {
                    continue;
                };
                if let Err(err) = state.relay(&code, &player_id, payload).await {
                    let _ = tx.send(ServerMessage::Error {
                        message: err.to_string(),
                    });
                }
            }
        }
    }

    if let Some(code) = current_room.take() {
        handle_leave(&state, &code, &player_id).await;
    }
    connections.lock().await.remove(&player_id);
    writer.abort();
    warn!(%player_id, "client disconnected");
}

async fn handle_leave(state: &AppState, code: &str, player_id: &str) {
    if let Ok(Some((room, left))) = state.leave_room(code, player_id).await {
        let msg = ServerMessage::PlayerLeft {
            player_id: left.id.clone(),
            nickname: left.nickname.clone(),
            forfeited: room.phase == protocol::RoomPhase::Playing,
        };
        broadcast_raw(state, &room, msg).await;
        if !room.players.is_empty() {
            broadcast_room(state, &room).await;
        }
    }
}

async fn broadcast_room(state: &AppState, room: &protocol::RoomInfo) {
    broadcast_raw(
        state,
        room,
        ServerMessage::RoomUpdated {
            room: room.clone(),
        },
    )
    .await;
}

async fn broadcast_started(state: &AppState, room: &protocol::RoomInfo) {
    broadcast_raw(
        state,
        room,
        ServerMessage::GameStarted {
            room: room.clone(),
        },
    )
    .await;
}

async fn broadcast_raw(state: &AppState, room: &protocol::RoomInfo, msg: ServerMessage) {
    let connections = state.connections.lock().await;
    for player in &room.players {
        if let Some(tx) = connections.get(&player.id) {
            let _ = tx.send(msg.clone());
        }
    }
}
