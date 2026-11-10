use crate::Job;

/// Seconds to wait before the next attempt. Caps at 5 minutes.
pub fn backoff_secs(attempt: u32) -> u64 {
    let shift = attempt.saturating_sub(1).min(8);
    (2u64 << shift).min(300)
}

/// Network drops and HTTP 429 stay in the queue. A private or missing video does not.
pub fn is_transient_download_error(tail: &str) -> bool {
    let lower = tail.to_lowercase();
    if lower.trim().is_empty() {
        return false;
    }
    const HARD: &[&str] = &[
        "private video",
        "video unavailable",
        "this video is not available",
        "has been removed",
        "copyright",
        "unsupported url",
        "is not a valid url",
        "sign in to confirm",
        "403: forbidden",
        "http error 404",
        "404: not found",
    ];
    if HARD.iter().any(|needle| lower.contains(needle)) {
        return false;
    }
    const SOFT: &[&str] = &[
        "429",
        "too many requests",
        "503",
        "502",
        "504",
        "timed out",
        "timeout",
        "connection reset",
        "connection aborted",
        "connection refused",
        "temporary failure",
        "network is unreachable",
        "getaddrinfo",
        "name or service not known",
        "nodename nor servname",
        "tls",
        "ssl",
        "unable to download webpage",
        "incomplete read",
        "eof",
        "transport",
        "overloaded",
        "rate-limit",
        "rate limit",
        "no route to host",
        "failed to resolve",
    ];
    SOFT.iter().any(|needle| lower.contains(needle))
}

/// Put a downloading job back in the queue. A job the user paused stays paused.
pub fn resume_interrupted(jobs: &mut [Job]) {
    for job in jobs.iter_mut() {
        if job.state == "downloading" {
            job.state = "queued".into();
            job.speed = None;
            job.eta = None;
            job.retry_at = None;
        }
    }
}

/// Keep the job queued and remember when it may run again.
pub fn apply_transient_retry(job: &mut Job) -> u64 {
    job.attempts = job.attempts.saturating_add(1);
    let secs = backoff_secs(job.attempts);
    let when = chrono::Utc::now() + chrono::Duration::seconds(secs as i64);
    job.retry_at = Some(when.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    job.state = "queued".into();
    job.progress = 0.0;
    job.speed = None;
    job.eta = None;
    job.error = None;
    secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_then_caps() {
        assert_eq!(backoff_secs(1), 2);
        assert_eq!(backoff_secs(2), 4);
        assert_eq!(backoff_secs(3), 8);
        assert_eq!(backoff_secs(8), 256);
        assert_eq!(backoff_secs(9), 300);
        assert_eq!(backoff_secs(20), 300);
    }

    #[test]
    fn rate_limit_and_network_are_retried_but_a_private_video_is_not() {
        assert!(is_transient_download_error(
            "ERROR: Unable to download video data: HTTP Error 429: Too Many Requests"
        ));
        assert!(is_transient_download_error(
            "ERROR: Unable to download webpage: The read operation timed out"
        ));
        assert!(is_transient_download_error(
            "ERROR: Unable to download webpage: <urlopen error [Errno -2] Name or service not known>"
        ));
        assert!(!is_transient_download_error(
            "ERROR: Private video. Sign in if you've been granted access"
        ));
        assert!(!is_transient_download_error(
            "ERROR: unable to download video data: HTTP Error 403: Forbidden"
        ));
        assert!(!is_transient_download_error(""));
    }
}
