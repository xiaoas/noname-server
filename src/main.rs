use salvo::prelude::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let router = Router::new()
        .handle(noname_server::client_connected);
    tracing::info!("Listening on http://127.0.0.1:8080");
    Server::new(TcpListener::bind("127.0.0.1:8080")).serve(router).await;
}
