use crate::error::Result;
use crate::models::{CheckResult, Finding, Report};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::topology::{self, ComposeTopology, ServiceConfig, SourceTopology};

// ── Public API ──────────────────────────────────────────────────────

pub fn analyze(project_root: &Path) -> Result<Report> {
    let mut report = Report::new("drift-detect");

    // Phase 1: Gather all evidence
    let evidence = gather_evidence(project_root)?;

    // Phase 2: Run drift checks
    report.add(check_config_compose_drift(&evidence));
    report.add(check_config_source_drift(&evidence));
    report.add(check_binding_topology_drift(&evidence));
    report.add(check_workflow_drift(&evidence));
    report.add(check_contract_domain_drift(&evidence));
    report.add(check_compose_profile_drift(&evidence));

    Ok(report)
}

// ── Evidence gathering ──────────────────────────────────────────────

struct Evidence {
    configs: HashMap<String, ServiceConfig>,
    compose: Option<ComposeTopology>,
    source: Option<SourceTopology>,
    makefile_targets: HashSet<String>,
    dev_doc_targets: HashSet<String>,
    dev_doc_cli_commands: HashSet<String>,
    cli_subcommands: HashSet<String>,
    domain_events: Vec<String>,
    registry_events: Vec<String>,
    config_bindings: Vec<ConfigBinding>,
}

#[derive(Debug, Clone)]
struct ConfigBinding {
    name: String,
    topic: String,
    source_file: String,
}

fn gather_evidence(project_root: &Path) -> Result<Evidence> {
    let mut evidence = Evidence {
        configs: HashMap::new(),
        compose: None,
        source: None,
        makefile_targets: HashSet::new(),
        dev_doc_targets: HashSet::new(),
        dev_doc_cli_commands: HashSet::new(),
        cli_subcommands: known_cli_subcommands(),
        domain_events: Vec::new(),
        registry_events: Vec::new(),
        config_bindings: Vec::new(),
    };

    // Configs
    let configs_dir = project_root.join("deploy/configs");
    if configs_dir.is_dir() {
        evidence.configs = topology::configs::parse_all_configs(&configs_dir)?;
    }

    // Compose
    let compose_path = project_root.join("deploy/compose/docker-compose.yaml");
    if compose_path.is_file() {
        evidence.compose = topology::compose::parse_compose(&compose_path).ok();
    }

    // Source
    let internal_dir = project_root.join("internal");
    if internal_dir.is_dir() {
        evidence.source = topology::source::scan_source(&internal_dir).ok();
        scan_domain_events(&internal_dir, &mut evidence.domain_events);
        scan_registry_events(&internal_dir, &mut evidence.registry_events);
        scan_config_bindings(&internal_dir, &mut evidence.config_bindings, project_root);
    }

    // Makefile
    let makefile_path = project_root.join("Makefile");
    if makefile_path.is_file() {
        evidence.makefile_targets = extract_makefile_targets(&makefile_path);
    }

    // DEVELOPMENT.md
    let dev_doc_path = project_root.join("DEVELOPMENT.md");
    if dev_doc_path.is_file() {
        let (targets, commands) = extract_dev_doc_references(&dev_doc_path);
        evidence.dev_doc_targets = targets;
        evidence.dev_doc_cli_commands = commands;
    }

    Ok(evidence)
}

fn known_cli_subcommands() -> HashSet<String> {
    [
        "doctor",
        "topology-doctor",
        "contract-audit",
        "runtime-bindings",
        "arch-guard",
        "runtime-smoke",
        "scenario-smoke",
        "results-inspect",
        "quality-gate",
        "trace-pack",
        "drift-detect",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

// ── Scanners ────────────────────────────────────────────────────────

fn scan_domain_events(internal_dir: &Path, events: &mut Vec<String>) {
    let domain_dir = internal_dir.join("domain");
    if !domain_dir.is_dir() {
        return;
    }
    scan_go_files_for_events(&domain_dir, events);
}

fn scan_go_files_for_events(dir: &Path, events: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_go_files_for_events(&path, events);
        } else if path.extension().and_then(|e| e.to_str()) == Some("go") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                extract_event_names(&content, events);
            }
        }
    }
}

fn extract_event_names(content: &str, events: &mut Vec<String>) {
    // Match patterns like: "config.draft_created" or "config.activated"
    // Only extract from lines that look like event name assignments, not struct tags
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        // Only consider lines in event-related context:
        // - Contains EventName, event_name, Type:, or Name: in an event context
        // - Or is near (within 5 lines of) "Event" or "event" keyword
        let is_event_context = trimmed.contains("EventName")
            || trimmed.contains("event_name")
            || (trimmed.contains("Name") && nearby_contains(&lines, i, 5, "Event"));

        if !is_event_context {
            continue;
        }

        for val in extract_all_quoted(trimmed) {
            if is_event_name(&val) {
                events.push(val);
            }
        }
    }
    events.sort();
    events.dedup();
}

fn nearby_contains(lines: &[&str], center: usize, radius: usize, keyword: &str) -> bool {
    let start = center.saturating_sub(radius);
    let end = (center + radius).min(lines.len());
    for i in start..end {
        if lines[i].contains(keyword) {
            return true;
        }
    }
    false
}

