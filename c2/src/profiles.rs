use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use shared::{
    pretty_print::print_failed,
    tasks::{NewAgentStaging, StageType, WyrmResult},
};
use tokio::io;

use crate::logging::log_error_async;

#[derive(Deserialize, Debug, Default)]
pub struct Profile {
    pub server: Server,
    pub implants: BTreeMap<String, Implant>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Server {
    pub token: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct Network {
    pub address: String,
    pub uri: Vec<String>,
    pub port: u16,
    pub token: Option<String>,
    pub sleep: Option<u64>,
    pub useragent: Option<String>,
    pub jitter: Option<u64>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Implant {
    pub anti_sandbox: Option<AntiSandbox>,
    pub debug: Option<bool>,
    pub network: Network,
    pub evasion: Evasion,
}

#[derive(Deserialize, Debug, Default)]
pub struct AntiSandbox {
    pub trig: Option<bool>,
    pub ram: Option<bool>,
}

#[derive(Deserialize, Debug, Default)]
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
        listener_profile_name: &String,
        implant_profile_name: &String,
        stage_type: StageType,
    ) -> WyrmResult<NewAgentStaging> {
        // TODO at some point we can do away with building a NewAgentStaging; that is no longer required.
        // leaving in for now, the validation & translation is required but not necessarily converting to
        // a NewAgentStaging.
        // This fn isn't the end of the world and a huge waste of compute power, so I'm happy leaving it in for
        // now.

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

        let pe_name = format!("{}.{}", listener_profile_name, implant_profile_name);

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
        })
    }
}

/// Parse profiles from within the /profiles/* directory relative to the c2
/// crate to load configurable user profiles at runtime.
pub async fn parse_profiles() -> io::Result<Profile> {
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
        log_error_async("Could not open dir profiles.").await;

        return Err(io::Error::other("Could not open dir"));
    }

    //
    // We now only support 1 profile toml in the profile directory. If more than one is detected,
    // then return an error, logging the error internally.
    //
    if profile_paths.len() > 1 {
        let msg = "You can only have one `profile.toml` in /c2/profiles. Please consolidate \
            into one profile. You may specify multiple implant configurations to build, but you must \
            have one, and only one, `profile.toml`.";
        print_failed(msg);
        log_error_async(msg).await;

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
            log_error_async(&msg).await;
            return Err(io::Error::other(msg));
        }
    };

    Ok(profile)
}

pub async fn get_profile(needle: &str) -> io::Result<Profile> {
    let mut path = PathBuf::from("./profiles");

    let needle = if needle.ends_with(".toml") {
        needle.to_owned()
    } else {
        let mut tmp = needle.to_owned();
        tmp.push_str(".toml");
        tmp
    };

    path.push(&needle);

    if path.is_file() {
        read_profile(&path).await
    } else {
        let msg = format!("Could not open profile `{needle}`, was not a file.");
        log_error_async(&msg).await;
        Err(io::Error::other(msg))
    }
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
