use crate::error::Result;
use std::collections::HashMap;
use std::path::Path;

/// Topology constants extracted from Go source files.
#[derive(Debug, Clone, Default)]
pub struct SourceTopology {
    /// Stream name -> list of subject patterns
    pub streams: HashMap<String, Vec<String>>,
    /// Durable consumer name -> stream name
    pub durables: HashMap<String, String>,
    /// All discovered subject strings
    pub subjects: Vec<String>,
}

/// Scan Go source files under `internal/` for topology constants.
pub fn scan_source(internal_dir: &Path) -> Result<SourceTopology> {
    let mut topo = SourceTopology::default();
    let mut subject_set = std::collections::HashSet::new();

    scan_dir(internal_dir, &mut topo, &mut subject_set)?;

    topo.subjects = subject_set.into_iter().collect();
    topo.subjects.sort();

    Ok(topo)
}

fn scan_dir(
    dir: &Path,
    topo: &mut SourceTopology,
    subjects: &mut std::collections::HashSet<String>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, topo, subjects)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("go") {
            scan_file(&path, topo, subjects)?;
        }
    }

    Ok(())
}

fn scan_file(
    path: &Path,
    topo: &mut SourceTopology,
    subjects: &mut std::collections::HashSet<String>,
) -> Result<()> {
    let content = std::fs::read_to_string(path)?;

    // Extract stream names: look for quoted UPPER_SNAKE_CASE strings near stream/Stream
    extract_streams(&content, topo);

    // Extract durable consumer names: look for Durable: "..." patterns
    extract_durables(&content, topo);

    // Extract NATS subject patterns: look for dotted strings like "foo.bar.baz"
    extract_subjects(&content, subjects);

    Ok(())
}

fn extract_streams(content: &str, topo: &mut SourceTopology) {
    // Match patterns like:
    //   Stream: "DATA_PLANE_INGESTION"
    //   Name: "DATA_PLANE_INGESTION"
    //   Name:     "CONFIGCTL_EVENTS"
    // followed by Subjects: []string{"pattern.>"}
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        // Look for stream name assignments
        let stream_name = extract_quoted_stream_name(line);
        if let Some(name) = stream_name {
            // Search nearby lines (within 10 lines) for Subjects
            let subjects = find_subjects_near(&lines, i, 10);
            if !subjects.is_empty() {
                topo.streams
                    .entry(name)
                    .or_default()
                    .extend(subjects);
            } else {
                // Still register the stream even without subjects
                topo.streams.entry(name).or_default();
            }
        }
    }

    // Deduplicate subjects per stream
    for subjects in topo.streams.values_mut() {
        subjects.sort();
        subjects.dedup();
    }
}

fn extract_quoted_stream_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("//") {
        return None;
    }

    // Match lines containing Name:, Stream:, or the word "stream"/"Stream"
    // that also contain an UPPER_SNAKE_CASE quoted string
    let is_stream_context = trimmed.contains("Name:")
        || trimmed.contains("Stream")
        || trimmed.contains("stream");

    if !is_stream_context {
        return None;
    }

    for word in extract_all_quoted(trimmed) {
        if is_stream_name(&word) {
            return Some(word);
        }
    }

    None
}

fn extract_durables(content: &str, topo: &mut SourceTopology) {
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Match: Durable: "validator-dataplane-v1"
        // or ValidatorDurable: "validator-dataplane-v1"
        if trimmed.contains("Durable") && !trimmed.starts_with("//") {
            for val in extract_all_quoted(trimmed) {
                if val.contains('-') && val.chars().all(|c| c.is_alphanumeric() || c == '-') {
                    // Find stream name nearby (15 lines), fall back to whole file
                    let stream = find_stream_name_near(&lines, i, 15)
                        .or_else(|| find_stream_name_near(&lines, lines.len() / 2, lines.len()));
                    if let Some(stream_name) = stream {
                        topo.durables.insert(val, stream_name);
                    }
                }
            }
        }
    }
}

fn extract_subjects(content: &str, subjects: &mut std::collections::HashSet<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        for val in extract_all_quoted(trimmed) {
            if is_nats_subject(&val) {
                subjects.insert(val);
            }
        }
    }
}

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

