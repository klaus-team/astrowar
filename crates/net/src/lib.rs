use futures_util::{SinkExt, StreamExt};
use protocol::{
    decode_server_relayed, encode_client_relay, ClientMessage, ServerMessage, PROTOCOL_VERSION,
};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use url::Url;

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("invalid server url: {0}")]
    InvalidUrl(String),
    #[error(transparent)]
    Connect(#[from] tokio_tungstenite::tungstenite::Error),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("connection closed")]
    Closed,
    #[error("protocol error: {0}")]
    Protocol(String),
}

pub fn default_server_url() -> String {
    if let Ok(url) = std::env::var("ASTROWAR_DEFAULT_SERVER_URL") {
        if !url.is_empty() {
            return url;
        }
    }
    if let Some(url) = option_env!("ASTROWAR_DEFAULT_SERVER_URL") {
        if !url.is_empty() {
            return url.to_string();
        }
    }
    "ws://127.0.0.1:8080/ws".to_string()
}

pub struct Client {
    stream: WsStream,
    pub player_id: String,
    pub server_url: String,
}

impl Client {
    pub async fn connect(server_url: &str) -> Result<Self, NetError> {
        Url::parse(server_url).map_err(|e| NetError::InvalidUrl(e.to_string()))?;
        let (mut stream, _) = connect_async(server_url).await?;
        let player_id = match wait_welcome(&mut stream).await? {
            ServerMessage::Welcome {
                protocol_version,
                player_id,
            } => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(NetError::Protocol(format!(
                        "protocol mismatch: server={protocol_version} client={PROTOCOL_VERSION}"
                    )));
                }
                player_id
            }
            other => {
                return Err(NetError::Protocol(format!(
                    "expected welcome, got {other:?}"
                )))
            }
        };

        let mut client = Self {
            stream,
            player_id,
            server_url: server_url.to_string(),
        };
        client
            .send(ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
            })
            .await?;
        Ok(client)
    }

    pub async fn send(&mut self, message: ClientMessage) -> Result<(), NetError> {
        match message {
            ClientMessage::Relay { payload } => {
                let frame = encode_client_relay(&payload);
                self.stream.send(Message::Binary(frame.into())).await?;
            }
            other => {
                let text = serde_json::to_string(&other)?;
                self.stream.send(Message::Text(text.into())).await?;
            }
        }
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<ServerMessage, NetError> {
        while let Some(frame) = self.stream.next().await {
            let message = frame?;
            match message {
                Message::Text(text) => {
                    return Ok(serde_json::from_str(&text)?);
                }
                Message::Binary(bytes) => {
                    let (from_player_id, payload) = decode_server_relayed(&bytes)
                        .map_err(|err| NetError::Protocol(err.to_string()))?;
                    return Ok(ServerMessage::Relayed {
                        from_player_id,
                        payload,
                    });
                }
                Message::Close(_) => return Err(NetError::Closed),
                _ => continue,
            }
        }
        Err(NetError::Closed)
    }
}

async fn wait_welcome(stream: &mut WsStream) -> Result<ServerMessage, NetError> {
    while let Some(frame) = stream.next().await {
        let message = frame?;
        match message {
            Message::Text(text) => return Ok(serde_json::from_str(&text)?),
            Message::Close(_) => return Err(NetError::Closed),
            _ => continue,
        }
    }
    Err(NetError::Closed)
}