fn is_event_name(s: &str) -> bool {
    if s.is_empty() || !s.contains('.') {
        return false;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 {
        return false;
    }
    // Must be word.word_word style (e.g., config.draft_created)
    // The second part must contain an underscore (verb_noun pattern) to distinguish
    // from struct field access like "scope.key" or "artifact.id"
    let domain = parts[0];
    let action = parts[1];

    if domain.is_empty() || action.is_empty() {
        return false;
    }

    // Domain must be a known event domain or at least look like one
    let valid_chars = |p: &str| {
        p.chars()
            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
    };

    if !valid_chars(domain) || !valid_chars(action) {
        return false;
    }

    // Action part must contain underscore (e.g., draft_created, not just "key" or "id")
    // OR be a known lifecycle verb
    let known_verbs = [
        "activated",
        "deactivated",
        "compiled",
        "validated",
        "rejected",
        "archived",
    ];
    action.contains('_') || known_verbs.contains(&action)
}

fn scan_registry_events(internal_dir: &Path, events: &mut Vec<String>) {
    let adapters_dir = internal_dir.join("adapters");
    if !adapters_dir.is_dir() {
        return;
    }
    scan_registry_files(&adapters_dir, events);
    events.sort();
    events.dedup();
}

fn scan_registry_files(dir: &Path, events: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_registry_files(&path, events);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.contains("registry") && name.ends_with(".go") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // Extract event type values from registry specs
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with("//") {
                            continue;
                        }
                        if trimmed.contains("Type:") || trimmed.contains("type:") {
                            for val in extract_all_quoted(trimmed) {
                                if is_event_name(&val) {
                                    events.push(val);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn scan_config_bindings(
    internal_dir: &Path,
    bindings: &mut Vec<ConfigBinding>,
    project_root: &Path,
) {
    // Scan test fixtures for binding examples
    let tests_dir = project_root.join("tests/http");
    if tests_dir.is_dir() {
        scan_http_fixtures_for_bindings(&tests_dir, bindings);
    }
    // Scan deploy configs for binding declarations
    let configs_dir = project_root.join("deploy/configs");
    if configs_dir.is_dir() {
        scan_configs_for_bindings(&configs_dir, bindings);
    }
    // Scan source for binding references
    scan_source_for_bindings(internal_dir, bindings);
}

fn scan_http_fixtures_for_bindings(dir: &Path, bindings: &mut Vec<ConfigBinding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("http") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                extract_bindings_from_json_content(&content, &path, bindings);
            }
        }
    }
}

fn scan_configs_for_bindings(dir: &Path, bindings: &mut Vec<ConfigBinding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonc") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                extract_bindings_from_json_content(&content, &path, bindings);
            }
        }
    }
}

fn scan_source_for_bindings(internal_dir: &Path, bindings: &mut Vec<ConfigBinding>) {
    scan_go_for_bindings(internal_dir, bindings);
}

