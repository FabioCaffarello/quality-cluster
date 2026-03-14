use std::path::Path;

use crate::error::Result;

/// An envelope field discovered from the Go source.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EnvelopeField {
    pub name: String,
    pub json_tag: String,
    pub required: bool, // true if validated as non-empty in Validate()
}

/// Envelope contract extracted from envelope.go.
#[derive(Debug)]
pub struct EnvelopeContract {
    #[allow(dead_code)]
    pub fields: Vec<EnvelopeField>,
    pub valid_kinds: Vec<String>,
    pub default_content_type: Option<String>,
    pub required_fields: Vec<String>,
    pub file: String,
}

/// Codec usage extracted from codec.go.
#[derive(Debug)]
pub struct CodecUsage {
    pub encode_kind_checks: Vec<KindCheck>,
    pub decode_kind_checks: Vec<KindCheck>,
    pub serialization_format: String, // "cbor" or "json"
    pub file: String,
}

#[derive(Debug, Clone)]
pub struct KindCheck {
    pub function: String,
    pub expected_kind: String,
    #[allow(dead_code)]
    pub expected_type_field: String, // "RequestType", "ReplyType", "Type"
}

/// Scan envelope.go for the envelope contract.
pub fn scan_envelope(internal_dir: &Path) -> Result<Option<EnvelopeContract>> {
    let envelope_file = internal_dir.join("shared/envelope/envelope.go");
    if !envelope_file.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&envelope_file)?;
    let rel = format!("internal/shared/envelope/envelope.go");

    let fields = extract_envelope_fields(&content);
    let valid_kinds = extract_valid_kinds(&content);
    let default_content_type = extract_default_content_type(&content);
    let required_fields = extract_required_fields(&content);

    Ok(Some(EnvelopeContract {
        fields,
        valid_kinds,
        default_content_type,
        required_fields,
        file: rel,
    }))
}

/// Scan codec.go for codec/serialization patterns.
pub fn scan_codec(internal_dir: &Path) -> Result<Option<CodecUsage>> {
    let codec_file = internal_dir.join("adapters/nats/codec.go");
    if !codec_file.is_file() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&codec_file)?;
    let rel = "internal/adapters/nats/codec.go".to_string();

    let mut encode_checks = Vec::new();
    let mut decode_checks = Vec::new();
    let serialization = if content.contains("cbor.Marshal") || content.contains("cbor.Unmarshal") {
        "cbor".to_string()
    } else if content.contains("json.Marshal") || content.contains("json.Unmarshal") {
        "json".to_string()
    } else {
        "unknown".to_string()
    };

    // Extract kind checks from encode/decode functions
    extract_kind_checks(&content, &mut encode_checks, &mut decode_checks);

    Ok(Some(CodecUsage {
        encode_kind_checks: encode_checks,
        decode_kind_checks: decode_checks,
        serialization_format: serialization,
        file: rel,
    }))
}

fn extract_envelope_fields(source: &str) -> Vec<EnvelopeField> {
    let mut fields = Vec::new();

    // Find the Envelope struct definition
    let struct_start = match source.find("struct {") {
        Some(pos) => {
            // Make sure this is the Envelope struct
            let before = &source[..pos];
            if !before.contains("Envelope[T any]") && !before.contains("Envelope[") {
                // Try to find the right one
                if let Some(pos2) = source.find("Envelope[T any] struct {") {
                    pos2 + "Envelope[T any] ".len()
                } else {
                    return fields;
                }
            } else {
                pos
            }
        }
        None => return fields,
    };

    if let Some(end) = find_struct_end(source, struct_start + "struct {".len()) {
        let block = &source[struct_start..end];

        for line in block.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed == "struct {" || trimmed == "}" {
                continue;
            }

            // Parse fields like: ID  string  `json:"id"`
            if let Some(field) = parse_struct_field(trimmed) {
                fields.push(field);
            }
        }
    }

    fields
}

fn parse_struct_field(line: &str) -> Option<EnvelopeField> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let name = parts[0].to_string();
    // Skip if it doesn't start with uppercase (not an exported field)
    if !name.chars().next().map_or(false, |c| c.is_uppercase()) {
        return None;
    }

    // Extract json tag
    let json_tag = if let Some(tag_start) = line.find("`json:\"") {
        let after = &line[tag_start + "`json:\"".len()..];
        if let Some(tag_end) = after.find('"') {
            let full_tag = &after[..tag_end];
            // Remove omitempty suffix for the field name
            full_tag.split(',').next().unwrap_or(full_tag).to_string()
        } else {
            name.to_lowercase()
        }
    } else {
        return None; // No json tag, skip
    };

    Some(EnvelopeField {
        name,
        json_tag,
        required: false, // Will be set later
    })
}

