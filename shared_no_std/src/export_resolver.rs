//! Export resolver is a local copy of my https://github.com/0xflux/PE-Export-Resolver crate.
//! Currently the module cannot depend on certain windows crate, so some FFI may have to be
//! adjusted by hand in this module. Doing so should also reduce the overall binary size.

use core::{
    arch::asm,
    ffi::CStr,
    ffi::c_void,
    mem::transmute,
    ops::Add,
    ptr::{null_mut, read_unaligned},
    slice::from_raw_parts,
};

use windows_sys::Win32::System::{
    Diagnostics::Debug::{IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_NT_HEADERS64, IMAGE_SECTION_HEADER},
    SystemServices::{
        IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_EXPORT_DIRECTORY, IMAGE_NT_SIGNATURE,
    },
};

pub enum ExportResolveError {
    TargetFunctionNotFound,
    ModuleNotFound,
    MagicByteMismatch,
    FnNameNotUtf8,
}

/// Get the base address of a specified module. Obtains the base address by reading from the TEB -> PEB ->
/// PEB_LDR_DATA -> InMemoryOrderModuleList -> InMemoryOrderLinks -> DllBase
///
/// Returns the DLL base address as a Option<usize>
#[allow(unused_variables)]
#[allow(unused_assignments)]
#[inline(always)]
fn get_module_base(module_name: &str) -> Option<usize> {
    let mut peb: usize;
    let mut ldr: usize;
    let mut in_memory_order_module_list: usize;
    let mut current_entry: usize;

    unsafe {
        // get the peb and module list
        asm!(
            "mov {peb}, gs:[0x60]",
            "mov {ldr}, [{peb} + 0x18]",
            "mov {in_memory_order_module_list}, [{ldr} + 0x10]", // points to the Flink
            peb = out(reg) peb,
            ldr = out(reg) ldr,
            in_memory_order_module_list = out(reg) in_memory_order_module_list,
        );

        // set the current entry to the head of the list
        current_entry = in_memory_order_module_list;

        // iterate the modules searching for
        loop {
            // get the attributes we are after of the current entry
            let dll_base = read_unaligned(current_entry.add(0x30) as *const usize);
            let module_name_address = read_unaligned(current_entry.add(0x60) as *const usize);
            let module_length = read_unaligned(current_entry.add(0x58) as *const u16);

            // check if the module name address is valid and not zero
            if module_name_address != 0 && module_length > 0 {
                // read the module name from memory
                let dll_name_slice = from_raw_parts(
                    module_name_address as *const u16,
                    (module_length / 2) as usize,
                );

                let mut buf = [0u8; 256];
                let mut buf_len = 0;

                // do we have a match on the module name?
                for i in 0..(module_length / 2) as usize {
                    if i >= 256 {
                        break;
                    }

                    let wide_char = dll_name_slice[i];
                    buf[i] = (wide_char & 0xFF) as u8;
                    buf_len = i + 1;

                    if wide_char == 0 {
                        break;
                    }
                }

                if strings_equal_ignore_case(&buf[..buf_len], module_name.as_bytes()) {
                    return Some(dll_base);
                }
            } else {
                return None;
            }

            // dereference current_entry which contains the value of the next LDR_DATA_TABLE_ENTRY (specifically a pointer to LIST_ENTRY
            // within the next LDR_DATA_TABLE_ENTRY)
            current_entry = *(current_entry as *const usize);

            // If we have looped back to the start, break
            if current_entry == in_memory_order_module_list {
                return None;
            }
        }
    }
}

#[inline(always)]
fn to_lowercase_ascii(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { c + 32 } else { c }
}

#[inline(always)]
fn strings_equal_ignore_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for i in 0..a.len() {
        let char_a = to_lowercase_ascii(a[i]);
        let char_b = to_lowercase_ascii(b[i]);

        if char_a != char_b {
            return false;
        }
    }

    true
}

