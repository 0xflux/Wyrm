use serde::{Deserialize, Serialize};

pub mod net;
pub mod pretty_print;
pub mod process;
pub mod task_types;
pub mod tasks;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StagedResourceDataNoSqlx {
    pub agent_name: String,
    pub c2_endpoint: String,
    pub staged_endpoint: String,
    pub pe_name: String,
    pub sleep_time: i64,
    pub port: i16,
    pub num_downloads: i64,
}