fn scan_go_for_bindings(dir: &Path, bindings: &mut Vec<ConfigBinding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_go_for_bindings(&path, bindings);
        } else if path.extension().and_then(|e| e.to_str()) == Some("go") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                // Look for binding name references in test or fixture code
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    if trimmed.contains("BindingName") || trimmed.contains("binding_name") {
                        for val in extract_all_quoted(trimmed) {
                            if !val.is_empty()
                                && val.len() < 64
                                && val
                                    .chars()
                                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                            {
                                bindings.push(ConfigBinding {
                                    name: val,
                                    topic: String::new(),
                                    source_file: path.display().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_bindings_from_json_content(
    content: &str,
    path: &Path,
    bindings: &mut Vec<ConfigBinding>,
) {
    // Look for "bindings" arrays with "name" and "topic" fields
    // Simple heuristic: find "name": "..." and "topic": "..." near each other
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim().trim_matches('"');
        if trimmed.contains("\"name\"") && i + 5 < lines.len() {
            let name = extract_json_string_value(trimmed, "name");
            if let Some(name) = name {
                // Look for topic nearby
                let mut topic = None;
                for j in i.saturating_sub(3)..((i + 5).min(lines.len())) {
                    if let Some(t) = extract_json_string_value(lines[j].trim(), "topic") {
                        topic = Some(t);
                        break;
                    }
                }
                if let Some(topic) = topic {
                    bindings.push(ConfigBinding {
                        name,
                        topic,
                        source_file: path.display().to_string(),
                    });
                }
            }
        }
    }
}

fn extract_json_string_value(line: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if !line.contains(&pattern) {
        return None;
    }
    // Find value after the key
    let after_key = line.splitn(2, &pattern).nth(1)?;
    let after_colon = after_key.splitn(2, ':').nth(1)?.trim();
    // Extract quoted value
    let start = after_colon.find('"')?;
    let rest = &after_colon[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_makefile_targets(path: &Path) -> HashSet<String> {
    let mut targets = HashSet::new();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return targets,
    };

    for line in content.lines() {
        // Match target declarations: "target:" or "target: deps"
        // Skip variable assignments, comments, conditionals
        let trimmed = line.trim();
        if trimmed.starts_with('#')
            || trimmed.starts_with('\t')
            || trimmed.starts_with(' ')
            || trimmed.starts_with('@')
            || trimmed.starts_with("define ")
            || trimmed.starts_with("endef")
            || trimmed.starts_with("ifeq")
            || trimmed.starts_with("ifneq")
            || trimmed.starts_with("endif")
            || trimmed.starts_with("else")
            || trimmed.is_empty()
        {
            continue;
        }
        // Skip variable assignments
        if trimmed.contains("?=")
            || trimmed.contains(":=")
            || trimmed.contains("+=")
            || (trimmed.contains('=') && !trimmed.contains(':'))
        {
            continue;
        }
        // Skip .PHONY and .DEFAULT_GOAL
        if trimmed.starts_with('.') {
            continue;
        }
        // Skip lines with $(...)
        if trimmed.starts_with("$(") {
            continue;
        }
        // Extract target name
        if let Some(colon_pos) = trimmed.find(':') {
            let target = trimmed[..colon_pos].trim();
            if !target.is_empty()
                && !target.contains(' ')
                && !target.contains('$')
                && !target.contains('/')
            {
                targets.insert(target.to_string());
            }
        }
    }

    targets
}

fn extract_dev_doc_references(path: &Path) -> (HashSet<String>, HashSet<String>) {
    let mut targets = HashSet::new();
    let mut commands = HashSet::new();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (targets, commands),
    };

    // Common English words that follow "make" but aren't targets
    let make_stopwords: HashSet<&str> = [
        "a", "an", "the", "it", "is", "to", "your", "sure", "changes",
        "sense", "this", "that", "any", "no", "not", "use", "certain",
    ]
    .iter()
    .copied()
    .collect();

    for line in content.lines() {
        // Extract `make <target>` references
        let mut rest = line;
        while let Some(pos) = rest.find("make ") {
            let after = &rest[pos + 5..];
            let end = after
                .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(after.len());
            let target = &after[..end];
            if !target.is_empty() && !make_stopwords.contains(target) {
                targets.insert(target.to_string());
            }
            rest = &after[end..];
        }

        // Extract `raccoon-cli <subcommand>` references
        let mut rest = line;
        while let Some(pos) = rest.find("raccoon-cli ") {
            let after = &rest[pos + 12..];
            // Skip flags
            let after = after.trim_start();
            let cmd_start = if after.starts_with('-') {
                // Find next non-flag word
                let words: Vec<&str> = after.split_whitespace().collect();
                let mut cmd = "";
                for w in &words {
                    if !w.starts_with('-') {
                        cmd = w;
                        break;
                    }
                }
                cmd
            } else {
                let end = after
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(after.len());
                &after[..end]
            };
            // Strip trailing backticks or punctuation
            let cmd_clean = cmd_start.trim_end_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == ',' || c == '.');
            if !cmd_clean.is_empty() {
                commands.insert(cmd_clean.to_string());
            }
            rest = &after[cmd_start.len()..];
        }
    }

    (targets, commands)
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

// ── Drift checks ────────────────────────────────────────────────────

fn check_config_compose_drift(evidence: &Evidence) -> CheckResult {
    let mut findings = Vec::new();

    let compose = match &evidence.compose {
        Some(c) => c,
        None => {
            return CheckResult::skip(
                "config-compose-drift",
                "docker-compose.yaml not available",
            )
        }
    };

    if evidence.configs.is_empty() {
        return CheckResult::skip(
            "config-compose-drift",
            "no configs found in deploy/configs/",
        );
    }

    // Services with configs but not in compose
    let compose_services: HashSet<&str> = compose.services.keys().map(|s| s.as_str()).collect();
    let config_services: HashSet<&str> = evidence.configs.keys().map(|s| s.as_str()).collect();

    // Configs are expected for application services, not infra (nats, kafka)
    let app_services: HashSet<&str> = ["configctl", "server", "consumer", "emulator", "validator"]
        .iter()
        .copied()
        .collect();

    for svc in config_services.difference(&compose_services) {
        if app_services.contains(*svc) {
            findings.push(
                Finding::warning(
                    "config-without-compose",
                    format!("config '{svc}' exists but no matching compose service"),
                )
                .with_why("config declares runtime settings for a service that doesn't exist in compose — the config is dead weight")
                .with_help(format!("add '{svc}' service to deploy/compose/docker-compose.yaml or remove deploy/configs/{svc}.jsonc")),
            );
        }
    }

    // App services in compose without config
    for svc in compose_services.intersection(&app_services) {
        if !config_services.contains(*svc) {
            findings.push(
                Finding::warning(
                    "compose-without-config",
                    format!("compose service '{svc}' has no deploy/configs/{svc}.jsonc"),
                )
                .with_why("service runs with default/hardcoded settings — explicit config makes behavior visible and auditable")
                .with_help(format!("create deploy/configs/{svc}.jsonc with at minimum the transport settings")),
            );
        }
    }

    // Transport drift: config declares kafka but compose service doesn't depend on kafka
    for (name, cfg) in &evidence.configs {
        if let Some(svc) = compose.services.get(name.as_str()) {
            if !cfg.kafka_brokers.is_empty() && !svc.depends_on.contains(&"kafka".to_string()) {
                findings.push(
                    Finding::error(
                        "transport-drift",
                        format!("'{name}' config declares kafka brokers but compose service doesn't depend on kafka"),
                    )
                    .with_why("service will fail to start if kafka isn't running — the dependency must be declared")
                    .with_help(format!("add 'kafka' to depends_on of '{name}' in docker-compose.yaml")),
                );
            }
            if cfg.nats_url.is_some() && !svc.depends_on.contains(&"nats".to_string()) {
                findings.push(
                    Finding::error(
                        "transport-drift",
                        format!("'{name}' config declares nats url but compose service doesn't depend on nats"),
                    )
                    .with_why("service will fail to connect if nats isn't running — the dependency must be declared")
                    .with_help(format!("add 'nats' to depends_on of '{name}' in docker-compose.yaml")),
                );
            }
        }
    }

    CheckResult::from_findings("config-compose-drift", findings)
}

fn check_config_source_drift(evidence: &Evidence) -> CheckResult {
    let mut findings = Vec::new();

    let source = match &evidence.source {
        Some(s) => s,
        None => return CheckResult::skip("config-source-drift", "source not scanned"),
    };

    // Check that streams referenced in source exist as expected transport infrastructure
    let expected_streams: Vec<&str> = vec!["DATA_PLANE_INGESTION", "CONFIGCTL_EVENTS"];
    for stream in &expected_streams {
        if !source.streams.contains_key(*stream) {
            findings.push(
                Finding::error(
                    "stream-drift",
                    format!("expected stream '{stream}' not found in source — may have been renamed or removed"),
                )
                .with_why("durable consumers and subject routing depend on this stream name — renaming it without updating all references breaks the pipeline")
                .with_help("search source for the stream constant and verify it matches the registry definition"),
            );
        }
    }

    // Check that durable consumers still target the right streams
    for (durable, stream) in &source.durables {
        if !source.streams.contains_key(stream.as_str()) {
            findings.push(
                Finding::error(
                    "durable-target-drift",
                    format!("durable '{durable}' targets stream '{stream}' which doesn't exist"),
                )
                .with_why("durable consumer will fail to bind at runtime — messages will not be delivered")
                .with_help(format!("update durable '{durable}' to reference an existing stream")),
            );
        }
    }

    // Check that subject prefixes in configs align with source subject patterns
    let dataplane_prefix = "dataplane.ingestion.received";
    let has_dataplane_subjects = source.subjects.iter().any(|s| s.starts_with(dataplane_prefix));

    if !has_dataplane_subjects && !source.streams.is_empty() {
        findings.push(
            Finding::warning(
                "subject-prefix-drift",
                format!("no subjects with prefix '{dataplane_prefix}' found in source — subject naming may have drifted"),
            )
            .with_why("consumer and validator depend on predictable subject prefixes for routing")
            .with_help("check dataplane registry for current subject prefix convention"),
        );
    }

    // Verify stream-subject alignment (stream declares subjects that actually exist)
    for (stream_name, stream_subjects) in &source.streams {
        for pattern in stream_subjects {
            let prefix = pattern.trim_end_matches(".>");
            let has_matching = source.subjects.iter().any(|s| s.starts_with(prefix) || s == pattern);
            if !has_matching {
                findings.push(
                    Finding::warning(
                        "stream-subject-drift",
                        format!("stream '{stream_name}' declares subject pattern '{pattern}' but no matching concrete subjects found"),
                    )
                    .with_why("stream may be capturing zero messages if no publisher uses this subject pattern")
                    .with_help(format!("verify publishers emit to subjects matching '{pattern}'")),
                );
            }
        }
    }

    CheckResult::from_findings("config-source-drift", findings)
}

fn check_binding_topology_drift(evidence: &Evidence) -> CheckResult {
    let mut findings = Vec::new();

    let source = match &evidence.source {
        Some(s) => s,
        None => return CheckResult::skip("binding-topology-drift", "source not scanned"),
    };

    // Check that discovered bindings reference topics that align with the routing infrastructure
    let binding_names: HashSet<String> = evidence
        .config_bindings
        .iter()
        .map(|b| b.name.clone())
        .collect();

    let binding_topics: HashSet<String> = evidence
        .config_bindings
        .iter()
        .filter(|b| !b.topic.is_empty())
        .map(|b| b.topic.clone())
        .collect();

    // If we have bindings with topics, verify the routing infrastructure supports them
    if !binding_topics.is_empty() {
        let has_dataplane_stream = source.streams.contains_key("DATA_PLANE_INGESTION");
        if !has_dataplane_stream {
            findings.push(
                Finding::error(
                    "binding-stream-drift",
                    format!(
                        "bindings reference {} topic(s) but DATA_PLANE_INGESTION stream not found in source",
                        binding_topics.len()
                    ),
                )
                .with_why("ingested messages from these topics will have no stream to land in — data pipeline is broken")
                .with_help("verify the dataplane registry defines DATA_PLANE_INGESTION stream"),
            );
        }

        let has_validator_durable = source.durables.contains_key("validator-dataplane-v1");
        if !has_validator_durable {
            findings.push(
                Finding::error(
                    "binding-consumer-drift",
                    format!(
                        "bindings reference {} topic(s) but validator durable consumer not found",
                        binding_topics.len()
                    ),
                )
                .with_why("messages will accumulate in the stream with no consumer processing them")
                .with_help("verify the dataplane registry defines the validator durable consumer"),
            );
        }
    }

    // Check for duplicate binding names across sources
    let mut seen_names: HashMap<&str, Vec<&str>> = HashMap::new();
    for b in &evidence.config_bindings {
        seen_names
            .entry(b.name.as_str())
            .or_default()
            .push(b.source_file.as_str());
    }
    for (name, sources) in &seen_names {
        if sources.len() > 2 {
            // Allow config + test fixture duplicates, flag if more
            let unique_sources: HashSet<&&str> = sources.iter().collect();
            if unique_sources.len() > 2 {
                findings.push(
                    Finding::warning(
                        "binding-duplicate",
                        format!(
                            "binding '{name}' appears in {} distinct sources",
                            unique_sources.len()
                        ),
                    )
                    .with_why("duplicate binding definitions risk inconsistency if one is updated and the others aren't")
                    .with_help("consolidate to a single authoritative binding declaration"),
                );
            }
        }
    }

    // Check bootstrap infrastructure exists
    if !binding_names.is_empty() {
        let has_bootstrap = source.subjects.iter().any(|s| s.contains("bootstrap") || s.contains("runtime"));
        if !has_bootstrap {
            findings.push(
                Finding::info(
                    "binding-bootstrap",
                    "bindings exist but no bootstrap-related subjects found in source — bootstrap may use HTTP instead of NATS",
                ),
            );
        }
    }

    CheckResult::from_findings("binding-topology-drift", findings)
}

fn check_workflow_drift(evidence: &Evidence) -> CheckResult {
    let mut findings = Vec::new();

    if evidence.makefile_targets.is_empty() && evidence.dev_doc_targets.is_empty() {
        return CheckResult::skip("workflow-drift", "no Makefile or DEVELOPMENT.md found");
    }

    // Targets referenced in DEVELOPMENT.md but not in Makefile
    for target in &evidence.dev_doc_targets {
        if !evidence.makefile_targets.contains(target) {
            // Skip common false positives
            if ["test", "build", "docker-build"].contains(&target.as_str()) {
                // These might be parsed from different context
                continue;
            }
            findings.push(
                Finding::error(
                    "doc-target-drift",
                    format!("DEVELOPMENT.md references `make {target}` but target not found in Makefile"),
                )
                .with_why("developers following the documented workflow will get 'No rule to make target' errors")
                .with_help(format!("add '{target}' target to Makefile or update DEVELOPMENT.md")),
            );
        }
    }

    // CLI commands referenced in DEVELOPMENT.md but not known subcommands
    for cmd in &evidence.dev_doc_cli_commands {
        if !evidence.cli_subcommands.contains(cmd) {
            // Skip flags and noise
            if cmd.starts_with('-') || cmd.len() < 3 {
                continue;
            }
            findings.push(
                Finding::warning(
                    "doc-command-drift",
                    format!("DEVELOPMENT.md references `raccoon-cli {cmd}` which is not a known subcommand"),
                )
                .with_why("developers will get CLI parse errors following the documentation")
                .with_help(format!("update DEVELOPMENT.md to use a valid raccoon-cli subcommand")),
            );
        }
    }

    // Makefile workflow targets that should be documented
    let workflow_targets = ["check", "verify", "check-deep", "smoke", "trace-pack", "results-inspect"];
    for target in &workflow_targets {
        if evidence.makefile_targets.contains(*target)
            && !evidence.dev_doc_targets.contains(*target)
        {
            findings.push(
                Finding::warning(
                    "undocumented-target",
                    format!("Makefile has workflow target '{target}' not referenced in DEVELOPMENT.md"),
                )
                .with_why("developers won't discover this workflow step from the documentation")
                .with_help(format!("add `make {target}` to the DEVELOPMENT.md workflow section")),
            );
        }
    }

    CheckResult::from_findings("workflow-drift", findings)
}

fn check_contract_domain_drift(evidence: &Evidence) -> CheckResult {
    let mut findings = Vec::new();

    if evidence.domain_events.is_empty() && evidence.registry_events.is_empty() {
        return CheckResult::skip(
            "contract-domain-drift",
            "no domain events or registry events found",
        );
    }

    let domain_set: HashSet<&str> = evidence.domain_events.iter().map(|s| s.as_str()).collect();
    let registry_set: HashSet<&str> = evidence
        .registry_events
        .iter()
        .map(|s| s.as_str())
        .collect();

    // Domain events not in registry (declared but never published)
    for event in domain_set.difference(&registry_set) {
        findings.push(
            Finding::warning(
                "domain-event-unregistered",
                format!("domain event '{event}' has no matching registry spec"),
            )
            .with_why("event is defined but never wired to a transport — it cannot be consumed by other services")
            .with_help("add a matching EventSpec to the adapter registry"),
        );
    }

    // Registry events not in domain (transport spec without domain event)
    for event in registry_set.difference(&domain_set) {
        findings.push(
            Finding::warning(
                "registry-event-orphan",
                format!("registry spec for '{event}' has no matching domain event definition"),
            )
            .with_why("transport is wired for an event that no domain code produces — spec may be stale")
            .with_help("verify the domain event exists or remove the stale registry spec"),
        );
    }

    CheckResult::from_findings("contract-domain-drift", findings)
}

fn check_compose_profile_drift(evidence: &Evidence) -> CheckResult {
    let mut findings = Vec::new();

    let compose = match &evidence.compose {
        Some(c) => c,
        None => return CheckResult::skip("compose-profile-drift", "docker-compose.yaml not available"),
    };

    // Collect all profiles
    let mut profiles_per_service: Vec<(&str, &Vec<String>)> = Vec::new();
    let mut all_profiles: HashSet<String> = HashSet::new();

    for (name, svc) in &compose.services {
        if !svc.profiles.is_empty() {
            profiles_per_service.push((name.as_str(), &svc.profiles));
            for p in &svc.profiles {
                all_profiles.insert(p.clone());
            }
        }
    }

    // Check for services without any profile assignment (they run in all profiles)
    let infra_services: HashSet<&str> = ["nats", "kafka"].iter().copied().collect();
    for (name, svc) in &compose.services {
        if svc.profiles.is_empty() && !infra_services.contains(name.as_str()) {
            // This is not necessarily drift, but worth noting
            findings.push(
                Finding::info(
                    "profile-unassigned",
                    format!("compose service '{name}' has no profile assignment — runs in all profiles"),
                ),
            );
        }
    }

    // Check that Makefile profile targets (up-core, up-runtime, up-dataplane) reference valid profiles
    let expected_profiles = ["core", "runtime", "dataplane", "all"];
    for profile in &expected_profiles {
        if !all_profiles.contains(*profile) && !all_profiles.is_empty() {
            // Only warn if compose uses profiles at all but this one is missing
            findings.push(
                Finding::warning(
                    "missing-profile",
                    format!("expected compose profile '{profile}' not found in any service"),
                )
                .with_why(format!("Makefile target 'up-{0}' uses --profile {0} which won't match any service", profile))
                .with_help(format!("assign profile '{profile}' to relevant services in docker-compose.yaml")),
            );
        }
    }

    CheckResult::from_findings("compose-profile-drift", findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CheckStatus, Severity};

    // ── Helper builders ─────────────────────────────────────────────

    fn make_consumer_config() -> ServiceConfig {
        ServiceConfig {
            name: "consumer".into(),
            kafka_brokers: vec!["kafka:9092".into()],
            kafka_consumer_group: Some("quality-service-consumer-v1".into()),
            kafka_client_id: Some("quality-service-consumer".into()),
            nats_url: Some("nats://nats:4222".into()),
            bootstrap_base_url: Some("http://server:8080".into()),
        }
    }

    fn make_emulator_config() -> ServiceConfig {
        ServiceConfig {
            name: "emulator".into(),
            kafka_brokers: vec!["kafka:9092".into()],
            kafka_consumer_group: None,
            kafka_client_id: Some("quality-service-emulator".into()),
            nats_url: None,
            bootstrap_base_url: Some("http://server:8080".into()),
        }
    }

    fn make_validator_config() -> ServiceConfig {
        ServiceConfig {
            name: "validator".into(),
            kafka_brokers: vec![],
            kafka_consumer_group: None,
            kafka_client_id: None,
            nats_url: Some("nats://nats:4222".into()),
            bootstrap_base_url: None,
        }
    }

    fn make_source_topology() -> SourceTopology {
        let mut streams = HashMap::new();
        streams.insert(
            "DATA_PLANE_INGESTION".into(),
            vec!["dataplane.ingestion.received.>".into()],
        );
        streams.insert(
            "CONFIGCTL_EVENTS".into(),
            vec!["configctl.events.config.>".into()],
        );

        let mut durables = HashMap::new();
        durables.insert(
            "validator-dataplane-v1".into(),
            "DATA_PLANE_INGESTION".into(),
        );
        durables.insert(
            "validator-runtime-cache-v1".into(),
            "CONFIGCTL_EVENTS".into(),
        );

        let subjects = vec![
            "dataplane.ingestion.received.>".into(),
            "configctl.events.config.>".into(),
            "configctl.events.config.activated".into(),
            "configctl.control.create_draft".into(),
        ];

        SourceTopology {
            streams,
            durables,
            subjects,
        }
    }

    fn make_compose_topology() -> ComposeTopology {
        use topology::compose::ComposeService;
        let mut services = HashMap::new();

        services.insert(
            "nats".into(),
            ComposeService {
                name: "nats".into(),
                depends_on: vec![],
                profiles: vec!["core".into(), "all".into()],
                ports: vec!["4222:4222".into()],
                internal_port: None,
            },
        );
        services.insert(
            "kafka".into(),
            ComposeService {
                name: "kafka".into(),
                depends_on: vec![],
                profiles: vec!["dataplane".into(), "all".into()],
                ports: vec![],
                internal_port: Some("9092".into()),
            },
        );
        services.insert(
            "configctl".into(),
            ComposeService {
                name: "configctl".into(),
                depends_on: vec!["nats".into()],
                profiles: vec!["core".into(), "all".into()],
                ports: vec![],
                internal_port: None,
            },
        );
        services.insert(
            "server".into(),
            ComposeService {
                name: "server".into(),
                depends_on: vec!["nats".into(), "configctl".into()],
                profiles: vec!["core".into(), "all".into()],
                ports: vec!["8080:8080".into()],
                internal_port: None,
            },
        );
        services.insert(
            "consumer".into(),
            ComposeService {
                name: "consumer".into(),
                depends_on: vec!["nats".into(), "server".into(), "kafka".into()],
                profiles: vec!["dataplane".into(), "all".into()],
                ports: vec![],
                internal_port: None,
            },
        );
        services.insert(
            "emulator".into(),
            ComposeService {
                name: "emulator".into(),
                depends_on: vec![
                    "server".into(),
                    "kafka".into(),
                    "consumer".into(),
                    "validator".into(),
                ],
                profiles: vec!["dataplane".into(), "all".into()],
                ports: vec![],
                internal_port: None,
            },
        );
        services.insert(
            "validator".into(),
            ComposeService {
                name: "validator".into(),
                depends_on: vec!["nats".into(), "configctl".into()],
                profiles: vec!["runtime".into(), "all".into()],
                ports: vec![],
                internal_port: None,
            },
        );

        ComposeTopology { services }
    }

    fn make_evidence() -> Evidence {
        let mut configs = HashMap::new();
        configs.insert("consumer".into(), make_consumer_config());
        configs.insert("emulator".into(), make_emulator_config());
        configs.insert("validator".into(), make_validator_config());

        let mut makefile_targets = HashSet::new();
        for t in &[
            "help",
            "tidy",
            "test",
            "build",
            "up-core",
            "up-runtime",
            "up-dataplane",
            "up-all",
            "down",
            "restart",
            "logs",
            "ps",
            "clean",
            "check",
            "verify",
            "check-deep",
            "smoke",
            "trace-pack",
            "results-inspect",
            "quality-gate",
            "quality-gate-ci",
            "quality-gate-deep",
            "raccoon-build",
            "raccoon-test",
            "docker-build",
            "compose-config",
        ] {
            makefile_targets.insert(t.to_string());
        }

        let mut dev_doc_targets = HashSet::new();
        for t in &[
            "check",
            "verify",
            "check-deep",
            "smoke",
            "trace-pack",
            "results-inspect",
            "up-dataplane",
            "ps",
            "logs",
            "down",
        ] {
            dev_doc_targets.insert(t.to_string());
        }

        Evidence {
            configs,
            compose: Some(make_compose_topology()),
            source: Some(make_source_topology()),
            makefile_targets,
            dev_doc_targets,
            dev_doc_cli_commands: [
                "doctor",
                "topology-doctor",
                "contract-audit",
                "runtime-bindings",
                "quality-gate",
                "results-inspect",
                "runtime-smoke",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            cli_subcommands: known_cli_subcommands(),
            domain_events: vec![
                "config.activated".into(),
                "config.compiled".into(),
                "config.deactivated".into(),
                "config.draft_created".into(),
                "config.validated".into(),
            ],
            registry_events: vec![
                "config.activated".into(),
                "config.compiled".into(),
                "config.deactivated".into(),
                "config.draft_created".into(),
                "config.validated".into(),
            ],
            config_bindings: vec![],
        }
    }

    // ── config-compose-drift ────────────────────────────────────────

    #[test]
    fn config_compose_drift_passes_when_aligned() {
        let evidence = make_evidence();
        let result = check_config_compose_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn config_compose_drift_warns_config_without_service() {
        let mut evidence = make_evidence();
        // Add a config for a service not in compose
        evidence.configs.insert(
            "scheduler".into(),
            ServiceConfig {
                name: "scheduler".into(),
                ..Default::default()
            },
        );
        let result = check_config_compose_drift(&evidence);
        // scheduler is not in app_services, so no warning
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn config_compose_drift_skips_without_compose() {
        let mut evidence = make_evidence();
        evidence.compose = None;
        let result = check_config_compose_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn config_compose_drift_detects_transport_without_dependency() {
        let mut evidence = make_evidence();
        // Remove kafka dependency from consumer in compose
        let compose = evidence.compose.as_mut().unwrap();
        let consumer = compose.services.get_mut("consumer").unwrap();
        consumer.depends_on.retain(|d| d != "kafka");

        let result = check_config_compose_drift(&evidence);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.severity == Severity::Error && f.message.contains("kafka")),
            "should detect kafka config without compose dependency"
        );
    }

    #[test]
    fn config_compose_drift_detects_nats_without_dependency() {
        let mut evidence = make_evidence();
        let compose = evidence.compose.as_mut().unwrap();
        let consumer = compose.services.get_mut("consumer").unwrap();
        consumer.depends_on.retain(|d| d != "nats");

        let result = check_config_compose_drift(&evidence);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.severity == Severity::Error
                    && f.message.contains("nats")
                    && f.message.contains("consumer")),
        );
    }

    #[test]
    fn config_compose_drift_warns_compose_without_config() {
        let mut evidence = make_evidence();
        evidence.configs.remove("consumer");
        let result = check_config_compose_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("consumer") && f.message.contains("no deploy/configs")));
    }

    // ── config-source-drift ─────────────────────────────────────────

    #[test]
    fn config_source_drift_passes_when_aligned() {
        let evidence = make_evidence();
        let result = check_config_source_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn config_source_drift_skips_without_source() {
        let mut evidence = make_evidence();
        evidence.source = None;
        let result = check_config_source_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn config_source_drift_detects_missing_stream() {
        let mut evidence = make_evidence();
        let source = evidence.source.as_mut().unwrap();
        source.streams.remove("DATA_PLANE_INGESTION");

        let result = check_config_source_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error
                && f.message.contains("DATA_PLANE_INGESTION")));
    }

    #[test]
    fn config_source_drift_detects_durable_orphan() {
        let mut evidence = make_evidence();
        let source = evidence.source.as_mut().unwrap();
        source
            .durables
            .insert("orphan-durable".into(), "NONEXISTENT_STREAM".into());

        let result = check_config_source_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error
                && f.message.contains("NONEXISTENT_STREAM")));
    }

    #[test]
    fn config_source_drift_warns_missing_subject_prefix() {
        let mut evidence = make_evidence();
        let source = evidence.source.as_mut().unwrap();
        source.subjects.clear();

        let result = check_config_source_drift(&evidence);
        assert!(result.findings.iter().any(|f| f.severity == Severity::Warning));
    }

    // ── binding-topology-drift ──────────────────────────────────────

    #[test]
    fn binding_topology_drift_passes_empty_bindings() {
        let evidence = make_evidence();
        let result = check_binding_topology_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn binding_topology_drift_skips_without_source() {
        let mut evidence = make_evidence();
        evidence.source = None;
        let result = check_binding_topology_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn binding_topology_drift_detects_missing_stream() {
        let mut evidence = make_evidence();
        evidence.config_bindings.push(ConfigBinding {
            name: "orders".into(),
            topic: "orders.v1".into(),
            source_file: "test.jsonc".into(),
        });
        let source = evidence.source.as_mut().unwrap();
        source.streams.remove("DATA_PLANE_INGESTION");

        let result = check_binding_topology_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error
                && f.message.contains("DATA_PLANE_INGESTION")));
    }

    #[test]
    fn binding_topology_drift_detects_missing_durable() {
        let mut evidence = make_evidence();
        evidence.config_bindings.push(ConfigBinding {
            name: "orders".into(),
            topic: "orders.v1".into(),
            source_file: "test.jsonc".into(),
        });
        let source = evidence.source.as_mut().unwrap();
        source.durables.remove("validator-dataplane-v1");

        let result = check_binding_topology_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.contains("durable")));
    }

    // ── workflow-drift ──────────────────────────────────────────────

    #[test]
    fn workflow_drift_passes_when_aligned() {
        let evidence = make_evidence();
        let result = check_workflow_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn workflow_drift_skips_when_no_sources() {
        let mut evidence = make_evidence();
        evidence.makefile_targets.clear();
        evidence.dev_doc_targets.clear();
        let result = check_workflow_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn workflow_drift_detects_doc_target_not_in_makefile() {
        let mut evidence = make_evidence();
        evidence
            .dev_doc_targets
            .insert("deploy-staging".to_string());

        let result = check_workflow_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error
                && f.message.contains("deploy-staging")));
    }

    #[test]
    fn workflow_drift_detects_unknown_cli_command() {
        let mut evidence = make_evidence();
        evidence
            .dev_doc_cli_commands
            .insert("deep-audit".to_string());

        let result = check_workflow_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("deep-audit")));
    }

    #[test]
    fn workflow_drift_warns_undocumented_workflow_target() {
        let mut evidence = make_evidence();
        // Remove "check" from dev doc but keep in makefile
        evidence.dev_doc_targets.remove("check");

        let result = check_workflow_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Warning && f.message.contains("check")));
    }

    // ── contract-domain-drift ───────────────────────────────────────

    #[test]
    fn contract_domain_drift_passes_when_aligned() {
        let evidence = make_evidence();
        let result = check_contract_domain_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn contract_domain_drift_skips_when_empty() {
        let mut evidence = make_evidence();
        evidence.domain_events.clear();
        evidence.registry_events.clear();
        let result = check_contract_domain_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn contract_domain_drift_warns_domain_event_not_in_registry() {
        let mut evidence = make_evidence();
        evidence.domain_events.push("config.rejected".into());

        let result = check_contract_domain_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("config.rejected")
                && f.message.contains("no matching registry")));
    }

    #[test]
    fn contract_domain_drift_warns_registry_event_not_in_domain() {
        let mut evidence = make_evidence();
        evidence
            .registry_events
            .push("config.ingestion_runtime_changed".into());

        let result = check_contract_domain_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("config.ingestion_runtime_changed")
                && f.message.contains("no matching domain")));
    }

    // ── compose-profile-drift ───────────────────────────────────────

    #[test]
    fn compose_profile_drift_passes_when_aligned() {
        let evidence = make_evidence();
        let result = check_compose_profile_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn compose_profile_drift_skips_without_compose() {
        let mut evidence = make_evidence();
        evidence.compose = None;
        let result = check_compose_profile_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn compose_profile_drift_warns_missing_expected_profile() {
        let mut evidence = make_evidence();
        // Remove "runtime" profile from validator
        let compose = evidence.compose.as_mut().unwrap();
        let validator = compose.services.get_mut("validator").unwrap();
        validator.profiles.retain(|p| p != "runtime");
        // Also remove from all services to ensure "runtime" is truly absent
        for svc in compose.services.values_mut() {
            svc.profiles.retain(|p| p != "runtime");
        }

        let result = check_compose_profile_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("runtime")));
    }

    // ── Makefile parser ─────────────────────────────────────────────

    #[test]
    fn extract_makefile_targets_finds_standard_targets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Makefile");
        std::fs::write(
            &path,
            r#"
SHELL := /usr/bin/env bash
GO ?= go

.PHONY: help tidy test build

help:
	@echo "help"

tidy:
	go mod tidy

test:
	go test ./...

build: tidy
	go build

up-core:
	docker compose up
"#,
        )
        .unwrap();

        let targets = extract_makefile_targets(&path);
        assert!(targets.contains("help"), "targets: {targets:?}");
        assert!(targets.contains("tidy"));
        assert!(targets.contains("test"));
        assert!(targets.contains("build"));
        assert!(targets.contains("up-core"));
    }

    #[test]
    fn extract_makefile_targets_skips_variables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Makefile");
        std::fs::write(
            &path,
            r#"
GO ?= go
SHELL := /usr/bin/env bash
BUILD_DIR ?= bin

real-target:
	echo ok
"#,
        )
        .unwrap();

        let targets = extract_makefile_targets(&path);
        assert!(!targets.contains("GO"));
        assert!(!targets.contains("SHELL"));
        assert!(!targets.contains("BUILD_DIR"));
        assert!(targets.contains("real-target"));
    }

    #[test]
    fn extract_makefile_targets_handles_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Makefile");
        std::fs::write(&path, "").unwrap();

        let targets = extract_makefile_targets(&path);
        assert!(targets.is_empty());
    }

    #[test]
    fn extract_makefile_targets_handles_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Makefile");

        let targets = extract_makefile_targets(&path);
        assert!(targets.is_empty());
    }

    // ── DEVELOPMENT.md parser ───────────────────────────────────────

    #[test]
    fn extract_dev_doc_references_finds_make_targets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DEVELOPMENT.md");
        std::fs::write(
            &path,
            r#"
# Workflow

```sh
make check
make verify
make up-dataplane
```

Run `make smoke` to test.
"#,
        )
        .unwrap();

        let (targets, _) = extract_dev_doc_references(&path);
        assert!(targets.contains("check"), "targets: {targets:?}");
        assert!(targets.contains("verify"));
        assert!(targets.contains("up-dataplane"));
        assert!(targets.contains("smoke"));
    }

    #[test]
    fn extract_dev_doc_references_finds_cli_commands() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DEVELOPMENT.md");
        std::fs::write(
            &path,
            r#"
```sh
raccoon-cli doctor
raccoon-cli --json topology-doctor
raccoon-cli -v contract-audit
```
"#,
        )
        .unwrap();

        let (_, commands) = extract_dev_doc_references(&path);
        assert!(commands.contains("doctor"), "commands: {commands:?}");
        assert!(commands.contains("topology-doctor"));
        assert!(commands.contains("contract-audit"));
    }

    #[test]
    fn extract_dev_doc_handles_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DEVELOPMENT.md");
        std::fs::write(&path, "").unwrap();

        let (targets, commands) = extract_dev_doc_references(&path);
        assert!(targets.is_empty());
        assert!(commands.is_empty());
    }

    // ── Event name detection ────────────────────────────────────────

    #[test]
    fn is_event_name_valid_events() {
        assert!(is_event_name("config.activated"));
        assert!(is_event_name("config.draft_created"));
        assert!(is_event_name("config.ingestion_runtime_changed"));
    }

    #[test]
    fn is_event_name_rejects_non_events() {
        assert!(!is_event_name(""));
        assert!(!is_event_name("single_word"));
        assert!(!is_event_name("too.many.dots.here"));
        assert!(!is_event_name("HAS.CAPS"));
        assert!(!is_event_name("nats://url:4222"));
    }

    // ── Full integration ────────────────────────────────────────────

    #[test]
    fn analyze_on_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let report = analyze(dir.path()).unwrap();
        assert_eq!(report.title, "drift-detect");
        // Most checks should skip gracefully
        let skip_count = report
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Skip)
            .count();
        assert!(skip_count >= 3, "expected most checks to skip on empty dir");
    }

    #[test]
    fn analyze_on_minimal_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/configs")).unwrap();
        std::fs::write(
            dir.path().join("deploy/configs/consumer.jsonc"),
            r#"{"kafka": {"brokers": ["kafka:9092"]}, "nats": {"url": "nats://nats:4222"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Makefile"),
            "check:\n\techo ok\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        assert_eq!(report.title, "drift-detect");
    }

    // ── JSON string value extraction ────────────────────────────────

    #[test]
    fn extract_json_string_value_finds_value() {
        let val = extract_json_string_value(r#""name": "orders""#, "name");
        assert_eq!(val, Some("orders".to_string()));
    }

    #[test]
    fn extract_json_string_value_returns_none_for_missing_key() {
        let val = extract_json_string_value(r#""name": "orders""#, "topic");
        assert_eq!(val, None);
    }

    #[test]
    fn extract_json_string_value_handles_spaces() {
        let val = extract_json_string_value(r#"  "topic" : "orders.v1"  "#, "topic");
        assert_eq!(val, Some("orders.v1".to_string()));
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn config_compose_drift_with_empty_configs_skips() {
        let mut evidence = make_evidence();
        evidence.configs.clear();
        let result = check_config_compose_drift(&evidence);
        assert_eq!(result.status, CheckStatus::Skip);
    }

    #[test]
    fn config_source_drift_detects_orphan_stream_subject() {
        let mut evidence = make_evidence();
        let source = evidence.source.as_mut().unwrap();
        source.streams.insert(
            "ORPHAN_STREAM".into(),
            vec!["orphan.events.>".into()],
        );

        let result = check_config_source_drift(&evidence);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("ORPHAN_STREAM")));
    }

    #[test]
    fn binding_topology_drift_with_bindings_and_full_infra_passes() {
        let mut evidence = make_evidence();
        evidence.config_bindings.push(ConfigBinding {
            name: "orders".into(),
            topic: "orders.v1".into(),
            source_file: "test.jsonc".into(),
        });

        let result = check_binding_topology_drift(&evidence);
        // Should pass because DATA_PLANE_INGESTION stream and validator-dataplane-v1 durable exist
        let errors = result
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        assert_eq!(errors, 0, "expected no errors with full infra");
    }

    #[test]
    fn findings_have_why_and_help() {
        let mut evidence = make_evidence();
        evidence
            .dev_doc_targets
            .insert("nonexistent-target".to_string());

        let result = check_workflow_drift(&evidence);
        for finding in &result.findings {
            if finding.severity == Severity::Error {
                assert!(
                    finding.why.is_some(),
                    "error finding should have why: {:?}",
                    finding
                );
                assert!(
                    finding.help.is_some(),
                    "error finding should have help: {:?}",
                    finding
                );
            }
        }
    }
}
