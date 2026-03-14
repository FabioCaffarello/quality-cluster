use crate::error::Result;
use crate::models::{CheckResult, Finding, Report};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub mod compose;
pub mod configs;
pub mod source;

pub use compose::ComposeTopology;
pub use configs::ServiceConfig;
pub use source::SourceTopology;

// ── Discovered topology ─────────────────────────────────────────────

/// A stage in the pipeline (emulator, kafka, consumer, nats/jetstream, validator).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum Stage {
    Emulator,
    Kafka,
    Consumer,
    JetStream,
    Validator,
    ConfigCtl,
    Server,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stage::Emulator => write!(f, "emulator"),
            Stage::Kafka => write!(f, "kafka"),
            Stage::Consumer => write!(f, "consumer"),
            Stage::JetStream => write!(f, "jetstream"),
            Stage::Validator => write!(f, "validator"),
            Stage::ConfigCtl => write!(f, "configctl"),
            Stage::Server => write!(f, "server"),
        }
    }
}

/// An edge connecting two stages via a named transport.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Edge {
    pub from: Stage,
    pub to: Stage,
    pub transport: String,
    pub detail: String,
}

/// Full discovered topology.
#[derive(Debug, Default)]
pub struct Topology {
    pub configs: HashMap<String, ServiceConfig>,
    pub compose: Option<ComposeTopology>,
    pub source: Option<SourceTopology>,
    #[allow(dead_code)]
    pub edges: Vec<Edge>,
}

// ── Main analysis entry point ────────────────────────────────────────

pub fn analyze(project_root: &Path) -> Result<Report> {
    let mut report = Report::new("topology-doctor");
    let mut topo = Topology::default();

    // Phase 1: Parse config files
    let configs_dir = project_root.join("deploy/configs");
    if configs_dir.is_dir() {
        report.add(CheckResult::pass("configs-dir-exists"));
        topo.configs = configs::parse_all_configs(&configs_dir)?;
        report.add(check_configs(&topo.configs));
    } else {
        report.add(CheckResult::from_findings(
            "configs-dir-exists",
            vec![Finding::error(
                "configs-dir",
                "deploy/configs directory not found",
            )
            .with_why("all topology checks depend on service configs to validate transport consistency")
            .with_help("run `raccoon-cli doctor` to verify project structure first")],
        ));
        return Ok(report);
    }

    // Phase 2: Parse docker-compose
    let compose_path = project_root.join("deploy/compose/docker-compose.yaml");
    if compose_path.is_file() {
        match compose::parse_compose(&compose_path) {
            Ok(ct) => {
                report.add(check_compose(&ct));
                report.add(check_compose_dependencies(&ct));
                topo.compose = Some(ct);
            }
            Err(e) => {
                report.add(CheckResult::from_findings(
                    "compose-parse",
                    vec![Finding::error("compose", format!("failed to parse: {e}"))],
                ));
            }
        }
    } else {
        report.add(CheckResult::from_findings(
            "compose-exists",
            vec![Finding::warning("compose", "docker-compose.yaml not found")],
        ));
    }

    // Phase 3: Scan source for topology constants
    let internal_dir = project_root.join("internal");
    if internal_dir.is_dir() {
        match source::scan_source(&internal_dir) {
            Ok(st) => {
                report.add(check_source_streams(&st));
                report.add(check_source_durables(&st));
                report.add(check_source_subjects(&st));
                topo.source = Some(st);
            }
            Err(e) => {
                report.add(CheckResult::from_findings(
                    "source-scan",
                    vec![Finding::error("source", format!("failed to scan: {e}"))],
                ));
            }
        }
    }

    // Phase 4: Cross-validate
    report.add(check_kafka_broker_consistency(&topo));
    report.add(check_nats_url_consistency(&topo));
    report.add(check_bootstrap_url_consistency(&topo));
    report.add(check_stream_subject_alignment(&topo));
    report.add(check_durable_stream_alignment(&topo));
    report.add(check_pipeline_continuity(&topo));

    Ok(report)
}

// ── Individual checks ────────────────────────────────────────────────

