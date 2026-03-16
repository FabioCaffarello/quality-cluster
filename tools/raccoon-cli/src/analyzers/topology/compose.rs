use crate::error::Result;
use std::collections::HashMap;
use std::path::Path;

/// Extracted compose service metadata.
#[derive(Debug, Clone, Default)]
pub struct ComposeService {
    #[allow(dead_code)]
    pub name: String,
    pub image: Option<String>,
    pub depends_on: Vec<String>,
    pub profiles: Vec<String>,
    pub ports: Vec<String>,
    pub internal_port: Option<String>,
}

/// Extracted compose topology.
#[derive(Debug, Clone, Default)]
pub struct ComposeTopology {
    pub services: HashMap<String, ComposeService>,
}

/// Parse docker-compose.yaml using line-by-line YAML parsing.
/// Minimal parser — handles the compose structure we need without a full YAML lib.
pub fn parse_compose(path: &Path) -> Result<ComposeTopology> {
    let content = std::fs::read_to_string(path)?;
    let mut topo = ComposeTopology::default();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut in_services = false;
    let mut current_service: Option<String> = None;
    let mut current_section: Option<String> = None;
    let mut service_indent: usize = 0;
    let mut section_indent: usize = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // Skip x- extension blocks
        if indent == 0 && trimmed.starts_with("x-") {
            let anchor_indent = indent;
            i += 1;
            while i < lines.len() {
                let next = lines[i].trim();
                let ni = lines[i].len() - lines[i].trim_start().len();
                if !next.is_empty()
                    && !next.starts_with('#')
                    && ni <= anchor_indent
                    && !next.starts_with(' ')
                {
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Top-level keys
        if indent == 0 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            in_services = trimmed == "services:";
            current_service = None;
            current_section = None;
            i += 1;
            continue;
        }

        if !in_services {
            i += 1;
            continue;
        }

        // If we have a current section, check if we're still in it
        if let (Some(ref svc_name), Some(ref section)) = (&current_service, &current_section) {
            if indent > section_indent {
                // We're inside the section — parse items
                let svc = topo.services.get_mut(svc_name.as_str()).unwrap();
                parse_section_item(section, trimmed, svc);
                i += 1;
                continue;
            } else {
                // Exited the section
                current_section = None;
                // Fall through to re-process this line
            }
        }

        // If we have a current service, check if we're still in it
        if let Some(ref _svc_name) = current_service {
            if indent <= service_indent {
                current_service = None;
                current_section = None;
                // Fall through to re-process
            }
        }

        // Detect service name
        if current_service.is_none() && in_services {
            if trimmed.ends_with(':') && !trimmed.starts_with('-') && !trimmed.starts_with("<<") {
                let name = trimmed.trim_end_matches(':').trim().to_string();
                service_indent = indent;
                topo.services
                    .entry(name.clone())
                    .or_insert_with(|| ComposeService {
                        name: name.clone(),
                        ..Default::default()
                    });
                current_service = Some(name);
                current_section = None;
                i += 1;
                continue;
            }
        }

        // Inside a service — detect section starts
        if current_service.is_some() && indent > service_indent {
            // Check for known section keys
            let key = if trimmed.ends_with(':') {
                Some(trimmed.trim_end_matches(':').trim())
            } else if trimmed.contains(": ") || trimmed.contains(":[") || trimmed.contains(": [") {
                Some(trimmed.splitn(2, ':').next().unwrap().trim())
            } else {
                None
            };

            if let Some(key) = key {
                match key {
                    "depends_on" | "ports" | "profiles" | "environment" => {
                        current_section = Some(key.to_string());
                        section_indent = indent;

                        // Handle inline values (profiles: ["a", "b"])
                        if let Some(rest) = trimmed.splitn(2, ':').nth(1) {
                            let rest = rest.trim();
                            if rest.starts_with('[') {
                                let svc_name = current_service.as_ref().unwrap();
                                let svc = topo.services.get_mut(svc_name.as_str()).unwrap();
                                parse_inline_list(key, rest, svc);
                                current_section = None;
                            }
                        }

                        i += 1;
                        continue;
                    }
                    "image" => {
                        if let Some(rest) = trimmed.split_once(':').map(|(_, value)| value.trim()) {
                            let svc_name = current_service.as_ref().unwrap();
                            let svc = topo.services.get_mut(svc_name.as_str()).unwrap();
                            let image = rest.trim_matches('"').trim_matches('\'');
                            if !image.is_empty() {
                                svc.image = Some(image.to_string());
                            }
                        }

                        i += 1;
                        continue;
                    }
                    _ => {}
                }
            }
        }

        i += 1;
    }

    Ok(topo)
}

fn parse_section_item(section: &str, trimmed: &str, svc: &mut ComposeService) {
    match section {
        "depends_on" => {
            // Map form: "nats:" or "nats:\n  condition: ..."
            if trimmed.ends_with(':') && !trimmed.starts_with("condition") {
                let dep = trimmed.trim_end_matches(':').trim();
                if !dep.is_empty() {
                    svc.depends_on.push(dep.to_string());
                }
            }
            // List form: "- nats"
            if trimmed.starts_with('-') {
                let dep = trimmed.trim_start_matches('-').trim().trim_end_matches(':');
                if !dep.is_empty() && dep != "condition" {
                    svc.depends_on.push(dep.to_string());
                }
            }
            // Skip "condition: service_healthy" and similar
        }
        "ports" => {
            if trimmed.starts_with('-') {
                let port = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !port.is_empty() {
                    svc.ports.push(port.to_string());
                }
            }
        }
        "profiles" => {
            if trimmed.starts_with('-') {
                let profile = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !profile.is_empty() {
                    svc.profiles.push(profile.to_string());
                }
            }
            if trimmed.starts_with('[') {
                parse_inline_list("profiles", trimmed, svc);
            }
        }
        "environment" => {
            if let Some(rest) = trimmed.strip_prefix("KAFKA_CFG_LISTENERS:") {
                let listeners = rest.trim().trim_matches('"');
                for listener in listeners.split(',') {
                    if listener.trim().starts_with("PLAINTEXT://") {
                        if let Some(port) = listener.trim().rsplit(':').next() {
                            svc.internal_port = Some(port.to_string());
                            break;
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_inline_list(section: &str, value: &str, svc: &mut ComposeService) {
    let inner = value.trim_start_matches('[').trim_end_matches(']');
    for item in inner.split(',') {
        let item = item.trim().trim_matches('"').trim_matches('\'');
        if !item.is_empty() {
            match section {
                "profiles" => svc.profiles.push(item.to_string()),
                "ports" => svc.ports.push(item.to_string()),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_compose(dir: &Path, content: &str) -> std::path::PathBuf {
        let path = dir.join("docker-compose.yaml");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parse_minimal_compose() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  nats:
    image: nats:2.10
    ports:
      - "4222:4222"
  consumer:
    image: quality-service/consumer:dev
    depends_on:
      nats:
        condition: service_healthy
      kafka:
        condition: service_healthy
    profiles: ["dataplane", "all"]
"#,
        );

        let topo = parse_compose(&path).unwrap();
        assert!(topo.services.contains_key("nats"));
        assert!(topo.services.contains_key("consumer"));

        let consumer = &topo.services["consumer"];
        assert!(consumer.depends_on.contains(&"nats".to_string()));
        assert!(consumer.depends_on.contains(&"kafka".to_string()));
        assert!(consumer.profiles.contains(&"dataplane".to_string()));
        assert!(consumer.profiles.contains(&"all".to_string()));
        assert_eq!(
            consumer.image.as_deref(),
            Some("quality-service/consumer:dev")
        );
    }

    #[test]
    fn parse_compose_with_all_services() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  nats:
    image: nats
  kafka:
    image: kafka
  configctl:
    image: configctl
    depends_on:
      nats:
        condition: service_healthy
  server:
    image: server
    depends_on:
      nats:
        condition: service_healthy
      configctl:
        condition: service_healthy
  consumer:
    image: consumer
    depends_on:
      nats:
        condition: service_healthy
      server:
        condition: service_healthy
      kafka:
        condition: service_healthy
  emulator:
    image: emulator
    depends_on:
      server:
        condition: service_healthy
      kafka:
        condition: service_healthy
      consumer:
        condition: service_healthy
      validator:
        condition: service_healthy
  validator:
    image: validator
    depends_on:
      nats:
        condition: service_healthy
      configctl:
        condition: service_healthy
networks:
  default:
    driver: bridge
"#,
        );

        let topo = parse_compose(&path).unwrap();
        assert_eq!(topo.services.len(), 7);

        let emulator = &topo.services["emulator"];
        assert_eq!(emulator.depends_on.len(), 4);
        assert!(emulator.depends_on.contains(&"validator".to_string()));
    }

    #[test]
    fn parse_compose_handles_ports() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  nats:
    image: nats
    ports:
      - "127.0.0.1:4222:4222"
      - "127.0.0.1:8222:8222"
"#,
        );

        let topo = parse_compose(&path).unwrap();
        let nats = &topo.services["nats"];
        assert_eq!(nats.ports.len(), 2);
    }

    #[test]
    fn parse_compose_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(dir.path(), "");
        let topo = parse_compose(&path).unwrap();
        assert!(topo.services.is_empty());
    }

    #[test]
    fn parse_compose_services_only_no_depends() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  standalone:
    image: standalone:latest
"#,
        );
        let topo = parse_compose(&path).unwrap();
        assert!(topo.services.contains_key("standalone"));
        assert!(topo.services["standalone"].depends_on.is_empty());
    }

    #[test]
    fn parse_compose_list_form_depends_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  app:
    image: app:latest
    depends_on:
      - nats
      - kafka
  nats:
    image: nats
  kafka:
    image: kafka
"#,
        );
        let topo = parse_compose(&path).unwrap();
        let app = &topo.services["app"];
        assert!(app.depends_on.contains(&"nats".to_string()));
        assert!(app.depends_on.contains(&"kafka".to_string()));
    }

    #[test]
    fn parse_compose_skips_x_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"x-common: &common
  restart: unless-stopped
  logging:
    driver: json-file

services:
  svc:
    image: svc:latest
"#,
        );
        let topo = parse_compose(&path).unwrap();
        assert_eq!(topo.services.len(), 1);
        assert!(topo.services.contains_key("svc"));
    }

    #[test]
    fn parse_compose_skips_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"# Top-level comment
