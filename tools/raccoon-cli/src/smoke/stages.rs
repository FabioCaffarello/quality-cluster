use super::api::ApiClient;
use super::compose;
use super::SmokeConfig;
use crate::models::{CheckResult, Finding};
use serde_json::Value;
use std::thread;
use std::time::{Duration, Instant};

/// Synthetic config content for the smoke test.
pub fn smoke_config_content(config: &SmokeConfig) -> serde_json::Value {
    serde_json::json!({
        "metadata": {
            "name": "Smoke Test",
            "description": format!("raccoon-cli runtime smoke test config ({})", config.run_id)
        },
        "bindings": [
            {
                "name": config.binding_name,
                "topic": "smoke.events.created"
            }
        ],
        "fields": [
            { "name": "event_id", "type": "string", "required": true },
            { "name": "status",   "type": "string", "required": true },
            { "name": "amount",   "type": "number", "required": false }
        ],
        "rules": [
            {
                "name": "event_id_required",
                "field": "event_id",
                "operator": "required",
                "severity": "error"
            },
            {
                "name": "status_not_empty",
                "field": "status",
                "operator": "not_empty",
                "severity": "error"
            }
        ]
    })
}

/// Stage 1: Bootstrap — verify compose services are running.
pub fn bootstrap(config: &SmokeConfig) -> CheckResult {
    bootstrap_required(config, compose::REQUIRED_SERVICES, "make up-dataplane")
}

pub fn bootstrap_required(
    config: &SmokeConfig,
    required_services: &[&str],
    help_command: &str,
) -> CheckResult {
    if !config.compose_file.exists() {
        return CheckResult::from_findings(
            "bootstrap",
            vec![Finding::error(
                "bootstrap",
                format!("compose file not found: {}", config.compose_file.display()),
            )],
        );
    }

    let running = match compose::running_services(&config.compose_file) {
        Ok(r) => r,
        Err(e) => {
            return CheckResult::from_findings("bootstrap", vec![Finding::error("bootstrap", e)]);
        }
    };

    let missing = compose::missing_required_services(&running, required_services);
    if missing.is_empty() {
        let mut result = CheckResult::pass("bootstrap");
        result.findings.push(Finding::info(
            "bootstrap",
            format!(
                "required services available: {}",
                required_services.join(", ")
            ),
        ));
        result
    } else {
        CheckResult::from_findings(
            "bootstrap",
            vec![Finding::error(
                "bootstrap",
                format!(
                    "missing services: {}. Run `{help_command}` first.",
                    missing.join(", ")
                ),
            )],
        )
    }
}

/// Stage 2: Readiness — poll healthz and readyz until both return 200.
pub fn readiness(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url, &config.run_id);
    let deadline = Instant::now() + Duration::from_secs(config.readiness_timeout_secs);
    let interval = Duration::from_millis(config.poll_interval_ms);

    let mut last_health_err = String::new();
    let mut last_ready_err = String::new();

    while Instant::now() < deadline {
        match client.healthz() {
            Ok(200) => {}
            Ok(code) => {
                last_health_err = format!("/healthz returned {code}");
                thread::sleep(interval);
                continue;
            }
            Err(e) => {
                last_health_err = e;
                thread::sleep(interval);
                continue;
            }
        }

        match client.readyz() {
            Ok(200) => {
                let mut result = CheckResult::pass("readiness");
                result
                    .findings
                    .push(Finding::info("readiness", "healthz=200, readyz=200"));
                return result;
            }
            Ok(code) => {
                last_ready_err = format!("/readyz returned {code}");
            }
            Err(e) => {
                last_ready_err = e;
            }
        }

        thread::sleep(interval);
    }

    let mut msg = format!(
        "timed out after {}s waiting for readiness",
        config.readiness_timeout_secs
    );
    if !last_health_err.is_empty() {
        msg.push_str(&format!(". Last healthz error: {last_health_err}"));
    }
    if !last_ready_err.is_empty() {
        msg.push_str(&format!(". Last readyz error: {last_ready_err}"));
    }

    CheckResult::from_findings("readiness", vec![Finding::error("readiness", msg)])
}

