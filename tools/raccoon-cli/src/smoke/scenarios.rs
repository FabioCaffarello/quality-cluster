use crate::models::{CheckResult, Finding, Report};
use super::api::ApiClient;
use super::stages;
use super::SmokeConfig;
use serde::Serialize;
use std::thread;
use std::time::{Duration, Instant};

/// A named, reproducible validation scenario for the quality-service cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scenario {
    HappyPath,
    ConfigLifecycle,
    InvalidPayload,
    MissingBinding,
    ReadinessProbe,
}

impl Scenario {
    pub fn name(&self) -> &'static str {
        match self {
            Scenario::HappyPath => "happy-path",
            Scenario::ConfigLifecycle => "config-lifecycle",
            Scenario::InvalidPayload => "invalid-payload",
            Scenario::MissingBinding => "missing-binding",
            Scenario::ReadinessProbe => "readiness-probe",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Scenario::HappyPath => {
                "Full E2E: config lifecycle + data plane + validation results (passed + failed)"
            }
            Scenario::ConfigLifecycle => {
                "Control plane only: draft -> validate -> compile -> activate -> verify active config"
            }
            Scenario::InvalidPayload => {
                "Activate config and verify validator catches invalid payloads (failed results)"
            }
            Scenario::MissingBinding => {
                "Query non-existent binding/scope and verify empty results (no errors)"
            }
            Scenario::ReadinessProbe => {
                "Verify cluster bootstrap and readiness (healthz + readyz)"
            }
        }
    }

    pub fn preconditions(&self) -> &'static [&'static str] {
        match self {
            Scenario::HappyPath => &[
                "All 7 compose services running (make up-dataplane)",
                "Server responding on base-url",
                "Emulator publishing synthetic data",
            ],
            Scenario::ConfigLifecycle => &[
                "Core services running: nats, configctl, server",
                "Server responding on base-url",
            ],
            Scenario::InvalidPayload => &[
                "All 7 compose services running (make up-dataplane)",
                "Emulator producing invalid samples",
            ],
            Scenario::MissingBinding => &[
                "Server responding on base-url",
            ],
            Scenario::ReadinessProbe => &[
                "Compose services running",
                "Server reachable on base-url",
            ],
        }
    }

    /// Parse a scenario name string into a Scenario enum.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "happy-path" => Some(Scenario::HappyPath),
            "config-lifecycle" => Some(Scenario::ConfigLifecycle),
            "invalid-payload" => Some(Scenario::InvalidPayload),
            "missing-binding" => Some(Scenario::MissingBinding),
            "readiness-probe" => Some(Scenario::ReadinessProbe),
            _ => None,
        }
    }

    /// All available scenario names.
    pub fn all_names() -> &'static [&'static str] {
        &[
            "happy-path",
            "config-lifecycle",
            "invalid-payload",
            "missing-binding",
            "readiness-probe",
        ]
    }

    /// All available scenarios.
    pub fn all() -> &'static [Scenario] {
        &[
            Scenario::HappyPath,
            Scenario::ConfigLifecycle,
            Scenario::InvalidPayload,
            Scenario::MissingBinding,
            Scenario::ReadinessProbe,
        ]
    }
}

impl std::fmt::Display for Scenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Run a named scenario and return a Report.
pub fn run_scenario(scenario: Scenario, config: &SmokeConfig) -> Report {
    match scenario {
        Scenario::HappyPath => run_happy_path(config),
        Scenario::ConfigLifecycle => run_config_lifecycle(config),
        Scenario::InvalidPayload => run_invalid_payload(config),
        Scenario::MissingBinding => run_missing_binding(config),
        Scenario::ReadinessProbe => run_readiness_probe(config),
    }
}

/// List all scenarios with their descriptions (for --list output).
pub fn list_scenarios() -> Vec<(&'static str, &'static str)> {
    Scenario::all()
        .iter()
        .map(|s| (s.name(), s.description()))
        .collect()
}

