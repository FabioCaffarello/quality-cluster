use std::path::Path;

use crate::error::Result;

/// A validated field requirement from the DataPlane Message.Validate() method.
#[derive(Debug, Clone)]
pub struct ValidatedField {
    pub path: String, // e.g., "binding.name", "metadata.message_id"
    #[allow(dead_code)]
    pub condition: String, // "not_empty", "not_zero", "valid_json"
}

/// DataPlane contract derived from contracts.go.
#[derive(Debug)]
pub struct DataPlaneContract {
    pub message_fields: Vec<String>, // top-level fields of Message struct
    pub validated_fields: Vec<ValidatedField>,
    pub default_content_type: Option<String>,
    pub default_source: Option<String>,
    pub message_id_format: Option<String>,
    pub file: String,
}

/// Scan the dataplane contracts.go file.
pub fn scan_dataplane(internal_dir: &Path) -> Result<Option<DataPlaneContract>> {
    let contracts_file = internal_dir.join("application/dataplane/contracts.go");
    if !contracts_file.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&contracts_file)?;
    let rel = "internal/application/dataplane/contracts.go".to_string();

    let message_fields = extract_message_fields(&content);
    let validated_fields = extract_validated_fields(&content);
    let default_content_type = extract_const_value(&content, "ContentTypeJSON");
    let default_source = extract_const_value(&content, "SourceKafka");
    let message_id_format = extract_message_id_format(&content);

    Ok(Some(DataPlaneContract {
        message_fields,
        validated_fields,
        default_content_type,
        default_source,
        message_id_format,
        file: rel,
    }))
}

fn extract_message_fields(source: &str) -> Vec<String> {
    let mut fields = Vec::new();

    // Find "type Message struct {"
    let marker = "type Message struct {";
    if let Some(start) = source.find(marker) {
        let brace_start = start + marker.len();
        if let Some(end) = find_closing_brace(source, brace_start) {
            let body = &source[brace_start..end];
            for line in body.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }
                // Extract json tag
                if let Some(tag) = extract_json_tag(trimmed) {
                    fields.push(tag);
                }
            }
        }
    }

    fields
}

fn extract_validated_fields(source: &str) -> Vec<ValidatedField> {
    let mut fields = Vec::new();

    // Find the Validate() method on Message
    let validate_marker = "func (m Message) Validate()";
    if let Some(start) = source.find(validate_marker) {
        if let Some(brace) = source[start..].find('{') {
            let body_start = start + brace + 1;
            if let Some(end) = find_closing_brace(source, body_start) {
                let body = &source[body_start..end];

                // Collect lines for context-window lookups
                let lines: Vec<&str> = body.lines().collect();
                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();

                    // Look for Field: "xxx" in ValidationIssue blocks
                    if let Some(field_pos) = trimmed.find("Field:") {
                        let after_field = &trimmed[field_pos + "Field:".len()..];
                        if let Some(tag_start) = after_field.find('"') {
                            if let Some(tag_end) = after_field[tag_start + 1..].find('"') {
                                let path =
                                    after_field[tag_start + 1..tag_start + 1 + tag_end].to_string();

                                // Check the same line and adjacent lines for Message context
                                let window: String = lines
                                    [idx.saturating_sub(2)..std::cmp::min(idx + 3, lines.len())]
                                    .join(" ");

                                let condition = if body.contains("json.Valid(m.Payload)")
                                    && path == "payload"
                                {
                                    "valid_json".to_string()
                                } else if window.contains("must not be zero")
                                    || window.contains("IsZero")
                                {
                                    "not_zero".to_string()
                                } else {
                                    "not_empty".to_string()
                                };

                                fields.push(ValidatedField { path, condition });
                            }
                        }
                    }
                }
            }
        }
    }

    fields
}

fn extract_const_value(source: &str, name: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains(name) && trimmed.contains("= \"") {
            if let Some(start) = trimmed.find("= \"") {
                let after = &trimmed[start + 3..];
                if let Some(end) = after.find('"') {
                    return Some(after[..end].to_string());
                }
            }
        }
    }
    None
}

fn extract_message_id_format(source: &str) -> Option<String> {
    // Look for the MessageIDForKafkaRecord function and its format string
    if let Some(pos) = source.find("MessageIDForKafkaRecord") {
        // Find the Sprintf format string
        if let Some(fmt_pos) = source[pos..].find("Sprintf(") {
            let after = &source[pos + fmt_pos..];
            if let Some(start) = after.find('"') {
                if let Some(end) = after[start + 1..].find('"') {
                    return Some(after[start + 1..start + 1 + end].to_string());
                }
            }
        }
    }
    None
}