services:
  # Service comment
  svc:
    image: svc:latest
    # depends_on comment:
"#,
        );
        let topo = parse_compose(&path).unwrap();
        assert_eq!(topo.services.len(), 1);
    }

    #[test]
    fn parse_compose_extracts_environment_kafka_port() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  kafka:
    image: bitnami/kafka
    environment:
      KAFKA_CFG_LISTENERS: "PLAINTEXT://:9092,CONTROLLER://:9093"
"#,
        );
        let topo = parse_compose(&path).unwrap();
        let kafka = &topo.services["kafka"];
        assert_eq!(kafka.internal_port.as_deref(), Some("9092"));
    }

    #[test]
    fn parse_compose_extracts_image_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  kafka:
    image: bitnamilegacy/kafka:3.9.0
  nats:
    image: "nats:2.10.18-alpine"
"#,
        );
        let topo = parse_compose(&path).unwrap();
        assert_eq!(
            topo.services["kafka"].image.as_deref(),
            Some("bitnamilegacy/kafka:3.9.0")
        );
        assert_eq!(
            topo.services["nats"].image.as_deref(),
            Some("nats:2.10.18-alpine")
        );
    }

    #[test]
    fn parse_compose_multiple_top_level_sections() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_compose(
            dir.path(),
            r#"services:
  svc:
    image: svc
networks:
  default:
    driver: bridge
volumes:
  data:
"#,
        );
        let topo = parse_compose(&path).unwrap();
        assert_eq!(topo.services.len(), 1);
        assert!(topo.services.contains_key("svc"));
    }
}
