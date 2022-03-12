use std::{sync::{Arc, Weak}, collections::{HashSet, HashMap}};
use serde::{Serialize, Deserialize};
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};
use salvo::extra::ws::Message;
use serde_json::Value;

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
pub enum JoinedRoom { // implement -> Option<RwLock<Room>> ?
    None,
    Guest(String),
    Owner(Room)
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
            JoinedRoom::Owner(room) => Some(&room)
        }
    }
    pub fn send(&self, msg: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
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
        Value::Array(args) => {
            if let Some(Value::String(cmd)) = args.get(0) {
                if cmd == "heartbeat" { // we don't need heartbeat anymore
                    return Ok(())
                }
                if cmd == "key" {
                    return handlers::key(id, &args[1..]).await;
                }
                 // check key
                if super::ONLINE_CLIENTS.read().await.get(id).unwrap().key == "" {
                    return Err(MessageHandleError::Unauthorized);
                }
                
                todo!()
            } else {
                Err(MessageHandleError::InvalidMessageFormat("message command is expected".to_string()))
            }
        },
        _ => Err(MessageHandleError::InvalidMessageFormat("Top level Array is expected".to_string())),
    }
}

mod handlers {
    use serde_json::Value;

    #[derive(Error, Debug)]
    pub enum MessageHandleError {
        #[error("message has format issues: `{0}`")]
        InvalidMessageFormat(String),
        #[error("client is not allowed to send this command")]
        Unauthorized,
    }
    pub async fn key(id: &str, args: &[Value]) -> Result<(), super::MessageHandleError> {
        todo!()
    }
}