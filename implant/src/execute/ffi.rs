use std::ffi::{c_long, c_void};

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
    pub parent: IUnknownVtbl,
    pub GetRuntime: unsafe extern "system" fn(
        *mut ICLRMetaHost,
        pwzVersion: *const u16,
        riid: *const GUID,
        ppRuntime: *mut *mut c_void,
    ) -> i32,
    pub GetVersionFromFile: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub EnumerateInstalledRuntimes:
        unsafe extern "system" fn(this: *mut c_void, ppEnumerator: *mut *mut c_void) -> i32,
    pub EnumerateLoadedRuntimes: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub RequestRuntimeLoadedNotification: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub QueryLegacyV2RuntimeBinding: unsafe extern "system" fn(this: *mut c_void) -> i32,
    pub ExitProcess: unsafe extern "system" fn(this: *mut c_void) -> i32,
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
    pub parent: IUnknownVtbl,
    pub CreateLogicalThreadState: unsafe extern "system" fn(this: *mut ICorRuntimeHost) -> i32,
    pub DeleteLogicalThreadState: unsafe extern "system" fn(this: *mut ICorRuntimeHost) -> i32,
    pub SwitchInLogicalThreadState:
        unsafe extern "system" fn(this: *mut ICorRuntimeHost, pFiberCookie: *mut u32) -> i32,
    pub SwitchOutLogicalThreadState:
        unsafe extern "system" fn(this: *mut ICorRuntimeHost, pFiberCookie: *mut *mut u32) -> i32,
    pub LocksHeldByLogicalThread:
        unsafe extern "system" fn(this: *mut ICorRuntimeHost, pCount: *mut u32) -> i32,
    pub MapFile: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        hFile: HANDLE,
        hMapAddress: *mut c_void,
    ) -> i32,
    pub GetConfiguration: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        pConfiguration: *mut *mut c_void,
    ) -> i32,
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
    pub EnumDomains:
        unsafe extern "system" fn(this: *mut ICorRuntimeHost, hEnum: *mut *mut c_void) -> i32,
    pub NextDomain: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        hEnum: *mut c_void,
        pAppDomain: *mut *mut IUnknown,
    ) -> i32,
    pub CloseEnum: unsafe extern "system" fn(this: *mut ICorRuntimeHost, hEnum: *mut c_void) -> i32,
    pub CreateDomainEx: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        pwzFriendlyName: *const u16,
        pSetup: *mut IUnknown,
        pEvidence: *mut IUnknown,
        pAppDomain: *mut *mut IUnknown,
    ) -> i32,
    pub CreateDomainSetup: unsafe extern "system" fn(
        this: *mut ICorRuntimeHost,
        pAppDomain: *mut *mut IUnknown,
    ) -> i32,
    pub CreateEvidence:
        unsafe extern "system" fn(this: *mut ICorRuntimeHost, pEvidence: *mut *mut IUnknown) -> i32,
    pub UnloadDomain:
        unsafe extern "system" fn(this: *mut ICorRuntimeHost, pAppDomain: *mut IUnknown) -> i32,
    pub CurrentDomain: unsafe extern "system" fn(
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
    pub parent: IUnknownVtbl,
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
    pub LoadErrorString: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        iResourceID: u32,
        pwzBuffer: *mut u16,
        pcchBuffer: *mut u32,
        iLocaleID: u32,
    ) -> i32,
    pub LoadLibrary: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pwzDllName: *const u16,
        ppProc: *mut *mut c_void,
    ) -> i32,
    pub GetProcAddress: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pszProcName: *const i8,
        ppProc: *mut *mut c_void,
    ) -> i32,
    pub GetInterface: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        rclsid: *const GUID,
        riid: *const GUID,
        ppUnk: *mut *mut c_void,
    ) -> i32,
    pub IsLoadable:
        unsafe extern "system" fn(this: *mut ICLRRuntimeInfo, pbLoadable: *mut BOOL) -> i32,
    pub SetDefaultStartupFlags: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        dwStartupFlags: u32,
        pwzHostConfigFile: *const u16,
    ) -> i32,
    pub GetDefaultStartupFlags: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pdwStartupFlags: *mut u32,
        pwzHostConfigFile: *mut u16,
        pcchHostConfigFile: *mut u32,
    ) -> i32,
    pub BindAsLegacyV2Runtime: unsafe extern "system" fn(this: *mut ICLRRuntimeInfo) -> i32,
    pub IsStarted: unsafe extern "system" fn(
        this: *mut ICLRRuntimeInfo,
        pbStarted: *mut BOOL,
        pdwStartupFlags: *mut u32,
    ) -> i32,
}