fn check_configs(configs: &HashMap<String, ServiceConfig>) -> CheckResult {
    let mut findings = Vec::new();
    let expected = ["consumer", "emulator", "validator"];

    for name in &expected {
        if !configs.contains_key(*name) {
            findings.push(Finding::warning(
                "config-present",
                format!("config for '{name}' not found in deploy/configs/"),
            ));
        }
    }

    // Consumer must have both kafka and nats
    if let Some(consumer) = configs.get("consumer") {
        if consumer.kafka_brokers.is_empty() {
            findings.push(Finding::error(
                "consumer-kafka",
                "consumer config has no kafka brokers",
            )
            .with_why("consumer bridges Kafka to JetStream; without kafka config the pipeline is broken")
            .with_help("add kafka.brokers to deploy/configs/consumer.jsonc"));
        }
        if consumer.nats_url.is_none() {
            findings.push(Finding::error(
                "consumer-nats",
                "consumer config has no nats url",
            )
            .with_why("consumer publishes to JetStream via NATS; without it data never reaches the validator")
            .with_help("add nats.url to deploy/configs/consumer.jsonc"));
        }
        if consumer.kafka_consumer_group.is_none() {
            findings.push(Finding::warning(
                "consumer-group",
                "consumer config has no consumer_group",
            )
            .with_why("without a consumer group, Kafka assigns a random one on each restart")
            .with_help("add kafka.consumer_group to deploy/configs/consumer.jsonc"));
        }
    }

    // Emulator must have kafka
    if let Some(emulator) = configs.get("emulator") {
        if emulator.kafka_brokers.is_empty() {
            findings.push(Finding::error(
                "emulator-kafka",
                "emulator config has no kafka brokers",
            )
            .with_why("emulator produces test data to Kafka; without it no data enters the pipeline")
            .with_help("add kafka.brokers to deploy/configs/emulator.jsonc"));
        }
    }

    // Validator must have nats
    if let Some(validator) = configs.get("validator") {
        if validator.nats_url.is_none() {
            findings.push(Finding::error(
                "validator-nats",
                "validator config has no nats url",
            )
            .with_why("validator consumes from JetStream via NATS; without it validation never runs")
            .with_help("add nats.url to deploy/configs/validator.jsonc"));
        }
    }

    CheckResult::from_findings("config-completeness", findings)
}

fn check_compose(ct: &ComposeTopology) -> CheckResult {
    let mut findings = Vec::new();
    let expected = ["nats", "kafka", "configctl", "server", "consumer", "emulator", "validator"];

    for name in &expected {
        if !ct.services.contains_key(*name) {
            findings.push(Finding::error(
                "compose-service",
                format!("service '{name}' not found in docker-compose"),
            )
            .with_why("runtime-smoke expects all services to be defined; missing services break the local environment")
            .with_help(format!("add '{name}' service to deploy/compose/docker-compose.yaml")));
        }
    }

    CheckResult::from_findings("compose-services", findings)
}

fn check_compose_dependencies(ct: &ComposeTopology) -> CheckResult {
    let mut findings = Vec::new();

    let expected_deps: &[(&str, &[&str])] = &[
        ("consumer", &["nats", "server", "kafka"]),
        ("emulator", &["server", "kafka", "consumer", "validator"]),
        ("validator", &["nats", "configctl"]),
        ("server", &["nats", "configctl"]),
        ("configctl", &["nats"]),
    ];

    for (service, deps) in expected_deps {
        if let Some(svc) = ct.services.get(*service) {
            for dep in *deps {
                if !svc.depends_on.contains(&dep.to_string()) {
                    findings.push(Finding::warning(
                        "compose-dependency",
                        format!("'{service}' should depend on '{dep}'"),
                    )
                    .with_location(format!("docker-compose.yaml:{service}")));
                }
            }
        }
    }

    CheckResult::from_findings("compose-dependencies", findings)
}

