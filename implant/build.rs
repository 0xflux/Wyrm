use std::env;

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
    ];

    for key in envs {
        println!("cargo:rerun-if-env-changed={key}");
    }

    for var in envs {
        if let Ok(val) = env::var(var) {
            println!("cargo:rustc-env={var}={val}");
        }
    }
}