#[repr(C)]
pub struct _AppDomain {
    pub vtable: *const _AppDomainVtbl,
}

#[repr(C)]
pub struct _AppDomainVtbl {
    pub parent: IUnknownVtbl,
    pub GetTypeInfoCount: *const c_void,
    pub GetTypeInfo: *const c_void,
    pub GetIDsOfNames: *const c_void,
    pub Invoke: *const c_void,
    pub ToString: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub Equals: *const c_void,
    pub GetHashCode: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut c_long) -> i32,
    pub GetType: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut c_void) -> i32,
    pub InitializeLifetimeService: *const c_void,
    pub GetLifetimeService: *const c_void,
    pub get_Evidence: *const c_void,
    pub set_Evidence: *const c_void,
    pub get_DomainUnload: *const c_void,
    pub set_DomainUnload: *const c_void,
    pub get_AssemblyLoad: *const c_void,
    pub set_AssemblyLoad: *const c_void,
    pub get_ProcessExit: *const c_void,
    pub set_ProcessExit: *const c_void,
    pub get_TypeResolve: *const c_void,
    pub set_TypeResolve: *const c_void,
    pub get_ResourceResolve: *const c_void,
    pub set_ResourceResolve: *const c_void,
    pub get_AssemblyResolve: *const c_void,
    pub get_UnhandledException: *const c_void,
    pub set_UnhandledException: *const c_void,
    pub DefineDynamicAssembly: *const c_void,
    pub DefineDynamicAssembly_2: *const c_void,
    pub DefineDynamicAssembly_3: *const c_void,
    pub DefineDynamicAssembly_4: *const c_void,
    pub DefineDynamicAssembly_5: *const c_void,
    pub DefineDynamicAssembly_6: *const c_void,
    pub DefineDynamicAssembly_7: *const c_void,
    pub DefineDynamicAssembly_8: *const c_void,
    pub DefineDynamicAssembly_9: *const c_void,
    pub CreateInstance: *const c_void,
    pub CreateInstanceFrom: *const c_void,
    pub CreateInstance_2: *const c_void,
    pub CreateInstanceFrom_2: *const c_void,
    pub CreateInstance_3: *const c_void,
    pub CreateInstanceFrom_3: *const c_void,
    pub Load: *const c_void,
    pub Load_2: unsafe extern "system" fn(
        this: *mut c_void,
        assemblyString: *mut u16,
        pRetVal: *mut *mut _Assembly,
    ) -> i32,
    pub Load_3: unsafe extern "system" fn(
        this: *mut c_void,
        rawAssembly: *mut SAFEARRAY,
        pRetVal: *mut *mut _Assembly,
    ) -> i32,
    pub Load_4: *const c_void,
    pub Load_5: *const c_void,
    pub Load_6: *const c_void,
    pub Load_7: *const c_void,
    pub ExecuteAssembly: *const c_void,
    pub ExecuteAssembly_2: *const c_void,
    pub ExecuteAssembly_3: *const c_void,
    pub get_FriendlyName: *const c_void,
    pub get_BaseDirectory: *const c_void,
    pub get_RelativeSearchPath: *const c_void,
    pub get_ShadowCopyFiles: *const c_void,
    pub GetAssemblies: *const c_void,
    pub AppendPrivatePath: *const c_void,
    pub ClearPrivatePath: *const c_void,
    pub ClearShadowCopyPath: *const c_void,
    pub SetData: *const c_void,
    pub GetData: *const c_void,
    pub SetAppDomainPolicy: *const c_void,
    pub SetThreadPrincipal: *const c_void,
    pub SetPrincipalPolicy: *const c_void,
    pub DoCallBack: *const c_void,
    pub get_DynamicDirectory: *const c_void,
}

#[repr(C)]
pub struct _Assembly {
    pub vtable: *const _AssemblyVtbl,
}

