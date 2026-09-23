use net::{Client, NetError};
use protocol::{
    ClientMessage, GameDurationMinutes, GameMode, RoomInfo, ServerMessage,
};
use std::sync::Mutex;
use std::thread;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub enum NetCommand {
    ConnectAndCreate {
        url: String,
        nickname: String,
        duration: GameDurationMinutes,
    },
    ConnectAndJoin {
        url: String,
        nickname: String,
        code: String,
    },
    StartGame,
    Leave,
    Relay {
        payload: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub enum NetEvent {
    Connected {
        player_id: String,
    },
    RoomUpdated(RoomInfo),
    GameStarted(RoomInfo),
    PlayerLeft {
        player_id: String,
        nickname: String,
        forfeited: bool,
    },
    Relayed {
        from_player_id: String,
        payload: Vec<u8>,
    },
    Error(String),
    Disconnected,
}

#[derive(bevy::prelude::Resource)]
pub struct NetBridge {
    cmd_tx: UnboundedSender<NetCommand>,
    event_rx: Mutex<UnboundedReceiver<NetEvent>>,
}

impl NetBridge {
    pub fn spawn() -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        let (event_tx, event_rx) = unbounded_channel();
        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            runtime.block_on(net_loop(cmd_rx, event_tx));
        });
        Self {
            cmd_tx,
            event_rx: Mutex::new(event_rx),
        }
    }

    pub fn send(&self, command: NetCommand) {
        let _ = self.cmd_tx.send(command);
    }

    pub fn poll(&self) -> Option<NetEvent> {
        self.event_rx.lock().ok()?.try_recv().ok()
    }
}

async fn net_loop(mut cmd_rx: UnboundedReceiver<NetCommand>, event_tx: UnboundedSender<NetEvent>) {
    let mut client: Option<Client> = None;

    loop {
        tokio::select! {
            command = cmd_rx.recv() => {
                let Some(command) = command else {
                    break;
                };
                match command {
                    NetCommand::Leave => {
                        if let Some(mut active) = client.take() {
                            let _ = active.send(ClientMessage::Leave).await;
                        }
                        let _ = event_tx.send(NetEvent::Disconnected);
                    }
                    NetCommand::StartGame => {
                        if let Some(active) = client.as_mut() {
                            if let Err(err) = active.send(ClientMessage::StartGame).await {
                                let _ = event_tx.send(NetEvent::Error(err.to_string()));
                            }
                        } else {
                            let _ = event_tx.send(NetEvent::Error("not connected".into()));
                        }
                    }
                    NetCommand::Relay { payload } => {
                        if let Some(active) = client.as_mut() {
                            if let Err(err) = active.send(ClientMessage::Relay { payload }).await {
                                let _ = event_tx.send(NetEvent::Error(err.to_string()));
                            }
                        }
                    }
                    NetCommand::ConnectAndCreate { url, nickname, duration } => {
                        client = None;
                        match connect_and_create(&url, nickname, duration).await {
                            Ok((connected, room)) => {
                                let player_id = connected.player_id.clone();
                                client = Some(connected);
                                let _ = event_tx.send(NetEvent::Connected { player_id });
                                let _ = event_tx.send(NetEvent::RoomUpdated(room));
                            }
                            Err(err) => {
                                let _ = event_tx.send(NetEvent::Error(err.to_string()));
                            }
                        }
                    }
                    NetCommand::ConnectAndJoin { url, nickname, code } => {
                        client = None;
                        match connect_and_join(&url, nickname, code).await {
                            Ok((connected, room)) => {
                                let player_id = connected.player_id.clone();
                                client = Some(connected);
                                let _ = event_tx.send(NetEvent::Connected { player_id });
                                let _ = event_tx.send(NetEvent::RoomUpdated(room));
                            }
                            Err(err) => {
                                let _ = event_tx.send(NetEvent::Error(err.to_string()));
                            }
                        }
                    }
                }
            }
            result = recv_optional(&mut client), if client.is_some() => {
                match result {
                    Ok(ServerMessage::RoomUpdated { room }) => {
                        let _ = event_tx.send(NetEvent::RoomUpdated(room));
                    }
                    Ok(ServerMessage::GameStarted { room }) => {
                        let _ = event_tx.send(NetEvent::GameStarted(room));
                    }
                    Ok(ServerMessage::PlayerLeft { player_id, nickname, forfeited }) => {
                        let _ = event_tx.send(NetEvent::PlayerLeft {
                            player_id,
                            nickname,
                            forfeited,
                        });
                    }
                    Ok(ServerMessage::Error { message }) => {
                        let _ = event_tx.send(NetEvent::Error(message));
                    }
                    Ok(ServerMessage::Relayed {
                        from_player_id,
                        payload,
                    }) => {
                        let _ = event_tx.send(NetEvent::Relayed {
                            from_player_id,
                            payload,
                        });
                    }
                    Ok(ServerMessage::Welcome { .. }) => {}
                    Err(err) => {
                        client = None;
                        let _ = event_tx.send(NetEvent::Error(err.to_string()));
                        let _ = event_tx.send(NetEvent::Disconnected);
                    }
                }
            }
        }
    }
}

async fn recv_optional(client: &mut Option<Client>) -> Result<ServerMessage, NetError> {
    client
        .as_mut()
        .expect("client checked by select")
        .recv()
        .await
}

async fn connect_and_create(
    url: &str,
    nickname: String,
    duration: GameDurationMinutes,
) -> Result<(Client, RoomInfo), NetError> {
    let mut client = Client::connect(url).await?;
    client
        .send(ClientMessage::CreateRoom {
            nickname,
            mode: GameMode::Competitive,
            duration_minutes: duration,
        })
        .await?;
    let room = wait_room(&mut client).await?;
    Ok((client, room))
}

async fn connect_and_join(
    url: &str,
    nickname: String,
    code: String,
) -> Result<(Client, RoomInfo), NetError> {
    let mut client = Client::connect(url).await?;
    client
        .send(ClientMessage::JoinRoom { code, nickname })
        .await?;
    let room = wait_room(&mut client).await?;
    Ok((client, room))
}

async fn wait_room(client: &mut Client) -> Result<RoomInfo, NetError> {
    loop {
        match client.recv().await? {
            ServerMessage::RoomUpdated { room } => return Ok(room),
            ServerMessage::Error { message } => return Err(NetError::Protocol(message)),
            ServerMessage::Welcome { .. }
            | ServerMessage::GameStarted { .. }
            | ServerMessage::PlayerLeft { .. }
            | ServerMessage::Relayed { .. } => continue,
        }
    }
}
