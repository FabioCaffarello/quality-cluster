#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedBootstrapDiagnostics {
    pub source: String,
    pub event: String,
    pub signature: String,
    pub runtime_refs: Vec<String>,
}

pub fn parse_consumer_bootstrap_diagnostics(raw: &str) -> Option<LoadedBootstrapDiagnostics> {
    parse_loaded_bootstrap_diagnostics(raw, "consumer", &["consumer runtime ready"])
}

pub fn parse_emulator_bootstrap_diagnostics(raw: &str) -> Option<LoadedBootstrapDiagnostics> {
    parse_loaded_bootstrap_diagnostics(
        raw,
        "emulator",
        &["emulator bootstrap refreshed", "emulator started"],
    )
}

pub fn compact_bootstrap_signature(signature: &str) -> String {
    signature
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ; ")
}

fn parse_loaded_bootstrap_diagnostics(
    raw: &str,
    source: &str,
    markers: &[&str],
) -> Option<LoadedBootstrapDiagnostics> {
    for line in raw.lines().rev() {
        if !markers.iter().any(|marker| line.contains(marker)) {
            continue;
        }

        let signature = parse_slog_field(line, "bootstrap_signature")?;
        let runtime_refs = parse_runtime_refs(&parse_slog_field(line, "runtime_refs")?);
        let event = markers
            .iter()
            .find(|marker| line.contains(**marker))
            .copied()
            .unwrap_or("loaded")
            .to_string();

        return Some(LoadedBootstrapDiagnostics {
            source: source.to_string(),
            event,
            signature,
            runtime_refs,
        });
    }

    None
}

fn parse_slog_field(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=");
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];

    if let Some(after_quote) = rest.strip_prefix('"') {
        let mut value = String::new();
        let mut escaped = false;
        for ch in after_quote.chars() {
            if escaped {
                value.push(match ch {
                    'n' => '\n',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => return Some(value),
                other => value.push(other),
            }
        }
        return Some(value);
    }

    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn parse_runtime_refs(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(trimmed)
        .trim();

    if inner.is_empty() {
        return Vec::new();
    }

    inner
        .split_whitespace()
        .map(|part| part.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_consumer_bootstrap_diagnostics_uses_latest_matching_line() {
        let parsed = parse_consumer_bootstrap_diagnostics(
            r#"time=2026-03-16T18:00:00Z level=INFO msg="consumer runtime ready" bootstrap_signature="old" runtime_refs="[tenant:br:old:artifact-old]"
time=2026-03-16T18:00:01Z level=INFO msg="consumer runtime ready" bootstrap_signature="new" runtime_refs="[tenant:br:new:artifact-new]""#,
        )
        .expect("consumer diagnostics");

        assert_eq!(parsed.source, "consumer");
        assert_eq!(parsed.signature, "new");
        assert_eq!(parsed.runtime_refs, vec!["tenant:br:new:artifact-new"]);
    }

    #[test]
    fn parse_emulator_bootstrap_diagnostics_accepts_started_event() {
        let parsed = parse_emulator_bootstrap_diagnostics(
            r#"time=2026-03-16T18:00:01Z level=INFO msg="emulator started" bootstrap_signature="emulator-signature" runtime_refs="[tenant:br:ver-br:artifact-br]""#,
        )
        .expect("emulator diagnostics");

        assert_eq!(parsed.source, "emulator");
        assert_eq!(parsed.event, "emulator started");
    }

    #[test]
    fn compact_bootstrap_signature_flattens_multiline_values() {
        let compacted = compact_bootstrap_signature("binding|a\nruntime|b\n");
        assert_eq!(compacted, "binding|a ; runtime|b");
    }
}
