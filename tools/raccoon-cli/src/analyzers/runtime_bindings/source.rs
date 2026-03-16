use crate::error::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Runtime binding source-level constants extracted from Go source files.
#[derive(Debug, Clone, Default)]
pub struct RuntimeBindingSource {
    /// Subject prefix for data-plane ingestion (e.g., "dataplane.ingestion.received").
    pub subject_prefix: String,
    /// Wildcard subject pattern (e.g., "dataplane.ingestion.received.>").
    pub subject_pattern: String,
    /// Stream name -> list of subject patterns.
    pub stream_subjects: HashMap<String, Vec<String>>,
    /// Durable consumer name -> stream name.
    pub durable_consumers: HashMap<String, String>,
    /// Lifecycle event names found in source (e.g., "config.activated").
    pub lifecycle_events: HashSet<String>,
    /// Kafka topics referenced in source or config.
    pub kafka_topics_referenced: HashSet<String>,
    /// Whether a bootstrap client exists (runtimebootstrap/client.go).
    pub has_bootstrap_client: bool,
    /// Whether a topology builder exists (dataplane/topology.go).
    pub has_topology_builder: bool,
    /// Whether a runtime cache exists (validator/runtime_cache.go).
    pub has_runtime_cache: bool,
    /// Whether a validation worker exists.
    pub has_validation_worker: bool,
    /// Bootstrap scopes from deploy configs: (scope_kind, scope_key).
    pub bootstrap_scopes: Vec<(String, String)>,
}

/// Scan Go source files under `internal/` for runtime binding constants.
pub fn scan_runtime_bindings(internal_dir: &Path) -> Result<RuntimeBindingSource> {
    let mut src = RuntimeBindingSource::default();

    scan_dir(internal_dir, &mut src)?;

    // Check for key files existence
    let bootstrap_path = internal_dir.join("application/runtimebootstrap/client.go");
    src.has_bootstrap_client = bootstrap_path.is_file();

    let topology_path = internal_dir.join("application/dataplane/topology.go");
    src.has_topology_builder = topology_path.is_file();

    let cache_path = internal_dir.join("actors/scopes/validator/runtime_cache.go");
    src.has_runtime_cache = cache_path.is_file();

    let worker_path = internal_dir.join("actors/scopes/validator/validation_worker.go");
    src.has_validation_worker = worker_path.is_file();

    // Synthesize wildcard pattern from prefix if not found as literal
    if !src.subject_prefix.is_empty() && src.subject_pattern.is_empty() {
        src.subject_pattern = format!("{}.>", src.subject_prefix);
    }

    // Ensure stream subjects include the synthesized pattern
    if !src.subject_prefix.is_empty() {
        let wildcard = format!("{}.>", src.subject_prefix);
        if let Some(subjects) = src.stream_subjects.get_mut("DATA_PLANE_INGESTION") {
            if !subjects.contains(&wildcard) {
                subjects.push(wildcard);
            }
        }
    }

    // Also scan deploy/configs for bootstrap scopes (relative to internal's parent)
    if let Some(project_root) = internal_dir.parent() {
        let configs_dir = project_root.join("deploy/configs");
        if configs_dir.is_dir() {
            scan_bootstrap_scopes(&configs_dir, &mut src)?;
        }
    }

    Ok(src)
}

fn scan_dir(dir: &Path, src: &mut RuntimeBindingSource) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, src)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("go") {
            scan_go_file(&path, src)?;
        }
    }

    Ok(())
}

fn scan_go_file(path: &Path, src: &mut RuntimeBindingSource) -> Result<()> {
    let content = std::fs::read_to_string(path)?;

    extract_subject_prefix(&content, src);
    extract_streams(&content, src);
    extract_durables(&content, src);
    extract_lifecycle_events(&content, src);

    Ok(())
}

fn extract_subject_prefix(content: &str, src: &mut RuntimeBindingSource) {
    // Look for: const subjectPrefix = "dataplane.ingestion.received"
    // or: SubjectPrefix: "..."
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        // Match: const subjectPrefix = "..."
        if trimmed.contains("subjectPrefix") || trimmed.contains("SubjectPrefix") {
            for val in extract_all_quoted(trimmed) {
                if val.starts_with("dataplane.") && val.contains("ingestion") {
                    if !val.ends_with(".>") {
                        src.subject_prefix = val;
                    } else {
                        src.subject_pattern = val;
                    }
                }
            }
        }

        // Match: SubjectPattern: "..."
        if trimmed.contains("SubjectPattern") {
            for val in extract_all_quoted(trimmed) {
                if val.ends_with(".>") && val.contains("dataplane") {
                    src.subject_pattern = val;
                }
            }
        }
    }
}

fn extract_streams(content: &str, src: &mut RuntimeBindingSource) {
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        let is_stream_context =
            trimmed.contains("Name:") || trimmed.contains("Stream") || trimmed.contains("stream");

        if !is_stream_context {
            continue;
        }

        for word in extract_all_quoted(trimmed) {
            if is_stream_name(&word) {
                let subjects = find_subjects_near(&lines, i, 10);
                src.stream_subjects
                    .entry(word)
                    .or_default()
                    .extend(subjects);
            }
        }
    }

    // Deduplicate subjects per stream
    for subjects in src.stream_subjects.values_mut() {
        subjects.sort();
        subjects.dedup();
    }
}

