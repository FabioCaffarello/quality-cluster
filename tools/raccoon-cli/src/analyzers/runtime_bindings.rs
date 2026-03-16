use crate::error::Result;
use crate::models::{CheckResult, Finding, Report};
use std::collections::{HashMap, HashSet};
use std::path::Path;

mod configs;
mod source;

pub use configs::BindingDefinition;
pub use source::RuntimeBindingSource;

// ── Discovered runtime binding ──────────────────────────────────────

/// A fully resolved runtime binding combining config declaration
/// with source-derived routing and validation rules.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedBinding {
    /// Config name that declares this binding (metadata.name from YAML/JSON).
    pub config_name: String,
    /// Binding name within the config.
    pub binding_name: String,
    /// Kafka topic this binding consumes from.
    pub kafka_topic: String,
    /// JetStream subject derived from the subject pattern.
    pub jetstream_subject: Option<String>,
    /// Activation scope (kind:key).
    pub scope: String,
    /// Fields declared for validation.
    pub field_count: usize,
    /// Rules declared for validation.
    pub rule_count: usize,
    /// Source of the binding definition.
    pub source_file: Option<String>,
    /// Issues found during resolution.
    pub issues: Vec<String>,
}

/// Full runtime bindings index built from scanning configs and source.
#[derive(Debug, Default)]
pub struct BindingsIndex {
    /// Config-declared bindings (from tests/http/*.http fixture files and Go source).
    pub config_bindings: Vec<BindingDefinition>,
    /// Source-level routing constants.
    pub source: Option<RuntimeBindingSource>,
    /// Resolved bindings after cross-referencing.
    pub resolved: Vec<ResolvedBinding>,
}

// ── Main analysis entry point ───────────────────────────────────────

pub fn analyze(project_root: &Path) -> Result<Report> {
    let mut report = Report::new("runtime-bindings");
    let mut index = BindingsIndex::default();

    // Phase 1: Scan Go source for binding definitions, subject patterns, routing constants
    let internal_dir = project_root.join("internal");
    if !internal_dir.is_dir() {
        report.add(CheckResult::from_findings(
            "internal-dir",
            vec![Finding::error("internal-dir", "internal/ directory not found")
                .with_why("runtime-bindings scans Go source for binding definitions and routing constants")
                .with_help("run `raccoon-cli doctor` to verify project structure first")],
        ));
        return Ok(report);
    }

    match source::scan_runtime_bindings(&internal_dir) {
        Ok(src) => {
            report.add(check_subject_pattern(&src));
            report.add(check_routing_constants(&src));
            report.add(check_lifecycle_events(&src));
            index.source = Some(src);
        }
        Err(e) => {
            report.add(CheckResult::from_findings(
                "source-scan",
                vec![Finding::error("source", format!("failed to scan: {e}"))],
            ));
        }
    }

    // Phase 2: Scan config fixtures for binding definitions
    let configs_dir = project_root.join("deploy/configs");
    if configs_dir.is_dir() {
        match configs::scan_binding_configs(&configs_dir) {
            Ok(bindings) => {
                report.add(check_config_bindings(&bindings));
                index.config_bindings = bindings;
            }
            Err(e) => {
                report.add(CheckResult::from_findings(
                    "config-scan",
                    vec![Finding::error("config", format!("failed to scan: {e}"))],
                ));
            }
        }
    }

    // Phase 3: Scan HTTP test fixtures for example binding payloads
    let http_dir = project_root.join("tests/http");
    if http_dir.is_dir() {
        match configs::scan_http_fixtures(&http_dir) {
            Ok(fixture_bindings) => {
                report.add(check_fixture_bindings(&fixture_bindings));
                // Merge fixture bindings with existing ones (if not already present)
                for fb in fixture_bindings {
                    let already = index
                        .config_bindings
                        .iter()
                        .any(|b| b.name == fb.name && b.topic == fb.topic);
                    if !already {
                        index.config_bindings.push(fb);
                    }
                }
            }
            Err(_) => {
                // Non-fatal: HTTP fixtures are optional
            }
        }
    }

    // Phase 4: Resolve bindings by cross-referencing config with source
    index.resolved = resolve_bindings(&index);
    report.add(check_resolved_bindings(&index.resolved));

    // Phase 5: Cross-validation checks
    report.add(check_topic_subject_mapping(&index));
    report.add(check_binding_consumer_coverage(&index));
    report.add(check_binding_validator_coverage(&index));
    report.add(check_scope_consistency(&index));
    report.add(check_drift(&index));

    Ok(report)
}

