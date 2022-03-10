use std::sync::{Arc, Weak};

use tokio::sync::{mpsc, RwLock};

use salvo::extra::ws::Message;

pub struct Client {
    tx: mpsc::UnboundedSender<Result<Message, salvo::Error>>,
    nick_name: String,
    avatar: String,
    room: JoinedRoom,
}
pub enum JoinedRoom { // implement -> Option<RwLock<Room>> ?
    Guest(Weak<RwLock<Room>>),
    Owner(Arc<RwLock<Room>>)
}
pub struct Room {

}
impl Drop for Room {
    fn drop(&mut self) {
        todo!()
    }
}