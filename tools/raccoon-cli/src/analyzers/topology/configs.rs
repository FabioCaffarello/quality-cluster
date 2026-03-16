use crate::error::{CliError, Result};
use std::collections::HashMap;
use std::path::Path;

/// Extracted service configuration from a JSONC config file.
#[derive(Debug, Clone, Default)]
pub struct ServiceConfig {
    pub name: String,
    pub kafka_brokers: Vec<String>,
    pub kafka_consumer_group: Option<String>,
    pub kafka_client_id: Option<String>,
    pub nats_url: Option<String>,
    pub bootstrap_base_url: Option<String>,
    pub bootstrap_reconcile_interval: Option<String>,
}

/// Parse all .jsonc config files from the configs directory.
pub fn parse_all_configs(configs_dir: &Path) -> Result<HashMap<String, ServiceConfig>> {
    let mut configs = HashMap::new();

    let entries = std::fs::read_dir(configs_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonc") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        match parse_config(&path) {
            Ok(mut cfg) => {
                cfg.name = name.clone();
                configs.insert(name, cfg);
            }
            Err(_) => {
                // Non-fatal: register as a config with no data
                let mut cfg = ServiceConfig::default();
                cfg.name = name.clone();
                configs.insert(name, cfg);
            }
        }
    }

    Ok(configs)
}

/// Parse a single JSONC config file, extracting topology-relevant fields.
/// Uses a simple JSONC-to-JSON approach (strip // comments) then serde_json.
fn parse_config(path: &Path) -> Result<ServiceConfig> {
    let raw = std::fs::read_to_string(path)?;
    let cleaned = strip_jsonc_comments(&raw);

    let value: serde_json::Value =
        serde_json::from_str(&cleaned).map_err(|e| CliError::Command {
            message: format!("parse {}: {e}", path.display()),
        })?;

    let mut cfg = ServiceConfig::default();

    // Extract kafka settings
    if let Some(kafka) = value.get("kafka") {
        if let Some(brokers) = kafka.get("brokers").and_then(|b| b.as_array()) {
            cfg.kafka_brokers = brokers
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(group) = kafka.get("consumer_group").and_then(|v| v.as_str()) {
            cfg.kafka_consumer_group = Some(group.to_string());
        }
        if let Some(id) = kafka.get("client_id").and_then(|v| v.as_str()) {
            cfg.kafka_client_id = Some(id.to_string());
        }
    }

    // Extract nats settings
    if let Some(nats) = value.get("nats") {
        if let Some(url) = nats.get("url").and_then(|v| v.as_str()) {
            cfg.nats_url = Some(url.to_string());
        }
    }

    // Extract bootstrap settings
    if let Some(bootstrap) = value.get("bootstrap") {
        if let Some(url) = bootstrap.get("base_url").and_then(|v| v.as_str()) {
            cfg.bootstrap_base_url = Some(url.to_string());
        }
        if let Some(interval) = bootstrap.get("reconcile_interval").and_then(|v| v.as_str()) {
            cfg.bootstrap_reconcile_interval = Some(interval.to_string());
        }
    }

    Ok(cfg)
}

/// Strip single-line JSONC comments (// ...) from the input.
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
            // Skip until end of line
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
    fn strip_comments_simple() {
        let input = r#"{
  // this is a comment
  "key": "value"
}"#;
        let cleaned = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn strip_comments_preserves_url_with_slashes() {
        let input = r#"{
  "url": "http://server:8080"
}"#;
        let cleaned = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["url"], "http://server:8080");
    }

    #[test]
    fn parse_consumer_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("consumer.jsonc");
        std::fs::write(
            &path,
            r#"{
  // consumer config
  "kafka": {
    "enabled": true,
    "brokers": ["kafka:9092"],
    "client_id": "quality-service-consumer",
    "consumer_group": "quality-service-consumer-v1"
  },
  "nats": {
    "enabled": true,
    "url": "nats://nats:4222"
  },
  "bootstrap": {
    "base_url": "http://server:8080"
  }
}"#,
        )
        .unwrap();

        let cfg = parse_config(&path).unwrap();
        assert_eq!(cfg.kafka_brokers, vec!["kafka:9092"]);
        assert_eq!(
            cfg.kafka_consumer_group.as_deref(),
            Some("quality-service-consumer-v1")
        );
        assert_eq!(cfg.nats_url.as_deref(), Some("nats://nats:4222"));
        assert_eq!(
            cfg.bootstrap_base_url.as_deref(),
            Some("http://server:8080")
        );
        assert_eq!(cfg.bootstrap_reconcile_interval.as_deref(), None);
    }

    #[test]
    fn parse_all_configs_discovers_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("consumer.jsonc"),
            r#"{"kafka": {"brokers": ["kafka:9092"]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("emulator.jsonc"),
            r#"{"kafka": {"brokers": ["kafka:9092"]}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("readme.txt"), "not a config").unwrap();

        let configs = parse_all_configs(dir.path()).unwrap();
        assert_eq!(configs.len(), 2);
        assert!(configs.contains_key("consumer"));
        assert!(configs.contains_key("emulator"));
    }

    #[test]
    fn strip_comments_escaped_quotes_in_string() {
        let input = r#"{"key": "value with \"quoted\" inside"}"#;
        let cleaned = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert!(parsed["key"].as_str().unwrap().contains("quoted"));
    }

    #[test]
    fn strip_comments_inline_after_value() {
        let input = r#"{
  "key": "value" // inline comment
}"#;
        let cleaned = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn strip_comments_multiple_lines() {
        let input = r#"{
  // first comment
  "a": 1,
  // second comment
  "b": 2
  // trailing
}"#;
        let cleaned = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 2);
    }

    #[test]
    fn strip_comments_url_in_string_not_stripped() {
        let input = r#"{"nats": {"url": "nats://nats:4222"}, "http": "http://server:8080"}"#;
        let cleaned = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["nats"]["url"], "nats://nats:4222");
        assert_eq!(parsed["http"], "http://server:8080");
    }

    #[test]
    fn parse_config_empty_json_returns_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jsonc");
        std::fs::write(&path, "{}").unwrap();

        let cfg = parse_config(&path).unwrap();
        assert!(cfg.kafka_brokers.is_empty());
        assert!(cfg.nats_url.is_none());
        assert!(cfg.bootstrap_base_url.is_none());
        assert!(cfg.bootstrap_reconcile_interval.is_none());
    }

    #[test]
    fn parse_all_configs_malformed_jsonc_registers_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("broken.jsonc"), "not valid json").unwrap();

        let configs = parse_all_configs(dir.path()).unwrap();
        assert!(configs.contains_key("broken"));
        let cfg = &configs["broken"];
        assert!(cfg.kafka_brokers.is_empty());
    }

    #[test]
    fn parse_config_multiple_brokers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.jsonc");
        std::fs::write(
            &path,
            r#"{"kafka": {"brokers": ["kafka-1:9092", "kafka-2:9092", "kafka-3:9092"]}}"#,
        )
        .unwrap();

        let cfg = parse_config(&path).unwrap();
        assert_eq!(cfg.kafka_brokers.len(), 3);
    }

    #[test]
    fn parse_config_bootstrap_reconcile_interval() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("consumer.jsonc");
        std::fs::write(
            &path,
            r#"{
  "bootstrap": {
    "base_url": "http://server:8080",
    "reconcile_interval": "30s"
  }
}"#,
        )
        .unwrap();

        let cfg = parse_config(&path).unwrap();
        assert_eq!(cfg.bootstrap_reconcile_interval.as_deref(), Some("30s"));
    }
}
