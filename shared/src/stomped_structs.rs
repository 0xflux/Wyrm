//! This module provides structs which have had their serilisation names stomped for evasion purposes, primarily
//! these are used in the implant, but also used on the client, and / or C2.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::tasks::WyrmResult;

/// An individual process
#[derive(Deserialize, Serialize, Clone)]
#[serde(rename = "a")]
pub struct Process {
    #[serde(rename = "b")]
    pub pid: u32,
    #[serde(rename = "c")]
    pub name: String,
    #[serde(rename = "d")]
    pub user: String,
    #[serde(rename = "e")]
    pub ppid: u32,
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(rename = "a")]
pub struct RegQueryResult {
    #[serde(rename = "b")]
    pub subkeys: Vec<String>,
    #[serde(rename = "c")]
    pub values: BTreeMap<String, String>,
}

impl TryFrom<&str> for RegQueryResult {
    type Error = Vec<String>;

    fn try_from(value: &str) -> Result<Self, Vec<String>> {
        let results = match serde_json::from_str::<WyrmResult<String>>(value) {
            Ok(data) => match data {
                WyrmResult::Ok(inner_string_from_result) => {
                    match serde_json::from_str::<RegQueryResult>(&inner_string_from_result) {
                        Ok(results_as_vec) => results_as_vec,
                        Err(e) => {
                            return Err(vec![format!("Error {e}, {}", inner_string_from_result)]);
                        }
                    }
                }
                WyrmResult::Err(e) => {
                    return Err(vec![format!("Error with operation. {e}")]);
                }
            },
            Err(e) => {
                return Err(vec![format!("Could not deserialise response data. {e}.")]);
            }
        };

        return Ok(results);
    }
}

impl RegQueryResult {
    pub fn client_print_formatted(&self) -> Vec<String> {
        let mut result_printer = vec![];
        for v in &self.subkeys {
            result_printer.push(format!("[subkey] {v}"));
        }

        if !result_printer.is_empty() {
            result_printer.push("\t\t--".to_string());
        }

        const KEY_SZ: usize = 35;

        let v1 = "[Value name]";
        let v2 = "[Value data]";
        let f = format!("{:<KEY_SZ$}{}", v1, v2);

        result_printer.push(f);
        for (k, v) in &self.values {
            let f = format!("{:<KEY_SZ$}{}", k, v);
            result_printer.push(f);
        }

        result_printer
    }
}
