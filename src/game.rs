use std::{sync::{Arc, Weak}, collections::{HashSet, HashMap}};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, RwLock};
use salvo::extra::ws::Message;

pub struct Client {
    pub tx: mpsc::UnboundedSender<Result<Message, salvo::Error>>,
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
    pub async fn room(&self, clients: &HashMap<String, Client>) -> Option<&Room> {
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