// ── Binding resolution ──────────────────────────────────────────────

fn resolve_bindings(index: &BindingsIndex) -> Vec<ResolvedBinding> {
    let source = match &index.source {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut resolved = Vec::new();

    for binding in &index.config_bindings {
        let mut issues = Vec::new();

        // Derive JetStream subject using the subject pattern from source
        let subject = derive_jetstream_subject(
            &source.subject_prefix,
            &binding.scope_kind,
            &binding.scope_key,
            &binding.name,
        );

        // Check if the derived subject falls under a known stream
        if let Some(ref subj) = subject {
            let covered = source
                .stream_subjects
                .iter()
                .any(|(_, patterns)| patterns.iter().any(|p| subject_matches_pattern(subj, p)));
            if !covered {
                issues.push(format!(
                    "derived subject '{subj}' does not match any stream subscription pattern"
                ));
            }
        } else {
            issues.push(
                "could not derive JetStream subject (missing subject prefix in source)".into(),
            );
        }

        // Check if kafka topic appears in consumer config context
        if !source.kafka_topics_referenced.contains(&binding.topic) {
            // Not necessarily an error — topics are dynamic at runtime
            // but worth noting if we can observe them
        }

        resolved.push(ResolvedBinding {
            config_name: binding.config_name.clone(),
            binding_name: binding.name.clone(),
            kafka_topic: binding.topic.clone(),
            jetstream_subject: subject,
            scope: format!("{}:{}", binding.scope_kind, binding.scope_key),
            field_count: binding.field_count,
            rule_count: binding.rule_count,
            source_file: binding.source_file.clone(),
            issues,
        });
    }

    resolved
}

fn derive_jetstream_subject(
    prefix: &str,
    scope_kind: &str,
    scope_key: &str,
    binding_name: &str,
) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }

    let sanitized_kind = sanitize_token(scope_kind);
    let sanitized_key = sanitize_token(scope_key);
    let sanitized_name = sanitize_token(binding_name);

    Some(format!(
        "{prefix}.{sanitized_kind}.{sanitized_key}.{sanitized_name}"
    ))
}

/// Replicates the Go sanitizeToken function from dataplane/registry.go.
fn sanitize_token(raw: &str) -> String {
    let raw = raw.trim().to_lowercase();
    if raw.is_empty() {
        return "unknown".to_string();
    }

    let mut result = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            last_dash = false;
        } else if !last_dash {
            result.push('-');
            last_dash = true;
        }
    }

    let token = result.trim_matches('-').to_string();
    if token.is_empty() {
        "unknown".to_string()
    } else {
        token
    }
}

fn subject_matches_pattern(subject: &str, pattern: &str) -> bool {
    if pattern.ends_with(".>") {
        let prefix = &pattern[..pattern.len() - 2];
        subject.starts_with(prefix)
    } else if pattern == subject {
        true
    } else {
        false
    }
}

// ── Individual checks ───────────────────────────────────────────────

fn check_subject_pattern(src: &RuntimeBindingSource) -> CheckResult {
    let mut findings = Vec::new();

    if src.subject_prefix.is_empty() {
        findings.push(
            Finding::error(
                "subject-prefix",
                "dataplane ingestion subject prefix not found in source",
            )
            .with_why("without the subject prefix, runtime cannot derive JetStream subjects for bindings")
            .with_help("verify 'dataplane.ingestion.received' is defined in internal/application/dataplane/registry.go"),
        );
    } else {
        findings.push(Finding::info(
            "subject-prefix",
            format!("subject prefix: '{}'", src.subject_prefix),
        ));
    }

    if src.subject_pattern.is_empty() {
        findings.push(Finding::warning(
            "subject-pattern",
            "wildcard subject pattern not found",
        ));
    }

    CheckResult::from_findings("subject-pattern", findings)
}