fn extract_json_tag(line: &str) -> Option<String> {
    if let Some(start) = line.find("`json:\"") {
        let after = &line[start + "`json:\"".len()..];
        if let Some(end) = after.find('"') {
            let tag = &after[..end];
            return Some(tag.split(',').next().unwrap_or(tag).to_string());
        }
    }
    None
}

fn find_closing_brace(source: &str, start: usize) -> Option<usize> {
    let mut depth = 1;
    let bytes = source.as_bytes();
    let mut in_string = false;
    let mut i = start;

    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'`' {
            in_string = !in_string;
        } else if !in_string {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONTRACTS: &str = r#"
package dataplane

const (
    SourceKafka     = "kafka"
    ContentTypeJSON = "application/json"
)

type Message struct {
    Binding  BindingRecord   `json:"binding"`
    Origin   OriginRecord    `json:"origin"`
    Payload  json.RawMessage `json:"payload"`
    Metadata MetadataRecord  `json:"metadata"`
}

func (m Message) Validate() *problem.Problem {
    var issues []problem.ValidationIssue

    if strings.TrimSpace(m.Binding.Name) == "" {
        issues = append(issues, problem.ValidationIssue{Field: "binding.name", Message: "must not be empty"})
    }
    if strings.TrimSpace(m.Binding.Topic) == "" {
        issues = append(issues, problem.ValidationIssue{Field: "binding.topic", Message: "must not be empty"})
    }
    if strings.TrimSpace(m.Origin.Source) == "" {
        issues = append(issues, problem.ValidationIssue{Field: "origin.source", Message: "must not be empty"})
    }
    if strings.TrimSpace(m.Origin.Topic) == "" {
        issues = append(issues, problem.ValidationIssue{Field: "origin.topic", Message: "must not be empty"})
    }
    if strings.TrimSpace(m.Metadata.MessageID) == "" {
        issues = append(issues, problem.ValidationIssue{Field: "metadata.message_id", Message: "must not be empty"})
    }
    if m.Metadata.IngestedAt.IsZero() {
        issues = append(issues, problem.ValidationIssue{Field: "metadata.ingested_at", Message: "must not be zero"})
    }
    if strings.TrimSpace(m.Metadata.ContentType) == "" {
        issues = append(issues, problem.ValidationIssue{Field: "metadata.content_type", Message: "must not be empty"})
    }
    if len(m.Payload) == 0 {
        issues = append(issues, problem.ValidationIssue{Field: "payload", Message: "must not be empty"})
    } else if !json.Valid(m.Payload) {
        issues = append(issues, problem.ValidationIssue{Field: "payload", Message: "must be valid JSON"})
    }
    return nil
}

func MessageIDForKafkaRecord(binding configctlcontracts.ActiveIngestionBindingRecord, topic string, partition int, offset int64) string {
    return fmt.Sprintf(
        "%s:%s:%d:%d:%s:%s:%s",
        SourceKafka,
        topic,
        partition,
        offset,
        scopeKind+":"+scopeKey,
        versionID,
        bindingName,
    )
}
"#;

    #[test]
    fn extracts_message_fields() {
        let fields = extract_message_fields(SAMPLE_CONTRACTS);
        assert_eq!(fields, vec!["binding", "origin", "payload", "metadata"]);
    }

    #[test]
    fn extracts_validated_fields() {
        let fields = extract_validated_fields(SAMPLE_CONTRACTS);
        assert!(fields.len() >= 7);

        let binding_name = fields.iter().find(|f| f.path == "binding.name").unwrap();
        assert_eq!(binding_name.condition, "not_empty");

        let ingested_at = fields
            .iter()
            .find(|f| f.path == "metadata.ingested_at")
            .unwrap();
        assert_eq!(ingested_at.condition, "not_zero");

        assert!(fields.iter().any(|f| f.path == "payload"));
    }

    #[test]
    fn extracts_constants() {
        assert_eq!(
            extract_const_value(SAMPLE_CONTRACTS, "ContentTypeJSON"),
            Some("application/json".into())
        );
        assert_eq!(
            extract_const_value(SAMPLE_CONTRACTS, "SourceKafka"),
            Some("kafka".into())
        );
    }

    #[test]
    fn extracts_message_id_format() {
        let fmt = extract_message_id_format(SAMPLE_CONTRACTS);
        assert!(fmt.is_some());
        assert!(fmt.unwrap().contains("%s:%s:%d:%d"));
    }
}