fn extract_durables(content: &str, src: &mut RuntimeBindingSource) {
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || !trimmed.contains("Durable") {
            continue;
        }

        for val in extract_all_quoted(trimmed) {
            if val.contains('-') && val.chars().all(|c| c.is_alphanumeric() || c == '-') {
                let stream = find_stream_name_near(&lines, i, 15)
                    .or_else(|| find_stream_name_near(&lines, lines.len() / 2, lines.len()));
                if let Some(stream_name) = stream {
                    src.durable_consumers.insert(val, stream_name);
                }
            }
        }
    }
}

fn extract_lifecycle_events(content: &str, src: &mut RuntimeBindingSource) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        // Match: EventActivated events.Name = "config.activated"
        // or: EventDraftCreated events.Name = "config.draft_created"
        if trimmed.contains("events.Name") || trimmed.contains("EventName") {
            for val in extract_all_quoted(trimmed) {
                if val.starts_with("config.") {
                    src.lifecycle_events.insert(val);
                }
            }
        }
    }
}

fn scan_bootstrap_scopes(configs_dir: &Path, src: &mut RuntimeBindingSource) -> Result<()> {
    let entries = match std::fs::read_dir(configs_dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonc") {
            continue;
        }

        let raw = std::fs::read_to_string(&path)?;
        let cleaned = strip_jsonc_comments(&raw);

        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&cleaned) {
            if let Some(bootstrap) = value.get("bootstrap") {
                let kind = bootstrap
                    .get("scope_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("global");
                let key = bootstrap
                    .get("scope_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                src.bootstrap_scopes
                    .push((kind.to_string(), key.to_string()));
            }
        }
    }

    src.bootstrap_scopes.sort();
    src.bootstrap_scopes.dedup();

    Ok(())
}

// ── Helpers (shared with topology source scanner) ───────────────────

fn extract_all_quoted(s: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut rest = s;

    while let Some(start) = rest.find('"') {
        let after_quote = &rest[start + 1..];
        if let Some(end) = after_quote.find('"') {
            let value = &after_quote[..end];
            if !value.is_empty() {
                results.push(value.to_string());
            }
            rest = &after_quote[end + 1..];
        } else {
            break;
        }
    }

    results
}

fn is_stream_name(s: &str) -> bool {
    s.len() >= 3
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        && s.contains('_')
        && s.chars().next().map_or(false, |c| c.is_ascii_uppercase())
}

fn is_nats_subject(s: &str) -> bool {
    if s.is_empty() || s.len() < 3 {
        return false;
    }
    let segments: Vec<&str> = s.split('.').collect();
    if segments.len() < 2 {
        return false;
    }
    segments.iter().all(|seg| {
        !seg.is_empty()
            && seg
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '>' || c == '*')
    })
}

fn find_subjects_near(lines: &[&str], center: usize, radius: usize) -> Vec<String> {
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len());

    for i in start..end {
        let trimmed = lines[i].trim();
        if trimmed.contains("Subjects") && !trimmed.starts_with("//") {
            let mut subjects = Vec::new();
            for val in extract_all_quoted(trimmed) {
                if is_nats_subject(&val) {
                    subjects.push(val);
                }
            }
            for j in (i + 1)..((i + 5).min(lines.len())) {
                for val in extract_all_quoted(lines[j]) {
                    if is_nats_subject(&val) {
                        subjects.push(val);
                    }
                }
                if lines[j].trim().contains('}') || lines[j].trim().contains(']') {
                    break;
                }
            }
            if !subjects.is_empty() {
                return subjects;
            }
        }
    }

    Vec::new()
}

