/// The inner type for the [`AdminCommand::Copy`] and [`AdminCommand::Move`], represented as an tuple with
/// the format (from, to).
pub type FileCopyInner = (String, String);

/// Represents inner data for the [`AdminCommand::BuildAllBins`], as a tuple for:
/// (`profile_disk_name`, `save path`, `listener_profile`, `implant_profile`).
///
/// For `listener_profile` & `implant_profile`, a value of `None` will resolve to matching on `default`.
pub type BuildAllBins = (String, String, Option<String>, Option<String>);

pub type RegQueryInner = (String, Option<String>);
