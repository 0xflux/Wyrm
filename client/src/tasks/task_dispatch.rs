use std::{collections::HashMap, process::exit};

use chrono::Utc;
use leptos::prelude::{RwSignal, Update, Write, use_context};
use thiserror::Error;

use crate::{
    models::dashboard::{Agent, TabConsoleMessages},
    net::{ApiError, IsTaskingAgent, IsTaskingAgentErr},
    tasks::task_impl::{
        FileOperationTarget, RegOperationDelQuery, TaskDispatchError, change_directory,
        clear_terminal, copy_file, dir_listing, export_db, file_dropper, kill_agent, kill_process,
        list_processes, move_file, pillage, pull_file, pwd, reg_add, reg_query_del, remove_agent,
        remove_file, run_powershell_command, set_sleep, show_help, show_help_for_command,
        show_server_time, unknown_command,
    },
};

#[derive(Error, Debug)]
pub enum TaskingError {
    #[error("Error deserialising data {0}.")]
    SerdeError(#[from] serde_json::Error),

    #[error("API error {0}.")]
    ApiError(#[from] ApiError),

    #[error("Error trying to get agent to task.")]
    IsTaskingAgentErr(#[from] IsTaskingAgentErr),

    #[error("Dispatch error: {0}")]
    TaskDispatchError(#[from] TaskDispatchError),
}

pub type DispatchResult = Result<Option<Vec<u8>>, TaskingError>;

/// Entry point into dispatching tasks on the C2
pub async fn dispatch_task(input: String, agent: IsTaskingAgent) -> DispatchResult {
    // Collect each token individually
    let input_cl = input.clone();
    let tokens: Vec<&str> = input_cl.split_whitespace().collect();
    let result = dispatcher(tokens, input, agent.clone()).await;

    // Handle the error output for the user
    if let Err(ref e) = result {
        if let IsTaskingAgent::Yes(agent_id) = agent {
            let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
                use_context().expect("could not get RwSig connected_agents");

            let mut guard = connected_agents.write();
            if let Some(agent) = (*guard).get_mut(&agent_id) {
                agent.update(|lock| {
                    lock.output_messages.push(TabConsoleMessages {
                        completed_id: 0,
                        event: "[Error dispatching task]".to_string(),
                        time: Utc::now().to_string(),
                        messages: vec![e.to_string()],
                    })
                });
            }
        }
    }

    result
}

async fn dispatcher(tokens: Vec<&str>, raw_input: String, agent: IsTaskingAgent) -> DispatchResult {
    if tokens.is_empty() {
        return Ok(None);
    }

    //
    // Important note on usage:
    //
    // If you want to tokenise input where there could be multiple spaces and other tokens such as
    // ", then in the below rather than passing the pattern (which is an array of single chars), pass
    // the `raw_input` param which contains the unmodified, unflattened, and un-tokenised version of what
    // the user passed in.
    //

    match tokens.as_slice() {
        [""] | [" "] => Ok(None),
        // generic
        ["exit"] | ["quit"] => exit(0),
        ["clear"] | ["cls"] => clear_terminal(&agent).await,
        ["servertime"] => show_server_time().await,
        ["help"] => show_help(&agent).await,
        ["help", arg] => show_help_for_command(&agent, arg).await,

        // on &agent
        ["export_db"] => export_db(&agent).await,
        ["set", "sleep", time] => set_sleep(time, &agent).await,
        ["ps"] => list_processes(&agent).await,
        ["cd", pat @ ..] => change_directory(pat, &agent).await,
        ["pwd"] => pwd(&agent).await,
        ["kill_agent" | "ka"] => kill_agent(&agent).await,
        ["kill", pid] => kill_process(&agent, pid).await,
        ["remove_agent" | "ra"] => remove_agent(&agent).await,
        ["ls"] => dir_listing(&agent).await,
        ["pillage"] => pillage(&agent).await,
        ["run", args @ ..] => run_powershell_command(args, &agent).await,
        ["drop", args @ ..] => file_dropper(args, &agent).await,
        ["cp", _p @ ..] | ["copy", _p @ ..] => copy_file(raw_input, &agent).await,
        ["mv", _p @ ..] | ["move", _p @ ..] => move_file(raw_input, &agent).await,
        ["rm", _p @ ..] => remove_file(raw_input, FileOperationTarget::File, &agent).await,
        ["rm_d", _p @ ..] => remove_file(raw_input, FileOperationTarget::Dir, &agent).await,
        ["pull", _p @ ..] => pull_file(raw_input, &agent).await,
        ["reg", "query", _pat @ ..] => {
            reg_query_del(raw_input, &agent, RegOperationDelQuery::Query).await
        }
        ["reg", "add", _p @ ..] => reg_add(raw_input, &agent).await,
        ["reg", "del", _p @ ..] => {
            reg_query_del(raw_input, &agent, RegOperationDelQuery::Delete).await
        }
        _ => unknown_command(),
    }
}