// ── Scenario Implementations ─────────────────────────────────────────

/// happy-path: Full E2E pipeline validation.
/// Reuses all 6 runtime-smoke stages.
fn run_happy_path(config: &SmokeConfig) -> Report {
    let mut report = Report::new("scenario-smoke: happy-path");

    let stage_fns: Vec<(&str, Box<dyn Fn(&SmokeConfig) -> CheckResult>)> = vec![
        ("bootstrap", Box::new(stages::bootstrap)),
        ("readiness", Box::new(stages::readiness)),
        ("inject", Box::new(stages::inject)),
        ("route", Box::new(stages::route)),
        ("consume", Box::new(stages::consume)),
        ("validate", Box::new(stages::validate)),
    ];

    run_stages_sequential(&mut report, &stage_fns, config);
    report
}

/// config-lifecycle: Control plane only — no data plane required.
fn run_config_lifecycle(config: &SmokeConfig) -> Report {
    let mut report = Report::new("scenario-smoke: config-lifecycle");
    let client = ApiClient::new(&config.base_url);

    // Stage 1: Readiness (reuse existing stage)
    let readiness = stages::readiness(config);
    let ready = readiness.status == crate::models::CheckStatus::Pass;
    report.add(readiness);
    if !ready {
        report.add(CheckResult::skip("create-draft", "skipped: readiness failed"));
        report.add(CheckResult::skip("validate", "skipped: readiness failed"));
        report.add(CheckResult::skip("compile", "skipped: readiness failed"));
        report.add(CheckResult::skip("activate", "skipped: readiness failed"));
        report.add(CheckResult::skip("verify-active", "skipped: readiness failed"));
        return report;
    }

    // Stage 2: Create draft
    let content = stages::smoke_config_content();
    let draft_resp = match client.create_draft("scenario-lifecycle", &content) {
        Ok(v) => v,
        Err(e) => {
            report.add(CheckResult::from_findings(
                "create-draft",
                vec![Finding::error("create-draft", format!("create draft failed: {e}"))],
            ));
            skip_remaining(&mut report, &["validate", "compile", "activate", "verify-active"], "create-draft");
            return report;
        }
    };

    let config_id = match draft_resp.get("id").and_then(|v| v.as_str()) {
        Some(id) => {
            let mut result = CheckResult::pass("create-draft");
            result.findings.push(Finding::info(
                "create-draft",
                format!("draft created: {id}"),
            ));
            report.add(result);
            id.to_string()
        }
        None => {
            report.add(CheckResult::from_findings(
                "create-draft",
                vec![Finding::error("create-draft", "response missing 'id' field")],
            ));
            skip_remaining(&mut report, &["validate", "compile", "activate", "verify-active"], "create-draft");
            return report;
        }
    };

    // Stage 3: Validate
    match client.validate_config(&config_id) {
        Ok(resp) => {
            let valid = resp.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
            if valid {
                let mut result = CheckResult::pass("validate");
                result.findings.push(Finding::info("validate", "config validated successfully"));
                report.add(result);
            } else {
                let diags = resp.get("diagnostics")
                    .and_then(|d| serde_json::to_string(d).ok())
                    .unwrap_or_default();
                report.add(CheckResult::from_findings(
                    "validate",
                    vec![Finding::error("validate", format!("validation returned invalid: {diags}"))],
                ));
                skip_remaining(&mut report, &["compile", "activate", "verify-active"], "validate");
                return report;
            }
        }
        Err(e) => {
            report.add(CheckResult::from_findings(
                "validate",
                vec![Finding::error("validate", format!("validate request failed: {e}"))],
            ));
            skip_remaining(&mut report, &["compile", "activate", "verify-active"], "validate");
            return report;
        }
    }

    // Stage 4: Compile
    match client.compile_config(&config_id) {
        Ok(resp) => {
            let has_artifact = resp.get("artifact").is_some();
            if has_artifact {
                let mut result = CheckResult::pass("compile");
                result.findings.push(Finding::info("compile", "compilation artifact generated"));
                report.add(result);
            } else {
                let mut result = CheckResult::pass("compile");
                result.findings.push(Finding::info("compile", "compiled (no artifact in response)"));
                report.add(result);
            }
        }
        Err(e) => {
            report.add(CheckResult::from_findings(
                "compile",
                vec![Finding::error("compile", format!("compile request failed: {e}"))],
            ));
            skip_remaining(&mut report, &["activate", "verify-active"], "compile");
            return report;
        }
    }

    // Stage 5: Activate
    if let Err(e) = client.activate_config(&config_id) {
        report.add(CheckResult::from_findings(
            "activate",
            vec![Finding::error("activate", format!("activate request failed: {e}"))],
        ));
        skip_remaining(&mut report, &["verify-active"], "activate");
        return report;
    }
    let mut result = CheckResult::pass("activate");
    result.findings.push(Finding::info("activate", format!("config {config_id} activated in global:default")));
    report.add(result);

    // Stage 6: Verify active config
    match client.get_active_config() {
        Ok(resp) => {
            let active_id = resp.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if active_id == config_id {
                let mut result = CheckResult::pass("verify-active");
                result.findings.push(Finding::info(
                    "verify-active",
                    "active config matches the activated version",
                ));
                report.add(result);
            } else if !active_id.is_empty() {
                let mut result = CheckResult::pass("verify-active");
                result.findings.push(Finding::info(
                    "verify-active",
                    format!("active config found (id: {active_id})"),
                ));
                report.add(result);
            } else {
                report.add(CheckResult::from_findings(
                    "verify-active",
                    vec![Finding::error(
                        "verify-active",
                        "no active config found after activation",
                    )],
                ));
            }
        }
        Err(e) => {
            report.add(CheckResult::from_findings(
                "verify-active",
                vec![Finding::error("verify-active", format!("failed to query active config: {e}"))],
            ));
        }
    }

    report
}

