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
