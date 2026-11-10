/// Google OAuth query and form bodies for the desktop Drive login.
/// Values are percent-encoded once. The URL is a single string so a shell
/// opener cannot split it on `&`.

pub const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
pub const GOOGLE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";

pub fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(value: &str) -> String {
    let bytes = value.replace('+', " ");
    let mut out = Vec::new();
    let raw = bytes.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' && index + 2 < raw.len() {
            if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&raw[index + 1..index + 3]).unwrap_or(""), 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(raw[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Authorization URL for the installed-app loopback flow.
/// `redirect_uri` is `http://127.0.0.1:<port>` with no path.
pub fn google_auth_url(client_id: &str, redirect_uri: &str, code_challenge: &str, state: &str) -> String {
    let body = query(&[
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("scope", DRIVE_FILE_SCOPE),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ]);
    format!("{GOOGLE_AUTH_ENDPOINT}?{body}")
}

pub fn google_token_exchange_body(
    client_id: &str,
    client_secret: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> String {
    query(&[
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("code_verifier", code_verifier),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
    ])
}

pub fn google_token_refresh_body(client_id: &str, client_secret: &str, refresh_token: &str) -> String {
    query(&[
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ])
}

/// `code` from the loopback redirect, after `state` matches.
pub fn oauth_callback_code(request_url: &str, expected_state: &str) -> Result<String, &'static str> {
    let query = request_url.split_once('?').map(|(_, query)| query).unwrap_or("");
    let pairs = parse_query(query);
    let error = pairs.iter().find(|(key, _)| key == "error").map(|(_, value)| value.as_str());
    if error.is_some() {
        return Err("o google recusou o login");
    }
    let state = pairs.iter().find(|(key, _)| key == "state").map(|(_, value)| value.as_str());
    if state != Some(expected_state) {
        return Err("o retorno do google não confere");
    }
    pairs
        .into_iter()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
        .ok_or("o google não devolveu o código")
}

pub fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_url_contains_every_required_parameter_once() {
        let redirect = "http://127.0.0.1:47821";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        let url = google_auth_url("client id/with spaces", redirect, challenge, "state+value");

        assert_eq!(url.matches('?').count(), 1);
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("client%20id%2Fwith%20spaces"));
        assert!(url.contains("http%3A%2F%2F127.0.0.1%3A47821"));
        assert!(url.contains("state%2Bvalue"));
        assert!(!url.contains("%25"), "values must be encoded once");

        let pairs = parse_query(url.split_once('?').unwrap().1);
        let value = |name: &str| -> Vec<String> {
            pairs.iter().filter(|(key, _)| key == name).map(|(_, item)| item.clone()).collect()
        };
        assert_eq!(value("response_type"), vec!["code"]);
        assert_eq!(value("client_id"), vec!["client id/with spaces"]);
        assert_eq!(value("redirect_uri"), vec![redirect]);
        assert_eq!(value("scope"), vec![DRIVE_FILE_SCOPE]);
        assert_eq!(value("code_challenge"), vec![challenge]);
        assert_eq!(value("code_challenge_method"), vec!["S256"]);
        assert_eq!(value("state"), vec!["state+value"]);
        assert_eq!(value("access_type"), vec!["offline"]);
        assert_eq!(value("prompt"), vec!["consent"]);
        assert_eq!(pairs.len(), 9);
    }

    #[test]
    fn token_exchange_uses_the_same_redirect_and_the_verifier() {
        let redirect = "http://127.0.0.1:49152";
        let auth = google_auth_url("id.apps.googleusercontent.com", redirect, "challenge", "abc");
        let auth_redirect = parse_query(auth.split_once('?').unwrap().1)
            .into_iter()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1;
        let body = google_token_exchange_body("id.apps.googleusercontent.com", "secret/with+slash", "auth-code", "verifier", redirect);
        let pairs = parse_query(&body);
        let value = |name: &str| pairs.iter().find(|(key, _)| key == name).map(|(_, item)| item.clone());
        assert_eq!(value("grant_type").as_deref(), Some("authorization_code"));
        assert_eq!(value("client_id").as_deref(), Some("id.apps.googleusercontent.com"));
        assert_eq!(value("client_secret").as_deref(), Some("secret/with+slash"));
        assert_eq!(value("code").as_deref(), Some("auth-code"));
        assert_eq!(value("code_verifier").as_deref(), Some("verifier"));
        assert_eq!(value("redirect_uri").as_deref(), Some(redirect));
        assert_eq!(value("redirect_uri").unwrap(), auth_redirect);
        assert!(body.contains("secret%2Fwith%2Bslash"));
        assert!(!body.contains("%252F"));
    }

    #[test]
    fn refresh_sends_the_refresh_token_and_the_client() {
        let body = google_token_refresh_body("id", "secret", "1//refresh");
        let pairs = parse_query(&body);
        let value = |name: &str| pairs.iter().find(|(key, _)| key == name).map(|(_, item)| item.clone());
        assert_eq!(value("grant_type").as_deref(), Some("refresh_token"));
        assert_eq!(value("client_id").as_deref(), Some("id"));
        assert_eq!(value("client_secret").as_deref(), Some("secret"));
        assert_eq!(value("refresh_token").as_deref(), Some("1//refresh"));
        assert!(value("code_verifier").is_none());
        assert!(body.contains("1%2F%2Frefresh"));
    }

    #[test]
    fn callback_requires_the_matching_state() {
        assert_eq!(oauth_callback_code("/?code=abc&state=xyz", "xyz").unwrap(), "abc");
        assert!(oauth_callback_code("/?code=abc&state=nope", "xyz").is_err());
        assert!(oauth_callback_code("/?error=access_denied&state=xyz", "xyz").is_err());
    }
}
