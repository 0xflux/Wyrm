use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware::from_fn_with_state,
    routing::{get, post},
    serve,
};
use shared::pretty_print::print_info;
use tower_http::services::ServeDir;

use crate::{
    api::{
        dashboard::{
            close_tab, draw_tabs, poll_connected_agents, select_agent_tab, select_agent_tab_idx,
            send_command, show_implant_messages,
        },
        file_upload::upload_file_api,
        login::try_login,
        pages::{
            build_all_profiles_page, logout, serve_dash, serve_login, staged_resources_page,
            upload_file_page,
        },
        profile_builder::build_all_profiles,
        staged_resources::fetch_staged_resources,
    },
    middleware::check_logged_in,
    models::AppState,
};

mod api;
mod middleware;
mod models;
mod net;
mod tasks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //
    // Environment setup
    //
    print_info("Starting Wyrm GUI");

    let static_files = ServeDir::new("static");

    let state = Arc::new(AppState::new());
    let max_upload_mb = 50000000; // this is stupidly big, and will be controlled on the C2.

    println!("Max upload {max_upload_mb}");

    //
    // Build the routes
    //

    let app = Router::new()
        .route("/", get(serve_login))
        .route("/dashboard", get(serve_dash))
        .route("/file_upload", get(upload_file_page))
        .route("/build_profiles", get(build_all_profiles_page))
        .route("/staged_resources", get(staged_resources_page))
        .route("/logout", get(logout))
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
        .route("/api/stage_all", post(build_all_profiles))
        .route("/api/list_staged_resources", get(fetch_staged_resources))
        //
        // Static content
        //
        .nest_service("/static", static_files)
        // Max upload sz of 500 MB
        .layer(DefaultBodyLimit::max(max_upload_mb * 1024 * 1024))
        .layer(from_fn_with_state(state.clone(), check_logged_in))
        .with_state(state.clone());

    //
    // Serve the app
    //

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4040").await?;

    print_info("Started app..");

    serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