/// Stage 3: Inject — create config, validate, compile, activate.
pub fn inject(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url, &config.run_id);
    let content = smoke_config_content(config);

    // Create draft
    let draft_resp = match client.create_draft(&config.config_key, &content) {
        Ok(v) => v,
        Err(e) => {
            return CheckResult::from_findings(
                "inject",
                vec![Finding::error(
                    "inject",
                    format!("create draft failed: {e}"),
                )],
            );
        }
    };

    let config_id = match draft_resp
        .get("config")
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
    {
        Some(id) => id.to_string(),
        None => {
            return CheckResult::from_findings(
                "inject",
                vec![Finding::error(
                    "inject",
                    format!(
                        "create draft response missing 'config.id' field: {}",
                        serde_json::to_string(&draft_resp).unwrap_or_default()
                    ),
                )],
            );
        }
    };

    // Validate
    if let Err(e) = client.validate_config(&config_id) {
        return CheckResult::from_findings(
            "inject",
            vec![Finding::error("inject", format!("validate failed: {e}"))],
        );
    }

    // Compile
    if let Err(e) = client.compile_config(&config_id) {
        return CheckResult::from_findings(
            "inject",
            vec![Finding::error("inject", format!("compile failed: {e}"))],
        );
    }

    // Activate
    if let Err(e) = client.activate_config(&config_id, &config.scope_kind, &config.scope_key) {
        return CheckResult::from_findings(
            "inject",
            vec![Finding::error("inject", format!("activate failed: {e}"))],
        );
    }

    let mut result = CheckResult::pass("inject");
    result.findings.push(Finding::info(
        "inject",
        format!(
            "config {config_id} created, validated, compiled, activated in {}:{}",
            config.scope_kind, config.scope_key
        ),
    ));
    result
}

/// Stage 4: Route — verify ingestion bindings are projected.
pub fn route(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url, &config.run_id);
    let deadline = Instant::now() + Duration::from_secs(10);
    let interval = Duration::from_millis(config.poll_interval_ms);

    while Instant::now() < deadline {
        match client.ingestion_bindings(&config.scope_kind, &config.scope_key) {
            Ok(resp) => {
                let bindings = resp
                    .get("bindings")
                    .and_then(|b| b.as_array())
                    .map(|entries| {
                        entries
                            .iter()
                            .filter(|entry| {
                                entry
                                    .get("binding")
                                    .and_then(|binding| binding.get("name"))
                                    .and_then(|v| v.as_str())
                                    == Some(config.binding_name.as_str())
                            })
                            .count()
                    })
                    .unwrap_or(0);

                if bindings > 0 {
                    let mut result = CheckResult::pass("route");
                    result.findings.push(Finding::info(
                        "route",
                        format!(
                            "{bindings} active ingestion binding(s) found for {}:{}",
                            config.scope_kind, config.scope_key
                        ),
                    ));
                    return result;
                }
            }
            Err(_) => {}
        }
        thread::sleep(interval);
    }

    CheckResult::from_findings(
        "route",
        vec![Finding::error(
            "route",
            "no active ingestion bindings found within 10s after activation",
        )],
    )
}

/// Stage 5: Consume — wait for validation results to appear.
/// The emulator publishes synthetic data every ~5s, so we wait up to results_timeout_secs.
pub fn consume(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url, &config.run_id);
    let deadline = Instant::now() + Duration::from_secs(config.results_timeout_secs);
    let interval = Duration::from_millis(config.poll_interval_ms);

    while Instant::now() < deadline {
        match client.validation_results(&config.scope_kind, &config.scope_key, 10) {
            Ok(resp) => {
                let count = resp
                    .get("results")
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                if count > 0 {
                    let mut result = CheckResult::pass("consume");
                    result.findings.push(Finding::info(
                        "consume",
                        format!(
                            "{count} validation result(s) received (Kafka->consumer->JetStream->validator pipeline confirmed)"
                        ),
                    ));
                    return result;
                }
            }
            Err(_) => {}
        }
        thread::sleep(interval);
    }

    let mut findings = vec![Finding::error(
        "consume",
        format!(
            "no validation results within {}s. Data pipeline may be stuck.",
            config.results_timeout_secs
        ),
    )];
    findings.extend(diagnose_pipeline_gap(&client, config));
    findings.push(Finding::info(
        "consume",
        "inspect consumer/emulator logs and run `raccoon-cli trace-pack` to confirm CONFIGCTL_EVENTS refresh durables and JetStream state",
    ));

    CheckResult::from_findings("consume", findings)
}

