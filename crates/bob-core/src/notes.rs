/// Section of a changelog that starts at `## <version>` and stops before the next `## `.
pub fn changelog_section(markdown: &str, version: &str) -> Option<String> {
    let wanted = version.trim().trim_start_matches('v');
    if wanted.is_empty() {
        return None;
    }
    let mut collecting = false;
    let mut lines = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("## ") {
            if collecting {
                break;
            }
            let title_version = title
                .trim()
                .trim_start_matches('v')
                .split_whitespace()
                .next()
                .unwrap_or("");
            collecting = title_version == wanted;
            if collecting {
                lines.push(line);
            }
            continue;
        }
        if collecting {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        return None;
    }
    let text = lines.join("\n").trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "# changelog\n\n## 0.2.0\n\n- fila\n- presets\n\n## 0.1.0\n\n- primeira versão\n";

    #[test]
    fn picks_the_matching_version() {
        let section = changelog_section(SAMPLE, "0.2.0").unwrap();
        assert!(section.starts_with("## 0.2.0"));
        assert!(section.contains("presets"));
        assert!(!section.contains("0.1.0"));
    }

    #[test]
    fn accepts_a_v_prefix() {
        let section = changelog_section(SAMPLE, "v0.1.0").unwrap();
        assert!(section.contains("primeira versão"));
        assert!(!section.contains("presets"));
    }

    #[test]
    fn missing_version_is_none() {
        assert!(changelog_section(SAMPLE, "0.9.0").is_none());
    }
}
