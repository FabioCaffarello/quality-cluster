use crate::error::Result;
use std::path::Path;

/// A binding definition extracted from config sources or HTTP test fixtures.
#[derive(Debug, Clone)]
pub struct BindingDefinition {
    /// Binding name (e.g., "user-events").
    pub name: String,
    /// Kafka topic (e.g., "users-topic").
    pub topic: String,
    /// Config name that declares this binding (e.g., metadata.name).
    pub config_name: String,
    /// Activation scope kind (default: "global").
    pub scope_kind: String,
    /// Activation scope key (default: "default").
    pub scope_key: String,
    /// Number of fields declared.
    pub field_count: usize,
    /// Number of rules declared.
    pub rule_count: usize,
    /// Source file where this binding was found.
    pub source_file: Option<String>,
}

/// Scan deploy/configs/ for service configs that reference bootstrap scopes
/// and parse any embedded or referenced config payloads.
///
/// Note: deploy configs don't contain binding definitions directly (bindings
/// are created via the API at runtime). We extract scope info and structural
/// hints here.
pub fn scan_binding_configs(_configs_dir: &Path) -> Result<Vec<BindingDefinition>> {
    // Deploy configs (consumer.jsonc, etc.) don't contain binding definitions.
    // Bindings are created at runtime via the API (CreateDraft → Validate → Compile → Activate).
    // We return empty here — actual bindings come from HTTP fixtures or source analysis.
    Ok(Vec::new())
}

/// Scan tests/http/ directory for .http fixture files that contain example
/// config payloads with binding definitions.
pub fn scan_http_fixtures(http_dir: &Path) -> Result<Vec<BindingDefinition>> {
    let mut bindings = Vec::new();

    let entries = std::fs::read_dir(http_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("http") {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Look for JSON or YAML payloads containing binding definitions
        extract_bindings_from_http_fixture(&content, file_name, &mut bindings)?;
    }

    Ok(bindings)
}

/// Extract binding definitions from HTTP fixture file content.
/// These files contain HTTP requests with JSON/YAML bodies that include
/// config payloads with bindings, fields, and rules.
fn extract_bindings_from_http_fixture(
    content: &str,
    file_name: &str,
    bindings: &mut Vec<BindingDefinition>,
) -> Result<()> {
    // HTTP fixtures may contain multiple requests separated by ###
    // Look for JSON bodies with "bindings" arrays
    let mut in_body = false;
    let mut body_lines: Vec<String> = Vec::new();
    let mut brace_depth: i32 = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Start of a JSON body (after a blank line following headers, or a line starting with {)
        if !in_body && trimmed.starts_with('{') {
            in_body = true;
            body_lines.clear();
            brace_depth = 0;
        }

        if in_body {
            body_lines.push(line.to_string());
            brace_depth += trimmed.chars().filter(|&c| c == '{').count() as i32;
            brace_depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;

            if brace_depth <= 0 {
                // End of JSON body
                let body = body_lines.join("\n");
                parse_config_body(&body, file_name, bindings);
                in_body = false;
                body_lines.clear();
            }
        }
    }

    Ok(())
}

fn parse_config_body(body: &str, file_name: &str, bindings: &mut Vec<BindingDefinition>) {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Check if this is a create_draft payload with "content" containing YAML/JSON
    if let Some(content_str) = value.get("content").and_then(|v| v.as_str()) {
        // Try parsing the embedded content as JSON first, then YAML-like
        if let Ok(inner) = serde_json::from_str::<serde_json::Value>(content_str) {
            extract_bindings_from_value(&inner, file_name, bindings);
            return;
        }
        // Try parsing as YAML (simple line-based extraction)
        extract_bindings_from_yaml(content_str, file_name, bindings);
        return;
    }

    // Check if this is a direct config document with "bindings" array
    extract_bindings_from_value(&value, file_name, bindings);
}

fn extract_bindings_from_value(
    value: &serde_json::Value,
    file_name: &str,
    bindings: &mut Vec<BindingDefinition>,
) {
    let binding_array = match value.get("bindings").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return,
    };

    let config_name = value
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let field_count = value
        .get("fields")
        .and_then(|v| v.as_array())
        .map_or(0, |a| a.len());

    let rule_count = value
        .get("rules")
        .and_then(|v| v.as_array())
        .map_or(0, |a| a.len());

    for binding in binding_array {
        let name = binding
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let topic = binding
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if name.is_empty() && topic.is_empty() {
            continue;
        }

        bindings.push(BindingDefinition {
            name,
            topic,
            config_name: config_name.to_string(),
            scope_kind: "global".to_string(),
            scope_key: "default".to_string(),
            field_count,
            rule_count,
            source_file: Some(format!("tests/http/{file_name}")),
        });
    }
}

