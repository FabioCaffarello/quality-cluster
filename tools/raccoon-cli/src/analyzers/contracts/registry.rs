use std::path::Path;

use crate::error::Result;

/// A control (request/reply) specification extracted from a registry Go file.
#[derive(Debug, Clone)]
pub struct ControlSpec {
    pub name: String,
    pub subject: String,
    pub request_type: String,
    pub reply_type: String,
    pub queue_group: String,
    pub file: String,
}

/// An event specification extracted from a registry Go file.
#[derive(Debug, Clone)]
pub struct EventSpecRecord {
    pub name: String,
    pub subject: String,
    pub event_type: String,
    #[allow(dead_code)]
    pub stream_name: Option<String>,
    pub file: String,
}

/// A JetStream stream specification.
#[derive(Debug, Clone)]
pub struct StreamSpecRecord {
    pub name: String,
    pub subjects: Vec<String>,
    #[allow(dead_code)]
    pub file: String,
}

/// A durable consumer specification.
#[derive(Debug, Clone)]
pub struct ConsumerSpecRecord {
    pub durable: String,
    pub stream_name: String,
    pub filter_subjects: Vec<String>,
    pub file: String,
}

/// All registry artifacts discovered from source.
#[derive(Debug, Default)]
pub struct RegistryIndex {
    pub control_specs: Vec<ControlSpec>,
    pub event_specs: Vec<EventSpecRecord>,
    pub streams: Vec<StreamSpecRecord>,
    pub consumers: Vec<ConsumerSpecRecord>,
}

/// Scan all `*_registry.go` files under `internal/` for registry specifications.
pub fn scan_registries(internal_dir: &Path) -> Result<RegistryIndex> {
    let mut index = RegistryIndex::default();

    let registry_files = find_registry_files(internal_dir)?;
    for path in &registry_files {
        let content = std::fs::read_to_string(path)?;
        let rel = path
            .strip_prefix(internal_dir.parent().unwrap_or(internal_dir))
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        extract_control_specs(&content, &rel, &mut index.control_specs);
        extract_event_specs(&content, &rel, &mut index.event_specs);
        extract_stream_specs(&content, &rel, &mut index.streams);
        extract_consumer_specs(&content, &rel, &mut index.consumers);
    }

    // Also scan the dataplane registry.go for stream/subject constants
    let dp_registry = internal_dir.join("application/dataplane/registry.go");
    if dp_registry.is_file() {
        let content = std::fs::read_to_string(&dp_registry)?;
        let rel = dp_registry
            .strip_prefix(internal_dir.parent().unwrap_or(internal_dir))
            .unwrap_or(&dp_registry)
            .to_string_lossy()
            .to_string();
        extract_dataplane_registry(&content, &rel, &mut index);
    }

    Ok(index)
}

fn find_registry_files(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    collect_files(dir, "_registry.go", &mut files)?;
    Ok(files)
}

fn collect_files(dir: &Path, suffix: &str, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, suffix, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |n| n.ends_with(suffix))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// Extract ControlSpec blocks from Go source.
/// Looks for patterns like:
/// ```
/// SomeName: ControlSpec{
///     Subject:     "x.control.y",
///     RequestType: "x.command.y",
///     ReplyType:   "x.reply.y",
///     QueueGroup:  "x.control",
/// },
/// ```
fn extract_control_specs(source: &str, file: &str, out: &mut Vec<ControlSpec>) {
    // Find ControlSpec blocks
    let mut i = 0;
    while i < source.len() {
        // Look for "ControlSpec{" pattern
        if let Some(pos) = source[i..].find("ControlSpec{") {
            let abs_pos = i + pos;

            // Find the name before ControlSpec - look backward for identifier
            let name = extract_field_name(&source[..abs_pos]);

            // Find the closing brace for this block
            if let Some(block_end) = find_closing_brace(source, abs_pos + "ControlSpec{".len()) {
                let block = &source[abs_pos..block_end + 1];

                let subject = extract_string_field(block, "Subject");
                let request_type = extract_string_field(block, "RequestType");
                let reply_type = extract_string_field(block, "ReplyType");
                let queue_group = extract_string_field(block, "QueueGroup");

                if let (Some(subj), Some(req), Some(rep)) = (subject, request_type, reply_type) {
                    out.push(ControlSpec {
                        name: name.unwrap_or_else(|| "unknown".into()),
                        subject: subj,
                        request_type: req,
                        reply_type: rep,
                        queue_group: queue_group.unwrap_or_default(),
                        file: file.to_string(),
                    });
                }

                i = block_end + 1;
            } else {
                i = abs_pos + 1;
            }
        } else {
            break;
        }
    }
}

