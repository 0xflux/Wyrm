use serde::{Deserialize, Serialize};

/// The inner type for the [`AdminCommand::Copy`] and [`AdminCommand::Move`], represented as an tuple with
/// the format (from, to).
pub type FileCopyInner = (String, String);

/// Represents inner data for the [`AdminCommand::BuildAllBins`], as a tuple for:
/// (`profile_disk_name`, `save path`, `listener_profile`, `implant_profile`).
///
/// For `listener_profile` & `implant_profile`, a value of `None` will resolve to matching on `default`.
pub type BuildAllBins = (String, String, Option<String>, Option<String>);

pub type RegQueryInner = (String, Option<String>);

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum RegType {
    String = 0,
    U32,
    U64,
}

// pub const REG_TYPE_STRING: u32 = 0b0001;
// pub const REG_TYPE_U32: u32 = 0b0010;
// pub const REG_TYPE_U64: u32 = 0b0100;

/// Inner type for a `reg add` operation, containing:
/// - the key,
/// - the value,
/// - the data
/// - the type (as a [`RegType`]).
pub type RegAddInner = (String, String, String, RegType);
