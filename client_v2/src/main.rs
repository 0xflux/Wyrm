use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    routing::{get, post},
    serve,
};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;

use crate::{
    api::{
        dashboard::poll_connected_agents,
        login::try_login,
        pages::{serve_dash, serve_login},
    },
    net::Credentials,
};

mod api;
mod models;
mod net;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //
    // Environment setup
    //

    dotenvy::dotenv()
        .expect("dotenv file must be present and must contain admin token, refer to docs.");

    let static_files = ServeDir::new("static");

    let state = Arc::new(AppState::new());

    //
    // Build the routes
    //

    let app = Router::new()
        .route("/", get(serve_login))
        .route("/dashboard", get(serve_dash))
        .route("/api/do_login", post(try_login))
        .route("/api/dashboard/poll_agents", get(poll_connected_agents))
        .nest_service("/static", static_files)
        .with_state(state.clone());

    //
    // Serve the app
    //

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4040").await?;

    serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

pub struct AppState {
    creds: RwLock<Option<Credentials>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            creds: RwLock::new(Some(Credentials {
                username: "flux".into(),
                password: "password".into(),
                admin_env_token: "fdgiyh%^l!udjfh78364LU7&%df!!".into(),
                c2_url: "http://127.0.0.1:8080".into(),
            })),
        }
    }
}