fn check_routing_constants(src: &RuntimeBindingSource) -> CheckResult {
    let mut findings = Vec::new();

    // Check for DATA_PLANE_INGESTION stream
    if !src.stream_subjects.contains_key("DATA_PLANE_INGESTION") {
        findings.push(
            Finding::error(
                "dataplane-stream",
                "DATA_PLANE_INGESTION stream not found in source",
            )
            .with_why("this stream carries all ingested messages from consumer to validator")
            .with_help("check internal/adapters/nats/dataplane_registry.go"),
        );
    }

    // Check for validator durable consumer
    if !src.durable_consumers.contains_key("validator-dataplane-v1") {
        findings.push(
            Finding::error(
                "validator-durable",
                "validator-dataplane-v1 durable consumer not found in source",
            )
            .with_why("validator needs a durable consumer to receive data-plane messages reliably")
            .with_help("check internal/adapters/nats/dataplane_registry.go"),
        );
    }

    // Check for runtime cache durable
    if !src
        .durable_consumers
        .contains_key("validator-runtime-cache-v1")
    {
        findings.push(
            Finding::warning(
                "runtime-cache-durable",
                "validator-runtime-cache-v1 durable not found",
            )
            .with_why("validator caches RuntimeProjection from activation events via this durable"),
        );
    }

    for (durable, purpose) in [
        (
            "consumer-runtime-refresh-v1",
            "consumer refreshes aggregate bootstrap when ingestion runtime changes",
        ),
        (
            "emulator-runtime-refresh-v1",
            "emulator refreshes aggregate bootstrap when ingestion runtime changes",
        ),
    ] {
        if !src.durable_consumers.contains_key(durable) {
            findings.push(
                Finding::error(
                    "runtime-refresh-durable",
                    format!("{durable} durable consumer not found in source"),
                )
                .with_why(purpose)
                .with_help("check internal/adapters/nats/configctl_registry.go"),
            );
        }
    }

    CheckResult::from_findings("routing-constants", findings)
}

fn check_lifecycle_events(src: &RuntimeBindingSource) -> CheckResult {
    let mut findings = Vec::new();

    let expected_events = [
        (
            "config.activated",
            "triggers RuntimeProjection cache update in validator",
        ),
        (
            "config.deactivated",
            "clears cached projection when config is deactivated",
        ),
        (
            "config.ingestion_runtime_changed",
            "notifies consumer and emulator to re-bootstrap bindings",
        ),
    ];

    for (event, purpose) in &expected_events {
        if !src.lifecycle_events.contains(*event) {
            findings.push(
                Finding::warning(
                    "lifecycle-event",
                    format!("lifecycle event '{event}' not found in source"),
                )
                .with_why(*purpose),
            );
        }
    }

    CheckResult::from_findings("lifecycle-events", findings)
}

fn check_config_bindings(bindings: &[BindingDefinition]) -> CheckResult {
    let mut findings = Vec::new();

    if bindings.is_empty() {
        findings.push(Finding::warning(
            "config-bindings",
            "no binding definitions found in deploy/configs/",
        ));
        return CheckResult::from_findings("config-bindings", findings);
    }

    // Check for duplicate binding names within the same config
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();
    for b in bindings {
        seen.entry(b.name.clone())
            .or_default()
            .push(b.config_name.clone());
    }
    for (name, configs) in &seen {
        if configs.len() > 1 {
            let configs_str = configs.join(", ");
            findings.push(
                Finding::warning(
                    "duplicate-binding",
                    format!("binding '{name}' appears in multiple configs: {configs_str}"),
                )
                .with_why(
                    "duplicate binding names across configs may cause routing conflicts at runtime",
                ),
            );
        }
    }

    // Check for duplicate topics
    let mut topic_map: HashMap<String, Vec<String>> = HashMap::new();
    for b in bindings {
        topic_map
            .entry(b.topic.clone())
            .or_default()
            .push(b.name.clone());
    }
    for (topic, names) in &topic_map {
        if names.len() > 1 {
            let names_str = names.join(", ");
            findings.push(Finding::info(
                "shared-topic",
                format!("topic '{topic}' is consumed by multiple bindings: {names_str}"),
            ));
        }
    }

    // Check for empty fields/rules
    for b in bindings {
        if b.field_count == 0 {
            findings.push(
                Finding::warning(
                    "empty-fields",
                    format!("binding '{}' has no fields defined", b.name),
                )
                .with_why("without fields, no validation can be performed on incoming messages"),
            );
        }
        if b.rule_count == 0 {
            findings.push(
                Finding::warning(
                    "empty-rules",
                    format!("binding '{}' has no rules defined", b.name),
                )
                .with_why("without rules, messages will always pass validation"),
            );
        }
    }

    CheckResult::from_findings("config-bindings", findings)
}

