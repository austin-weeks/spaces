mod channel;
mod location;
mod user_state;
mod ws_msg;

use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{Query, State, WebSocketUpgrade, ws::WebSocket},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing,
};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::net::TcpListener;

use crate::{channel::Channel, location::LocationClient};

const DEFAULT_HOST: &str = "localhost";
const DEFAULT_PORT: u16 = 8923;

#[derive(Clone)]
struct AppState {
    location_client: Arc<LocationClient>,
    channel: Arc<Channel>,
    client_secret: Option<Arc<String>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = dotenvy::dotenv() {
        tracing::warn!(error = ?e, "failed to load .env file");
    }

    let client_secret = match env::var("SPACES_CLIENT_SECRET") {
        Ok(s) => Some(s),
        Err(env::VarError::NotPresent) => {
            tracing::warn!("SPACES_CLIENT_SECRET not set");
            None
        }
        Err(e) => {
            tracing::error!(error = ?e, "failed to read SPACES_CLIENT_SECRET env variable");
            None
        }
    };

    let host = env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_owned());
    let port = env::var("PORT")
        .map(|p| p.parse().unwrap())
        .unwrap_or(DEFAULT_PORT);

    let sock = TcpListener::bind(format!("{host}:{port}"))
        .await
        .expect("bind socket");

    tracing::info!("listening on {}:{}", host, port);

    let app_state = AppState {
        location_client: Arc::new(LocationClient::new()),
        channel: Arc::new(Channel::new()),
        client_secret: client_secret.map(Arc::new),
    };
    let service = Router::new()
        .route("/ws", routing::any(ws_upgrade_handler))
        .with_state(app_state)
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(sock, service).await.expect("axum serve");
}

#[derive(Deserialize)]
struct QueryParams {
    secret: Option<String>,
}

async fn ws_upgrade_handler(
    ws_upgrade: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<QueryParams>,
    headers: HeaderMap,
) -> Response {
    tracing::debug!("received upgrade request");
    if let Some(required_secret) = &state.client_secret {
        let Some(secret) = params.secret else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        if secret != required_secret.as_str() {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    let ip_addr = headers
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .map(ToOwned::to_owned);

    ws_upgrade.on_upgrade(move |ws| handle_ws(ws, state, ip_addr))
}

async fn handle_ws(ws: WebSocket, state: AppState, ip_addr: Option<String>) {
    let ws_id = state.channel.clone().register_socket(ws).await;
    if let Some(ip_string) = ip_addr {
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
}