/// invalid-payload: Prove the validator catches bad data.
fn run_invalid_payload(config: &SmokeConfig) -> Report {
    let mut report = Report::new("scenario-smoke: invalid-payload");

    // Reuse bootstrap + readiness + inject + route stages
    let stage_fns: Vec<(&str, Box<dyn Fn(&SmokeConfig) -> CheckResult>)> = vec![
        ("bootstrap", Box::new(stages::bootstrap)),
        ("readiness", Box::new(stages::readiness)),
        ("inject", Box::new(stages::inject)),
        ("route", Box::new(stages::route)),
    ];

    let failed_at = run_stages_sequential(&mut report, &stage_fns, config);
    if failed_at.is_some() {
        report.add(CheckResult::skip("wait-for-failures", "skipped: prior stage failed"));
        report.add(CheckResult::skip("verify-violations", "skipped: prior stage failed"));
        return report;
    }

    // Stage 5: Wait specifically for failed validation results
    let client = ApiClient::new(&config.base_url);
    let deadline = Instant::now() + Duration::from_secs(config.results_timeout_secs);
    let interval = Duration::from_millis(config.poll_interval_ms);

    let mut found_failed = false;
    while Instant::now() < deadline {
        if let Ok(resp) = client.validation_results(20) {
            if let Some(results) = resp.get("results").and_then(|r| r.as_array()) {
                found_failed = results.iter().any(|r| {
                    r.get("status").and_then(|s| s.as_str()) == Some("failed")
                });
                if found_failed {
                    break;
                }
            }
        }
        thread::sleep(interval);
    }

    if found_failed {
        let mut result = CheckResult::pass("wait-for-failures");
        result.findings.push(Finding::info(
            "wait-for-failures",
            "validator produced 'failed' results from invalid emulator samples",
        ));
        report.add(result);
    } else {
        report.add(CheckResult::from_findings(
            "wait-for-failures",
            vec![Finding::error(
                "wait-for-failures",
                format!(
                    "no 'failed' validation results within {}s — emulator may not have produced invalid samples yet",
                    config.results_timeout_secs
                ),
            )
            .with_why("invalid payloads should be caught by validation rules")
            .with_help("check emulator logs; ensure it produces SyntheticScenarioInvalidMissingField samples")],
        ));
        report.add(CheckResult::skip("verify-violations", "skipped: no failed results to inspect"));
        return report;
    }

    // Stage 6: Verify violation structure
    match client.validation_results(20) {
        Ok(resp) => {
            let results = resp.get("results").and_then(|r| r.as_array());
            let mut findings = Vec::new();
            let mut has_violations = false;

            if let Some(results) = results {
                for entry in results {
                    if entry.get("status").and_then(|s| s.as_str()) == Some("failed") {
                        if let Some(violations) = entry.get("violations").and_then(|v| v.as_array()) {
                            if !violations.is_empty() {
                                has_violations = true;
                                let violation = &violations[0];
                                let rule = violation.get("rule").and_then(|r| r.as_str()).unwrap_or("unknown");
                                let field = violation.get("field").and_then(|f| f.as_str()).unwrap_or("unknown");
                                findings.push(Finding::info(
                                    "verify-violations",
                                    format!("violation found: rule={rule}, field={field}"),
                                ));
                                break;
                            }
                        }
                    }
                }
            }

            if has_violations {
                findings.insert(0, Finding::info(
                    "verify-violations",
                    "failed results contain structured violations with rule/field/severity",
                ));
                report.add(CheckResult::from_findings("verify-violations", findings));
            } else {
                report.add(CheckResult::from_findings(
                    "verify-violations",
                    vec![Finding::error(
                        "verify-violations",
                        "failed results have no violations array — validator may not be recording violation details",
                    )],
                ));
            }
        }
        Err(e) => {
            report.add(CheckResult::from_findings(
                "verify-violations",
                vec![Finding::error("verify-violations", format!("failed to fetch results: {e}"))],
            ));
        }
    }

    report
}

