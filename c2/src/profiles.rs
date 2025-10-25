use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use shared::tasks::{NewAgentStaging, StageType, WyrmResult};
use tokio::io;

use crate::logging::log_error_async;

#[derive(Deserialize, Debug)]
pub struct Profile {
    pub sleep: Option<u64>,
    pub useragent: Option<String>,
    pub server: Server,
    pub listeners: BTreeMap<String, Listener>,
    pub implants: BTreeMap<String, Implant>,
}

#[derive(Deserialize, Debug)]
pub struct Server {
    pub uri: Vec<String>,
    pub token: String,
    pub jitter: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct Listener {
    pub address: String,
    pub port: u16,
    pub token: Option<String>,
    pub sleep: Option<u64>,
    pub protocol: String,
}

#[derive(Deserialize, Debug)]
pub struct Implant {
    pub anti_sandbox: Option<AntiSandbox>,
    pub debug: Option<bool>,
    pub patch_etw: Option<bool>,
    pub timestomp: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct AntiSandbox {
    pub trig: Option<bool>,
    pub ram: Option<bool>,
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

        let listener = match self.listeners.get(listener_profile_name) {
            Some(l) => l,
            None => {
                return WyrmResult::Err(format!("Could not find listener {listener_profile_name}"));
            }
        };

        let implant = match self.implants.get(implant_profile_name) {
            Some(i) => i,
            None => {
                return WyrmResult::Err(format!(
                    "Could not find implant profile {implant_profile_name}"
                ));
            }
        };

        let build_debug = implant.debug.unwrap_or_default();
        let patch_etw = implant.patch_etw.unwrap_or_default();

        // Unwrap a sleep time from either profile specific, a higher order key, or if none found, use
        // a default of 1 hr (3600 seconds).
        let sleep_time = match listener.sleep {
            Some(s) => s,
            None => self.sleep.unwrap_or(3600),
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

        let agent_security_token = if let Some(token) = &listener.token {
            token.clone()
        } else {
            self.server.token.clone()
        };

        let useragent = if let Some(ua) = &self.useragent {
            ua.clone()
        } else {
            "Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36".into()
        };

        // Validate we have at least 1 URI endpoint before insertion, otherwise error
        if self.server.uri.is_empty() {
            return WyrmResult::Err(String::from("At least 1 URI is required for the server."));
        }

        WyrmResult::Ok(NewAgentStaging {
            // TODO not required
            implant_name: String::new(),
            default_sleep_time,
            c2_address: listener.address.clone(),
            c2_endpoints: self.server.uri.clone(),
            // TODO not required
            staging_endpoint: String::new(),
            pe_name,
            port: listener.port,
            agent_security_token,
            antisandbox_trig,
            antisandbox_ram,
            stage_type,
            build_debug,
            useragent,
            patch_etw,
            jitter: self.server.jitter,
            timestomp: implant.timestomp.clone(),
        })
    }
}

/// Parse profiles from within the /profiles/* directory relative to the c2
/// crate to load configurable user profiles at runtime.
pub async fn parse_profiles() -> io::Result<Vec<Profile>> {
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

    let mut profiles: Vec<Profile> = Vec::new();

    for p_path in profile_paths {
        let temp_path = path.join(&p_path);

        let profile = match read_profile(&temp_path).await {
            Ok(p) => p,
            Err(e) => {
                log_error_async(&format!("{e:?}")).await;
                continue;
            }
        };

        profiles.push(profile);
    }

    Ok(profiles)
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

pub fn add_listeners_from_profiles(existing: &mut HashSet<String>, new: &[Profile]) {
    for p in new.iter() {
        for uri in &p.server.uri {
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

pub fn add_tokens_from_profiles(existing: &mut HashSet<String>, new: &[Profile]) {
    for p in new.iter() {
        // Add the default required token in the [server] attribute
        existing.insert(p.server.token.clone());

        for listener in p.listeners.values() {
            if let Some(tok) = &listener.token {
                existing.insert(tok.clone());
            }
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

    // Check the protocol matches the valid protocol names we can have implants built for
    for (name, listener) in &profile.listeners {
        if !(listener.protocol.eq("http")
            || listener.protocol.eq("https")
            || listener.protocol.eq("smb"))
        {
            return Err(io::Error::other(format!(
                "Listener type was missing when reading profile: {path:?}, profile name: {name}"
            )));
        }
    }

    Ok(profile)
}