fn extract_bindings_from_yaml(
    content: &str,
    file_name: &str,
    bindings: &mut Vec<BindingDefinition>,
) {
    // Simple YAML extraction for binding definitions
    // Looks for patterns like:
    //   bindings:
    //     - name: user-events
    //       topic: users-topic
    let mut in_bindings = false;
    let mut current_name = String::new();
    let mut current_topic = String::new();
    let mut config_name = String::from("unknown");
    let mut field_count = 0;
    let mut rule_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Extract metadata.name
        if trimmed.starts_with("name:") && !in_bindings {
            let val = trimmed.trim_start_matches("name:").trim().trim_matches('"');
            if !val.is_empty() {
                config_name = val.to_string();
            }
        }

        if trimmed == "bindings:" {
            in_bindings = true;
            continue;
        }

        if in_bindings {
            if trimmed.starts_with("- name:") || trimmed.starts_with("name:") {
                // Save previous binding if we have one
                if !current_name.is_empty() || !current_topic.is_empty() {
                    bindings.push(BindingDefinition {
                        name: current_name.clone(),
                        topic: current_topic.clone(),
                        config_name: config_name.clone(),
                        scope_kind: "global".to_string(),
                        scope_key: "default".to_string(),
                        field_count,
                        rule_count,
                        source_file: Some(format!("tests/http/{file_name}")),
                    });
                }
                current_name = trimmed
                    .trim_start_matches("- ")
                    .trim_start_matches("name:")
                    .trim()
                    .trim_matches('"')
                    .to_string();
                current_topic.clear();
            } else if trimmed.starts_with("topic:") {
                current_topic = trimmed
                    .trim_start_matches("topic:")
                    .trim()
                    .trim_matches('"')
                    .to_string();
            } else if !trimmed.starts_with('-') && !trimmed.starts_with("topic:") && !trimmed.is_empty()
                && !trimmed.starts_with('#')
            {
                // End of bindings section
                in_bindings = false;
            }
        }

        // Count fields and rules sections
        if trimmed == "fields:" {
            in_bindings = false;
        }
        if trimmed.starts_with("- name:") && !in_bindings {
            // Could be fields or rules
            if content[..content.find(trimmed).unwrap_or(0)]
                .lines()
                .rev()
                .take(10)
                .any(|l| l.trim() == "fields:")
            {
                field_count += 1;
            }
            if content[..content.find(trimmed).unwrap_or(0)]
                .lines()
                .rev()
                .take(10)
                .any(|l| l.trim() == "rules:")
            {
                rule_count += 1;
            }
        }
    }

    // Save last binding
    if !current_name.is_empty() || !current_topic.is_empty() {
        bindings.push(BindingDefinition {
            name: current_name,
            topic: current_topic,
            config_name,
            scope_kind: "global".to_string(),
            scope_key: "default".to_string(),
            field_count,
            rule_count,
            source_file: Some(format!("tests/http/{file_name}")),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_binding_configs_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_binding_configs(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extract_bindings_from_json_body() {
        let body = r#"{
  "metadata": { "name": "user-quality" },
  "bindings": [
    { "name": "user-events", "topic": "users-topic" },
    { "name": "order-events", "topic": "orders-topic" }
  ],
  "fields": [
    { "name": "user_id", "type": "string", "required": true }
  ],
  "rules": [
    { "name": "uid_required", "field": "user_id", "operator": "required" }
  ]
}"#;
        let mut bindings = Vec::new();
        parse_config_body(body, "test.http", &mut bindings);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].name, "user-events");
        assert_eq!(bindings[0].topic, "users-topic");
        assert_eq!(bindings[0].config_name, "user-quality");
        assert_eq!(bindings[0].field_count, 1);
        assert_eq!(bindings[0].rule_count, 1);
        assert_eq!(bindings[1].name, "order-events");
    }

    #[test]
    fn extract_bindings_from_create_draft_payload() {
        let body = r#"{
  "format": "json",
  "content": "{\"metadata\":{\"name\":\"test-config\"},\"bindings\":[{\"name\":\"events\",\"topic\":\"evt-topic\"}],\"fields\":[],\"rules\":[]}"
}"#;
        let mut bindings = Vec::new();
        parse_config_body(body, "test.http", &mut bindings);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].name, "events");
        assert_eq!(bindings[0].topic, "evt-topic");
        assert_eq!(bindings[0].config_name, "test-config");
    }

    #[test]
    fn extract_bindings_from_yaml_content() {
        let yaml = r#"metadata:
  name: yaml-config
bindings:
  - name: clicks
    topic: click-stream
  - name: views
    topic: page-views
fields:
  - name: user_id
    type: string
rules:
  - name: uid_required
    field: user_id
"#;
        let mut bindings = Vec::new();
        extract_bindings_from_yaml(yaml, "test.http", &mut bindings);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].name, "clicks");
        assert_eq!(bindings[0].topic, "click-stream");
        assert_eq!(bindings[1].name, "views");
        assert_eq!(bindings[1].topic, "page-views");
    }

    #[test]
    fn extract_bindings_ignores_empty() {
        let body = r#"{"no_bindings": true}"#;
        let mut bindings = Vec::new();
        parse_config_body(body, "test.http", &mut bindings);
        assert!(bindings.is_empty());
    }

    #[test]
    fn extract_bindings_ignores_invalid_json() {
        let body = "not json at all";
        let mut bindings = Vec::new();
        parse_config_body(body, "test.http", &mut bindings);
        assert!(bindings.is_empty());
    }

    #[test]
    fn scan_http_fixtures_discovers_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("configctl.http"),
            r#"### Create draft
