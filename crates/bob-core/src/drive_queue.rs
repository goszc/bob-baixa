/// Upload queue transitions. The worker applies these; tests cover them without the network.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivePhase {
    Waiting,
    Uploading,
    Sent,
    Failed,
}

impl DrivePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Uploading => "uploading",
            Self::Sent => "sent",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "uploading" => Self::Uploading,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            _ => Self::Waiting,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveUploadState {
    pub phase: DrivePhase,
    pub attempts: u32,
    pub not_before: i64,
    pub sent: u64,
    pub total: u64,
    pub file_id: String,
    pub error: String,
    pub offset: u64,
}

impl DriveUploadState {
    pub fn fresh() -> Self {
        Self {
            phase: DrivePhase::Waiting,
            attempts: 0,
            not_before: 0,
            sent: 0,
            total: 0,
            file_id: String::new(),
            error: String::new(),
            offset: 0,
        }
    }

    /// A crash mid-upload must not leave the item unclaimable.
    pub fn recover_interrupted(&mut self) {
        if self.phase == DrivePhase::Uploading {
            self.phase = DrivePhase::Waiting;
            self.not_before = 0;
        }
    }

    pub fn claimable(&self, now: i64) -> bool {
        match self.phase {
            DrivePhase::Waiting | DrivePhase::Failed => self.not_before <= now,
            DrivePhase::Uploading | DrivePhase::Sent => false,
        }
    }

    pub fn begin(&mut self, total: u64) {
        self.phase = DrivePhase::Uploading;
        self.total = total;
        self.sent = self.offset.min(total);
        self.error.clear();
    }

    pub fn progress(&mut self, sent: u64, total: u64) {
        self.phase = DrivePhase::Uploading;
        self.sent = sent;
        self.total = total;
    }

    pub fn succeed(&mut self, file_id: &str) {
        self.phase = DrivePhase::Sent;
        self.file_id = file_id.to_string();
        self.error.clear();
        self.sent = self.total;
        self.not_before = 0;
        self.offset = 0;
    }

    pub fn fail(&mut self, message: &str, now: i64, wait_secs: u64) {
        self.phase = DrivePhase::Failed;
        self.attempts = self.attempts.saturating_add(1);
        self.error = message.chars().take(400).collect();
        self.not_before = now.saturating_add(wait_secs as i64);
        self.offset = 0;
    }

    pub fn retry(&mut self) {
        self.phase = DrivePhase::Waiting;
        self.not_before = 0;
        self.offset = 0;
        self.error.clear();
    }

    pub fn percent(&self) -> u32 {
        if self.phase == DrivePhase::Sent {
            return 100;
        }
        if self.total == 0 {
            return 0;
        }
        ((self.sent.min(self.total) as u128 * 100) / self.total as u128) as u32
    }
}

pub fn claim_index(items: &[DriveUploadState], now: i64) -> Option<usize> {
    items.iter().position(|item| item.claimable(now))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStep {
    Continue(u64),
    Done,
    Expired,
    Wait,
    Rejected(u16),
}

/// Google answers an unfinished resumable PUT with 308 Resume Incomplete.
/// That status is not a redirect: the next offset comes from the Range header.
pub fn chunk_step(status: u16, range: Option<&str>, sent_through: u64) -> ChunkStep {
    match status {
        308 => {
            let next = range
                .and_then(range_end)
                .map(|end| end.saturating_add(1))
                .unwrap_or(sent_through.saturating_add(1));
            ChunkStep::Continue(next)
        }
        200..=299 => ChunkStep::Done,
        401 => ChunkStep::Expired,
        429 | 503 => ChunkStep::Wait,
        other => ChunkStep::Rejected(other),
    }
}

pub fn range_end(header: &str) -> Option<u64> {
    let value = header.trim();
    let value = value.strip_prefix("bytes=").or_else(|| value.strip_prefix("bytes "))?;
    let end = value.split('-').nth(1)?.split('/').next()?.trim();
    end.parse().ok()
}

/// Drive appProperties reject values longer than 124 bytes.
pub fn truncate_property(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if out.len() + ch.len_utf8() > 120 {
            break;
        }
        out.push(ch);
    }
    out
}

pub fn drive_file_link(id: &str) -> String {
    format!("https://drive.google.com/file/d/{id}/view")
}

/// Windows `CRED_MAX_CREDENTIAL_BLOB_SIZE`. The password is stored as UTF-16, so the blob is two bytes per unit.
pub const WINDOWS_CREDENTIAL_BLOB_MAX: usize = 2560;

pub fn credential_blob_bytes(secret: &str) -> usize {
    secret.encode_utf16().count().saturating_mul(2)
}

pub fn credential_fits(secret: &str) -> bool {
    !secret.is_empty() && credential_blob_bytes(secret) <= WINDOWS_CREDENTIAL_BLOB_MAX
}

/// Credential Manager keeps only the refresh token. Older builds wrote the whole token JSON.
pub fn stored_refresh(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_start_matches('\u{feff}');
    if raw.is_empty() {
        return None;
    }
    if let Some(rest) = raw.strip_prefix('{') {
        let _ = rest;
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        let token = value
            .get("refresh_token")
            .or_else(|| value.get("refreshToken"))
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim();
        if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        }
    } else {
        Some(raw.to_string())
    }
}

pub fn token_gap(error: &str) -> bool {
    error.starts_with("sem acesso ao drive")
}

