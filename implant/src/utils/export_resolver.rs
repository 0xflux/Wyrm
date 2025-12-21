//! Export resolver is a local copy of my https://github.com/0xflux/PE-Export-Resolver crate.
//! Currently the module cannot depend on certain windows crate, so some FFI may have to be
//! adjusted by hand in this module. Doing so should also reduce the overall binary size.

use core::{arch::asm, ffi::c_void, ops::Add, ptr::read, slice::from_raw_parts};

use windows_sys::Win32::System::{
    SystemInformation::IMAGE_FILE_MACHINE,
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
            let dll_base = *(current_entry.add(0x30) as *const usize);
            let module_name_address = *(current_entry.add(0x60) as *const usize);
            let module_length = *(current_entry.add(0x58) as *const u16);

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

#[inline]
fn to_lowercase_ascii(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { c + 32 } else { c }
}

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
    let dos_header: IMAGE_DOS_HEADER = unsafe { read(dll_base as *const IMAGE_DOS_HEADER) };
    if dos_header.e_magic != IMAGE_DOS_SIGNATURE {
        return Err(ExportResolveError::MagicByteMismatch);
    }

    // check the NT headers
    let nt_headers =
        unsafe { read(dll_base.offset(dos_header.e_lfanew as isize) as *const IMAGE_NT_HEADERS64) };
    if nt_headers.Signature != IMAGE_NT_SIGNATURE {
        return Err(ExportResolveError::MagicByteMismatch);
    }

    // get the export directory
    // https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-image_data_directory
    // found from first item in the DataDirectory; then we take the structure in memory at dll_base + RVA
    let export_dir_rva = nt_headers.OptionalHeader.DataDirectory[0].VirtualAddress;
    let export_offset = unsafe { dll_base.add(export_dir_rva as usize) };
    let export_dir: IMAGE_EXPORT_DIRECTORY =
        unsafe { read(export_offset as *const IMAGE_EXPORT_DIRECTORY) };

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

#[repr(C)]
#[allow(non_snake_case)]
pub struct IMAGE_NT_HEADERS64 {
    pub Signature: u32,
    pub FileHeader: IMAGE_FILE_HEADER,
    pub OptionalHeader: IMAGE_OPTIONAL_HEADER64,
}

#[repr(C)]
#[allow(non_snake_case)]
pub struct IMAGE_FILE_HEADER {
    pub Machine: IMAGE_FILE_MACHINE,
    pub NumberOfSections: u16,
    pub TimeDateStamp: u32,
    pub PointerToSymbolTable: u32,
    pub NumberOfSymbols: u32,
    pub SizeOfOptionalHeader: u16,
    pub Characteristics: IMAGE_FILE_CHARACTERISTICS,
}

#[repr(C, packed(4))]
#[allow(non_snake_case)]
pub struct IMAGE_OPTIONAL_HEADER64 {
    pub Magic: IMAGE_OPTIONAL_HEADER_MAGIC,
    pub MajorLinkerVersion: u8,
    pub MinorLinkerVersion: u8,
    pub SizeOfCode: u32,
    pub SizeOfInitializedData: u32,
    pub SizeOfUninitializedData: u32,
    pub AddressOfEntryPoint: u32,
    pub BaseOfCode: u32,
    pub ImageBase: u64,
    pub SectionAlignment: u32,
    pub FileAlignment: u32,
    pub MajorOperatingSystemVersion: u16,
    pub MinorOperatingSystemVersion: u16,
    pub MajorImageVersion: u16,
    pub MinorImageVersion: u16,
    pub MajorSubsystemVersion: u16,
    pub MinorSubsystemVersion: u16,
    pub Win32VersionValue: u32,
    pub SizeOfImage: u32,
    pub SizeOfHeaders: u32,
    pub CheckSum: u32,
    pub Subsystem: IMAGE_SUBSYSTEM,
    pub DllCharacteristics: IMAGE_DLL_CHARACTERISTICS,
    pub SizeOfStackReserve: u64,
    pub SizeOfStackCommit: u64,
    pub SizeOfHeapReserve: u64,
    pub SizeOfHeapCommit: u64,
    pub LoaderFlags: u32,
    pub NumberOfRvaAndSizes: u32,
    pub DataDirectory: [IMAGE_DATA_DIRECTORY; 16],
}

#[repr(C)]
#[allow(non_snake_case)]
pub struct IMAGE_DATA_DIRECTORY {
    pub VirtualAddress: u32,
    pub Size: u32,
}

#[repr(transparent)]
#[allow(non_camel_case_types)]
pub struct IMAGE_DLL_CHARACTERISTICS(pub u16);

#[repr(transparent)]
#[allow(non_camel_case_types)]
pub struct IMAGE_SUBSYSTEM(pub u16);

#[repr(transparent)]
#[allow(non_camel_case_types)]
pub struct IMAGE_OPTIONAL_HEADER_MAGIC(pub u16);

#[repr(transparent)]
#[allow(non_camel_case_types)]
pub struct IMAGE_FILE_CHARACTERISTICS(pub u16);
