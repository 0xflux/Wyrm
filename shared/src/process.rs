use serde::{Deserialize, Serialize};

/// An individual process
#[derive(Debug, Deserialize, Serialize)]
pub struct Process {
    pub pid: u32,
    pub name: String,
}