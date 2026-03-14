use std::path::Path;

use crate::error::Result;

/// A domain event discovered from Go source.
#[derive(Debug, Clone)]
pub struct DomainEventDef {
    #[allow(dead_code)]
    pub const_name: String,
    pub event_name: String, // e.g., "config.draft_created"
    pub struct_name: String,
    pub has_metadata: bool,
    pub file: String,
}

/// All domain events discovered from the codebase.
#[derive(Debug, Default)]
pub struct DomainEventIndex {
    pub events: Vec<DomainEventDef>,
}

/// Scan domain event definitions from Go source files.
pub fn scan_domain_events(internal_dir: &Path) -> Result<DomainEventIndex> {
    let mut index = DomainEventIndex::default();

    // Scan domain event files
    let events_file = internal_dir.join("domain/configctl/events.go");
    if events_file.is_file() {
        let content = std::fs::read_to_string(&events_file)?;
        let rel = "internal/domain/configctl/events.go".to_string();
        extract_domain_events(&content, &rel, &mut index);
    }

    Ok(index)
}

fn extract_domain_events(source: &str, file: &str, index: &mut DomainEventIndex) {
    // Step 1: Extract event name constants
    // Pattern: EventXxx events.Name = "config.xxx"
    let mut event_consts: Vec<(String, String)> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        // Match patterns like: EventDraftCreated events.Name = "config.draft_created"
        // or: EventDraftCreated            events.Name = "config.draft_created"
        if trimmed.starts_with("Event") && trimmed.contains("events.Name") && trimmed.contains("= \"") {
            let const_name = trimmed.split_whitespace().next().unwrap_or("").to_string();
            if let Some(start) = trimmed.find('"') {
                if let Some(end) = trimmed[start + 1..].find('"') {
                    let event_name = trimmed[start + 1..start + 1 + end].to_string();
                    event_consts.push((const_name, event_name));
                }
            }
        }
    }

    // Step 2: Extract event struct definitions and check for Metadata field
    let event_structs = extract_event_structs(source);

    // Step 3: Match event name constants to their EventName() method implementations
    // Pattern: func (e XxxEvent) EventName() events.Name { return EventYyy }
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains("EventName()") && trimmed.contains("return Event") {
            // Extract struct name from receiver
            let struct_name = extract_receiver_type(trimmed);
            // Extract the returned constant
            let const_name = extract_return_value(trimmed);

            if let (Some(sname), Some(cname)) = (struct_name, const_name) {
                // Find the matching event name
                if let Some((_, event_name)) = event_consts.iter().find(|(c, _)| *c == cname) {
                    let has_metadata = event_structs
                        .iter()
                        .find(|(name, _)| *name == sname)
                        .map_or(false, |(_, has_meta)| *has_meta);

                    index.events.push(DomainEventDef {
                        const_name: cname.clone(),
                        event_name: event_name.clone(),
                        struct_name: sname,
                        has_metadata,
                        file: file.to_string(),
                    });
                }
            }
        }
    }
}

/// Extract event struct definitions and whether they have a Metadata field.
/// Returns (struct_name, has_metadata) pairs.
fn extract_event_structs(source: &str) -> Vec<(String, bool)> {
    let mut results = Vec::new();
    let mut i = 0;

    while i < source.len() {
        // Look for "type XxxEvent struct {"
        if let Some(pos) = source[i..].find("Event struct {") {
            let abs_pos = i + pos;
            // Get the type name
            let before = &source[i..abs_pos];
            let type_start = before.rfind("type ").map(|p| i + p + 5);

            if let Some(start) = type_start {
                let type_name = source[start..abs_pos + "Event".len()].trim().to_string();

                // Find the struct body
                let brace_start = abs_pos + "Event struct {".len();
                if let Some(end) = find_closing_brace(source, brace_start) {
                    let body = &source[brace_start..end];
                    let has_metadata = body.contains("Metadata") && body.contains("events.Metadata");
                    results.push((type_name, has_metadata));
                    i = end + 1;
                    continue;
                }
            }
            i = abs_pos + 1;
        } else {
            break;
        }
    }

    results
}

/// Extract the receiver type from a method signature like `func (e FooEvent) MethodName()`.
fn extract_receiver_type(line: &str) -> Option<String> {
    // Pattern: func (x TypeName) MethodName
    let after_paren = line.find('(').and_then(|p| {
        let rest = &line[p + 1..];
        rest.find(')').map(|end| &rest[..end])
    })?;

    let parts: Vec<&str> = after_paren.split_whitespace().collect();
    if parts.len() >= 2 {
        Some(parts[1].trim_start_matches('*').to_string())
    } else {
        None
    }
}

