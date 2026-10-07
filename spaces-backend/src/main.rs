mod channel;
mod location;
mod user_state;
mod ws_msg;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{ConnectInfo, State, WebSocketUpgrade, ws::WebSocket},
    response::Response,
    routing,
};
use tokio::net::TcpListener;

use crate::{channel::Channel, location::LocationClient};

const DEFAULT_HOST: &str = "localhost";
const DEFAULT_PORT: u16 = 8923;

#[derive(Clone)]
struct AppState {
    location_client: Arc<LocationClient>,
    channel: Arc<Channel>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let host = DEFAULT_HOST;
    let port = DEFAULT_PORT;

    let sock = TcpListener::bind((host, port)).await.expect("bind socket");

    tracing::info!("listening on {}:{}", host, port);

    let app_state = AppState {
        location_client: Arc::new(LocationClient::new()),
        channel: Arc::new(Channel::new()),
    };
    axum::serve(
        sock,
        Router::new()
            .route("/ws", routing::any(ws_upgrade_handler))
            .with_state(app_state)
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("axum serve");
}

async fn ws_upgrade_handler(
    ws_upgrade: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(sock_addr): ConnectInfo<SocketAddr>,
) -> Response {
    tracing::debug!("received upgrade request");
    ws_upgrade.on_upgrade(move |ws| handle_ws(ws, state, sock_addr))
}

async fn handle_ws(ws: WebSocket, state: AppState, sock_addr: SocketAddr) {
    let ip_string = sock_addr.to_string();
    let ws_id = state.channel.clone().register_socket(ws).await;
    tokio::spawn(async move {
        if let Ok(location) = state
            .location_client
            .query_location(&ip_string)
            .await
            .inspect_err(|e| tracing::warn!(ip = ip_string, error = ?e, "fetch location"))
        {
            state.channel.clone().add_location(ws_id, location).await;
        }
    });
}