fn check_fixture_bindings(bindings: &[BindingDefinition]) -> CheckResult {
    let mut findings = Vec::new();

    if bindings.is_empty() {
        // Not an error — fixtures are optional
        return CheckResult::pass("fixture-bindings");
    }

    for b in bindings {
        if b.topic.is_empty() {
            findings.push(Finding::warning(
                "fixture-binding",
                format!("fixture binding '{}' has no topic", b.name),
            ));
        }
    }

    findings.push(Finding::info(
        "fixture-bindings",
        format!("{} binding(s) found in HTTP test fixtures", bindings.len()),
    ));

    CheckResult::from_findings("fixture-bindings", findings)
}

fn check_resolved_bindings(resolved: &[ResolvedBinding]) -> CheckResult {
    let mut findings = Vec::new();

    if resolved.is_empty() {
        findings.push(Finding::warning(
            "resolved-bindings",
            "no bindings could be resolved (no config + source cross-reference)",
        ));
        return CheckResult::from_findings("resolved-bindings", findings);
    }

    let mut error_count = 0;
    for rb in resolved {
        if !rb.issues.is_empty() {
            for issue in &rb.issues {
                findings.push(
                    Finding::warning(
                        "binding-resolution",
                        format!(
                            "binding '{}' ({}): {}",
                            rb.binding_name, rb.kafka_topic, issue
                        ),
                    )
                    .with_location(rb.source_file.as_deref().unwrap_or("config")),
                );
            }
            error_count += 1;
        }
    }

    // Summary
    let ok_count = resolved.len() - error_count;
    findings.push(Finding::info(
        "resolved-summary",
        format!(
            "{} binding(s) resolved: {} clean, {} with issues",
            resolved.len(),
            ok_count,
            error_count
        ),
    ));

    CheckResult::from_findings("resolved-bindings", findings)
}

fn check_topic_subject_mapping(index: &BindingsIndex) -> CheckResult {
    let source = match &index.source {
        Some(s) => s,
        None => return CheckResult::skip("topic-subject-mapping", "source not scanned"),
    };

    let mut findings = Vec::new();

    for rb in &index.resolved {
        if let Some(ref subject) = rb.jetstream_subject {
            // Verify the subject falls within the DATA_PLANE_INGESTION stream subscription
            if let Some(patterns) = source.stream_subjects.get("DATA_PLANE_INGESTION") {
                let covered = patterns.iter().any(|p| subject_matches_pattern(subject, p));
                if !covered {
                    findings.push(
                        Finding::error(
                            "topic-subject",
                            format!(
                                "binding '{}': derived subject '{}' is outside DATA_PLANE_INGESTION stream scope",
                                rb.binding_name, subject
                            ),
                        )
                        .with_why("messages published to this subject will not be received by the validator")
                        .with_help("check the stream's subject filter pattern in dataplane_registry.go"),
                    );
                }
            }
        }
    }

    CheckResult::from_findings("topic-subject-mapping", findings)
}

fn check_binding_consumer_coverage(index: &BindingsIndex) -> CheckResult {
    let source = match &index.source {
        Some(s) => s,
        None => return CheckResult::skip("consumer-coverage", "source not scanned"),
    };

    let mut findings = Vec::new();

    // Check that the consumer has bootstrap URL configured
    if !source.has_bootstrap_client {
        findings.push(
            Finding::error(
                "consumer-bootstrap",
                "consumer bootstrap client not found in source",
            )
            .with_why("consumer needs to fetch active bindings from server at startup to know which topics to subscribe to")
            .with_help("verify internal/application/runtimebootstrap/client.go exists"),
        );
    }

    // Check that runtime topology builder exists
    if !source.has_topology_builder {
        findings.push(
            Finding::error(
                "topology-builder",
                "RuntimeTopology builder not found in source",
            )
            .with_why("without topology builder, consumer cannot route Kafka messages to correct JetStream subjects")
            .with_help("verify internal/application/dataplane/topology.go exists"),
        );
    }

    CheckResult::from_findings("consumer-coverage", findings)
}

fn check_binding_validator_coverage(index: &BindingsIndex) -> CheckResult {
    let source = match &index.source {
        Some(s) => s,
        None => return CheckResult::skip("validator-coverage", "source not scanned"),
    };

    let mut findings = Vec::new();

    if !source.has_runtime_cache {
        findings.push(
            Finding::error(
                "runtime-cache",
                "validator RuntimeProjection cache not found in source",
            )
            .with_why("validator caches active RuntimeProjection (with rules) to evaluate incoming messages")
            .with_help("verify internal/actors/scopes/validator/runtime_cache.go exists"),
        );
    }

    if !source.has_validation_worker {
        findings.push(
            Finding::error(
                "validation-worker",
                "validation worker not found in source",
            )
            .with_why("without a validation worker, incoming data-plane messages cannot be evaluated against rules"),
        );
    }

    CheckResult::from_findings("validator-coverage", findings)
}