#[repr(C)]
pub struct _AssemblyVtbl {
    pub parent: IUnknownVtbl,
    pub GetTypeInfoCount: *const c_void,
    pub GetTypeInfo: *const c_void,
    pub GetIDsOfNames: *const c_void,
    pub Invoke: *const c_void,
    pub ToString: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub Equals: *const c_void,
    pub GetHashCode: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut c_long) -> i32,
    pub GetType: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut c_void) -> i32,
    pub get_CodeBase: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub get_EscapedCodeBase:
        unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub GetName: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut c_void) -> i32,
    pub GetName_2: *const c_void,
    pub get_FullName: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub get_EntryPoint:
        unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut _MethodInfo) -> i32,
    pub GetType_2: unsafe extern "system" fn(
        this: *mut c_void,
        name: *mut u16,
        pRetVal: *mut *mut c_void,
    ) -> i32,
    pub GetType_3: *const c_void,
    pub GetExportedTypes: *const c_void,
    pub GetTypes: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut SAFEARRAY) -> i32,
    pub GetManifestResourceStream: *const c_void,
    pub GetManifestResourceStream_2: *const c_void,
    pub GetFile: *const c_void,
    pub GetFiles: *const c_void,
    pub GetFiles_2: *const c_void,
    pub GetManifestResourceNames: *const c_void,
    pub GetManifestResourceInfo: *const c_void,
    pub get_Location: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub get_Evidence: *const c_void,
    pub GetCustomAttributes: *const c_void,
    pub GetCustomAttributes_2: *const c_void,
    pub IsDefined: *const c_void,
    pub GetObjectData: *const c_void,
    pub add_ModuleResolve: *const c_void,
    pub remove_ModuleResolve: *const c_void,
    pub GetType_4: *const c_void,
    pub GetSatelliteAssembly: *const c_void,
    pub GetSatelliteAssembly_2: *const c_void,
    pub LoadModule: *const c_void,
    pub LoadModule_2: *const c_void,
    pub CreateInstance: unsafe extern "system" fn(
        this: *mut c_void,
        typeName: *mut u16,
        pRetVal: *mut VARIANT,
    ) -> i32,
    pub CreateInstance_2: *const c_void,
    pub CreateInstance_3: *const c_void,
    pub GetLoadedModules: *const c_void,
    pub GetLoadedModules_2: *const c_void,
    pub GetModules: *const c_void,
    pub GetModules_2: *const c_void,
    pub GetModule: *const c_void,
    pub GetReferencedAssemblies: *const c_void,
    pub get_GlobalAssemblyCache: *const c_void,
}

#[repr(C)]
pub struct _MethodInfo {
    pub vtable: *const _MethodInfoVtbl,
}

#[repr(C)]
pub struct _MethodInfoVtbl {
    pub parent: IUnknownVtbl,
    pub GetTypeInfoCount: *const c_void,
    pub GetTypeInfo: *const c_void,
    pub GetIDsOfNames: *const c_void,
    pub Invoke: *const c_void,
    pub ToString: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub Equals: *const c_void,
    pub GetHashCode: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut c_long) -> i32,
    pub GetType: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut c_void) -> i32,
    pub get_MemberType: *const c_void,
    pub get_name: unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut u16) -> i32,
    pub get_DeclaringType: *const c_void,
    pub get_ReflectedType: *const c_void,
    pub GetCustomAttributes: *const c_void,
    pub GetCustomAttributes_2: *const c_void,
    pub IsDefined: *const c_void,
    pub GetParameters:
        unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut SAFEARRAY) -> i32,
    pub GetMethodImplementationFlags: *const c_void,
    pub get_MethodHandle: *const c_void,
    pub get_Attributes: *const c_void,
    pub get_CallingConvention: *const c_void,
    pub Invoke_2: *const c_void,
    pub get_IsPublic: *const c_void,
    pub get_IsPrivate: *const c_void,
    pub get_IsFamily: *const c_void,
    pub get_IsAssembly: *const c_void,
    pub get_IsFamilyAndAssembly: *const c_void,
    pub get_IsFamilyOrAssembly: *const c_void,
    pub get_IsStatic: *const c_void,
    pub get_IsFinal: *const c_void,
    pub get_IsVirtual: *const c_void,
    pub get_IsHideBySig: *const c_void,
    pub get_IsAbstract: *const c_void,
    pub get_IsSpecialName: *const c_void,
    pub get_IsConstructor: *const c_void,
    pub Invoke_3: unsafe extern "system" fn(
        this: *mut c_void,
        obj: VARIANT,
        parameters: *mut SAFEARRAY,
        pRetVal: *mut VARIANT,
    ) -> i32,
    pub get_returnType: *const c_void,
    pub get_ReturnTypeCustomAttributes: *const c_void,
    pub GetBaseDefinition:
        unsafe extern "system" fn(this: *mut c_void, pRetVal: *mut *mut _MethodInfo) -> i32,
}
