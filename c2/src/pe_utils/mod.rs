use std::{io::SeekFrom, path::Path};

use chrono::NaiveDateTime;
use thiserror::Error;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use crate::{
    logging::log_error_async,
    pe_utils::types::{IMAGE_DOS_HEADER, IMAGE_NT_HEADERS64},
};

mod types;

#[derive(Error, Debug)]
pub enum PeScrubError {
    #[error("unable to open file, {0}")]
    FileOpen(String),
    #[error("unable to read buffer from file object, {0}")]
    FileRead(String),
    #[error("did not match on magic bytes, got: {0}")]
    MagicBytesMZ(u16),
    #[error("could not read file content, but not a file read error..")]
    NoRead,
    #[error("datetime was not formatted correctly, must be british formatting - %d/%m/%Y %H:%M:%S")]
    DTMismatch,
    #[error("Circuit breaker hit in loop")]
    CircuitBreaker,
    #[error("the buffer was too small")]
    BuffTooSmall,
    #[error("could not write to file, {0}")]
    FileWriteError(String),
}

/// Timestomps the compiled time of a given PE.
///
/// # Args
/// - `dt_str`: The datetime in British format for the binary to have in its compiled time headers.
/// - `build_path`: The path to the file to timestomp on disk.
///
/// # Returns
/// The function only returns meaningful data on error, being [`TimestompError`]. On success nothing is returned,
/// the original file is modified in place.
pub async fn timestomp_binary_compile_date(
    dt_str: &str,
    build_path: &Path,
) -> Result<(), PeScrubError> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(build_path)
        .await
        .map_err(|e| PeScrubError::FileOpen(e.to_string()))?;

    //
    // Read the first 2 kb of the binary into our buffer and grab the e_lfanew so we can offset to the
    // TimeDateStamp field
    //
    const INITIAL_LEN: usize = 2000;
    let mut buf = Vec::with_capacity(INITIAL_LEN);
    unsafe { buf.set_len(INITIAL_LEN) };

    if let Err(e) = file.read_exact(&mut buf).await {
        return Err(PeScrubError::FileRead(e.to_string()));
    }

    let p_dos_header = buf.as_ptr() as *const IMAGE_DOS_HEADER;

    // SAFETY: We know this is not null
    let dos_header = unsafe { &*(p_dos_header) };
    if dos_header.e_magic != 0x5a4d {
        return Err(PeScrubError::MagicBytesMZ(dos_header.e_magic));
    }

    // check that we have the NT header in the buffer, if not, then just read the whole file,
    // but this should not happen
    if dos_header.e_lfanew as usize + size_of::<IMAGE_NT_HEADERS64>() > buf.len() {
        return Err(PeScrubError::BuffTooSmall);
    }

    //
    // Create the datetime as epoch then write to the original file at the correct offset (e_lfanew + 8 bytes)
    //
    let timestamp = str_to_epoch(dt_str)?;

    const OFFSET_TIMESTAMP: u64 = 8;
    file.seek(SeekFrom::Start(
        dos_header.e_lfanew as u64 + OFFSET_TIMESTAMP,
    ))
    .await
    .map_err(|e| PeScrubError::FileWriteError(e.to_string()))?;

    file.write_all(&timestamp.to_le_bytes())
        .await
        .map_err(|e| PeScrubError::FileWriteError(e.to_string()))?;

    file.flush()
        .await
        .map_err(|e| PeScrubError::FileWriteError(e.to_string()))?;

    Ok(())
}

fn str_to_epoch(dt_str: &str) -> Result<u32, PeScrubError> {
    let datetime = match NaiveDateTime::parse_from_str(dt_str, "%d/%m/%Y %H:%M:%S") {
        Ok(d) => d,
        Err(_) => return Err(PeScrubError::DTMismatch),
    };

    Ok(datetime.and_utc().timestamp() as u32)
}

/// Scrubs all occurrences of `needle` from the file at `path`, overwriting it in place.
///
/// If `replacement`` is:
/// - `None`: the bytes are zeroed out.
/// - `Some(r)`: the bytes are zeroed and then the first `r.len()` bytes are replaced with `r`.
///
/// # Error
/// Function returns a [`PeScrubError`] if an error occurs.
pub async fn scrub_strings(
    build_path: &Path,
    needle: &[u8],
    replacement: Option<&[u8]>,
) -> Result<(), PeScrubError> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(build_path)
        .await
        .map_err(|e| PeScrubError::FileOpen(e.to_string()))?;

    let file_len = file.metadata().await.unwrap().len() as usize;

    let mut buf = Vec::with_capacity(file_len);
    unsafe { buf.set_len(file_len) };

    if let Err(e) = file.read_exact(&mut buf).await {
        return Err(PeScrubError::FileRead(e.to_string()));
    }

    const CIRCUIT_BREAKER_MAX: u32 = 10000;
    let mut i = 0;

    while let Some(pos) = buf.windows(needle.len()).position(|w| w.eq(needle)) {
        let end = pos + needle.len();
        if let Some(replacement) = replacement {
            if replacement.len() > needle.len() {
                let s = String::from_utf8_lossy(needle);
                log_error_async(&format!(
                    "Could not scrub string {s}, replacement was longer than input."
                ))
                .await;

                continue;
            }

            buf[pos..end].fill(0);

            let end_replacement = pos + replacement.len();
            buf[pos..end_replacement].copy_from_slice(replacement);
        } else {
            buf[pos..end].fill(0);
        }

        i += 1;
        if i >= CIRCUIT_BREAKER_MAX {
            //
            // We hit the circuit breaker for the loop - write what changes were made to the binary,
            // and return an error, discontinuing the loop.
            //
            return commit_files(&mut file, &mut buf).await;
        }
    }

    commit_files(&mut file, &mut buf).await
}

async fn commit_files(file: &mut File, buf: &mut Vec<u8>) -> Result<(), PeScrubError> {
    file.seek(SeekFrom::Start(0))
        .await
        .map_err(|e| PeScrubError::FileWriteError(e.to_string()))?;

    file.write_all(&buf)
        .await
        .map_err(|e| PeScrubError::FileWriteError(e.to_string()))?;

    file.flush()
        .await
        .map_err(|e| PeScrubError::FileWriteError(e.to_string()))?;

    Ok(())
}
