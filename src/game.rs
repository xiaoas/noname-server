use std::{sync::{Arc, Weak}, collections::{HashSet, HashMap}};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, RwLock};
use salvo::extra::ws::Message;
use serde_json::Value;
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

pub fn handle_message(id: &str, msg: Value) -> Result<(), Box<dyn std::error::Error>> {
    match msg {
        Value::Array()
        _: 
    }
}