fn check_source_streams(st: &SourceTopology) -> CheckResult {
    let mut findings = Vec::new();

    let expected_streams = ["DATA_PLANE_INGESTION", "CONFIGCTL_EVENTS"];
    for stream in &expected_streams {
        if !st.streams.contains_key(*stream) {
            findings.push(Finding::error(
                "stream-defined",
                format!("expected stream '{stream}' not found in source"),
            )
            .with_why("JetStream streams are required for durable message delivery in the pipeline")
            .with_help("verify the stream constant is defined in the NATS adapter or JetStream setup code"));
        }
    }

    CheckResult::from_findings("source-streams", findings)
}

fn check_source_durables(st: &SourceTopology) -> CheckResult {
    let mut findings = Vec::new();

    let expected_durables = ["validator-dataplane-v1", "validator-runtime-cache-v1"];
    for durable in &expected_durables {
        if !st.durables.contains_key(*durable) {
            findings.push(Finding::error(
                "durable-defined",
                format!("expected durable consumer '{durable}' not found in source"),
            ));
        }
    }

    CheckResult::from_findings("source-durables", findings)
}

fn check_source_subjects(st: &SourceTopology) -> CheckResult {
    let mut findings = Vec::new();

    let expected_prefixes = [
        "dataplane.ingestion.received",
        "configctl.events.config",
        "configctl.control",
    ];

    for prefix in &expected_prefixes {
        let found = st
            .subjects
            .iter()
            .any(|s| s.starts_with(prefix));
        if !found {
            findings.push(Finding::warning(
                "subject-prefix",
                format!("no subjects with prefix '{prefix}' found in source"),
            ));
        }
    }

    CheckResult::from_findings("source-subjects", findings)
}