fn find_stream_name_near(lines: &[&str], center: usize, radius: usize) -> Option<String> {
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len());

    for i in start..end {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for val in extract_all_quoted(trimmed) {
            if is_stream_name(&val) {
                return Some(val);
            }
        }
    }

    None
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape_next = false;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if escape_next {
            result.push(chars[i]);
            escape_next = false;
            i += 1;
            continue;
        }
        if chars[i] == '\\' && in_string {
            result.push(chars[i]);
            escape_next = true;
            i += 1;
            continue;
        }
        if chars[i] == '"' {
            in_string = !in_string;
            result.push(chars[i]);
            i += 1;
            continue;
        }
        if !in_string && i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_subject_prefix_from_const() {
        let content = r#"
const subjectPrefix = "dataplane.ingestion.received"
"#;
        let mut src = RuntimeBindingSource::default();
        extract_subject_prefix(content, &mut src);
        assert_eq!(src.subject_prefix, "dataplane.ingestion.received");
    }

    #[test]
    fn extract_subject_prefix_and_pattern() {
        let content = r#"
    SubjectPrefix:    subjectPrefix,
    SubjectPattern:   subjectPrefix + ".>",
    SubjectPrefix:  "dataplane.ingestion.received",
    SubjectPattern: "dataplane.ingestion.received.>",
"#;
        let mut src = RuntimeBindingSource::default();
        extract_subject_prefix(content, &mut src);
        assert_eq!(src.subject_prefix, "dataplane.ingestion.received");
        assert_eq!(src.subject_pattern, "dataplane.ingestion.received.>");
    }

    #[test]
    fn extract_streams_from_source() {
        let content = r#"
func DefaultDataPlaneRegistry() DataPlaneRegistry {
    return DataPlaneRegistry{
        Ingested: DataPlaneEventSpec{
            Stream: StreamSpec{
                Name:     "DATA_PLANE_INGESTION",
                Subjects: []string{"dataplane.ingestion.received.>"},
            },
        },
    }
}
"#;
        let mut src = RuntimeBindingSource::default();
        extract_streams(content, &mut src);
        assert!(src.stream_subjects.contains_key("DATA_PLANE_INGESTION"));
    }

    #[test]
    fn extract_durables_from_source() {
        let content = r#"
    ValidatorIngested: ConsumerSpec{
        Durable: "validator-dataplane-v1",
        Event: EventSpec{
            Stream: StreamSpec{
                Name: "DATA_PLANE_INGESTION",
            },
        },
    },
"#;
        let mut src = RuntimeBindingSource::default();
        extract_durables(content, &mut src);
        assert_eq!(
            src.durable_consumers.get("validator-dataplane-v1"),
            Some(&"DATA_PLANE_INGESTION".to_string())
        );
    }

    #[test]
    fn extract_lifecycle_events_from_source() {
        let content = r#"
    EventActivated               events.Name = "config.activated"
    EventDeactivated             events.Name = "config.deactivated"
    EventIngestionRuntimeChanged events.Name = "config.ingestion_runtime_changed"
"#;
        let mut src = RuntimeBindingSource::default();
        extract_lifecycle_events(content, &mut src);
        assert!(src.lifecycle_events.contains("config.activated"));
        assert!(src.lifecycle_events.contains("config.deactivated"));
        assert!(src
            .lifecycle_events
            .contains("config.ingestion_runtime_changed"));
    }

    #[test]
    fn extract_lifecycle_events_skips_comments() {
        let content = r#"
    // EventActivated events.Name = "config.activated"
    EventDeactivated events.Name = "config.deactivated"
"#;
        let mut src = RuntimeBindingSource::default();
        extract_lifecycle_events(content, &mut src);
        assert!(!src.lifecycle_events.contains("config.activated"));
        assert!(src.lifecycle_events.contains("config.deactivated"));
    }

    #[test]
    fn scan_bootstrap_scopes_from_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("consumer.jsonc"),
            r#"{
  // consumer config
  "bootstrap": {
    "base_url": "http://server:8080",
    "scope_kind": "global",
    "scope_key": "default"
  }
}"#,
        )
        .unwrap();

        let mut src = RuntimeBindingSource::default();
        scan_bootstrap_scopes(dir.path(), &mut src).unwrap();
        assert_eq!(
            src.bootstrap_scopes,
            vec![("global".into(), "default".into())]
        );
    }

    #[test]
    fn scan_runtime_bindings_on_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("internal")).unwrap();
        let result = scan_runtime_bindings(&dir.path().join("internal")).unwrap();
        assert!(result.subject_prefix.is_empty());
        assert!(!result.has_bootstrap_client);
    }

    #[test]
    fn scan_runtime_bindings_with_go_files() {
        let dir = tempfile::tempdir().unwrap();
        let internal = dir.path().join("internal");
        let dataplane = internal.join("application/dataplane");
        let bootstrap = internal.join("application/runtimebootstrap");
        let validator = internal.join("actors/scopes/validator");
        std::fs::create_dir_all(&dataplane).unwrap();
        std::fs::create_dir_all(&bootstrap).unwrap();
        std::fs::create_dir_all(&validator).unwrap();

        // Create registry.go
        std::fs::write(
            dataplane.join("registry.go"),
            r#"package dataplane
const subjectPrefix = "dataplane.ingestion.received"
func DefaultRegistry() Registry {
    return Registry{
        JetStream: JetStreamRegistry{
            Ingested: IngestedRoute{
                Stream: "DATA_PLANE_INGESTION",
                SubjectPrefix: subjectPrefix,
                SubjectPattern: "dataplane.ingestion.received.>",
            },
        },
    }
}
"#,
        )
        .unwrap();

        // Create topology.go
        std::fs::write(dataplane.join("topology.go"), "package dataplane\n").unwrap();

        // Create client.go
        std::fs::write(bootstrap.join("client.go"), "package runtimebootstrap\n").unwrap();

        // Create validator files
        std::fs::write(validator.join("runtime_cache.go"), "package validator\n").unwrap();
        std::fs::write(
            validator.join("validation_worker.go"),
            "package validator\n",
        )
        .unwrap();

        let result = scan_runtime_bindings(&internal).unwrap();
        assert_eq!(result.subject_prefix, "dataplane.ingestion.received");
        assert!(result.has_bootstrap_client);
        assert!(result.has_topology_builder);
        assert!(result.has_runtime_cache);
        assert!(result.has_validation_worker);
    }
}