/// Resumable upload metadata. The body is UTF-8 JSON; names are not re-encoded.
pub fn upload_metadata_json(name: &str, parent: &str, props: &[(&str, &str)]) -> String {
    let mut properties = serde_json::Map::new();
    for (key, value) in props {
        properties.insert((*key).to_string(), serde_json::Value::String((*value).to_string()));
    }
    serde_json::to_string(&serde_json::json!({
        "name": name,
        "parents": [parent],
        "appProperties": properties,
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_stays_visible_and_waits_before_the_next_claim() {
        let mut item = DriveUploadState::fresh();
        assert_eq!(claim_index(&[item.clone()], 1_000), Some(0));
        item.begin(1_000);
        item.progress(250, 1_000);
        assert_eq!(item.percent(), 25);
        assert!(claim_index(&[item.clone()], 1_000).is_none());
        item.fail("envio interrompido (308)", 1_000, 8);
        assert_eq!(item.phase, DrivePhase::Failed);
        assert_eq!(item.error, "envio interrompido (308)");
        assert_eq!(item.attempts, 1);
        assert!(claim_index(&[item.clone()], 1_007).is_none());
        assert_eq!(claim_index(&[item.clone()], 1_008), Some(0));
        item.retry();
        assert_eq!(item.phase, DrivePhase::Waiting);
        assert!(item.error.is_empty());
        assert_eq!(claim_index(&[item.clone()], 0), Some(0));
    }

    #[test]
    fn success_records_the_link_and_an_interrupted_upload_can_be_claimed_again() {
        let mut item = DriveUploadState::fresh();
        item.begin(10);
        item.succeed("abc");
        assert_eq!(item.phase, DrivePhase::Sent);
        assert_eq!(item.percent(), 100);
        assert_eq!(drive_file_link(&item.file_id), "https://drive.google.com/file/d/abc/view");
        assert!(claim_index(&[item.clone()], 9_999).is_none());

        let mut crashed = DriveUploadState::fresh();
        crashed.begin(10);
        crashed.recover_interrupted();
        assert_eq!(crashed.phase, DrivePhase::Waiting);
        assert_eq!(claim_index(&[crashed], 0), Some(0));
    }

    #[test]
    fn resume_incomplete_is_not_a_redirect() {
        assert_eq!(chunk_step(308, Some("bytes=0-8388607"), 8388607), ChunkStep::Continue(8388608));
        assert_eq!(chunk_step(308, None, 99), ChunkStep::Continue(100));
        assert_eq!(chunk_step(200, None, 99), ChunkStep::Done);
        assert_eq!(chunk_step(201, None, 99), ChunkStep::Done);
        assert_eq!(chunk_step(401, None, 0), ChunkStep::Expired);
        assert_eq!(chunk_step(429, None, 0), ChunkStep::Wait);
        assert_eq!(chunk_step(400, None, 0), ChunkStep::Rejected(400));
        assert_eq!(range_end("bytes=0-99"), Some(99));
    }

    #[test]
    fn property_values_stay_inside_the_drive_limit() {
        let long = "á".repeat(80);
        let cut = truncate_property(&long);
        assert!(cut.len() <= 120);
        assert!(cut.is_char_boundary(cut.len()));
        assert_eq!(truncate_property("Curta"), "Curta");
    }

    #[test]
    fn a_missing_token_is_an_error_instead_of_a_silent_wait() {
        let mut item = DriveUploadState::fresh();
        assert_eq!(item.phase, DrivePhase::Waiting);
        let reason = "sem acesso ao drive: não consegui ler o refresh token";
        item.fail(reason, 50, 3_600);
        assert_eq!(item.phase, DrivePhase::Failed);
        assert!(token_gap(&item.error));
        assert!(claim_index(&[item.clone()], 50).is_none());
        item.retry();
        assert_eq!(item.phase, DrivePhase::Waiting);
        assert!(claim_index(&[item], 50).is_some());
    }

    #[test]
    fn only_the_refresh_token_fits_in_the_windows_credential_blob() {
        let refresh = format!("1//{}", "b".repeat(180));
        let access = format!("ya29.{}", "a".repeat(2_000));
        let whole = format!(
            r#"{{"refreshToken":"{refresh}","accessToken":"{access}","expiresAt":1,"email":"gust.cust047@gmail.com"}}"#
        );
        assert!(credential_blob_bytes(&whole) > WINDOWS_CREDENTIAL_BLOB_MAX);
        assert!(!credential_fits(&whole));
        assert!(credential_fits(&refresh));
        assert_eq!(credential_blob_bytes("ab"), 4);
        assert_eq!(stored_refresh(&refresh).as_deref(), Some(refresh.as_str()));
        assert_eq!(stored_refresh(&whole).as_deref(), Some(refresh.as_str()));
        assert_eq!(stored_refresh("  ").as_deref(), None);
    }

    #[test]
    fn upload_metadata_keeps_fullwidth_unicode_as_utf8() {
        let names = [
            "Low Roar - \u{FF02}I'll Keep Coming\u{FF02} [KnrGMHhnqrw].mp4",
            "Péricles - Se Eu Largar O Freio \u{FF5C} OFICIAL [aSfyjDLvcVo].mp4",
        ];
        for name in names {
            let body = upload_metadata_json(name, "pasta", &[("bobTitle", name), ("bobKind", "video")]);
            let bytes = body.as_bytes();
            assert!(std::str::from_utf8(bytes).is_ok());
            let value: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(value["name"], name);
            assert_eq!(value["appProperties"]["bobTitle"], name);
            assert!(body.contains("\u{FF02}") || body.contains("\u{FF5C}") || body.contains("Péricles"));
            assert!(!body.contains('%'));
        }
    }
}
