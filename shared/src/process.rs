use serde::{Deserialize, Serialize};

/// An individual process
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "a")]
pub struct Process {
    #[serde(rename = "b")]
    pub pid: u32,
    #[serde(rename = "c")]
    pub name: String,
    #[serde(rename = "d")]
    pub user: String,
}