/// Check if a string looks like a JetStream stream name (UPPER_SNAKE_CASE).
fn is_stream_name(s: &str) -> bool {
    s.len() >= 3
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        && s.contains('_')
        && s.chars().next().map_or(false, |c| c.is_ascii_uppercase())
}

/// Check if a string looks like a NATS subject (dotted segments, may end with >).
fn is_nats_subject(s: &str) -> bool {
    if s.is_empty() || s.len() < 3 {
        return false;
    }

    let segments: Vec<&str> = s.split('.').collect();
    if segments.len() < 2 {
        return false;
    }

    // Each segment must be non-empty and contain only valid subject characters
    segments.iter().all(|seg| {
        !seg.is_empty()
            && seg
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '>' || c == '*')
    })
}

/// Search nearby lines for Subjects definition and extract patterns.
fn find_subjects_near(lines: &[&str], center: usize, radius: usize) -> Vec<String> {
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len());

    for i in start..end {
        let trimmed = lines[i].trim();
        if trimmed.contains("Subjects") && !trimmed.starts_with("//") {
            // Extract subjects from this line and following lines
            let mut subjects = Vec::new();
            for val in extract_all_quoted(trimmed) {
                if is_nats_subject(&val) {
                    subjects.push(val);
                }
            }
            // Check next few lines for more subjects
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

/// Search nearby lines for a stream name.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_stream_name_valid() {
        assert!(is_stream_name("DATA_PLANE_INGESTION"));
        assert!(is_stream_name("CONFIGCTL_EVENTS"));
        assert!(!is_stream_name("data_plane")); // lowercase
        assert!(!is_stream_name("SINGLE")); // no underscore
        assert!(!is_stream_name("AB")); // too short
    }

    #[test]
    fn is_nats_subject_valid() {
        assert!(is_nats_subject("dataplane.ingestion.received.>"));
        assert!(is_nats_subject("configctl.events.config.>"));
        assert!(is_nats_subject("configctl.control.create_draft"));
        assert!(is_nats_subject("validator.results.list"));
        assert!(!is_nats_subject("not a subject"));
        assert!(!is_nats_subject("single"));
        assert!(!is_nats_subject(""));
    }

    #[test]
    fn extract_all_quoted_finds_values() {
        let line = r#"Name: "DATA_PLANE_INGESTION", Subjects: []string{"dataplane.ingestion.received.>"}"#;
        let vals = extract_all_quoted(line);
        assert!(vals.contains(&"DATA_PLANE_INGESTION".to_string()));
        assert!(vals.contains(&"dataplane.ingestion.received.>".to_string()));
    }

    #[test]
    fn extract_streams_from_go_source() {
        let source = r#"
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
        let mut topo = SourceTopology::default();
        extract_streams(source, &mut topo);
        assert!(topo.streams.contains_key("DATA_PLANE_INGESTION"));
        assert!(topo.streams["DATA_PLANE_INGESTION"]
            .contains(&"dataplane.ingestion.received.>".to_string()));
    }

    #[test]
    fn extract_durables_from_go_source() {
        let source = r#"
    ValidatorIngested: ConsumerSpec{
        Durable: "validator-dataplane-v1",
        Event: EventSpec{
            Stream: StreamSpec{
                Name: "DATA_PLANE_INGESTION",
            },
        },
    },
"#;
        let mut topo = SourceTopology::default();
        extract_durables(source, &mut topo);
        assert_eq!(
            topo.durables.get("validator-dataplane-v1"),
            Some(&"DATA_PLANE_INGESTION".to_string())
        );
    }

    #[test]
    fn extract_subjects_from_go_source() {
        let source = r#"
    Subject: "configctl.events.config.activated",
    Subject: "configctl.control.create_draft",
    Subject: "validator.results.list",
"#;
        let mut subjects = std::collections::HashSet::new();
        extract_subjects(source, &mut subjects);
        assert!(subjects.contains("configctl.events.config.activated"));
        assert!(subjects.contains("configctl.control.create_draft"));
        assert!(subjects.contains("validator.results.list"));
    }

    #[test]
    fn scan_source_on_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_source(dir.path()).unwrap();
        assert!(result.streams.is_empty());
        assert!(result.durables.is_empty());
        assert!(result.subjects.is_empty());
    }

    #[test]
    fn scan_source_with_go_file() {
        let dir = tempfile::tempdir().unwrap();
        let go_file = dir.path().join("registry.go");
        std::fs::write(
            &go_file,
            r#"
package test

func Registry() {
    stream := StreamSpec{
        Name:     "TEST_STREAM",
        Subjects: []string{"test.events.>"},
    }
    consumer := ConsumerSpec{
        Durable: "test-consumer-v1",
        Event: EventSpec{
            Stream: StreamSpec{
                Name: "TEST_STREAM",
            },
        },
    }
    _ = ControlSpec{
        Subject: "test.control.action",
    }
}
"#,
        )
        .unwrap();

        let result = scan_source(dir.path()).unwrap();
        assert!(result.streams.contains_key("TEST_STREAM"));
        assert_eq!(
            result.durables.get("test-consumer-v1"),
            Some(&"TEST_STREAM".to_string())
        );
        assert!(result.subjects.contains(&"test.events.>".to_string()));
        assert!(result.subjects.contains(&"test.control.action".to_string()));
    }

    #[test]
    fn scan_source_skips_commented_lines() {
        let dir = tempfile::tempdir().unwrap();
        let go_file = dir.path().join("comments.go");
        std::fs::write(
            &go_file,
            r#"
package test
// Subject: "commented.out.subject"
// Name: "COMMENTED_STREAM"
func Real() {
    Subject: "real.subject.here",
}
"#,
        )
        .unwrap();

        let result = scan_source(dir.path()).unwrap();
        assert!(!result.subjects.contains(&"commented.out.subject".to_string()));
        assert!(!result.streams.contains_key("COMMENTED_STREAM"));
        assert!(result.subjects.contains(&"real.subject.here".to_string()));
    }

    #[test]
    fn scan_source_ignores_non_go_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("readme.md"),
            r#"Subject: "not.a.go.file""#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("test.txt"),
            r#"Name: "NOT_A_STREAM""#,
        )
        .unwrap();

        let result = scan_source(dir.path()).unwrap();
        assert!(result.streams.is_empty());
        assert!(result.subjects.is_empty());
    }

    #[test]
    fn scan_source_recurses_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("adapters/nats");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("registry.go"),
            r#"