/// Get the function address of a function in a specified DLL from the DLL Base.
///
/// # Parameters
/// * dll_name -> the name of the DLL / module you are wanting to query
/// * needle -> the function name (case sensitive) of the function you are looking for
///
/// # Returns
/// Option<*const c_void> -> the function address as a pointer
#[inline(always)]
pub fn resolve_address(
    dll_name: &str,
    needle: &str,
    dll_base: Option<usize>,
) -> Result<*const c_void, ExportResolveError> {
    // if the dll_base was already found from a previous search then use that
    // otherwise, if it was None, make a call to get_module_base
    let dll_base: *mut c_void = match dll_base {
        Some(base) => base as *mut c_void,
        None => match get_module_base(dll_name) {
            Some(a) => a as *mut c_void,
            None => {
                return Err(ExportResolveError::ModuleNotFound);
            }
        },
    };

    // check we match the DOS header, cast as pointer to tell the compiler to treat the memory
    // address as if it were a IMAGE_DOS_HEADER structure
    let dos_header: IMAGE_DOS_HEADER =
        unsafe { read_unaligned(dll_base as *const IMAGE_DOS_HEADER) };
    if dos_header.e_magic != IMAGE_DOS_SIGNATURE {
        return Err(ExportResolveError::MagicByteMismatch);
    }

    // check the NT headers
    let nt_headers = unsafe {
        read_unaligned(dll_base.offset(dos_header.e_lfanew as isize) as *const IMAGE_NT_HEADERS64)
    };
    if nt_headers.Signature != IMAGE_NT_SIGNATURE {
        return Err(ExportResolveError::MagicByteMismatch);
    }

    // get the export directory
    // https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-image_data_directory
    // found from first item in the DataDirectory; then we take the structure in memory at dll_base + RVA
    let export_dir_rva = nt_headers.OptionalHeader.DataDirectory[0].VirtualAddress;
    let export_offset = unsafe { dll_base.add(export_dir_rva as usize) };
    let export_dir: IMAGE_EXPORT_DIRECTORY =
        unsafe { read_unaligned(export_offset as *const IMAGE_EXPORT_DIRECTORY) };

    // get the addresses we need
    let address_of_functions_rva = export_dir.AddressOfFunctions as usize;
    let address_of_names_rva = export_dir.AddressOfNames as usize;
    let ordinals_rva = export_dir.AddressOfNameOrdinals as usize;

    let functions = unsafe { dll_base.add(address_of_functions_rva as usize) } as *const u32;
    let names = unsafe { dll_base.add(address_of_names_rva as usize) } as *const u32;
    let ordinals = unsafe { dll_base.add(ordinals_rva as usize) } as *const u16;

    // get the amount of names to iterate over
    let number_of_names = export_dir.NumberOfNames;

    for i in 0..number_of_names {
        // calculate the RVA of the function name
        let name_rva = unsafe { *names.offset(i.try_into().unwrap()) as usize };
        // actual memory address of the function name
        let name_addr = unsafe { dll_base.add(name_rva) };

        // read the function name
        let function_name = unsafe {
            let char = name_addr as *const u8;
            let mut len = 0;
            // iterate over the memory until a null terminator is found
            while *char.add(len) != 0 {
                len += 1;
            }

            core::slice::from_raw_parts(char, len)
        };

        let function_name = core::str::from_utf8(function_name).unwrap_or_default();
        if function_name.is_empty() {
            return Err(ExportResolveError::FnNameNotUtf8);
        }

        // if we have a match on our function name
        if function_name.eq(needle) {
            // calculate the RVA of the function address
            let ordinal = unsafe { *ordinals.offset(i.try_into().unwrap()) as usize };
            let fn_rva = unsafe { *functions.add(ordinal) as usize };
            // actual memory address of the function address
            let fn_addr = unsafe { dll_base.add(fn_rva) } as *const c_void;

            return Ok(fn_addr);
        }
    }

    Err(ExportResolveError::TargetFunctionNotFound)
}

#[inline(always)]
fn get_rva<T>(base_ptr: *mut u8, offset: usize) -> *mut T {
    (base_ptr as usize + offset) as *mut T
}

#[inline(always)]
pub fn find_export_address(
    base: *mut c_void,
    nt: *mut IMAGE_NT_HEADERS64,
    name: &str,
) -> Option<unsafe extern "system" fn()> {
    unsafe {
        let dir = (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];
        if dir.VirtualAddress == 0 || dir.Size == 0 {
            return None;
        }

        let exp_dir: *mut IMAGE_EXPORT_DIRECTORY = get_rva(base as _, dir.VirtualAddress as usize);

        if exp_dir.is_null() {
            return None;
        }

        let exp = read_unaligned(exp_dir);

        let names: *const u32 = get_rva(base as _, exp.AddressOfNames as usize);
        let funcs: *const u32 = get_rva(base as _, exp.AddressOfFunctions as usize);
        let ords: *const u16 = get_rva(base as _, exp.AddressOfNameOrdinals as usize);

        //
        // Iterate over the exported names searching for the exported function
        //
        for i in 0..exp.NumberOfNames {
            let name_rva = read_unaligned(names.add(i as usize)) as usize;
            let name_ptr = get_rva::<u8>(base as _, name_rva);
            let export_name = CStr::from_ptr(name_ptr as _).to_str().ok();
            if export_name == Some(name) {
                let ord_index = read_unaligned(ords.add(i as usize)) as usize;
                let func_rva = read_unaligned(funcs.add(ord_index)) as usize;
                let func_ptr = get_rva::<u8>(base as _, func_rva) as usize;
                return Some(transmute::<usize, unsafe extern "system" fn()>(func_ptr));
            }
        }

        // Did not find exported function
        None
    }
}