fn extract_valid_kinds(source: &str) -> Vec<String> {
    let mut kinds = Vec::new();

    // Look for Kind constants
    let kind_patterns = [
        ("KindCommand", "command"),
        ("KindEvent", "event"),
        ("KindRequest", "request"),
        ("KindReply", "reply"),
    ];

    for (const_name, value) in &kind_patterns {
        if source.contains(const_name) {
            kinds.push(value.to_string());
        }
    }

    kinds
}

fn extract_default_content_type(source: &str) -> Option<String> {
    // Look for DefaultContentType constant
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains("DefaultContentType") && trimmed.contains("=") {
            if let Some(start) = trimmed.find('"') {
                if let Some(end) = trimmed[start + 1..].find('"') {
                    return Some(trimmed[start + 1..start + 1 + end].to_string());
                }
            }
        }
    }
    None
}

fn extract_required_fields(source: &str) -> Vec<String> {
    let mut required = Vec::new();

    // Look for validation checks in Validate() function
    // Patterns like: if e.ID == "" or if e.Type == "" or e.Timestamp.IsZero()
    let validate_start = source.find("func (e Envelope");
    if let Some(start) = validate_start {
        // Find the Validate method
        if let Some(validate_pos) = source[start..].find("Validate()") {
            let from = start + validate_pos;
            // Get the function body
            if let Some(brace) = source[from..].find('{') {
                let body_start = from + brace + 1;
                if let Some(body_end) = find_struct_end(source, body_start) {
                    let body = &source[body_start..body_end];

                    // Extract field names from validation checks
                    for line in body.lines() {
                        let trimmed = line.trim();
                        if trimmed.contains("e.") && (trimmed.contains("== \"\"") || trimmed.contains(".IsZero()")) {
                            // Extract field name after "e."
                            if let Some(dot_pos) = trimmed.find("e.") {
                                let after = &trimmed[dot_pos + 2..];
                                let field_end = after.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(after.len());
                                let field = &after[..field_end];
                                if !field.is_empty() {
                                    required.push(field.to_string());
                                }
                            }
                        }
                    }

                    // Also look for Field: "x" in ValidationIssue blocks (may be inline or multi-line)
                    for line in body.lines() {
                        let trimmed = line.trim();
                        if let Some(field_pos) = trimmed.find("Field:") {
                            let after_field = &trimmed[field_pos + "Field:".len()..];
                            if let Some(start) = after_field.find('"') {
                                if let Some(end) = after_field[start + 1..].find('"') {
                                    required.push(after_field[start + 1..start + 1 + end].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Deduplicate
    required.sort();
    required.dedup();
    required
}

fn extract_kind_checks(source: &str, encode: &mut Vec<KindCheck>, decode: &mut Vec<KindCheck>) {
    // Parse function definitions and find kind assertions
    let functions = [
        ("encodeControlRequest", true, "KindCommand", "RequestType"),
        ("decodeControlRequest", false, "KindCommand", "RequestType"),
        ("encodeControlReply", true, "KindReply", "ReplyType"),
        ("decodeControlReply", false, "KindReply", "ReplyType"),
        ("encodeEvent", true, "KindEvent", "Type"),
        ("decodeEvent", false, "KindEvent", "Type"),
    ];

    for (func_name, is_encode, kind_const, type_field) in &functions {
        if source.contains(func_name) {
            let kind_value = match *kind_const {
                "KindCommand" => "command",
                "KindEvent" => "event",
                "KindReply" => "reply",
                "KindRequest" => "request",
                _ => "unknown",
            };

            let check = KindCheck {
                function: func_name.to_string(),
                expected_kind: kind_value.to_string(),
                expected_type_field: type_field.to_string(),
            };

            if *is_encode {
                encode.push(check);
            } else {
                decode.push(check);
            }
        }
    }
}

fn find_struct_end(source: &str, start: usize) -> Option<usize> {
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
        if c == b'"' || c == b'`' {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ENVELOPE: &str = r#"
package envelope

type Kind string

const (
    KindCommand Kind = "command"
    KindEvent   Kind = "event"
    KindRequest Kind = "request"
    KindReply   Kind = "reply"
)

const DefaultContentType = "application/json"

type Envelope[T any] struct {
    ID            string            `json:"id"`
    Kind          Kind              `json:"kind"`
    Type          string            `json:"type"`
    Source        string            `json:"source,omitempty"`
    Subject       string            `json:"subject,omitempty"`
    CorrelationID string            `json:"correlation_id,omitempty"`
    CausationID   string            `json:"causation_id,omitempty"`
    ReplyTo       string            `json:"reply_to,omitempty"`
    ContentType   string            `json:"content_type,omitempty"`
    Timestamp     time.Time         `json:"timestamp"`
    Headers       map[string]string `json:"headers,omitempty"`
    Payload       T                 `json:"payload,omitempty"`
    Problem       *problem.Problem  `json:"problem,omitempty"`
}

func (e Envelope[T]) Validate() *problem.Problem {
    if e.ID == "" {
        issues = append(issues, problem.ValidationIssue{Field: "id", Message: "must not be empty"})
    }
    if e.Type == "" {
        issues = append(issues, problem.ValidationIssue{Field: "type", Message: "must not be empty"})
    }
    if e.Timestamp.IsZero() {
        issues = append(issues, problem.ValidationIssue{Field: "timestamp", Message: "must not be zero"})
    }
    if e.ContentType == "" {
        issues = append(issues, problem.ValidationIssue{Field: "content_type", Message: "must not be empty"})
    }
}
"#;

    #[test]
    fn extracts_envelope_fields() {
        let fields = extract_envelope_fields(SAMPLE_ENVELOPE);
        assert!(fields.len() >= 10);

        let id = fields.iter().find(|f| f.name == "ID").unwrap();
        assert_eq!(id.json_tag, "id");

        let kind = fields.iter().find(|f| f.name == "Kind").unwrap();
        assert_eq!(kind.json_tag, "kind");

        let ct = fields.iter().find(|f| f.name == "ContentType").unwrap();
        assert_eq!(ct.json_tag, "content_type");
    }

    #[test]
    fn extracts_valid_kinds() {
        let kinds = extract_valid_kinds(SAMPLE_ENVELOPE);
        assert_eq!(kinds, vec!["command", "event", "request", "reply"]);
    }

    #[test]
    fn extracts_default_content_type() {
        let ct = extract_default_content_type(SAMPLE_ENVELOPE);
        assert_eq!(ct, Some("application/json".to_string()));
    }

    #[test]
    fn extracts_required_fields() {
        let required = extract_required_fields(SAMPLE_ENVELOPE);
        assert!(required.contains(&"id".to_string()));
        assert!(required.contains(&"type".to_string()));
        assert!(required.contains(&"timestamp".to_string()));
        assert!(required.contains(&"content_type".to_string()));
    }

    const SAMPLE_CODEC: &str = r#"
package nats

import "github.com/fxamacker/cbor/v2"

func encodeControlRequest[T any](ctx context.Context, spec ControlSpec, source string, payload T) ([]byte, error) {
    env := envelope.New(envelope.KindCommand, spec.RequestType, payload)
    data, err := cbor.Marshal(env)
    return data, nil
}

func decodeControlReply[T any](spec ControlSpec, data []byte) (T, error) {
    if err := cbor.Unmarshal(data, &env); err != nil { return zero, err }
    if env.Kind != envelope.KindReply { return zero, err }
    return env.Payload, nil
}

func encodeEvent[T any](spec EventSpec, source string, payload T) ([]byte, error) {
    env := envelope.New(envelope.KindEvent, spec.Type, payload)
    data, err := cbor.Marshal(env)
    return data, nil
}

func decodeEvent[T any](spec EventSpec, data []byte) (envelope.Envelope[T], error) {
    if env.Kind != envelope.KindEvent { return env, err }
    return env, nil
}
"#;

    #[test]
    fn detects_cbor_serialization() {
        let mut enc = Vec::new();
        let mut dec = Vec::new();
        extract_kind_checks(SAMPLE_CODEC, &mut enc, &mut dec);
        assert!(!enc.is_empty());
        assert!(!dec.is_empty());

        let enc_ctrl = enc.iter().find(|c| c.function == "encodeControlRequest").unwrap();
        assert_eq!(enc_ctrl.expected_kind, "command");

        let enc_event = enc.iter().find(|c| c.function == "encodeEvent").unwrap();
        assert_eq!(enc_event.expected_kind, "event");
    }

    #[test]
    fn codec_format_detected() {
        let format = if SAMPLE_CODEC.contains("cbor.Marshal") { "cbor" } else { "json" };
        assert_eq!(format, "cbor");
    }
}