/// Stage 6: Validate — check that results include both passed and failed entries.
/// The emulator produces one valid and one invalid sample per binding per cycle.
pub fn validate(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url, &config.run_id);

    let resp = match client.validation_results(&config.scope_kind, &config.scope_key, 20) {
        Ok(v) => v,
        Err(e) => {
            return CheckResult::from_findings(
                "validate",
                vec![Finding::error(
                    "validate",
                    format!("failed to fetch results: {e}"),
                )],
            );
        }
    };

    let results = match resp.get("results").and_then(|r| r.as_array()) {
        Some(arr) => arr,
        None => {
            return CheckResult::from_findings(
                "validate",
                vec![Finding::error(
                    "validate",
                    "response has no 'results' array",
                )],
            );
        }
    };

    let mut has_passed = false;
    let mut has_failed = false;
    let mut total = 0;

    for entry in results {
        total += 1;
        if let Some(status) = entry.get("status").and_then(|s| s.as_str()) {
            match status {
                "passed" => has_passed = true,
                "failed" => has_failed = true,
                _ => {}
            }
        }
    }

    let mut findings = Vec::new();
    findings.push(Finding::info(
        "validate",
        format!("{total} result(s) inspected"),
    ));

    if has_passed {
        findings.push(Finding::info(
            "validate",
            "found 'passed' result — valid payload processed correctly",
        ));
    } else {
        findings.push(Finding::error(
            "validate",
            "no 'passed' result found; expected emulator valid sample to pass validation",
        ));
    }

    if has_failed {
        findings.push(Finding::info(
            "validate",
            "found 'failed' result — invalid payload caught by rules",
        ));
    } else {
        findings.push(Finding::warning(
            "validate",
            "no 'failed' result found; emulator may not have produced invalid sample yet",
        ));
    }

    CheckResult::from_findings("validate", findings)
}

fn diagnose_pipeline_gap(client: &ApiClient, config: &SmokeConfig) -> Vec<Finding> {
    let mut findings = Vec::new();

    match client.ingestion_bindings(&config.scope_kind, &config.scope_key) {
        Ok(resp) => {
            let binding_count = matching_binding_count(&resp, &config.binding_name);
            if binding_count > 0 {
                findings.push(Finding::info(
                    "consume",
                    format!(
                        "configctl projects {binding_count} active binding(s) for '{}' in {}:{}",
                        config.binding_name, config.scope_kind, config.scope_key
                    ),
                ));
            } else {
                findings.push(Finding::warning(
                    "consume",
                    format!(
                        "configctl projection does not show active binding '{}' in {}:{}",
                        config.binding_name, config.scope_kind, config.scope_key
                    ),
                ));
            }
        }
        Err(e) => findings.push(Finding::warning(
            "consume",
            format!("failed to inspect active ingestion bindings during diagnosis: {e}"),
        )),
    }

    match client.validator_runtime(&config.scope_kind, &config.scope_key) {
        Ok(resp) => match runtime_diagnostic(&resp) {
            Some(message) => findings.push(Finding::info("consume", message)),
            None => findings.push(Finding::warning(
                "consume",
                format!(
                    "validator runtime endpoint returned no loaded runtime for {}:{}",
                    config.scope_kind, config.scope_key
                ),
            )),
        },
        Err(e) => findings.push(Finding::warning(
            "consume",
            format!("failed to inspect validator runtime during diagnosis: {e}"),
        )),
    }

    findings
}