/// Convert an RVA from the PE into a pointer inside a buffer which came from a file - NOT correctly mapped / relocated memory.
#[inline(always)]
unsafe fn rva_from_file<T>(
    file_base: *const u8,
    nt: *const IMAGE_NT_HEADERS64,
    rva: u32,
) -> *mut T {
    let num_sections = unsafe { *nt }.FileHeader.NumberOfSections as usize;

    let first_section = unsafe { (nt as *const u8).add(size_of::<IMAGE_NT_HEADERS64>()) }
        as *const IMAGE_SECTION_HEADER;

    for i in 0..num_sections {
        let sec = unsafe { &*first_section.add(i) };

        let va = sec.VirtualAddress;
        let raw = sec.PointerToRawData;
        let size = if sec.SizeOfRawData != 0 {
            sec.SizeOfRawData
        } else {
            unsafe { sec.Misc.VirtualSize }
        };

        if rva >= va && rva < va + size {
            let delta = rva - va;
            let file_off = raw + delta;
            return unsafe { file_base.add(file_off as usize) } as *mut T;
        }
    }

    null_mut()
}

pub enum ExportError {
    ImageTooSmall,
    ImageUnaligned,
    ExportNotFound,
}

#[inline(always)]
pub fn find_export_from_unmapped_file(
    file_base: &[u8],
    name: &str,
) -> Result<unsafe extern "system" fn(), ExportError> {
    // Check we are being safe
    if file_base.len() < size_of::<IMAGE_DOS_HEADER>() {
        return Err(ExportError::ImageTooSmall);
    }

    let file_base = file_base.as_ptr();

    let dos = unsafe { read_unaligned(file_base as *const IMAGE_DOS_HEADER) };
    let nt = unsafe { file_base.add(dos.e_lfanew as usize) } as *mut IMAGE_NT_HEADERS64;

    unsafe {
        let dir = (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];
        if dir.VirtualAddress == 0 || dir.Size == 0 {
            return Err(ExportError::ImageUnaligned);
        }

        let exp_dir: *mut IMAGE_EXPORT_DIRECTORY = rva_from_file(file_base, nt, dir.VirtualAddress);

        if exp_dir.is_null() {
            return Err(ExportError::ImageUnaligned);
        }

        let exp = read_unaligned(exp_dir);

        let names: *const u32 = rva_from_file(file_base, nt, exp.AddressOfNames);
        let funcs: *const u32 = rva_from_file(file_base, nt, exp.AddressOfFunctions);
        let ords: *const u16 = rva_from_file(file_base, nt, exp.AddressOfNameOrdinals);

        if names.is_null() || funcs.is_null() || ords.is_null() {
            return Err(ExportError::ImageUnaligned);
        }

        for i in 0..exp.NumberOfNames {
            let name_rva = read_unaligned(names.add(i as usize));
            let name_ptr = rva_from_file::<u8>(file_base, nt, name_rva);

            if name_ptr.is_null() {
                continue;
            }

            let export_name = CStr::from_ptr(name_ptr as *const i8).to_str().ok();
            if export_name == Some(name) {
                let ord_index = read_unaligned(ords.add(i as usize)) as usize;
                let func_rva = read_unaligned(funcs.add(ord_index)) as u32;
                let func_ptr = rva_from_file::<u8>(file_base, nt, func_rva) as usize;

                return Ok(transmute::<usize, unsafe extern "system" fn()>(func_ptr));
            }
        }

        Err(ExportError::ExportNotFound)
    }
}

pub fn calculate_memory_delta(buf_start_address: usize, fn_ptr_address: usize) -> Option<usize> {
    let res = fn_ptr_address.saturating_sub(buf_start_address);

    if res == 0 {
        return None;
    }

    Some(res)
}
