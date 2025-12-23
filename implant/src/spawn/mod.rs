use std::fs::File;

use shared::tasks::WyrmResult;

use crate::spawn::hollow_apc::spawn_sibling;

pub mod hollow_apc;

pub struct Spawn;

impl Spawn {
    pub fn spawn_sibling(path: &str) -> WyrmResult<String> {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                let msg: String = format!("Failed to open file. {}", e.to_string());
                #[cfg(debug_assertions)]
                {
                    use shared::pretty_print::print_failed;

                    print_failed(&msg);
                }

                return WyrmResult::Err(msg);
            }
        };

        let len = match f.metadata() {
            Ok(m) => m.len(),
            Err(e) => return WyrmResult::Err(e.to_string()),
        };

        let buf = Vec::with_capacity(len as usize);
        spawn_sibling(buf)
    }
}
