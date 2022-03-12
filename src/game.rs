use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use salvo::extra::ws::Message;
use serde_json::Value;
use tokio::sync::mpsc;

pub use handlers::MessageHandleError;
pub struct Client {
    pub tx: mpsc::UnboundedSender<Result<Message, salvo::Error>>,
    /// key given when init
    /// also used as room key
    pub key: String,
    pub nick_name: String,
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
            nick_name: String::new(),
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
    pub config: RoomConfig,
    // maybe not
    // members: HashSet<String>,
}

#[derive(Serialize, Deserialize)]
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
                    if let JoinedRoom::Guest(owner_id) = &c.room {
                        args.splice(
                            0..0,
                            [
                                Value::String("onmessage".to_string()),
                                Value::String(id.to_string()),
                            ],
                        );
                        if let Some(Err(error)) = map.get(owner_id).map(|c| c.send(&Value::Array(args))) {
                            tracing::error!(?error, "send onmessage fail");
                            return Err(MessageHandleError::ServerError(error.to_string()));
                        }
                        return Ok(());
                    }
                }

                todo!()
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

mod handlers {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum MessageHandleError {
        #[error("message has format issues: `{0}`")]
        InvalidMessageFormat(String),
        #[error("client is not allowed to send this command")]
        Unauthorized,
        
        #[error("{0}")]
        ServerError(String),
    }
    pub async fn key(id: &str, mut args: Vec<Value>) -> Result<(), MessageHandleError> {
        #[derive(Deserialize)]
        struct Arg1([String; 2]);

        if args.len() == 2 {
            let arg1 = args.pop().unwrap();
            let Arg1([key, version]) = serde_json::from_value(arg1)
                .map_err(|err| MessageHandleError::InvalidMessageFormat(err.to_string()))?;
            // TODO: we have not yet seen the proto's version checking code, so version will not be used
            // TODO: validate the given key against db
            crate::ONLINE_CLIENTS.write().await.get_mut(id).unwrap().key = key;
            return Ok(());
        }
        Err(MessageHandleError::InvalidMessageFormat(
            "invalid key args".to_string(),
        ))
    }
}
