pub mod game;
use std::collections::HashMap;

use futures_util::{FutureExt, StreamExt};
use once_cell::sync::Lazy;
use rand::distributions::{Distribution, Uniform};
use serde_json::json;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;

use salvo::extra::ws::{Message, WsHandler};
use salvo::prelude::*;

/// change to std::sync::RwLock might push performance to its limit
type Clients = RwLock<HashMap<String, game::Client>>;

static ONLINE_CLIENTS: Lazy<Clients> = Lazy::new(|| Clients::default());
static ID_SAMPLE_RANGE: Lazy<Uniform<usize>> =
    Lazy::new(|| Uniform::from(1_000_000_000..10_000_000_000));

#[fn_handler]
pub async fn client_connected(req: &mut Request, res: &mut Response) -> Result<(), HttpError> {
    let fut = WsHandler::new().handle(req, res)?;
    let fut = async move {
        if let Some(ws) = fut.await {
            // know what, we'll use the same algorithm from the proto to generate id
            let my_id = ID_SAMPLE_RANGE.sample(&mut rand::rngs::OsRng);
            let my_id = my_id.to_string();
            tracing::info!("new client: {}", my_id);

            // Split the socket into a sender and receive of messages.
            let (client_ws_tx, mut client_ws_rx) = ws.split();

            // Use an unbounded channel to handle buffering and flushing of messages
            // to the websocket
            let (tx, rx) = mpsc::unbounded_channel();
            let rx = UnboundedReceiverStream::new(rx);
            tokio::spawn(rx.forward(client_ws_tx).map(|result| {
                if let Err(e) = result {
                    tracing::error!(error = ?e, "websocket send error");
                }
            }));
            let fut = async move {
                ONLINE_CLIENTS
                    .write()
                    .await
                    .insert(my_id.clone(), game::Client::new(tx));
                if let Err(error) = client_welcome(&my_id).await {
                    tracing::warn!(?my_id, ?error, "client welcome error");
                    return;
                }
                while let Some(result) = client_ws_rx.next().await {
                    let msg = match result {
                        Ok(msg) => msg,
                        Err(e) => {
                            eprintln!("websocket error(uid={}): {}", my_id, e);
                            break;
                        }
                    };
                    if let Err(error) = client_message(&my_id, msg).await {
                        // client probably done somthing bad...
                        tracing::warn!(?my_id, ?error, "client message error");
                        break;
                    }
                }

                client_disconnected(&my_id).await;
            };
            tokio::spawn(fut);
        }
    };
    tokio::spawn(fut);
    Ok(())
}

/// Greet message
/// The only time we need to send the client a message actively is this roomlist msg.
/// all other data sent to the client are *pong* parts.
async fn client_welcome(my_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let msg = json!([
        "roomlist",
        [], // TODO
        [],
        [],
        my_id,
    ]);
    ONLINE_CLIENTS.read().await.get(my_id).unwrap().send(&msg)?;
    Ok(())
}
/// Beware that client might send a invalid message
async fn client_message(my_id: &str, msg: Message) -> Result<(), game::MessageHandleError> {
    let msg = if let Some(s) = msg.to_str() {
        s
    } else {
        return Err(game::MessageHandleError::InvalidMessageFormat(
            "invalid body type".to_string(),
        ));
    };
    let parsed_msg: serde_json::Value = if let Ok(s) = serde_json::from_str(msg) {
        s
    } else {
        tracing::warn!(my_id, "invalid json");
        return Err(game::MessageHandleError::InvalidMessageFormat(
            "invalid json".to_string(),
        ));
    };
    tracing::trace!(my_id, ?parsed_msg);
    game::handle_message(my_id, parsed_msg).await
    // let new_msg = format!("<Client#{}>: {}", my_id, msg);

    // // New message from this client, send it to everyone else (except same uid)...
    // for (&uid, tx) in ONLINE_CLIENTS.read().await.iter() {
    //     if my_id != uid {
    //         if let Err(_disconnected) = tx.send(Ok(Message::text(new_msg.clone()))) {
    //             // The tx is disconnected, our `client_disconnected` code
    //             // should be happening in another task, nothing more to
    //             // do here.
    //         }
    //     }
    // }
}

async fn client_disconnected(my_id: &str) {
    tracing::info!("client disconnect: {}", my_id);
    // Stream closed up, so remove from the client list
    ONLINE_CLIENTS.write().await.remove(my_id);
}