POST http://localhost:8080/configctl/drafts
Content-Type: application/json

{
  "format": "json",
  "content": "{\"metadata\":{\"name\":\"test\"},\"bindings\":[{\"name\":\"b1\",\"topic\":\"t1\"}],\"fields\":[{\"name\":\"f1\",\"type\":\"string\"}],\"rules\":[{\"name\":\"r1\",\"field\":\"f1\",\"operator\":\"required\"}]}"
}

### List configs
GET http://localhost:8080/configctl/configs
"#,
        )
        .unwrap();

        let bindings = scan_http_fixtures(dir.path()).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].name, "b1");
        assert_eq!(bindings[0].topic, "t1");
    }

    #[test]
    fn scan_http_fixtures_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let bindings = scan_http_fixtures(dir.path()).unwrap();
        assert!(bindings.is_empty());
    }

    #[test]
    fn scan_http_fixtures_skips_non_http_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# Not an HTTP fixture").unwrap();
        let bindings = scan_http_fixtures(dir.path()).unwrap();
        assert!(bindings.is_empty());
    }

    #[test]
    fn binding_definition_has_default_scope() {
        let body = r#"{
  "bindings": [{ "name": "x", "topic": "t" }],
  "fields": [],
  "rules": []
}"#;
        let mut bindings = Vec::new();
        parse_config_body(body, "test.http", &mut bindings);
        assert_eq!(bindings[0].scope_kind, "global");
        assert_eq!(bindings[0].scope_key, "default");
    }

    #[test]
    fn extract_from_http_fixture_multiple_bodies() {
        let content = r#"### First request
POST http://localhost:8080/api

{
  "bindings": [{ "name": "a", "topic": "ta" }],
  "fields": [],
  "rules": []
}

### Second request
POST http://localhost:8080/api

{
  "bindings": [{ "name": "b", "topic": "tb" }],
  "fields": [],
  "rules": []
}
"#;
        let mut bindings = Vec::new();
        extract_bindings_from_http_fixture(content, "test.http", &mut bindings).unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].name, "a");
        assert_eq!(bindings[1].name, "b");
    }
}
