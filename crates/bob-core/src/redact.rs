//! Strip secrets before a diagnostic leaves the machine.

use serde_json::Value;

const SECRET_KEYS: &[&str] = &[
    "geminikey",
    "acoustidkey",
    "clientsecret",
    "refreshtoken",
    "accesstoken",
    "apikey",
    "authorization",
    "password",
    "secret",
    "token",
    "idtoken",
    "cookie",
    "cookies",
];

pub fn redact_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if secret_key(&key) {
                    map.insert(key, Value::String("[redacted]".into()));
                } else if let Some(child) = map.get_mut(&key) {
                    redact_json(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json(item);
            }
        }
        Value::String(text) => *text = redact_text(text),
        _ => {}
    }
}

pub fn redact_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(end) = secret_span(input, index) {
            out.push_str("[redacted]");
            index = end;
            continue;
        }
        let ch = input[index..].chars().next().unwrap_or(' ');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn secret_key(key: &str) -> bool {
    let folded: String = key.chars().filter(|ch| *ch != '_' && *ch != '-').flat_map(|ch| ch.to_lowercase()).collect();
    SECRET_KEYS.iter().any(|name| folded == *name || folded.ends_with(name))
}

fn secret_span(input: &str, index: usize) -> Option<usize> {
    let rest = &input[index..];
    let lower = rest.to_ascii_lowercase();
    for marker in ["bearer ", "aiza", "ya29."] {
        if lower.starts_with(marker) {
            return Some(index + take_token(rest, marker.len()));
        }
    }
    if rest.starts_with("https://") || rest.starts_with("http://") {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let url = &rest[..end];
        if url.len() > 80 || url.contains("token") || url.contains("upload_id") || url.contains("key=") {
            return Some(index + end);
        }
    }
    None
}

fn take_token(rest: &str, from: usize) -> usize {
    let tail = rest.get(from..).unwrap_or("");
    let extra = tail.find(char::is_whitespace).unwrap_or(tail.len());
    from + extra
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_tokens_in_logs_disappear() {
        let mut value = serde_json::json!({
            "geminiKey": "AIzaSySecretValue1234567890",
            "acoustidKey": "abc",
            "nested": { "refresh_token": "1//secret" },
            "title": "Saudade"
        });
        redact_json(&mut value);
        assert_eq!(value["geminiKey"], "[redacted]");
        assert_eq!(value["acoustidKey"], "[redacted]");
        assert_eq!(value["nested"]["refresh_token"], "[redacted]");
        assert_eq!(value["title"], "Saudade");
        let log = "sessão https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=very-long-session-token-value-that-must-not-leak";
        let clean = redact_text(log);
        assert!(!clean.contains("upload_id"));
        assert!(clean.contains("[redacted]"));
        assert!(!redact_text("conectado como ana@example.com").contains("[redacted]"));
    }
}