/// Extract the return value from a single-line return statement.
fn extract_return_value(line: &str) -> Option<String> {
    if let Some(pos) = line.find("return ") {
        let after = &line[pos + 7..];
        let end = after.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(after.len());
        let value = &after[..end];
        if !value.is_empty() {
            return Some(value.to_string());
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

    const SAMPLE_EVENTS: &str = r#"
package configctl

import "internal/shared/events"

const (
    EventDraftCreated            events.Name = "config.draft_created"
    EventValidated               events.Name = "config.validated"
    EventCompiled                events.Name = "config.compiled"
    EventActivated               events.Name = "config.activated"
    EventArchived                events.Name = "config.archived"
)

type DraftCreatedEvent struct {
    Metadata     events.Metadata `json:"metadata"`
    ConfigSetID  string          `json:"config_set_id"`
    VersionID    string          `json:"version_id"`
}

type ConfigValidatedEvent struct {
    Metadata           events.Metadata `json:"metadata"`
    ConfigSetID        string          `json:"config_set_id"`
    VersionID          string          `json:"version_id"`
    DefinitionChecksum string          `json:"definition_checksum"`
}

type ConfigCompiledEvent struct {
    Metadata    events.Metadata `json:"metadata"`
    ConfigSetID string          `json:"config_set_id"`
}

type ConfigActivatedEvent struct {
    Metadata    events.Metadata `json:"metadata"`
    ConfigSetID string          `json:"config_set_id"`
}

type ConfigArchivedEvent struct {
    Metadata    events.Metadata `json:"metadata"`
    ConfigSetID string          `json:"config_set_id"`
}

func (e DraftCreatedEvent) EventName() events.Name                    { return EventDraftCreated }
func (e DraftCreatedEvent) EventMetadata() events.Metadata            { return e.Metadata }
func (e ConfigValidatedEvent) EventName() events.Name                 { return EventValidated }
func (e ConfigValidatedEvent) EventMetadata() events.Metadata         { return e.Metadata }
func (e ConfigCompiledEvent) EventName() events.Name                  { return EventCompiled }
func (e ConfigCompiledEvent) EventMetadata() events.Metadata          { return e.Metadata }
func (e ConfigActivatedEvent) EventName() events.Name                 { return EventActivated }
func (e ConfigActivatedEvent) EventMetadata() events.Metadata         { return e.Metadata }
func (e ConfigArchivedEvent) EventName() events.Name                  { return EventArchived }
func (e ConfigArchivedEvent) EventMetadata() events.Metadata          { return e.Metadata }
"#;

    #[test]
    fn extracts_domain_events() {
        let mut index = DomainEventIndex::default();
        extract_domain_events(SAMPLE_EVENTS, "test.go", &mut index);

        assert_eq!(index.events.len(), 5);

        let draft = index.events.iter().find(|e| e.event_name == "config.draft_created").unwrap();
        assert_eq!(draft.struct_name, "DraftCreatedEvent");
        assert_eq!(draft.const_name, "EventDraftCreated");
        assert!(draft.has_metadata);

        let validated = index.events.iter().find(|e| e.event_name == "config.validated").unwrap();
        assert_eq!(validated.struct_name, "ConfigValidatedEvent");
        assert!(validated.has_metadata);
    }

    #[test]
    fn all_events_have_metadata() {
        let mut index = DomainEventIndex::default();
        extract_domain_events(SAMPLE_EVENTS, "test.go", &mut index);

        for event in &index.events {
            assert!(event.has_metadata, "event {} should have metadata", event.struct_name);
        }
    }

    #[test]
    fn extract_receiver_type_works() {
        assert_eq!(
            extract_receiver_type("func (e DraftCreatedEvent) EventName() events.Name"),
            Some("DraftCreatedEvent".into())
        );
        assert_eq!(
            extract_receiver_type("func (e *ConfigValidatedEvent) EventName()"),
            Some("ConfigValidatedEvent".into())
        );
    }

    #[test]
    fn extract_return_value_works() {
        assert_eq!(
            extract_return_value("{ return EventDraftCreated }"),
            Some("EventDraftCreated".into())
        );
    }
}
