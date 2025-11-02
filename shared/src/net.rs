use serde::{Deserialize, Serialize};

use crate::tasks::{Command, Task};

const NET_XOR_KEY: u8 = 0x3d;
pub const STR_CRYPT_XOR_KEY: u8 = 0x1f;

pub const ADMIN_AUTH_SEPARATOR: &str = "=authdivider=";
pub const ADMIN_ENDPOINT: &str = "admin";
pub const ADMIN_LOGIN_ENDPOINT: &str = "admin_login";
/// The API endpoint for whether an unread notification exists for a specific agent
pub const NOTIFICATION_CHECK_AGENT_ENDPOINT: &str = "check_notifs";

pub type CompletedTasks = Vec<Vec<u16>>;
pub type TasksNetworkStream = Vec<Vec<u8>>;

#[derive(Serialize, Deserialize)]
pub struct AdminLoginPacket {
    pub username: String,
    pub password: String,
}

pub trait XorEncode {
    fn xor_network_stream(self) -> Self;
}

impl XorEncode for Vec<u8> {
    fn xor_network_stream(mut self) -> Self {
        for b in &mut self {
            *b ^= NET_XOR_KEY;
        }

        self
    }
}

pub fn encode_u16buf_to_u8buf(input: &[u16]) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(input.len());

    for word in input.iter() {
        let [lo, hi] = word.to_le_bytes();
        buf.push(lo);
        buf.push(hi);
    }

    buf
}

pub fn decode_u8buf_to_u16buf(input: &[u8]) -> Vec<u16> {
    let mut u16_bytes: Vec<u16> = Vec::with_capacity(input.len());

    for chunk in input.chunks_exact(2) {
        let lo = chunk[0];
        let hi = chunk[1];
        let word = u16::from_le_bytes([lo, hi]);
        u16_bytes.push(word);
    }

    u16_bytes
}

pub fn decode_http_response(byte_response: &[u8]) -> Task {
    const COMMAND_INT_BYTE_SZ: usize = 4;
    const TASK_ID_BYTE_SZ: usize = 4;
    const TIMESTAMP_BYTE_SZ: usize = 8;

    //
    // Pull out the task id (database ref)
    //
    let task_id = i32::from_le_bytes([
        byte_response[0],
        byte_response[1],
        byte_response[2],
        byte_response[3],
    ]);

    //
    // Pull out command
    //
    let command_int = u32::from_le_bytes([
        byte_response[4],
        byte_response[5],
        byte_response[6],
        byte_response[7],
    ]);
    let command = Command::from_u32(command_int);

    //
    // Pull out timestamp of completed task
    //
    let timestamp = i64::from_le_bytes([
        byte_response[8],
        byte_response[9],
        byte_response[10],
        byte_response[11],
        byte_response[12],
        byte_response[13],
        byte_response[14],
        byte_response[15],
    ]);

    // Check if we have trailing metadata, if not - return the data as obtained thus far
    let basic_packet_len = COMMAND_INT_BYTE_SZ + TIMESTAMP_BYTE_SZ + TASK_ID_BYTE_SZ;
    if byte_response.len() == basic_packet_len {
        return Task {
            id: task_id,
            command,
            metadata: None,
            completed_time: timestamp,
        };
    }

    //
    // We now know there is a message present, so we can pull it out of the u8 vec by
    // converting it to a utf-16 string.
    //

    let message_bytes = &byte_response[basic_packet_len..];
    let u16_bytes = decode_u8buf_to_u16buf(message_bytes);
    let task_metadata_string = String::from_utf16_lossy(&u16_bytes);

    Task {
        id: task_id,
        command,
        metadata: Some(task_metadata_string),
        completed_time: timestamp,
    }
}