/// Extract EventSpec blocks from Go source.
fn extract_event_specs(source: &str, file: &str, out: &mut Vec<EventSpecRecord>) {
    let mut i = 0;

    while i < source.len() {
        if let Some(pos) = source[i..].find("EventSpec{") {
            let abs_pos = i + pos;
            let name = extract_field_name(&source[..abs_pos]);

            if let Some(block_end) = find_closing_brace(source, abs_pos + "EventSpec{".len()) {
                let block = &source[abs_pos..block_end + 1];

                let subject = extract_string_field(block, "Subject");
                let event_type = extract_string_field(block, "Type");

                // Check if there's a Stream reference
                let stream_name = if block.contains("Stream:") || block.contains("Stream ") {
                    extract_string_field(block, "Name")
                } else {
                    None
                };

                if let (Some(subj), Some(typ)) = (subject, event_type) {
                    out.push(EventSpecRecord {
                        name: name.unwrap_or_else(|| "unknown".into()),
                        subject: subj,
                        event_type: typ,
                        stream_name,
                        file: file.to_string(),
                    });
                }

                i = block_end + 1;
            } else {
                i = abs_pos + 1;
            }
        } else {
            break;
        }
    }
}

/// Extract StreamSpec blocks.
fn extract_stream_specs(source: &str, file: &str, out: &mut Vec<StreamSpecRecord>) {
    let mut i = 0;

    while i < source.len() {
        if let Some(pos) = source[i..].find("StreamSpec{") {
            let abs_pos = i + pos;

            if let Some(block_end) = find_closing_brace(source, abs_pos + "StreamSpec{".len()) {
                let block = &source[abs_pos..block_end + 1];

                let name = extract_string_field(block, "Name");
                let subjects = extract_string_array(block, "Subjects");

                if let Some(n) = name {
                    // Avoid duplicates
                    if !out.iter().any(|s| s.name == n) {
                        out.push(StreamSpecRecord {
                            name: n,
                            subjects,
                            file: file.to_string(),
                        });
                    }
                }

                i = block_end + 1;
            } else {
                i = abs_pos + 1;
            }
        } else {
            break;
        }
    }
}

/// Extract ConsumerSpec blocks.
fn extract_consumer_specs(source: &str, file: &str, out: &mut Vec<ConsumerSpecRecord>) {
    let mut i = 0;

    while i < source.len() {
        if let Some(pos) = source[i..].find("ConsumerSpec{") {
            let abs_pos = i + pos;
            let _name = extract_field_name(&source[..abs_pos]);

            if let Some(block_end) = find_closing_brace(source, abs_pos + "ConsumerSpec{".len()) {
                let block = &source[abs_pos..block_end + 1];

                let durable = extract_string_field(block, "Durable");

                // Find stream name from nested EventSpec or Stream
                let stream_name = find_nested_stream_name(block);

                // Find filter subjects: either from nested EventSpec Subject or FilterSubjects
                let mut filter_subjects = extract_string_array(block, "FilterSubjects");
                if filter_subjects.is_empty() {
                    // Fallback: get the Event's Subject as filter
                    if let Some(subj) = extract_nested_event_subject(block) {
                        filter_subjects.push(subj);
                    }
                }

                if let Some(d) = durable {
                    out.push(ConsumerSpecRecord {
                        durable: d,
                        stream_name: stream_name.unwrap_or_default(),
                        filter_subjects,
                        file: file.to_string(),
                    });
                }

                i = block_end + 1;
            } else {
                i = abs_pos + 1;
            }
        } else {
            break;
        }
    }
}