/// missing-binding: Verify the system handles non-existent queries gracefully.
fn run_missing_binding(config: &SmokeConfig) -> Report {
    let mut report = Report::new("scenario-smoke: missing-binding");
    let client = ApiClient::new(&config.base_url);

    // Stage 1: Readiness
    let readiness = stages::readiness(config);
    let ready = readiness.status == crate::models::CheckStatus::Pass;
    report.add(readiness);
    if !ready {
        report.add(CheckResult::skip("query-missing-bindings", "skipped: readiness failed"));
        report.add(CheckResult::skip("query-missing-results", "skipped: readiness failed"));
        return report;
    }

    // Stage 2: Query bindings for a non-existent scope
    match client.ingestion_bindings_scoped("nonexistent-tenant", "nonexistent-key") {
        Ok(resp) => {
            let bindings = resp
                .get("bindings")
                .and_then(|b| b.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if bindings == 0 {
                let mut result = CheckResult::pass("query-missing-bindings");
                result.findings.push(Finding::info(
                    "query-missing-bindings",
                    "non-existent scope returns empty bindings (no error)",
                ));
                report.add(result);
            } else {
                report.add(CheckResult::from_findings(
                    "query-missing-bindings",
                    vec![Finding::warning(
                        "query-missing-bindings",
                        format!("expected 0 bindings for nonexistent scope, got {bindings}"),
                    )],
                ));
            }
        }
        Err(e) => {
            // A connection-level error is a real failure; a 404/empty is expected
            report.add(CheckResult::from_findings(
                "query-missing-bindings",
                vec![Finding::error(
                    "query-missing-bindings",
                    format!("request failed (expected empty response, not error): {e}"),
                )
                .with_why("querying non-existent scopes should return empty results, not HTTP errors")
                .with_help("check if the /runtime/ingestion/bindings endpoint handles unknown scopes")],
            ));
        }
    }

    // Stage 3: Query results for a non-existent scope
    match client.validation_results_scoped("nonexistent-tenant", "nonexistent-key", 10) {
        Ok(resp) => {
            let results = resp
                .get("results")
                .and_then(|r| r.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if results == 0 {
                let mut result = CheckResult::pass("query-missing-results");
                result.findings.push(Finding::info(
                    "query-missing-results",
                    "non-existent scope returns empty results (no error)",
                ));
                report.add(result);
            } else {
                report.add(CheckResult::from_findings(
                    "query-missing-results",
                    vec![Finding::warning(
                        "query-missing-results",
                        format!("expected 0 results for nonexistent scope, got {results}"),
                    )],
                ));
            }
        }
        Err(e) => {
            report.add(CheckResult::from_findings(
                "query-missing-results",
                vec![Finding::error(
                    "query-missing-results",
                    format!("request failed (expected empty response, not error): {e}"),
                )
                .with_why("querying non-existent scopes should return empty results, not HTTP errors")
                .with_help("check if the /runtime/validator/results endpoint handles unknown scopes")],
            ));
        }
    }

    report
}

/// readiness-probe: Quick cluster health check.
fn run_readiness_probe(config: &SmokeConfig) -> Report {
    let mut report = Report::new("scenario-smoke: readiness-probe");

    // Stage 1: Bootstrap
    let bootstrap = stages::bootstrap(config);
    let ok = bootstrap.status == crate::models::CheckStatus::Pass;
    report.add(bootstrap);
    if !ok {
        report.add(CheckResult::skip("readiness", "skipped: bootstrap failed"));
        return report;
    }

    // Stage 2: Readiness
    report.add(stages::readiness(config));
    report
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Run stages sequentially, skipping remaining on failure. Returns the name of the
/// first failed stage, if any.
fn run_stages_sequential<'a>(
    report: &mut Report,
    stage_fns: &[(&'a str, Box<dyn Fn(&SmokeConfig) -> CheckResult>)],
    config: &SmokeConfig,
) -> Option<&'a str> {
    let mut failed_at: Option<&str> = None;

    for (name, stage_fn) in stage_fns {
        if let Some(blocker) = failed_at {
            report.add(CheckResult::skip(
                *name,
                format!("skipped: {blocker} failed"),
            ));
            continue;
        }

        let result = stage_fn(config);
        let ok = result.status == crate::models::CheckStatus::Pass;
        report.add(result);
        if !ok {
            failed_at = Some(name);
        }
    }

    failed_at
}

/// Add skip results for remaining stage names.
fn skip_remaining(report: &mut Report, names: &[&str], blocker: &str) {
    for name in names {
        report.add(CheckResult::skip(*name, format!("skipped: {blocker} failed")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CheckStatus;

    // ── Scenario parsing ─────────────────────────────────────────────

    #[test]
    fn parse_all_valid_scenarios() {
        assert_eq!(Scenario::parse("happy-path"), Some(Scenario::HappyPath));
        assert_eq!(Scenario::parse("config-lifecycle"), Some(Scenario::ConfigLifecycle));
        assert_eq!(Scenario::parse("invalid-payload"), Some(Scenario::InvalidPayload));
        assert_eq!(Scenario::parse("missing-binding"), Some(Scenario::MissingBinding));
        assert_eq!(Scenario::parse("readiness-probe"), Some(Scenario::ReadinessProbe));
    }

    #[test]
    fn parse_invalid_scenario_returns_none() {
        assert_eq!(Scenario::parse("nonexistent"), None);
        assert_eq!(Scenario::parse(""), None);
        assert_eq!(Scenario::parse("HAPPY-PATH"), None);
    }

    #[test]
    fn all_names_matches_all_scenarios() {
        let names = Scenario::all_names();
        let scenarios = Scenario::all();
        assert_eq!(names.len(), scenarios.len());
        for (name, scenario) in names.iter().zip(scenarios.iter()) {
            assert_eq!(*name, scenario.name());
        }
    }

    #[test]
    fn scenario_display_matches_name() {
        for scenario in Scenario::all() {
            assert_eq!(scenario.to_string(), scenario.name());
        }
    }

    #[test]
    fn all_scenarios_have_descriptions() {
        for scenario in Scenario::all() {
            assert!(!scenario.description().is_empty());
        }
    }

    #[test]
    fn all_scenarios_have_preconditions() {
        for scenario in Scenario::all() {
            assert!(!scenario.preconditions().is_empty());
        }
    }

    #[test]
    fn list_scenarios_returns_all() {
        let list = list_scenarios();
        assert_eq!(list.len(), Scenario::all().len());
        for (name, desc) in &list {
            assert!(!name.is_empty());
            assert!(!desc.is_empty());
        }
    }

    // ── Scenario execution with unreachable server ───────────────────

    #[test]
    fn happy_path_fails_when_compose_missing() {
        let config = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run_scenario(Scenario::HappyPath, &config);
        assert!(!report.passed());
        assert_eq!(report.checks[0].name, "bootstrap");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
        // Remaining stages should be skipped
        for check in &report.checks[1..] {
            assert_eq!(check.status, CheckStatus::Skip);
        }
    }

    #[test]
    fn config_lifecycle_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        config.readiness_timeout_secs = 1;
        config.poll_interval_ms = 200;
        let report = run_scenario(Scenario::ConfigLifecycle, &config);
        assert!(!report.passed());
        assert_eq!(report.checks[0].name, "readiness");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
        // Remaining stages should be skipped
        for check in &report.checks[1..] {
            assert_eq!(check.status, CheckStatus::Skip);
        }
    }

    #[test]
    fn invalid_payload_fails_when_compose_missing() {
        let config = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run_scenario(Scenario::InvalidPayload, &config);
        assert!(!report.passed());
        assert_eq!(report.checks[0].name, "bootstrap");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
    }

    #[test]
    fn missing_binding_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        config.readiness_timeout_secs = 1;
        config.poll_interval_ms = 200;
        let report = run_scenario(Scenario::MissingBinding, &config);
        assert!(!report.passed());
        assert_eq!(report.checks[0].name, "readiness");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
    }

    #[test]
    fn readiness_probe_fails_when_compose_missing() {
        let config = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run_scenario(Scenario::ReadinessProbe, &config);
        assert!(!report.passed());
        assert_eq!(report.checks[0].name, "bootstrap");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
        assert_eq!(report.checks[1].status, CheckStatus::Skip);
    }

    // ── Report titles ────────────────────────────────────────────────

    #[test]
    fn scenario_report_titles_include_scenario_name() {
        let config = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        for scenario in Scenario::all() {
            let report = run_scenario(*scenario, &config);
            assert!(
                report.title.contains(scenario.name()),
                "report title '{}' should contain scenario name '{}'",
                report.title,
                scenario.name()
            );
        }
    }

    // ── Serialization ────────────────────────────────────────────────

    #[test]
    fn scenario_serializes_to_kebab_case() {
        let json = serde_json::to_string(&Scenario::HappyPath).unwrap();
        assert_eq!(json, "\"happy-path\"");
        let json = serde_json::to_string(&Scenario::ConfigLifecycle).unwrap();
        assert_eq!(json, "\"config-lifecycle\"");
    }

    // ── skip_remaining helper ────────────────────────────────────────

    #[test]
    fn skip_remaining_adds_correct_skip_checks() {
        let mut report = Report::new("test");
        skip_remaining(&mut report, &["a", "b", "c"], "blocker");
        assert_eq!(report.checks.len(), 3);
        for check in &report.checks {
            assert_eq!(check.status, CheckStatus::Skip);
            assert!(check.findings[0].message.contains("blocker"));
        }
    }
}
