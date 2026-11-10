//! Discord IPC frames. On Windows the socket is `\\.\pipe\discord-ipc-N`.

pub fn frame(opcode: u32, json: &str) -> Vec<u8> {
    let bytes = json.as_bytes();
    let mut out = Vec::with_capacity(8 + bytes.len());
    out.extend_from_slice(&opcode.to_le_bytes());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

pub fn handshake(client_id: &str) -> String {
    format!(r#"{{"v":1,"client_id":"{}"}}"#, client_id.replace('"', ""))
}

pub fn activity(pid: u32, nonce: u64, title: &str, artist: &str, playing: bool) -> String {
    let details = format!("ouvindo {} no bob baixa", clean(title));
    let state = clean(if artist.trim().is_empty() { "bob baixa" } else { artist });
    serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": pid,
            "activity": if playing {
                serde_json::json!({
                    "details": details,
                    "state": state,
                    "type": 2,
                    "instance": false
                })
            } else {
                serde_json::Value::Null
            }
        },
        "nonce": nonce.to_string()
    })
    .to_string()
}

pub fn pipe_path(index: u8) -> String {
    format!(r"\\.\pipe\discord-ipc-{index}")
}

fn clean(value: &str) -> String {
    value.chars().filter(|c| *c != '"' && *c != '\n' && *c != '\r').take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_is_a_little_endian_header_plus_json() {
        let bytes = frame(1, "{}");
        assert_eq!(&bytes[..4], &1u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &2u32.to_le_bytes());
        assert_eq!(&bytes[8..], b"{}");
        let text = activity(4, 1, "Saudade", "artista", true);
        assert!(text.contains("ouvindo Saudade no bob baixa"));
        assert_eq!(pipe_path(0), r"\\.\pipe\discord-ipc-0");
    }
}