fn matching_binding_count(resp: &Value, binding_name: &str) -> usize {
    resp.get("bindings")
        .and_then(|b| b.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry
                        .get("binding")
                        .and_then(|binding| binding.get("name"))
                        .and_then(|v| v.as_str())
                        == Some(binding_name)
                })
                .count()
        })
        .unwrap_or(0)
}

fn runtime_diagnostic(resp: &Value) -> Option<String> {
    let runtime = resp.get("runtime")?;
    let version_id = runtime
        .get("config")
        .and_then(|config| config.get("version_id"))
        .and_then(|v| v.as_str())
        .or_else(|| runtime.get("config_version_id").and_then(|v| v.as_str()));
    let loaded_at = runtime.get("loaded_at").and_then(|v| v.as_str());

    match (version_id, loaded_at) {
        (Some(version_id), Some(loaded_at)) => Some(format!(
            "validator runtime is loaded for config version {version_id} (loaded_at {loaded_at})"
        )),
        (Some(version_id), None) => Some(format!(
            "validator runtime is loaded for config version {version_id}"
        )),
        (None, Some(loaded_at)) => Some(format!(
            "validator runtime is present and was loaded at {loaded_at}"
        )),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CheckStatus;

    #[test]
    fn smoke_config_content_has_required_structure() {
        let config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        let content = smoke_config_content(&config);
        assert!(content.get("metadata").is_some());
        assert!(content.get("bindings").unwrap().as_array().unwrap().len() > 0);
        assert!(content.get("fields").unwrap().as_array().unwrap().len() > 0);
        assert!(content.get("rules").unwrap().as_array().unwrap().len() > 0);
    }

    #[test]
    fn bootstrap_fails_when_compose_file_missing() {
        let config = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let result = bootstrap(&config);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.findings[0]
            .message
            .contains("compose file not found"));
    }

    #[test]
    fn bootstrap_required_fails_when_compose_file_missing() {
        let config = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let result = bootstrap_required(&config, &["nats", "configctl", "server"], "make up-core");
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.findings[0]
            .message
            .contains("compose file not found"));
    }

    #[test]
    fn readiness_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        config.readiness_timeout_secs = 1;
        config.poll_interval_ms = 200;
        let result = readiness(&config);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.findings[0].message.contains("timed out"));
    }

    #[test]
    fn inject_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        let result = inject(&config);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.findings[0].message.contains("create draft failed"));
    }

    #[test]
    fn route_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        config.poll_interval_ms = 200;
        let result = route(&config);
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn consume_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        config.results_timeout_secs = 1;
        config.poll_interval_ms = 200;
        let result = consume(&config);
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn validate_fails_when_server_unreachable() {
        let mut config = SmokeConfig::new(std::path::Path::new("/tmp"), None);
        config.base_url = "http://127.0.0.1:19999".to_string();
        let result = validate(&config);
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn matching_binding_count_finds_named_binding() {
        let resp = serde_json::json!({
            "bindings": [
                { "binding": { "name": "orders" } },
                { "binding": { "name": "orders" } },
                { "binding": { "name": "payments" } }
            ]
        });
        assert_eq!(matching_binding_count(&resp, "orders"), 2);
        assert_eq!(matching_binding_count(&resp, "payments"), 1);
        assert_eq!(matching_binding_count(&resp, "missing"), 0);
    }

    #[test]
    fn runtime_diagnostic_reads_version_and_loaded_at() {
        let resp = serde_json::json!({
            "runtime": {
                "config": { "version_id": "cfg-123" },
                "loaded_at": "2026-03-16T15:00:00Z"
            }
        });
        let diagnostic = runtime_diagnostic(&resp).expect("expected runtime diagnostic");
        assert!(diagnostic.contains("cfg-123"));
        assert!(diagnostic.contains("loaded_at"));
    }

    #[test]
    fn runtime_diagnostic_supports_flat_version_field() {
        let resp = serde_json::json!({
            "runtime": {
                "config_version_id": "cfg-456"
            }
        });
        let diagnostic = runtime_diagnostic(&resp).expect("expected runtime diagnostic");
        assert!(diagnostic.contains("cfg-456"));
    }
}