/// Extract dataplane registry constants from registry.go.
/// Handles Go patterns where fields reference local consts (e.g., `SubjectPrefix: subjectPrefix`).
fn extract_dataplane_registry(source: &str, file: &str, index: &mut RegistryIndex) {
    // First resolve const values in the source
    let const_prefix = extract_const_string(source, "subjectPrefix");

    // Look for IngestedRoute struct
    if let Some(pos) = source.find("IngestedRoute{") {
        if let Some(block_end) = find_closing_brace(source, pos + "IngestedRoute{".len()) {
            let block = &source[pos..block_end + 1];

            let stream = extract_string_field(block, "Stream");
            let event_type = extract_string_field(block, "EventType");
            let _validator_durable = extract_string_field(block, "ValidatorDurable");

            // SubjectPrefix and SubjectPattern may reference the const, not be string literals
            let subject_prefix =
                extract_string_field(block, "SubjectPrefix").or_else(|| const_prefix.clone());

            // SubjectPattern is often `subjectPrefix + ".>"` — resolve it
            let subject_pattern = extract_string_field(block, "SubjectPattern")
                .or_else(|| subject_prefix.as_ref().map(|p| format!("{}.>", p)));

            let resolved_stream = stream.unwrap_or_else(|| "DATA_PLANE_INGESTION".into());
            let resolved_pattern = subject_pattern.unwrap_or_default();

            if !resolved_pattern.is_empty() {
                if !index.streams.iter().any(|s| s.name == resolved_stream) {
                    index.streams.push(StreamSpecRecord {
                        name: resolved_stream.clone(),
                        subjects: vec![resolved_pattern.clone()],
                        file: file.to_string(),
                    });
                }
            }

            if let Some(et) = event_type {
                if !resolved_pattern.is_empty() {
                    index.event_specs.push(EventSpecRecord {
                        name: "DataPlaneIngested".into(),
                        subject: resolved_pattern,
                        event_type: et,
                        stream_name: Some(resolved_stream),
                        file: file.to_string(),
                    });
                }
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Extract the field/variable name preceding a struct literal.
fn extract_field_name(before: &str) -> Option<String> {
    let trimmed = before.trim_end();
    // Skip colon/equals separators
    let trimmed = trimmed.trim_end_matches(|c: char| c == ':' || c == '=' || c.is_whitespace());

    // Take the last word (identifier)
    let start = trimmed
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|p| p + 1)
        .unwrap_or(0);

    let name = &trimmed[start..];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Extract a string value from a Go field assignment like `FieldName: "value"`.
fn extract_string_field(block: &str, field: &str) -> Option<String> {
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(field) {
            // Handle patterns like `Field:  "value"` or `Field: "value",`
            if let Some(start) = trimmed.find('"') {
                if let Some(end) = trimmed[start + 1..].find('"') {
                    return Some(trimmed[start + 1..start + 1 + end].to_string());
                }
            }
        }
    }
    None
}

/// Extract a Go string const like `const name = "value"`.
fn extract_const_string(source: &str, name: &str) -> Option<String> {
    let pattern = format!("{} = \"", name);
    if let Some(pos) = source.find(&pattern) {
        let start = pos + pattern.len();
        if let Some(end) = source[start..].find('"') {
            return Some(source[start..start + end].to_string());
        }
    }

    let pattern2 = format!("{}  = \"", name);
    if let Some(pos) = source.find(&pattern2) {
        let start = pos + pattern2.len();
        if let Some(end) = source[start..].find('"') {
            return Some(source[start..start + end].to_string());
        }
    }

    None
}

/// Extract a string array like `[]string{"a", "b"}` from a field.
fn extract_string_array(block: &str, field: &str) -> Vec<String> {
    let mut results = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(field) || trimmed.contains(&format!("{field}:")) {
            // Find all quoted strings after the field
            let search = if let Some(pos) = trimmed.find('{') {
                &trimmed[pos..]
            } else {
                trimmed
            };

            let mut i = 0;
            let bytes = search.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    if let Some(end) = search[i + 1..].find('"') {
                        results.push(search[i + 1..i + 1 + end].to_string());
                        i = i + 1 + end + 1;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }

    results
}

/// Find closing brace, handling nesting.
fn find_closing_brace(source: &str, start: usize) -> Option<usize> {
    let mut depth = 1;
    let bytes = source.as_bytes();
    let mut in_string = false;
    let mut escape = false;

    let mut i = start;
    while i < bytes.len() {
        let c = bytes[i];
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if c == b'\\' && in_string {
            escape = true;
            i += 1;
            continue;
        }
        if c == b'"' {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string {
            if c == b'{' {
                depth += 1;
            } else if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

/// Find stream name in a nested EventSpec or Stream block.
fn find_nested_stream_name(block: &str) -> Option<String> {
    // Look for StreamSpec{ or Stream: StreamSpec{ within the block
    if let Some(pos) = block.find("StreamSpec{") {
        if let Some(end) = find_closing_brace(block, pos + "StreamSpec{".len()) {
            let inner = &block[pos..end + 1];
            return extract_string_field(inner, "Name");
        }
    }
    // Try eventStream reference
    if block.contains("eventStream") {
        // The stream name comes from the shared eventStream variable - we can't resolve it here
        // but the caller can cross-reference
        return None;
    }
    None
}

/// Find the Subject from a nested EventSpec.
fn extract_nested_event_subject(block: &str) -> Option<String> {
    if let Some(pos) = block.find("EventSpec{") {
        if let Some(end) = find_closing_brace(block, pos + "EventSpec{".len()) {
            let inner = &block[pos..end + 1];
            return extract_string_field(inner, "Subject");
        }
    }
    // Fallback: look for Event: EventSpec or Event.Subject
    extract_string_field(block, "Subject")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_REGISTRY: &str = r#"
func DefaultConfigctlRegistry() ConfigctlRegistry {
    eventStream := StreamSpec{
        Name:     "CONFIGCTL_EVENTS",
        Subjects: []string{"configctl.events.config.>"},
        Storage:  jetstream.FileStorage,
        MaxAge:   24 * time.Hour,
        MaxBytes: 256 * 1024 * 1024,
    }

    return ConfigctlRegistry{
        CreateDraft: ControlSpec{
            Subject:     "configctl.control.create_draft",
            RequestType: "configctl.command.create_draft",
            ReplyType:   "configctl.reply.create_draft",
            QueueGroup:  "configctl.control",
        },
        GetConfig: ControlSpec{
            Subject:     "configctl.control.get_config",
            RequestType: "configctl.query.get_config",
            ReplyType:   "configctl.reply.get_config",
            QueueGroup:  "configctl.control",
        },
        DraftCreated: EventSpec{
            Subject: "configctl.events.config.draft_created",
            Type:    "configctl.event.config.draft_created",
            Stream:  eventStream,
        },
        ValidatorRuntime: ConsumerSpec{
            Durable: "validator-runtime-cache-v1",
            Event: EventSpec{
                Subject: "configctl.events.config.activated",
                Type:    "configctl.event.config.activated",
                Stream:  eventStream,
            },
            AckWait:    30 * time.Second,
            MaxDeliver: 10,
        },
    }
}
"#;

    #[test]
    fn extracts_control_specs() {
        let mut specs = Vec::new();
        extract_control_specs(SAMPLE_REGISTRY, "test.go", &mut specs);
        assert_eq!(specs.len(), 2);

        let create = specs.iter().find(|s| s.name == "CreateDraft").unwrap();
        assert_eq!(create.subject, "configctl.control.create_draft");
        assert_eq!(create.request_type, "configctl.command.create_draft");
        assert_eq!(create.reply_type, "configctl.reply.create_draft");
        assert_eq!(create.queue_group, "configctl.control");

        let get = specs.iter().find(|s| s.name == "GetConfig").unwrap();
        assert_eq!(get.request_type, "configctl.query.get_config");
    }

    #[test]
    fn extracts_event_specs() {
        let mut specs = Vec::new();
        extract_event_specs(SAMPLE_REGISTRY, "test.go", &mut specs);
        // Should find DraftCreated and the nested one in ConsumerSpec
        assert!(specs
            .iter()
            .any(|s| s.subject == "configctl.events.config.draft_created"));
        assert!(specs
            .iter()
            .any(|s| s.event_type == "configctl.event.config.draft_created"));
    }

    #[test]
    fn extracts_stream_specs() {
        let mut specs = Vec::new();
        extract_stream_specs(SAMPLE_REGISTRY, "test.go", &mut specs);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "CONFIGCTL_EVENTS");
        assert_eq!(specs[0].subjects, vec!["configctl.events.config.>"]);
    }

    #[test]
    fn extracts_consumer_specs() {
        let mut specs = Vec::new();
        extract_consumer_specs(SAMPLE_REGISTRY, "test.go", &mut specs);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].durable, "validator-runtime-cache-v1");
        assert!(specs[0]
            .filter_subjects
            .contains(&"configctl.events.config.activated".to_string()));
    }

    #[test]
    fn extract_string_field_works() {
        let block = r#"
            Subject:     "foo.bar.baz",
            RequestType: "foo.command.baz",
        "#;
        assert_eq!(
            extract_string_field(block, "Subject"),
            Some("foo.bar.baz".into())
        );
        assert_eq!(
            extract_string_field(block, "RequestType"),
            Some("foo.command.baz".into())
        );
        assert_eq!(extract_string_field(block, "Missing"), None);
    }

    #[test]
    fn extract_field_name_works() {
        assert_eq!(
            extract_field_name("        CreateDraft: "),
            Some("CreateDraft".into())
        );
        assert_eq!(
            extract_field_name("    GetConfig: "),
            Some("GetConfig".into())
        );
    }

    #[test]
    fn find_closing_brace_handles_nesting() {
        let s = "{ a { b } c }";
        assert_eq!(find_closing_brace(s, 1), Some(s.len() - 1));
    }

    #[test]
    fn find_closing_brace_handles_strings() {
        let s = r#"{ a "}" b }"#;
        assert_eq!(find_closing_brace(s, 1), Some(s.len() - 1));
    }

    #[test]
    fn extract_string_array_works() {
        let block = r#"Subjects: []string{"foo.>", "bar.>"}"#;
        let result = extract_string_array(block, "Subjects");
        assert_eq!(result, vec!["foo.>", "bar.>"]);
    }

    const VALIDATOR_RESULTS_REGISTRY: &str = r#"
func DefaultValidatorResultsRegistry() ValidatorResultsRegistry {
    return ValidatorResultsRegistry{
        List: ControlSpec{
            Subject:     "validator.results.list",
            RequestType: "validator.results.query.list",
            ReplyType:   "validator.results.reply.list",
            QueueGroup:  "validator.results",
        },
    }
}
"#;

    #[test]
    fn extracts_validator_results_control_spec() {
        let mut specs = Vec::new();
        extract_control_specs(VALIDATOR_RESULTS_REGISTRY, "test.go", &mut specs);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "List");
        assert_eq!(specs[0].subject, "validator.results.list");
    }
}
