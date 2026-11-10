//! ReplayGain from an EBU R128 measurement. Playback applies it; the file stays as it is.

pub fn replay_gain_db(measured_lufs: f64) -> Option<f64> {
    if !measured_lufs.is_finite() || measured_lufs < -70.0 || measured_lufs > 0.0 {
        return None;
    }
    Some((-14.0 - measured_lufs).clamp(-24.0, 12.0))
}

pub fn linear_from_db(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub fn parse_loudnorm(stderr: &str) -> Option<f64> {
    let start = stderr.rfind('{')?;
    let end = stderr[start..].find('}')? + start;
    let value: serde_json::Value = serde_json::from_str(&stderr[start..=end]).ok()?;
    let measured = if let Some(number) = value.get("input_i").and_then(|item| item.as_f64()) {
        number
    } else {
        value.get("input_i")?.as_str()?.parse().ok()?
    };
    replay_gain_db(measured)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_file_gets_a_positive_gain() {
        let text = "noise\n{\n\t\"input_i\" : \"-20.10\",\n\t\"input_tp\" : \"-1.00\"\n}\n";
        let db = parse_loudnorm(text).unwrap();
        assert!((db - 6.1).abs() < 0.05);
        assert!(linear_from_db(db) > 1.0);
        assert!(parse_loudnorm("{\"input_i\":\"-99\"}").is_none());
    }
}
