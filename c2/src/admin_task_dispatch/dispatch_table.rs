use std::sync::Arc;

use crate::{
    admin_task_dispatch::{
        delete_staged_resources, drop_file_handler, execute::dotex, export_completed_tasks_to_json,
        implant_builder::stage_file_upload_from_users_disk, list_agents, list_staged_resources,
        remove_agent_from_list, show_server_time, task_agent, task_agent_sleep,
    },
    app_state::AppState,
    logging::log_error_async,
};
use axum::extract::State;
use serde_json::Value;
use shared::tasks::{AdminCommand, Command};

/// Main dispatcher for admin commands issued on the server, which may, or may not, include an
/// implant UID.
pub async fn admin_dispatch(
    uid: Option<String>,
    command: AdminCommand,
    state: State<Arc<AppState>>,
) -> Vec<u8> {
    // Note, due to the use of generics with the function `task_agent`, if you are passing `None`
    // into the function, you will have to turbofish a type which does implement ToString - so,
    // to keep it simple, just turbofish String - it will be discarded as the `None` path will be
    // taken
    let result: Option<Value> = match command {
        AdminCommand::Sleep(time) => task_agent_sleep(time, uid.unwrap(), state).await,
        AdminCommand::ListAgents => list_agents(state).await,
        AdminCommand::ListProcesses => {
            task_agent::<String>(Command::Ps, None, uid.unwrap(), state).await
        }
        AdminCommand::GetUsername => todo!(),
        AdminCommand::ListUsersDirs => {
            task_agent::<String>(Command::Pillage, None, uid.unwrap(), state).await
        }
        AdminCommand::Pwd => task_agent::<String>(Command::Pwd, None, uid.unwrap(), state).await,
        AdminCommand::Cd(path_buf) => {
            task_agent(Command::Cd, Some(path_buf), uid.unwrap(), state).await
        }
        AdminCommand::KillAgent => {
            task_agent::<String>(Command::KillAgent, None, uid.unwrap(), state).await
        }
        AdminCommand::Ls => task_agent::<String>(Command::Ls, None, uid.unwrap(), state).await,
        AdminCommand::ShowServerTime => show_server_time(),
        AdminCommand::Login => Some(serde_json::to_value("success").unwrap()),
        AdminCommand::ListStagedResources => list_staged_resources(state).await,
        AdminCommand::Run(args) => task_agent(Command::Run, Some(args), uid.unwrap(), state).await,
        AdminCommand::DeleteStagedResource(download_endpoint) => {
            delete_staged_resources(state, download_endpoint).await
        }
        AdminCommand::RemoveAgentFromList => remove_agent_from_list(state, uid.unwrap()).await,
        AdminCommand::Undefined => panic!("This should never happen."),
        AdminCommand::StageFileOnC2(metadata) => {
            stage_file_upload_from_users_disk(metadata, state).await
        }
        AdminCommand::KillProcessById(pid) => {
            task_agent::<String>(Command::KillProcess, Some(pid), uid.unwrap(), state).await
        }
        AdminCommand::Drop(data) => drop_file_handler(uid, data, state).await,
        AdminCommand::Copy(inner) => {
            // Serialise the (String, String) to just a String so we can use it with the
            // generic task_agent.
            let inner_serialised = match serde_json::to_string(&inner) {
                Ok(s) => Some(s),
                Err(e) => {
                    log_error_async(&e.to_string()).await;
                    None
                }
            };

            if inner_serialised.is_some() {
                task_agent::<String>(Command::Copy, inner_serialised, uid.unwrap(), state).await
            } else {
                None
            }
        }
        AdminCommand::Move(inner) => {
            // Serialise the (String, String) to just a String so we can use it with the
            // generic task_agent.
            let inner_serialised = match serde_json::to_string(&inner) {
                Ok(s) => Some(s),
                Err(e) => {
                    log_error_async(&e.to_string()).await;
                    None
                }
            };

            if inner_serialised.is_some() {
                task_agent::<String>(Command::Move, inner_serialised, uid.unwrap(), state).await
            } else {
                // Error logged in above failure path
                None
            }
        }
        AdminCommand::Pull(file_path) => {
            task_agent(Command::Pull, Some(file_path), uid.unwrap(), state).await
        }
        AdminCommand::BuildAllBins(_) => None,
        AdminCommand::RegQuery(data) => match serde_json::to_string(&data) {
            Ok(s) => task_agent(Command::RegQuery, Some(s), uid.unwrap(), state).await,
            Err(e) => {
                log_error_async(&e.to_string()).await;
                None
            }
        },
        AdminCommand::RegAdd(data) => match serde_json::to_string(&data) {
            Ok(s) => task_agent(Command::RegAdd, Some(s), uid.unwrap(), state).await,
            Err(e) => {
                log_error_async(&e.to_string()).await;
                None
            }
        },
        AdminCommand::RegDelete(data) => match serde_json::to_string(&data) {
            Ok(s) => task_agent(Command::RegDelete, Some(s), uid.unwrap(), state).await,
            Err(e) => {
                log_error_async(&e.to_string()).await;
                None
            }
        },
        AdminCommand::RmFile(data) => match serde_json::to_string(&data) {
            Ok(s) => task_agent(Command::RmFile, Some(s), uid.unwrap(), state).await,
            Err(e) => {
                log_error_async(&e.to_string()).await;
                None
            }
        },
        AdminCommand::RmDir(data) => match serde_json::to_string(&data) {
            Ok(s) => task_agent(Command::RmDir, Some(s), uid.unwrap(), state).await,
            Err(e) => {
                log_error_async(&e.to_string()).await;
                None
            }
        },
        AdminCommand::ExportDb => export_completed_tasks_to_json(uid.unwrap(), state).await,
        AdminCommand::None => None,
        AdminCommand::DotEx(dot_ex_inner) => dotex(uid, dot_ex_inner, state.clone()).await,
        AdminCommand::WhoAmI => {
            task_agent::<String>(Command::WhoAmI, None, uid.unwrap(), state).await
        }
        AdminCommand::Spawn(inner) => match serde_json::to_string(&inner) {
            Ok(s) => task_agent(Command::Spawn, Some(s), uid.unwrap(), state).await,
            Err(e) => {
                log_error_async(&e.to_string()).await;
                None
            }
        },
    };

    serde_json::to_vec(&result).unwrap()
}
