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

mod handlers {
    use std::collections::HashMap;

    use super::JoinedRoom;
    use once_cell::sync::Lazy;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use thiserror::Error;
    #[derive(Error, Debug)]
    pub enum MessageHandleError {
        #[error("message has format issues: `{0}`")]
        InvalidMessageFormat(String),
        #[error("client is not allowed to send this command")]
        Unauthorized,
        #[error("{0}")]
        ServerError(String),
        #[error("{0}")]
        ClientError(String),
        #[error("game in invalid state: {0}")]
        GameStateError(String),
    }
    // pub static MESSAGE_HANDLERS: Lazy<HashMap<&str, fn(&str, Vec<Value>) -> Result<Value, MessageHandleError>>> =
    // Lazy::new(|| HashMap::from([
    //     ("create", create),
    // ]));
    pub async fn key(id: &str, mut args: Vec<Value>) -> Result<(), MessageHandleError> {
        #[derive(Deserialize)]
        struct Arg1([String; 2]);

        if args.len() == 2 {
            let arg1 = args.pop().unwrap();
            let Arg1([key, version]) = serde_json::from_value(arg1)
                .map_err(|err| MessageHandleError::InvalidMessageFormat(err.to_string()))?;

            tracing::debug!(handler = "key", %key, %version);

            // TODO: we have not yet seen the proto's version checking code, so version will not be used
            // TODO: validate the given key against db
            crate::ONLINE_CLIENTS.write().await.get_mut(id).unwrap().key = key;
            return Ok(());
        }
        Err(MessageHandleError::InvalidMessageFormat(
            "invalid key args".to_string(),
        ))
    }
    pub async fn create(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        #[derive(Deserialize)]
        struct Args(
            String,
            String,
            String,
            String,
            String,
            Option<super::RoomConfig>,
            Option<String>,
        );
        let Args(_, _, key, nickname, avatar, config, mode) =
            serde_json::from_value(Value::Array(args))
                .map_err(|err| MessageHandleError::InvalidMessageFormat(err.to_string()))?;

        tracing::debug!(handler = "create", %key, %nickname, %avatar, ?config, ?mode);

        let new_room = super::Room { config };
        let mut clients = crate::ONLINE_CLIENTS.write().await;
        let client = clients.get_mut(id).unwrap();
        client.nickname = nickname;
        client.avatar = avatar;
        if let JoinedRoom::None = client.room {
            return Err(MessageHandleError::ClientError("is in room".to_string()));
        }
        client.room = JoinedRoom::Owner(new_room);
        Ok(json!(["createroom", client.key.clone(),]))
    }
    pub async fn enter(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        let [_, _, key, nickname, avatar]: [String; 5] = serde_json::from_value(Value::Array(args))
            .map_err(|err| MessageHandleError::InvalidMessageFormat(err.to_string()))?;

        tracing::debug!(handler = "create", %key, %nickname, %avatar);

        let owner_id;
        let err_resp = || Ok(json!(["enterroomfailed"]));
        {
            let clients = crate::ONLINE_CLIENTS.read().await;
            let this_id;
            let owner_client;
            (this_id, owner_client) = if let Some(v) = clients.iter().find(|&(_, c)| c.key == key) {
                v
            } else {
                return err_resp();
            };
            owner_id = this_id.clone();
            if let JoinedRoom::Owner(super::Room{config: Some(config)}) = &owner_client.room {
                if !config.game_started || config.observe && config.observe_ready {
                    if let Err(error) = owner_client.send(&json!(["onconnection", id])) {
                        // client disconnected before response
                        tracing::info!(owner= %owner_id, ?error, "send onconnection fail");
                        return Err(MessageHandleError::ServerError(error.to_string()));
                    }
                } else {
                    return err_resp();
                }
            } else {
                return err_resp();
            }
        }
        let mut clients = crate::ONLINE_CLIENTS.write().await;
        let client = clients.get_mut(id).unwrap();
        client.room = JoinedRoom::Guest(owner_id);
        err_resp()
    }
    pub async fn changeAvatar(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
    pub async fn server(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
    pub async fn events(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
    pub async fn config(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
    pub async fn status(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
    pub async fn send(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
    pub async fn close(id: &str, args: Vec<Value>) -> Result<Value, MessageHandleError> {
        todo!()
    }
}
