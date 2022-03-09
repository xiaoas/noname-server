// Copyright (c) 2018-2020 Sean McArthur
// Licensed under the MIT license http://opensource.org/licenses/MIT
//
// port from https://github.com/seanmonstar/warp/blob/master/examples/websocket_chat.rs

use salvo::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let router = Router::new()
        .handle(noname_server::user_connected);
    tracing::info!("Listening on http://127.0.0.1:8080");
    Server::new(TcpListener::bind("127.0.0.1:8080")).serve(router).await;
}