fn check_kafka_broker_consistency(topo: &Topology) -> CheckResult {
    let mut findings = Vec::new();
    let mut broker_sets: Vec<(String, Vec<String>)> = Vec::new();

    for (name, cfg) in &topo.configs {
        if !cfg.kafka_brokers.is_empty() {
            broker_sets.push((name.clone(), cfg.kafka_brokers.clone()));
        }
    }

    if broker_sets.len() >= 2 {
        let reference = &broker_sets[0].1;
        for (name, brokers) in &broker_sets[1..] {
            let ref_set: HashSet<_> = reference.iter().collect();
            let this_set: HashSet<_> = brokers.iter().collect();
            if ref_set != this_set {
                findings.push(Finding::warning(
                    "kafka-brokers",
                    format!(
                        "kafka brokers differ between '{}' ({:?}) and '{}' ({:?})",
                        broker_sets[0].0, reference, name, brokers
                    ),
                ));
            }
        }
    }

    // Cross-check with compose
    if let Some(compose) = &topo.compose {
        if let Some(kafka) = compose.services.get("kafka") {
            for (name, cfg) in &topo.configs {
                for broker in &cfg.kafka_brokers {
                    // Extract hostname from broker address
                    let host = broker.split(':').next().unwrap_or(broker);
                    if host != "kafka" && host != "localhost" && host != "127.0.0.1" {
                        findings.push(Finding::warning(
                            "kafka-broker-host",
                            format!(
                                "'{name}' config broker '{broker}' hostname doesn't match compose service name 'kafka'"
                            ),
                        ));
                    }
                }
            }
            // Check internal port matches
            let internal_port = kafka.internal_port.as_deref().unwrap_or("9092");
            for (name, cfg) in &topo.configs {
                for broker in &cfg.kafka_brokers {
                    if let Some(port) = broker.split(':').nth(1) {
                        if port != internal_port {
                            findings.push(Finding::error(
                                "kafka-port",
                                format!(
                                    "'{name}' broker port {port} doesn't match compose internal port {internal_port}"
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }

    CheckResult::from_findings("kafka-broker-consistency", findings)
}

fn check_nats_url_consistency(topo: &Topology) -> CheckResult {
    let mut findings = Vec::new();
    let mut urls: Vec<(String, String)> = Vec::new();

    for (name, cfg) in &topo.configs {
        if let Some(url) = &cfg.nats_url {
            urls.push((name.clone(), url.clone()));
        }
    }

    if urls.len() >= 2 {
        let reference = &urls[0].1;
        for (name, url) in &urls[1..] {
            if url != reference {
                findings.push(Finding::warning(
                    "nats-url",
                    format!(
                        "NATS URL differs between '{}' ({}) and '{}' ({})",
                        urls[0].0, reference, name, url
                    ),
                ));
            }
        }
    }

    CheckResult::from_findings("nats-url-consistency", findings)
}

fn check_bootstrap_url_consistency(topo: &Topology) -> CheckResult {
    let mut findings = Vec::new();
    let mut urls: Vec<(String, String)> = Vec::new();

    for (name, cfg) in &topo.configs {
        if let Some(url) = &cfg.bootstrap_base_url {
            urls.push((name.clone(), url.clone()));
        }
    }

    if urls.len() >= 2 {
        let reference = &urls[0].1;
        for (name, url) in &urls[1..] {
            if url != reference {
                findings.push(Finding::warning(
                    "bootstrap-url",
                    format!(
                        "bootstrap base_url differs between '{}' ({}) and '{}' ({})",
                        urls[0].0, reference, name, url
                    ),
                ));
            }
        }
    }

    // Cross-check with compose server port
    if let Some(compose) = &topo.compose {
        if compose.services.contains_key("server") {
            for (name, cfg) in &topo.configs {
                if let Some(url) = &cfg.bootstrap_base_url {
                    let host = url
                        .trim_start_matches("http://")
                        .trim_start_matches("https://");
                    let hostname = host.split(':').next().unwrap_or(host);
                    if hostname != "server" && hostname != "localhost" && hostname != "127.0.0.1" {
                        findings.push(Finding::warning(
                            "bootstrap-host",
                            format!(
                                "'{name}' bootstrap URL hostname '{hostname}' doesn't match compose service 'server'"
                            ),
                        ));
                    }
                }
            }
        }
    }

    CheckResult::from_findings("bootstrap-url-consistency", findings)
}

fn check_stream_subject_alignment(topo: &Topology) -> CheckResult {
    let mut findings = Vec::new();
    let source = match &topo.source {
        Some(s) => s,
        None => return CheckResult::skip("stream-subject-alignment", "source not scanned"),
    };

    // For each stream, verify its subjects appear in the global subject list
    for (stream_name, stream_subjects) in &source.streams {
        for subject_pattern in stream_subjects {
            // A wildcard pattern like "dataplane.ingestion.received.>" should match
            // concrete subjects starting with the prefix
            let prefix = subject_pattern.trim_end_matches(".>");
            let has_matching = source.subjects.iter().any(|s| {
                s.starts_with(prefix) || s == subject_pattern
            });
            if !has_matching {
                findings.push(Finding::warning(
                    "stream-subject",
                    format!(
                        "stream '{stream_name}' declares subject '{subject_pattern}' but no matching subject found in source"
                    ),
                ));
            }
        }
    }

    CheckResult::from_findings("stream-subject-alignment", findings)
}

fn check_durable_stream_alignment(topo: &Topology) -> CheckResult {
    let mut findings = Vec::new();
    let source = match &topo.source {
        Some(s) => s,
        None => return CheckResult::skip("durable-stream-alignment", "source not scanned"),
    };

    for (durable_name, durable_stream) in &source.durables {
        if !source.streams.contains_key(durable_stream.as_str()) {
            findings.push(Finding::error(
                "durable-stream",
                format!(
                    "durable '{durable_name}' references stream '{durable_stream}' which was not found"
                ),
            ));
        }
    }

    CheckResult::from_findings("durable-stream-alignment", findings)
}

fn check_pipeline_continuity(topo: &Topology) -> CheckResult {
    let mut findings = Vec::new();

    // Verify the pipeline: emulator -> kafka -> consumer -> jetstream -> validator
    // Each stage must be a compose service and have matching config
    let pipeline = [
        ("emulator", "kafka", "emulator produces to kafka"),
        ("consumer", "kafka", "consumer reads from kafka"),
        ("consumer", "nats", "consumer publishes to jetstream"),
        ("validator", "nats", "validator consumes from jetstream"),
    ];

    for (service, transport, description) in &pipeline {
        let has_transport = topo.configs.get(*service).map_or(false, |cfg| match *transport {
            "kafka" => !cfg.kafka_brokers.is_empty(),
            "nats" => cfg.nats_url.is_some(),
            _ => false,
        });

        if !has_transport {
            findings.push(Finding::error(
                "pipeline-continuity",
                format!("{description}: '{service}' has no {transport} config"),
            ));
        }
    }

    // Verify consumer bridges kafka to nats (has both)
    if let Some(consumer) = topo.configs.get("consumer") {
        if consumer.kafka_brokers.is_empty() || consumer.nats_url.is_none() {
            findings.push(Finding::error(
                "pipeline-bridge",
                "consumer must have both kafka and nats to bridge the pipeline",
            ));
        }
    }

    // Verify the dataplane stream exists and has a durable consumer
    if let Some(source) = &topo.source {
        let has_dataplane_stream = source.streams.contains_key("DATA_PLANE_INGESTION");
        let has_validator_durable = source.durables.contains_key("validator-dataplane-v1");

        if has_dataplane_stream && !has_validator_durable {
            findings.push(Finding::error(
                "pipeline-subscriber",
                "DATA_PLANE_INGESTION stream exists but validator durable consumer not found",
            ));
        }
        if !has_dataplane_stream && has_validator_durable {
            findings.push(Finding::error(
                "pipeline-stream",
                "validator durable consumer exists but DATA_PLANE_INGESTION stream not found",
            ));
        }
    }

    CheckResult::from_findings("pipeline-continuity", findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Severity;

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

    #[test]
    fn config_check_passes_with_all_services() {
        let mut configs = HashMap::new();
        configs.insert("consumer".into(), make_consumer_config());
        configs.insert("emulator".into(), make_emulator_config());
        configs.insert("validator".into(), make_validator_config());

        let result = check_configs(&configs);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn config_check_warns_missing_service() {
        let mut configs = HashMap::new();
        configs.insert("consumer".into(), make_consumer_config());
        // missing emulator and validator

        let result = check_configs(&configs);
        assert!(result.findings.iter().any(|f| f.severity == Severity::Warning));
    }

    #[test]
    fn config_check_errors_consumer_without_kafka() {
        let mut configs = HashMap::new();
        let mut consumer = make_consumer_config();
        consumer.kafka_brokers.clear();
        configs.insert("consumer".into(), consumer);
        configs.insert("emulator".into(), make_emulator_config());
        configs.insert("validator".into(), make_validator_config());

        let result = check_configs(&configs);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.contains("kafka")));
    }

    #[test]
    fn kafka_broker_consistency_ok_when_matching() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        topo.configs.insert("emulator".into(), make_emulator_config());

        let result = check_kafka_broker_consistency(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn kafka_broker_consistency_warns_on_mismatch() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        let mut emulator = make_emulator_config();
        emulator.kafka_brokers = vec!["other-host:9092".into()];
        topo.configs.insert("emulator".into(), emulator);

        let result = check_kafka_broker_consistency(&topo);
        assert!(result.findings.iter().any(|f| f.severity == Severity::Warning));
    }

    #[test]
    fn nats_url_consistency_ok_when_matching() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        topo.configs.insert("validator".into(), make_validator_config());

        let result = check_nats_url_consistency(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn durable_stream_alignment_fails_on_orphan() {
        let mut source = make_source_topology();
        source.durables.insert("orphan-durable".into(), "NONEXISTENT_STREAM".into());
        let mut topo = Topology::default();
        topo.source = Some(source);

        let result = check_durable_stream_alignment(&topo);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.contains("NONEXISTENT_STREAM")));
    }

    #[test]
    fn pipeline_continuity_passes_with_complete_config() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        topo.configs.insert("emulator".into(), make_emulator_config());
        topo.configs.insert("validator".into(), make_validator_config());
        topo.source = Some(make_source_topology());

        let result = check_pipeline_continuity(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn pipeline_continuity_fails_without_validator_nats() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        topo.configs.insert("emulator".into(), make_emulator_config());
        let mut validator = make_validator_config();
        validator.nats_url = None;
        topo.configs.insert("validator".into(), validator);

        let result = check_pipeline_continuity(&topo);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.contains("validator")));
    }

    #[test]
    fn stream_subject_alignment_passes_when_matched() {
        let mut topo = Topology::default();
        topo.source = Some(make_source_topology());

        let result = check_stream_subject_alignment(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn stream_subject_alignment_warns_orphan_stream_subject() {
        let mut source = make_source_topology();
        source.streams.insert(
            "ORPHAN_STREAM".into(),
            vec!["orphan.events.>".into()],
        );
        let mut topo = Topology::default();
        topo.source = Some(source);

        let result = check_stream_subject_alignment(&topo);
        assert!(result
            .findings
            .iter()
            .any(|f| f.message.contains("ORPHAN_STREAM")));
    }

    #[test]
    fn stream_subject_alignment_skips_without_source() {
        let topo = Topology::default();
        let result = check_stream_subject_alignment(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Skip);
    }

    #[test]
    fn durable_stream_alignment_skips_without_source() {
        let topo = Topology::default();
        let result = check_durable_stream_alignment(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Skip);
    }

    #[test]
    fn durable_stream_alignment_passes_when_all_match() {
        let mut topo = Topology::default();
        topo.source = Some(make_source_topology());
        let result = check_durable_stream_alignment(&topo);
        assert_eq!(result.status, crate::models::CheckStatus::Pass);
    }

    #[test]
    fn nats_url_consistency_warns_on_mismatch() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        let mut validator = make_validator_config();
        validator.nats_url = Some("nats://other-host:4222".into());
        topo.configs.insert("validator".into(), validator);

        let result = check_nats_url_consistency(&topo);
        assert!(result.findings.iter().any(|f| f.severity == Severity::Warning));
    }

    #[test]
    fn bootstrap_url_consistency_warns_on_mismatch() {
        let mut topo = Topology::default();
        topo.configs.insert("consumer".into(), make_consumer_config());
        let mut emulator = make_emulator_config();
        emulator.bootstrap_base_url = Some("http://other-server:9090".into());
        topo.configs.insert("emulator".into(), emulator);

        let result = check_bootstrap_url_consistency(&topo);
        assert!(result.findings.iter().any(|f| f.severity == Severity::Warning));
    }

    #[test]
    fn pipeline_continuity_fails_without_consumer_bridge() {
        let mut topo = Topology::default();
        let mut consumer = make_consumer_config();
        consumer.kafka_brokers.clear(); // remove kafka → broken bridge
        topo.configs.insert("consumer".into(), consumer);
        topo.configs.insert("emulator".into(), make_emulator_config());
        topo.configs.insert("validator".into(), make_validator_config());

        let result = check_pipeline_continuity(&topo);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error));
    }

    #[test]
    fn config_check_errors_emulator_without_kafka() {
        let mut configs = HashMap::new();
        configs.insert("consumer".into(), make_consumer_config());
        let mut emulator = make_emulator_config();
        emulator.kafka_brokers.clear();
        configs.insert("emulator".into(), emulator);
        configs.insert("validator".into(), make_validator_config());

        let result = check_configs(&configs);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.contains("emulator")));
    }

    #[test]
    fn config_check_errors_validator_without_nats() {
        let mut configs = HashMap::new();
        configs.insert("consumer".into(), make_consumer_config());
        configs.insert("emulator".into(), make_emulator_config());
        let mut validator = make_validator_config();
        validator.nats_url = None;
        configs.insert("validator".into(), validator);

        let result = check_configs(&configs);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.message.contains("validator")));
    }

    #[test]
    fn analyze_returns_report_on_empty_configs_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/configs")).unwrap();

        let report = analyze(dir.path()).unwrap();
        assert_eq!(report.title, "topology-doctor");
        // Should proceed past phase 1 with empty configs
    }

    #[test]
    fn analyze_fails_when_no_configs_dir() {
        let dir = tempfile::tempdir().unwrap();
        let report = analyze(dir.path()).unwrap();
        assert!(!report.passed());
        assert!(report.checks.iter().any(|c| c.name == "configs-dir-exists"));
    }
}