package nats
func Reg() {
    Subject: "deep.nested.subject",
}
"#,
        )
        .unwrap();

        let result = scan_source(dir.path()).unwrap();
        assert!(result.subjects.contains(&"deep.nested.subject".to_string()));
    }

    #[test]
    fn is_stream_name_rejects_lowercase() {
        assert!(!is_stream_name("data_plane"));
        assert!(!is_stream_name("Mixed_Case"));
    }

    #[test]
    fn is_nats_subject_rejects_spaces_and_special_chars() {
        assert!(!is_nats_subject("has space.here"));
        assert!(!is_nats_subject("has/slash.here"));
        assert!(!is_nats_subject("ab")); // too short
    }

    #[test]
    fn extract_all_quoted_empty_string() {
        assert!(extract_all_quoted("no quotes here").is_empty());
    }

    #[test]
    fn extract_all_quoted_unclosed_quote() {
        let result = extract_all_quoted(r#"start "unclosed"#);
        assert!(result.is_empty()); // unclosed quote → no match
    }

    #[test]
    fn subjects_are_sorted_and_deduped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.go"),
            r#"
package test
func A() {
    Subject: "zzz.last.subject",
    Subject: "aaa.first.subject",
    Subject: "aaa.first.subject",
}
"#,
        )
        .unwrap();

        let result = scan_source(dir.path()).unwrap();
        // subjects should be sorted
        let positions: Vec<usize> = result
            .subjects
            .iter()
            .enumerate()
            .filter(|(_, s)| s.contains("aaa.") || s.contains("zzz."))
            .map(|(i, _)| i)
            .collect();
        if positions.len() >= 2 {
            assert!(positions[0] < positions[1], "subjects should be sorted");
        }
        // No duplicates
        let unique: std::collections::HashSet<&String> = result.subjects.iter().collect();
        assert_eq!(result.subjects.len(), unique.len());
    }
}
