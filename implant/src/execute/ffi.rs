use std::ffi::c_void;

use windows_sys::{
    Win32::{
        Foundation::HANDLE,
        System::{Com::SAFEARRAY, Variant::VARIANT},
    },
    core::{BOOL, GUID},
};

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut IUnknown,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> i32,
    pub AddRef: unsafe extern "system" fn(this: *mut IUnknown) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut IUnknown) -> u32,
}

#[repr(C)]
pub struct IUnknown {
    pub lpVtbl: *const IUnknownVtbl,
}

#[repr(C)]
pub struct ICLRMetaHostVtbl {
    // IUnknown
    pub QueryInterface:
        unsafe extern "system" fn(*mut ICLRMetaHost, *const GUID, *mut *mut c_void) -> i32,
    pub AddRef: unsafe extern "system" fn(*mut ICLRMetaHost) -> u32,
    pub Release: unsafe extern "system" fn(*mut ICLRMetaHost) -> u32,
    // ICLRMetaHost
    pub GetRuntime: unsafe extern "system" fn(
        *mut ICLRMetaHost,
        pwzVersion: *const u16,
        riid: *const GUID,
        ppRuntime: *mut *mut c_void,
    ) -> i32,
}

#[repr(C)]
pub struct ICLRMetaHost {
    pub lpVtbl: *const ICLRMetaHostVtbl,
}

#[repr(C)]
pub struct ICorRuntimeHost {
    pub vtable: *const ICorRuntimeHostVtbl,
}

#[repr(C)]
pub struct ICorRuntimeHostVtbl {
    pub Start: unsafe extern "system" fn(this: *mut ICorRuntimeHost) -> i32,
    pub Stop: unsafe extern "system" fn(this: *mut ICorRuntimeHost) -> i32,
    pub CreateDomain: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        pwzFriendlyName: *const u16,
        pIdentityArray: *mut IUnknown,
        pAppDomain: *mut *mut IUnknown,
    ) -> i32,
    pub GetDefaultDomain: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        pAppDomain: *mut *mut IUnknown,
    ) -> i32,
}

#[repr(C)]
pub struct ICLRRuntimeInfo {
    pub vtable: *const ICLRRuntimeInfoVtbl,
}

#[repr(C)]
pub struct ICLRRuntimeInfoVtbl {
    pub GetVersionString: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pwzBuffer: *mut u16,
        pcchBuffer: *mut u32,
    ) -> i32,
    pub GetRuntimeDirectory: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pwzBuffer: *mut u16,
        pcchBuffer: *mut u32,
    ) -> i32,
    pub IsLoaded: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        hndProcess: HANDLE,
        pbLoaded: *mut BOOL,
    ) -> i32,
    pub GetInterface: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        rclsid: *const GUID,
        riid: *const GUID,
        ppUnk: *mut *mut c_void,
    ) -> i32,
    pub IsLoadable:
        unsafe extern "system" fn(this: *mut ICLRRuntimeInfo, pbLoadable: *mut BOOL) -> i32,
    pub IsStarted: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pbStarted: *mut BOOL,
        pdwStartupFlags: *mut u32,
    ) -> i32,
}

#[repr(C)]
pub struct AppDomain {
    pub vtable: *const AppDomainVtbl,
}

#[repr(C)]
pub struct AppDomainVtbl {
    pub Load_3: unsafe extern "system" fn(
        this: *mut c_void,
        rawAssembly: *mut SAFEARRAY,
        pRetVal: *mut *mut _AssemblyVtbl,
    ) -> i32,
    pub ExecuteAssembly: *const c_void,
    pub ExecuteAssembly_2: *const c_void,
    pub ExecuteAssembly_3: *const c_void,
    pub SetData: *const c_void,
    pub GetData: *const c_void,
    pub CreateInstance: *const c_void,
    pub CreateInstanceFrom: *const c_void,
    pub CreateInstance_2: *const c_void,
    pub CreateInstanceFrom_2: *const c_void,
    pub CreateInstance_3: *const c_void,
    pub CreateInstanceFrom_3: *const c_void,
}

#[repr(C)]
pub struct _Assembly {
    pub vtable: *const _AssemblyVtbl,
}

#[repr(C)]
pub struct _AssemblyVtbl {
    pub get_EntryPoint:
        unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut _MethodInfo) -> i32,
}

#[repr(C)]
pub struct _MethodInfo {
    pub vtable: *const _MethodInfoVtbl,
}

#[repr(C)]
pub struct _MethodInfoVtbl {
    pub Invoke_3: unsafe extern "system" fn(
        this: *mut c_void,
        obj: VARIANT,
        parameters: *mut SAFEARRAY,
        pRetVal: *mut VARIANT,
    ) -> i32,
}
