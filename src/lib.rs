
use std::collections::HashMap;

use futures_util::{FutureExt, StreamExt};
use once_cell::sync::Lazy;
use rand::distributions::{Distribution, Uniform};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;

use salvo::extra::ws::{Message, WsHandler};
use salvo::prelude::*;

type Users = RwLock<HashMap<String, mpsc::UnboundedSender<Result<Message, salvo::Error>>>>;

static ONLINE_USERS: Lazy<Users> = Lazy::new(|| Users::default());
static ID_SAMPLE_RANGE: Lazy<Uniform<usize>> = Lazy::new(|| Uniform::from(1_000_000_000..10_000_000_000));

#[fn_handler]
pub async fn user_connected(req: &mut Request, res: &mut Response) -> Result<(), HttpError> {
    let fut = WsHandler::new().handle(req, res)?;
    let fut = async move {
        if let Some(ws) = fut.await {
            // know what, we'll use the same algorithm from the proto to generate id
            let my_id = ID_SAMPLE_RANGE.sample(&mut rand::rngs::OsRng);
            let my_id = my_id.to_string();
            tracing::info!("new user: {}", my_id);

            // Split the socket into a sender and receive of messages.
            let (user_ws_tx, mut user_ws_rx) = ws.split();

            // Use an unbounded channel to handle buffering and flushing of messages
            // to the websocket
            let (tx, rx) = mpsc::unbounded_channel();
            let rx = UnboundedReceiverStream::new(rx);
            let fut = rx.forward(user_ws_tx).map(|result| {
                if let Err(e) = result {
                    tracing::error!(error = ?e, "websocket send error");
                }
            });
            tokio::spawn(fut);
            let fut = async move {
                ONLINE_USERS.write().await.insert(my_id.clone(), tx);

                while let Some(result) = user_ws_rx.next().await {
                    let msg = match result {
                        Ok(msg) => msg,
                        Err(e) => {
                            eprintln!("websocket error(uid={}): {}", my_id, e);
                            break;
                        }
                    };
                    user_message(&my_id, msg).await;
                }

                user_disconnected(&my_id).await;
            };
            tokio::spawn(fut);
        }
    };
    tokio::spawn(fut);
    Ok(())
}

async fn user_message(my_id: &str, msg: Message) {
    let msg = if let Some(s) = msg.to_str() {
        s
    } else {
        return;
    };
    let parsed_msg: Vec<String> = if let Ok(s) = serde_json::from_str(msg) { s } else {
        tracing::warn!(my_id, "sent invalid msg");
        return; // forgive & resume the connection
    };
    tracing::trace!(my_id, ?parsed_msg);
    // let new_msg = format!("<User#{}>: {}", my_id, msg);

    // // New message from this user, send it to everyone else (except same uid)...
    // for (&uid, tx) in ONLINE_USERS.read().await.iter() {
    //     if my_id != uid {
    //         if let Err(_disconnected) = tx.send(Ok(Message::text(new_msg.clone()))) {
    //             // The tx is disconnected, our `user_disconnected` code
    //             // should be happening in another task, nothing more to
    //             // do here.
    //         }
    //     }
    // }
}

async fn user_disconnected(my_id: &str) {
    tracing::info!("user disconnect: {}", my_id);
    // Stream closed up, so remove from the user list
    ONLINE_USERS.write().await.remove(my_id);
}
