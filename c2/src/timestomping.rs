use std::{io::SeekFrom, path::Path};

use chrono::NaiveDateTime;
use thiserror::Error;
use tokio::{
    fs::OpenOptions,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

#[derive(Error, Debug)]
pub enum TimestompError {
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
) -> Result<(), TimestompError> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(build_path)
        .await
        .map_err(|e| TimestompError::FileOpen(e.to_string()))?;

    //
    // Read the first 2 kb of the binary into our buffer and grab the e_lfanew so we can offset to the
    // TimeDateStamp field
    //
    const INITIAL_LEN: usize = 2000;
    let mut buf = Vec::with_capacity(INITIAL_LEN);
    unsafe { buf.set_len(INITIAL_LEN) };

    if let Err(e) = file.read_exact(&mut buf).await {
        return Err(TimestompError::FileRead(e.to_string()));
    }

    let p_dos_header = buf.as_ptr() as *const IMAGE_DOS_HEADER;

    // SAFETY: We know this is not null
    let dos_header = unsafe { &*(p_dos_header) };
    if dos_header.e_magic != 0x5a4d {
        return Err(TimestompError::MagicBytesMZ(dos_header.e_magic));
    }

    // check that we have the NT header in the buffer, if not, then just read the whole file,
    // but this should not happen
    if dos_header.e_lfanew as usize + size_of::<IMAGE_NT_HEADERS64>() > buf.len() {
        return Err(TimestompError::BuffTooSmall);
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
    .map_err(|e| TimestompError::FileWriteError(e.to_string()))?;

    file.write_all(&timestamp.to_le_bytes())
        .await
        .map_err(|e| TimestompError::FileWriteError(e.to_string()))?;

    file.flush()
        .await
        .map_err(|e| TimestompError::FileWriteError(e.to_string()))?;

    Ok(())
}

fn str_to_epoch(dt_str: &str) -> Result<u32, TimestompError> {
    let datetime = match NaiveDateTime::parse_from_str(dt_str, "%d/%m/%Y %H:%M:%S") {
        Ok(d) => d,
        Err(_) => return Err(TimestompError::DTMismatch),
    };

    Ok(datetime.and_utc().timestamp() as u32)
}

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct IMAGE_FILE_HEADER {
    pub Machine: IMAGE_FILE_MACHINE,
    pub NumberOfSections: u16,
    pub TimeDateStamp: u32,
    pub PointerToSymbolTable: u32,
    pub NumberOfSymbols: u32,
    pub SizeOfOptionalHeader: u16,
    pub Characteristics: IMAGE_FILE_CHARACTERISTICS,
}

#[repr(transparent)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct IMAGE_FILE_MACHINE(pub u16);

#[repr(transparent)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct IMAGE_FILE_CHARACTERISTICS(pub u16);

#[repr(C)]
#[allow(non_snake_case, non_camel_case_types)]
pub struct IMAGE_NT_HEADERS64 {
    pub Signature: u32,
    pub FileHeader: IMAGE_FILE_HEADER,
    // OptionalHeader omited...
}

#[repr(C, packed(2))]
#[allow(non_snake_case, non_camel_case_types)]
pub struct IMAGE_DOS_HEADER {
    pub e_magic: u16,
    pub e_cblp: u16,
    pub e_cp: u16,
    pub e_crlc: u16,
    pub e_cparhdr: u16,
    pub e_minalloc: u16,
    pub e_maxalloc: u16,
    pub e_ss: u16,
    pub e_sp: u16,
    pub e_csum: u16,
    pub e_ip: u16,
    pub e_cs: u16,
    pub e_lfarlc: u16,
    pub e_ovno: u16,
    pub e_res: [u16; 4],
    pub e_oemid: u16,
    pub e_oeminfo: u16,
    pub e_res2: [u16; 10],
    pub e_lfanew: i32,
}
