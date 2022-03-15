use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use salvo::extra::ws::Message;
use serde_json::Value;
use tokio::sync::mpsc;
mod handlers;
pub use handlers::MessageHandleError;
pub struct Client {
    pub tx: mpsc::UnboundedSender<Result<Message, salvo::Error>>,
    /// key given when init
    /// also used as room key
    pub key: String,
    pub nickname: String,
    pub avatar: String,
    pub status: String,
    room: JoinedRoom,
}
pub enum JoinedRoom {
    // implement -> Option<RwLock<Room>> ?
    None,
    // owner ID instead of owner key!
    Guest(String),
    Owner(Room),
}
impl Client {
    pub fn new(tx: mpsc::UnboundedSender<Result<Message, salvo::Error>>) -> Self {
        Self {
            tx,
            key: String::new(),
            nickname: String::new(),
            avatar: String::new(),
            status: String::new(),
            room: JoinedRoom::None,
        }
    }
    pub async fn room<'a>(&'a self, clients: &'a HashMap<String, Client>) -> Option<&'a Room> {
        match &self.room {
            JoinedRoom::None => None,
            JoinedRoom::Guest(owner_key) => {
                let clients = clients;
                let joined_room = &clients.get(owner_key)?.room;
                if let JoinedRoom::Owner(room) = joined_room {
                    Some(room)
                } else {
                    None
                }
            }
            JoinedRoom::Owner(room) => Some(&room),
        }
    }
    pub fn send(&self, msg: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        let msg = msg.to_string();
        self.tx.send(Ok(Message::text(msg)))?;
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
pub struct Room {
    /// a room without config is not joinable by others
    pub config: Option<RoomConfig>,
    // maybe not
    // members: HashSet<String>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoomConfig {
    pub game_started: bool,
    pub observe: bool,
    pub observe_ready: bool,
}

pub async fn handle_message(id: &str, msg: Value) -> Result<(), MessageHandleError> {
    match msg {
        Value::Array(mut args) => {
            if let Some(Value::String(cmd)) = args.get(0) {
                if cmd == "heartbeat" {
                    // we don't need heartbeat anymore
                    return Ok(());
                }
                if cmd == "server" && args.get(1) == Some(&Value::String("cmd".to_string())) {
                    return handlers::key(id, args).await;
                }
                {
                    // check key
                    let map = crate::ONLINE_CLIENTS.read().await;
                    let c = map.get(id).unwrap();
                    if c.key.is_empty() {
                        return Err(MessageHandleError::Unauthorized);
                    }
                    // has owner: directly forward with onmessage
                    if let JoinedRoom::Guest(owner_id) = &c.room {
                        args.splice(
                            0..0,
                            [
                                Value::String("onmessage".to_string()),
                                Value::String(id.to_string()),
                            ],
                        );
                        if let Some(Err(error)) =
                            map.get(owner_id).map(|c| c.send(&Value::Array(args)))
                        {
                            tracing::error!(%error, "send onmessage fail");
                            return Err(MessageHandleError::ServerError(error.to_string()));
                        }
                        return Ok(());
                    }
                }
                if cmd == "server" {
                    Err(MessageHandleError::InvalidMessageFormat(
                        "message command should start with server".to_string(),
                    ))
                } else {
                    let cmd = if let Some(Value::String(cmd)) = args.get(1) {
                        cmd
                    } else {
                        return Err(MessageHandleError::InvalidMessageFormat(
                            "message command invalid".to_string(),
                        ));
                    };
                    let resp = match &cmd[..] {
                        // "key" => handlers::key(id, args).await, // handled already
                        "create" => handlers::create(id, args).await,
                        "enter" => handlers::enter(id, args).await,
                        "changeAvatar" => handlers::changeAvatar(id, args).await,
                        "server" => handlers::server(id, args).await,
                        "events" => handlers::events(id, args).await,
                        "config" => handlers::config(id, args).await,
                        "status" => handlers::status(id, args).await,
                        "send" => handlers::send(id, args).await,
                        "close" => handlers::close(id, args).await,

                        _ => Err(MessageHandleError::InvalidMessageFormat(
                            "message command invalid".to_string(),
                        )),
                    }?;
                    // respond
                    if let Some(Err(error)) = crate::ONLINE_CLIENTS
                        .read()
                        .await
                        .get(id)
                        .map(|c| c.send(&resp))
                    {
                        // client disconnected before response
                        tracing::warn!(?error, "send response fail");
                        return Err(MessageHandleError::ServerError(error.to_string()));
                    }
                    Ok(())
                }
            } else {
                Err(MessageHandleError::InvalidMessageFormat(
                    "message command is expected".to_string(),
                ))
            }
        }
        _ => Err(MessageHandleError::InvalidMessageFormat(
            "Top level Array is expected".to_string(),
        )),
    }
}