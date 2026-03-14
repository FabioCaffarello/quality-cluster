use crate::models::{CheckResult, Finding};
use super::api::ApiClient;
use super::compose;
use super::SmokeConfig;
use std::thread;
use std::time::{Duration, Instant};

/// Synthetic config content for the smoke test.
pub fn smoke_config_content() -> serde_json::Value {
    serde_json::json!({
        "metadata": {
            "name": "Smoke Test",
            "description": "raccoon-cli runtime smoke test config"
        },
        "bindings": [
            {
                "name": "smoke_events",
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
    if !config.compose_file.exists() {
        return CheckResult::from_findings(
            "bootstrap",
            vec![Finding::error(
                "bootstrap",
                format!(
                    "compose file not found: {}",
                    config.compose_file.display()
                ),
            )],
        );
    }

    let running = match compose::running_services(&config.compose_file) {
        Ok(r) => r,
        Err(e) => {
            return CheckResult::from_findings(
                "bootstrap",
                vec![Finding::error("bootstrap", e)],
            );
        }
    };

    let missing = compose::missing_services(&running);
    if missing.is_empty() {
        let mut result = CheckResult::pass("bootstrap");
        result.findings.push(Finding::info(
            "bootstrap",
            format!("{} services running", running.len()),
        ));
        result
    } else {
        CheckResult::from_findings(
            "bootstrap",
            vec![Finding::error(
                "bootstrap",
                format!(
                    "missing services: {}. Run `make up-dataplane` first.",
                    missing.join(", ")
                ),
            )],
        )
    }
}

/// Stage 2: Readiness — poll healthz and readyz until both return 200.
pub fn readiness(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url);
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
                result.findings.push(Finding::info(
                    "readiness",
                    "healthz=200, readyz=200",
                ));
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
    let client = ApiClient::new(&config.base_url);
    let content = smoke_config_content();

    // Create draft
    let draft_resp = match client.create_draft("raccoon-smoke", &content) {
        Ok(v) => v,
        Err(e) => {
            return CheckResult::from_findings(
                "inject",
                vec![Finding::error("inject", format!("create draft failed: {e}"))],
            );
        }
    };

    let config_id = match draft_resp.get("id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => {
            return CheckResult::from_findings(
                "inject",
                vec![Finding::error(
                    "inject",
                    format!(
                        "create draft response missing 'id' field: {}",
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
    if let Err(e) = client.activate_config(&config_id) {
        return CheckResult::from_findings(
            "inject",
            vec![Finding::error("inject", format!("activate failed: {e}"))],
        );
    }

    let mut result = CheckResult::pass("inject");
    result.findings.push(Finding::info(
        "inject",
        format!("config {config_id} created, validated, compiled, activated"),
    ));
    result
}

/// Stage 4: Route — verify ingestion bindings are projected.
pub fn route(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url);
    let deadline = Instant::now() + Duration::from_secs(10);
    let interval = Duration::from_millis(config.poll_interval_ms);

    while Instant::now() < deadline {
        match client.ingestion_bindings() {
            Ok(resp) => {
                let bindings = resp
                    .get("bindings")
                    .and_then(|b| b.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                if bindings > 0 {
                    let mut result = CheckResult::pass("route");
                    result.findings.push(Finding::info(
                        "route",
                        format!("{bindings} active ingestion binding(s) found"),
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
    let client = ApiClient::new(&config.base_url);
    let deadline = Instant::now() + Duration::from_secs(config.results_timeout_secs);
    let interval = Duration::from_millis(config.poll_interval_ms);

    while Instant::now() < deadline {
        match client.validation_results(10) {
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

    CheckResult::from_findings(
        "consume",
        vec![Finding::error(
            "consume",
            format!(
                "no validation results within {}s. Data pipeline may be stuck.",
                config.results_timeout_secs
            ),
        )],
    )
}

/// Stage 6: Validate — check that results include both passed and failed entries.
/// The emulator produces one valid and one invalid sample per binding per cycle.
pub fn validate(config: &SmokeConfig) -> CheckResult {
    let client = ApiClient::new(&config.base_url);

    let resp = match client.validation_results(20) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CheckStatus;

    #[test]
    fn smoke_config_content_has_required_structure() {
        let content = smoke_config_content();
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
        assert!(result.findings[0].message.contains("compose file not found"));
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
}