fn check_scope_consistency(index: &BindingsIndex) -> CheckResult {
    let mut findings = Vec::new();

    let scopes: HashSet<String> = index
        .config_bindings
        .iter()
        .map(|b| format!("{}:{}", b.scope_kind, b.scope_key))
        .collect();

    if scopes.len() > 1 {
        let scope_list: Vec<&String> = scopes.iter().collect();
        findings.push(
            Finding::warning(
                "multi-scope",
                format!(
                    "bindings reference multiple scopes: {}",
                    scope_list
                        .iter()
                        .map(|s| format!("'{s}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
            .with_why("multiple scopes increase routing complexity; ensure each service bootstraps the correct scope"),
        );
    }

    if !scopes.is_empty() && !scopes.contains("global:default") {
        findings.push(
            Finding::warning(
                "default-scope",
                "no bindings use the default scope 'global:default'",
            )
            .with_why("the default activation scope is 'global:default'; non-standard scopes require explicit configuration"),
        );
    }

    CheckResult::from_findings("scope-consistency", findings)
}

fn check_drift(index: &BindingsIndex) -> CheckResult {
    let source = match &index.source {
        Some(s) => s,
        None => return CheckResult::skip("drift-detection", "source not scanned"),
    };

    let mut findings = Vec::new();

    // Check: subject prefix in source matches expected convention
    if !source.subject_prefix.is_empty() && source.subject_prefix != "dataplane.ingestion.received"
    {
        findings.push(
            Finding::error(
                "prefix-drift",
                format!(
                    "subject prefix '{}' differs from expected 'dataplane.ingestion.received'",
                    source.subject_prefix
                ),
            )
            .with_why(
                "prefix drift means consumer and validator will not agree on JetStream subjects",
            )
            .with_help("align the prefix in internal/application/dataplane/registry.go"),
        );
    }

    // Check: event stream name
    if !source.stream_subjects.contains_key("CONFIGCTL_EVENTS") {
        findings.push(
            Finding::warning(
                "event-stream-drift",
                "CONFIGCTL_EVENTS stream not found — validator may not receive activation events",
            )
            .with_why("without this stream, the validator cannot cache RuntimeProjection from activation events"),
        );
    }

    // Check: validator durable consumer targets the right stream
    if let Some(stream) = source.durable_consumers.get("validator-dataplane-v1") {
        if stream != "DATA_PLANE_INGESTION" {
            findings.push(
                Finding::error(
                    "durable-stream-drift",
                    format!(
                        "validator durable targets stream '{stream}' instead of 'DATA_PLANE_INGESTION'"
                    ),
                )
                .with_why("durable consumer bound to wrong stream will never receive data-plane messages"),
            );
        }
    }

    if let Some(stream) = source.durable_consumers.get("validator-runtime-cache-v1") {
        if stream != "CONFIGCTL_EVENTS" {
            findings.push(
                Finding::error(
                    "runtime-durable-drift",
                    format!(
                        "runtime cache durable targets stream '{stream}' instead of 'CONFIGCTL_EVENTS'"
                    ),
                )
                .with_why("runtime cache durable bound to wrong stream will miss activation events"),
            );
        }
    }

    for durable in ["consumer-runtime-refresh-v1", "emulator-runtime-refresh-v1"] {
        if let Some(stream) = source.durable_consumers.get(durable) {
            if stream != "CONFIGCTL_EVENTS" {
                findings.push(
                    Finding::error(
                        "runtime-refresh-durable-drift",
                        format!(
                            "refresh durable '{durable}' targets stream '{stream}' instead of 'CONFIGCTL_EVENTS'"
                        ),
                    )
                    .with_why("event-driven dataplane refresh depends on CONFIGCTL_EVENTS; wrong stream means runtime changes will be missed"),
                );
            }
        }
    }

    // Check: bootstrap scope in deploy configs matches binding scopes
    let config_scopes: HashSet<String> = index
        .config_bindings
        .iter()
        .map(|b| format!("{}:{}", b.scope_kind, b.scope_key))
        .collect();
    let bootstrap_scopes: HashSet<String> = source
        .bootstrap_scopes
        .iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect();
    if !config_scopes.is_empty() && !bootstrap_scopes.is_empty() {
        for scope in &config_scopes {
            if !bootstrap_scopes.contains(scope) {
                findings.push(
                    Finding::warning(
                        "bootstrap-scope-drift",
                        format!(
                            "binding scope '{scope}' is not bootstrapped by any service config"
                        ),
                    )
                    .with_why("bindings in this scope will not be fetched during consumer/emulator bootstrap"),
                );
            }
        }
    }

    CheckResult::from_findings("drift-detection", findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source() -> RuntimeBindingSource {
        let mut stream_subjects = HashMap::new();
        stream_subjects.insert(
            "DATA_PLANE_INGESTION".into(),
            vec!["dataplane.ingestion.received.>".into()],
        );
        stream_subjects.insert(
            "CONFIGCTL_EVENTS".into(),
            vec!["configctl.events.config.>".into()],
        );

        let mut durable_consumers = HashMap::new();
        durable_consumers.insert(
            "validator-dataplane-v1".into(),
            "DATA_PLANE_INGESTION".into(),
        );
        durable_consumers.insert(
            "validator-runtime-cache-v1".into(),
            "CONFIGCTL_EVENTS".into(),
        );
        durable_consumers.insert(
            "consumer-runtime-refresh-v1".into(),
            "CONFIGCTL_EVENTS".into(),
        );
        durable_consumers.insert(
            "emulator-runtime-refresh-v1".into(),
            "CONFIGCTL_EVENTS".into(),
        );

        let mut lifecycle_events = HashSet::new();
        lifecycle_events.insert("config.activated".into());
        lifecycle_events.insert("config.deactivated".into());
        lifecycle_events.insert("config.ingestion_runtime_changed".into());

        RuntimeBindingSource {
            subject_prefix: "dataplane.ingestion.received".into(),
            subject_pattern: "dataplane.ingestion.received.>".into(),
            stream_subjects,
            durable_consumers,
            lifecycle_events,
            kafka_topics_referenced: HashSet::new(),
            has_bootstrap_client: true,
            has_topology_builder: true,
            has_runtime_cache: true,
            has_validation_worker: true,
            bootstrap_scopes: vec![("global".into(), "default".into())],
        }
    }

    fn make_binding(name: &str, topic: &str) -> BindingDefinition {
        BindingDefinition {
            name: name.into(),
            topic: topic.into(),
            config_name: "test-config".into(),
            scope_kind: "global".into(),
            scope_key: "default".into(),
            field_count: 3,
            rule_count: 2,
            source_file: None,
        }
    }

    // ── sanitize_token ──────────────────────────────────────────────

    #[test]
    fn sanitize_token_basic() {
        assert_eq!(sanitize_token("user-events"), "user-events");
        assert_eq!(sanitize_token("Global"), "global");
        assert_eq!(sanitize_token("  padded  "), "padded");
    }

    #[test]
    fn sanitize_token_special_chars() {
        assert_eq!(sanitize_token("hello_world!"), "hello-world");
        assert_eq!(sanitize_token("a@b#c"), "a-b-c");
        assert_eq!(sanitize_token(""), "unknown");
        assert_eq!(sanitize_token("!!!"), "unknown");
    }

    // ── subject_matches_pattern ─────────────────────────────────────

    #[test]
    fn subject_matches_wildcard() {
        assert!(subject_matches_pattern(
            "dataplane.ingestion.received.global.default.user-events",
            "dataplane.ingestion.received.>"
        ));
    }

    #[test]
    fn subject_matches_exact() {
        assert!(subject_matches_pattern(
            "configctl.events.config.activated",
            "configctl.events.config.activated"
        ));
    }

    #[test]
    fn subject_no_match() {
        assert!(!subject_matches_pattern(
            "other.subject.here",
            "dataplane.ingestion.received.>"
        ));
    }

    // ── derive_jetstream_subject ────────────────────────────────────

    #[test]
    fn derive_subject_standard() {
        let subject = derive_jetstream_subject(
            "dataplane.ingestion.received",
            "global",
            "default",
            "user-events",
        );
        assert_eq!(
            subject.unwrap(),
            "dataplane.ingestion.received.global.default.user-events"
        );
    }

    #[test]
    fn derive_subject_empty_prefix() {
        assert!(derive_jetstream_subject("", "global", "default", "x").is_none());
    }

    #[test]
    fn derive_subject_sanitizes_tokens() {
        let subject = derive_jetstream_subject(
            "dataplane.ingestion.received",
            "Global",
            "Default",
            "User Events",
        );
        assert_eq!(
            subject.unwrap(),
            "dataplane.ingestion.received.global.default.user-events"
        );
    }

    // ── check_subject_pattern ───────────────────────────────────────

    #[test]
    fn subject_pattern_passes_when_present() {
        let src = make_source();
        let result = check_subject_pattern(&src);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn subject_pattern_fails_when_missing() {
        let mut src = make_source();
        src.subject_prefix.clear();
        let result = check_subject_pattern(&src);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    // ── check_routing_constants ─────────────────────────────────────

    #[test]
    fn routing_constants_pass_when_complete() {
        let src = make_source();
        let result = check_routing_constants(&src);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn routing_constants_fail_missing_stream() {
        let mut src = make_source();
        src.stream_subjects.remove("DATA_PLANE_INGESTION");
        let result = check_routing_constants(&src);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    #[test]
    fn routing_constants_fail_missing_durable() {
        let mut src = make_source();
        src.durable_consumers.remove("validator-dataplane-v1");
        let result = check_routing_constants(&src);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    // ── check_lifecycle_events ──────────────────────────────────────

    #[test]
    fn lifecycle_events_pass_when_complete() {
        let src = make_source();
        let result = check_lifecycle_events(&src);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn lifecycle_events_warn_when_missing() {
        let mut src = make_source();
        src.lifecycle_events.clear();
        let result = check_lifecycle_events(&src);
        // Only warnings, so still passes
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
        assert!(!result.findings.is_empty());
    }

    // ── check_config_bindings ───────────────────────────────────────

    #[test]
    fn config_bindings_pass_with_valid_bindings() {
        let bindings = vec![make_binding("user-events", "users-topic")];
        let result = check_config_bindings(&bindings);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn config_bindings_warn_empty() {
        let result = check_config_bindings(&[]);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == crate::models::Severity::Warning));
    }

    #[test]
    fn config_bindings_warn_duplicate_names() {
        let bindings = vec![
            make_binding("user-events", "topic-a"),
            BindingDefinition {
                config_name: "other-config".into(),
                ..make_binding("user-events", "topic-b")
            },
        ];
        let result = check_config_bindings(&bindings);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("duplicate") || f.message.contains("multiple configs")));
    }

    #[test]
    fn config_bindings_warn_no_fields() {
        let mut binding = make_binding("no-fields", "topic");
        binding.field_count = 0;
        let result = check_config_bindings(&[binding]);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("no fields")));
    }

    #[test]
    fn config_bindings_warn_no_rules() {
        let mut binding = make_binding("no-rules", "topic");
        binding.rule_count = 0;
        let result = check_config_bindings(&[binding]);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("no rules")));
    }

    // ── check_resolved_bindings ─────────────────────────────────────

    #[test]
    fn resolved_bindings_pass_clean() {
        let resolved = vec![ResolvedBinding {
            config_name: "test".into(),
            binding_name: "user-events".into(),
            kafka_topic: "users-topic".into(),
            jetstream_subject: Some(
                "dataplane.ingestion.received.global.default.user-events".into(),
            ),
            scope: "global:default".into(),
            field_count: 3,
            rule_count: 2,
            source_file: None,
            issues: vec![],
        }];
        let result = check_resolved_bindings(&resolved);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn resolved_bindings_warn_with_issues() {
        let resolved = vec![ResolvedBinding {
            config_name: "test".into(),
            binding_name: "broken".into(),
            kafka_topic: "topic".into(),
            jetstream_subject: None,
            scope: "global:default".into(),
            field_count: 0,
            rule_count: 0,
            source_file: None,
            issues: vec!["could not derive subject".into()],
        }];
        let result = check_resolved_bindings(&resolved);
        assert!(result
            .findings
            .iter()
            .any(|f| { f.severity == crate::models::Severity::Warning }));
    }

    // ── check_drift ─────────────────────────────────────────────────

    #[test]
    fn drift_passes_standard_setup() {
        let index = BindingsIndex {
            config_bindings: vec![make_binding("user-events", "users-topic")],
            source: Some(make_source()),
            resolved: vec![],
        };
        let result = check_drift(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn drift_fails_wrong_prefix() {
        let mut src = make_source();
        src.subject_prefix = "wrong.prefix.here".into();
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(src),
            resolved: vec![],
        };
        let result = check_drift(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    #[test]
    fn drift_fails_wrong_durable_stream() {
        let mut src = make_source();
        src.durable_consumers
            .insert("validator-dataplane-v1".into(), "WRONG_STREAM".into());
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(src),
            resolved: vec![],
        };
        let result = check_drift(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    #[test]
    fn drift_skips_without_source() {
        let index = BindingsIndex::default();
        let result = check_drift(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Skip);
    }

    // ── check_scope_consistency ─────────────────────────────────────

    #[test]
    fn scope_consistency_passes_single_default() {
        let index = BindingsIndex {
            config_bindings: vec![make_binding("x", "t")],
            source: None,
            resolved: vec![],
        };
        let result = check_scope_consistency(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn scope_consistency_warns_multiple() {
        let mut b2 = make_binding("y", "t2");
        b2.scope_kind = "tenant".into();
        b2.scope_key = "acme".into();
        let index = BindingsIndex {
            config_bindings: vec![make_binding("x", "t"), b2],
            source: None,
            resolved: vec![],
        };
        let result = check_scope_consistency(&index);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("multiple scopes")));
    }

    // ── check_consumer_coverage ─────────────────────────────────────

    #[test]
    fn consumer_coverage_passes_when_present() {
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(make_source()),
            resolved: vec![],
        };
        let result = check_binding_consumer_coverage(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn consumer_coverage_fails_missing_bootstrap() {
        let mut src = make_source();
        src.has_bootstrap_client = false;
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(src),
            resolved: vec![],
        };
        let result = check_binding_consumer_coverage(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    // ── check_validator_coverage ────────────────────────────────────

    #[test]
    fn validator_coverage_passes_when_present() {
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(make_source()),
            resolved: vec![],
        };
        let result = check_binding_validator_coverage(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn validator_coverage_fails_missing_cache() {
        let mut src = make_source();
        src.has_runtime_cache = false;
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(src),
            resolved: vec![],
        };
        let result = check_binding_validator_coverage(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }

    // ── resolve_bindings ────────────────────────────────────────────

    #[test]
    fn resolve_bindings_produces_correct_subject() {
        let index = BindingsIndex {
            config_bindings: vec![make_binding("user-events", "users-topic")],
            source: Some(make_source()),
            resolved: vec![],
        };
        let resolved = resolve_bindings(&index);
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0].jetstream_subject.as_deref(),
            Some("dataplane.ingestion.received.global.default.user-events")
        );
        assert!(resolved[0].issues.is_empty());
    }

    #[test]
    fn resolve_bindings_empty_without_source() {
        let index = BindingsIndex {
            config_bindings: vec![make_binding("x", "t")],
            source: None,
            resolved: vec![],
        };
        let resolved = resolve_bindings(&index);
        assert!(resolved.is_empty());
    }

    // ── analyze ─────────────────────────────────────────────────────

    #[test]
    fn analyze_fails_without_internal_dir() {
        let dir = tempfile::tempdir().unwrap();
        let report = analyze(dir.path()).unwrap();
        assert!(!report.passed());
    }

    #[test]
    fn analyze_succeeds_on_empty_internal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("internal")).unwrap();
        let report = analyze(dir.path()).unwrap();
        assert_eq!(report.title, "runtime-bindings");
    }

    // ── check_topic_subject_mapping ─────────────────────────────────

    #[test]
    fn topic_subject_mapping_passes_valid() {
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(make_source()),
            resolved: vec![ResolvedBinding {
                config_name: "test".into(),
                binding_name: "user-events".into(),
                kafka_topic: "users-topic".into(),
                jetstream_subject: Some(
                    "dataplane.ingestion.received.global.default.user-events".into(),
                ),
                scope: "global:default".into(),
                field_count: 3,
                rule_count: 2,
                source_file: None,
                issues: vec![],
            }],
        };
        let result = check_topic_subject_mapping(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn topic_subject_mapping_fails_outside_stream() {
        let index = BindingsIndex {
            config_bindings: vec![],
            source: Some(make_source()),
            resolved: vec![ResolvedBinding {
                config_name: "test".into(),
                binding_name: "rogue".into(),
                kafka_topic: "rogue-topic".into(),
                jetstream_subject: Some("other.prefix.rogue".into()),
                scope: "global:default".into(),
                field_count: 1,
                rule_count: 1,
                source_file: None,
                issues: vec![],
            }],
        };
        let result = check_topic_subject_mapping(&index);
        assert_eq!(result.status, crate::models::CheckStatus::Fail);
    }
}
