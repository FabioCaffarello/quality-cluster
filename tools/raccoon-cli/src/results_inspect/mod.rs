use crate::error::{CliError, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

// ── Configuration ────────────────────────────────────────────────────

pub struct InspectConfig {
    pub base_url: String,
    pub scope_kind: String,
    pub scope_key: String,
    pub binding_name: Option<String>,
    pub topic: Option<String>,
    pub limit: u32,
    pub failed_only: bool,
    pub latest: Option<u32>,
}

// ── API response models ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct InspectReport {
    pub summary: Summary,
    pub results: Vec<ResultRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters_applied: Option<FiltersApplied>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub bindings: BTreeMap<String, BindingSummary>,
    pub violation_rules: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BindingSummary {
    pub topic: String,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FiltersApplied {
    pub scope_kind: String,
    pub scope_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    pub failed_only: bool,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultRecord {
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub binding: String,
    pub topic: String,
    pub status: String,
    pub config_key: String,
    pub config_version: u64,
    pub violations: Vec<ViolationRecord>,
    pub processed_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViolationRecord {
    pub rule: String,
    pub field: String,
    pub operator: String,
    pub severity: String,
    pub message: String,
}

// ── Run ──────────────────────────────────────────────────────────────

pub fn run(config: &InspectConfig) -> Result<InspectReport> {
    let raw_results = fetch_results(config)?;
    let mut records = parse_results(&raw_results)?;

    if config.failed_only {
        records.retain(|r| r.status == "failed");
    }

    if let Some(n) = config.latest {
        records.truncate(n as usize);
    }

    let summary = build_summary(&records);
    let filters = FiltersApplied {
        scope_kind: config.scope_kind.clone(),
        scope_key: config.scope_key.clone(),
        binding_name: config.binding_name.clone(),
        topic: config.topic.clone(),
        failed_only: config.failed_only,
        limit: config.limit,
    };

    Ok(InspectReport {
        summary,
        results: records,
        filters_applied: Some(filters),
    })
}

// ── Rendering ────────────────────────────────────────────────────────

pub fn render_human(report: &InspectReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    let s = &report.summary;
    writeln!(out, "=== Validation Results Inspection ===").unwrap();
    writeln!(out).unwrap();

    // Summary
    writeln!(out, "Total: {}  |  Passed: {}  |  Failed: {}", s.total, s.passed, s.failed).unwrap();

    if s.total == 0 {
        writeln!(out).unwrap();
        writeln!(out, "No validation results found.").unwrap();
        writeln!(out, "This may mean the validator has not processed any messages yet,").unwrap();
        writeln!(out, "or the applied filters excluded all results.").unwrap();

        if let Some(ref filters) = report.filters_applied {
            writeln!(out).unwrap();
            writeln!(out, "Filters: scope={}/{}", filters.scope_kind, filters.scope_key).unwrap();
            if let Some(ref b) = filters.binding_name {
                write!(out, ", binding={b}").unwrap();
            }
            if let Some(ref t) = filters.topic {
                write!(out, ", topic={t}").unwrap();
            }
            if filters.failed_only {
                write!(out, ", failed-only").unwrap();
            }
            writeln!(out).unwrap();
        }
        return out;
    }

    // Bindings breakdown
    if !s.bindings.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "Bindings:").unwrap();
        for (name, bs) in &s.bindings {
            let rate = if bs.passed + bs.failed > 0 {
                (bs.passed as f64 / (bs.passed + bs.failed) as f64) * 100.0
            } else {
                0.0
            };
            writeln!(
                out,
                "  {name} (topic: {})  passed: {} / failed: {} ({rate:.0}% pass rate)",
                bs.topic, bs.passed, bs.failed
            )
            .unwrap();
        }
    }

    // Violation rules breakdown
    if !s.violation_rules.is_empty() {
        writeln!(out).unwrap();
        writeln!(out, "Violation rules:").unwrap();
        let mut sorted: Vec<_> = s.violation_rules.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (rule, count) in sorted {
            writeln!(out, "  {rule}: {count}").unwrap();
        }
    }

    // Individual results
    if verbose || s.failed > 0 {
        writeln!(out).unwrap();
        let shown: Vec<_> = if !verbose {
            report.results.iter().filter(|r| r.status == "failed").collect()
        } else {
            report.results.iter().collect()
        };

        if !shown.is_empty() {
            let label = if verbose { "Results" } else { "Failed results" };
            writeln!(out, "{label} ({} shown):", shown.len()).unwrap();
            for r in &shown {
                writeln!(out).unwrap();
                let status_marker = if r.status == "passed" { "PASS" } else { "FAIL" };
                writeln!(
                    out,
                    "  [{status_marker}] msg={} binding={} topic={}",
                    r.message_id, r.binding, r.topic
                )
                .unwrap();
                writeln!(
                    out,
                    "         config={} v{} at {}",
                    r.config_key, r.config_version, r.processed_at
                )
                .unwrap();
                if let Some(ref cid) = r.correlation_id {
                    writeln!(out, "         correlation_id={cid}").unwrap();
                }
                for v in &r.violations {
                    writeln!(
                        out,
                        "         - [{sev}] {rule}: {field} ({op}) {msg}",
                        sev = v.severity,
                        rule = v.rule,
                        field = v.field,
                        op = v.operator,
                        msg = v.message
                    )
                    .unwrap();
                }
            }
        }
    }

    // Verdict
    writeln!(out).unwrap();
    if s.failed == 0 {
        writeln!(out, "> All validations passed.").unwrap();
    } else {
        let word = if s.failed == 1 { "result" } else { "results" };
        writeln!(out, "> {failed} {word} failed — review violations above.", failed = s.failed).unwrap();
    }

    out
}

pub fn render_json(report: &InspectReport) -> Result<String> {
    serde_json::to_string_pretty(report).map_err(CliError::Json)
}

// ── HTTP fetch ───────────────────────────────────────────────────────

fn fetch_results(config: &InspectConfig) -> Result<Value> {
    let mut url = format!(
        "{}/runtime/validator/results?scope_kind={}&scope_key={}&limit={}",
        config.base_url.trim_end_matches('/'),
        config.scope_kind,
        config.scope_key,
        config.limit
    );

    if let Some(ref binding) = config.binding_name {
        url.push_str(&format!("&binding_name={binding}"));
    }
    if let Some(ref topic) = config.topic {
        url.push_str(&format!("&topic={topic}"));
    }

    let correlation_id = format!("raccoon-inspect-{}", std::process::id());

    let resp = ureq::get(&url)
        .set("Accept", "application/json")
        .set("X-Correlation-ID", &correlation_id)
        .timeout(REQUEST_TIMEOUT)
        .call()
        .map_err(|e| {
            let msg = match &e {
                ureq::Error::Transport(t) => {
                    format!(
                        "cannot reach quality-service at {}: {}",
                        config.base_url,
                        t.message().unwrap_or("connection failed")
                    )
                }
                ureq::Error::Status(code, _) => {
                    format!(
                        "quality-service returned HTTP {code} for results query"
                    )
                }
            };
            CliError::Command { message: msg }
        })?;

    resp.into_json::<Value>().map_err(|e| CliError::Command {
        message: format!("failed to parse results response: {e}"),
    })
}

// ── Parsing ──────────────────────────────────────────────────────────

fn parse_results(body: &Value) -> Result<Vec<ResultRecord>> {
    let results_arr = body
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CliError::Command {
            message: "unexpected response format: missing 'results' array".to_string(),
        })?;

    let mut records = Vec::with_capacity(results_arr.len());
    for item in results_arr {
        records.push(parse_single_result(item));
    }
    Ok(records)
}

fn parse_single_result(v: &Value) -> ResultRecord {
    let violations = v
        .get("violations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|viol| ViolationRecord {
                    rule: str_field(viol, "rule"),
                    field: str_field(viol, "field"),
                    operator: str_field(viol, "operator"),
                    severity: str_field(viol, "severity"),
                    message: str_field(viol, "message"),
                })
                .collect()
        })
        .unwrap_or_default();

    let correlation_id = v
        .get("correlation_id")
        .and_then(|c| c.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    ResultRecord {
        message_id: str_field(v, "message_id"),
        correlation_id,
        binding: str_nested(v, &["binding", "name"]),
        topic: str_nested(v, &["binding", "topic"]),
        status: str_field(v, "status"),
        config_key: str_nested(v, &["config", "key"]),
        config_version: v
            .get("config")
            .and_then(|c| c.get("version"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        violations,
        processed_at: str_field(v, "processed_at"),
    }
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn str_nested(v: &Value, path: &[&str]) -> String {
    let mut current = v;
    for key in path {
        match current.get(key) {
            Some(next) => current = next,
            None => return String::new(),
        }
    }
    current.as_str().unwrap_or("").to_string()
}

// ── Summary ──────────────────────────────────────────────────────────

fn build_summary(records: &[ResultRecord]) -> Summary {
    let total = records.len();
    let passed = records.iter().filter(|r| r.status == "passed").count();
    let failed = records.iter().filter(|r| r.status == "failed").count();

    let mut bindings: BTreeMap<String, BindingSummary> = BTreeMap::new();
    let mut violation_rules: BTreeMap<String, usize> = BTreeMap::new();

    for r in records {
        let entry = bindings
            .entry(r.binding.clone())
            .or_insert_with(|| BindingSummary {
                topic: r.topic.clone(),
                passed: 0,
                failed: 0,
            });
        if r.status == "passed" {
            entry.passed += 1;
        } else {
            entry.failed += 1;
        }

        for v in &r.violations {
            *violation_rules.entry(v.rule.clone()).or_insert(0) += 1;
        }
    }

    Summary {
        total,
        passed,
        failed,
        bindings,
        violation_rules,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_api_response() -> Value {
        serde_json::json!({
            "results": [
                {
                    "message_id": "msg-001",
                    "correlation_id": "corr-1",
                    "binding": {
                        "name": "orders",
                        "topic": "orders.v1",
                        "scope": { "kind": "global", "key": "default" }
                    },
                    "config": {
                        "set_id": "set-1",
                        "key": "quality-rules",
                        "version_id": "ver-1",
                        "version": 3,
                        "definition_checksum": "abc123"
                    },
                    "status": "passed",
                    "processed_at": "2026-03-14T10:00:00Z"
                },
                {
                    "message_id": "msg-002",
                    "binding": {
                        "name": "orders",
                        "topic": "orders.v1",
                        "scope": { "kind": "global", "key": "default" }
                    },
                    "config": {
                        "set_id": "set-1",
                        "key": "quality-rules",
                        "version_id": "ver-1",
                        "version": 3,
                        "definition_checksum": "abc123"
                    },
                    "status": "failed",
                    "violations": [
                        {
                            "rule": "field-presence",
                            "field": "customer_id",
                            "operator": "required",
                            "severity": "error",
                            "message": "field customer_id is required"
                        },
                        {
                            "rule": "field-not-empty",
                            "field": "email",
                            "operator": "not_empty",
                            "severity": "error",
                            "message": "field email must not be empty"
                        }
                    ],
                    "processed_at": "2026-03-14T10:01:00Z"
                },
                {
                    "message_id": "msg-003",
                    "binding": {
                        "name": "events",
                        "topic": "events.v1",
                        "scope": { "kind": "global", "key": "default" }
                    },
                    "config": {
                        "set_id": "set-1",
                        "key": "quality-rules",
                        "version_id": "ver-1",
                        "version": 3,
                        "definition_checksum": "abc123"
                    },
                    "status": "passed",
                    "processed_at": "2026-03-14T10:02:00Z"
                }
            ]
        })
    }

    fn empty_api_response() -> Value {
        serde_json::json!({ "results": [] })
    }

    // ── parse_results ─────────────────────────────────────────────

    #[test]
    fn parse_results_extracts_all_records() {
        let records = parse_results(&sample_api_response()).unwrap();
        assert_eq!(records.len(), 3);
    }

    #[test]
    fn parse_results_extracts_fields_correctly() {
        let records = parse_results(&sample_api_response()).unwrap();
        let r = &records[0];
        assert_eq!(r.message_id, "msg-001");
        assert_eq!(r.correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(r.binding, "orders");
        assert_eq!(r.topic, "orders.v1");
        assert_eq!(r.status, "passed");
        assert_eq!(r.config_key, "quality-rules");
        assert_eq!(r.config_version, 3);
        assert!(r.violations.is_empty());
    }

    #[test]
    fn parse_results_extracts_violations() {
        let records = parse_results(&sample_api_response()).unwrap();
        let r = &records[1];
        assert_eq!(r.status, "failed");
        assert_eq!(r.violations.len(), 2);
        assert_eq!(r.violations[0].rule, "field-presence");
        assert_eq!(r.violations[0].field, "customer_id");
        assert_eq!(r.violations[0].operator, "required");
        assert_eq!(r.violations[1].rule, "field-not-empty");
    }

    #[test]
    fn parse_results_missing_correlation_id_is_none() {
        let records = parse_results(&sample_api_response()).unwrap();
        assert!(records[1].correlation_id.is_none());
    }

    #[test]
    fn parse_results_empty_response() {
        let records = parse_results(&empty_api_response()).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn parse_results_rejects_invalid_format() {
        let bad = serde_json::json!({"data": []});
        assert!(parse_results(&bad).is_err());
    }

    // ── build_summary ─────────────────────────────────────────────

    #[test]
    fn summary_counts_correct() {
        let records = parse_results(&sample_api_response()).unwrap();
        let s = build_summary(&records);
        assert_eq!(s.total, 3);
        assert_eq!(s.passed, 2);
        assert_eq!(s.failed, 1);
    }

    #[test]
    fn summary_bindings_breakdown() {
        let records = parse_results(&sample_api_response()).unwrap();
        let s = build_summary(&records);
        assert_eq!(s.bindings.len(), 2);

        let orders = s.bindings.get("orders").unwrap();
        assert_eq!(orders.topic, "orders.v1");
        assert_eq!(orders.passed, 1);
        assert_eq!(orders.failed, 1);

        let events = s.bindings.get("events").unwrap();
        assert_eq!(events.passed, 1);
        assert_eq!(events.failed, 0);
    }

    #[test]
    fn summary_violation_rules() {
        let records = parse_results(&sample_api_response()).unwrap();
        let s = build_summary(&records);
        assert_eq!(s.violation_rules.len(), 2);
        assert_eq!(s.violation_rules["field-presence"], 1);
        assert_eq!(s.violation_rules["field-not-empty"], 1);
    }

    #[test]
    fn summary_empty_records() {
        let s = build_summary(&[]);
        assert_eq!(s.total, 0);
        assert_eq!(s.passed, 0);
        assert_eq!(s.failed, 0);
        assert!(s.bindings.is_empty());
        assert!(s.violation_rules.is_empty());
    }

    // ── failed_only filter ────────────────────────────────────────

    #[test]
    fn failed_only_filters_passed() {
        let mut records = parse_results(&sample_api_response()).unwrap();
        records.retain(|r| r.status == "failed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].message_id, "msg-002");
    }

    // ── latest filter ─────────────────────────────────────────────

    #[test]
    fn latest_truncates_records() {
        let mut records = parse_results(&sample_api_response()).unwrap();
        records.truncate(2);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].message_id, "msg-001");
        assert_eq!(records[1].message_id, "msg-002");
    }

    #[test]
    fn latest_larger_than_total_keeps_all() {
        let mut records = parse_results(&sample_api_response()).unwrap();
        records.truncate(100);
        assert_eq!(records.len(), 3);
    }

    // ── render_human ──────────────────────────────────────────────

    #[test]
    fn render_human_shows_summary() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("Total: 3"));
        assert!(out.contains("Passed: 2"));
        assert!(out.contains("Failed: 1"));
    }

    #[test]
    fn render_human_shows_binding_breakdown() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("orders"));
        assert!(out.contains("events"));
        assert!(out.contains("pass rate"));
    }

    #[test]
    fn render_human_shows_violation_rules() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("field-presence"));
        assert!(out.contains("field-not-empty"));
    }

    #[test]
    fn render_human_non_verbose_shows_only_failed() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("Failed results"));
        assert!(out.contains("msg-002"));
        // Non-verbose should not show passed individual results
        assert!(!out.contains("[PASS] msg=msg-001"));
    }

    #[test]
    fn render_human_verbose_shows_all_results() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, true);
        assert!(out.contains("Results (3 shown)"));
        assert!(out.contains("[PASS] msg=msg-001"));
        assert!(out.contains("[FAIL] msg=msg-002"));
        assert!(out.contains("[PASS] msg=msg-003"));
    }

    #[test]
    fn render_human_shows_violations_detail() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("customer_id"));
        assert!(out.contains("required"));
    }

    #[test]
    fn render_human_empty_results_shows_message() {
        let report = InspectReport {
            summary: build_summary(&[]),
            results: vec![],
            filters_applied: Some(FiltersApplied {
                scope_kind: "global".into(),
                scope_key: "default".into(),
                binding_name: None,
                topic: None,
                failed_only: false,
                limit: 20,
            }),
        };
        let out = render_human(&report, false);
        assert!(out.contains("No validation results found"));
        assert!(out.contains("scope=global/default"));
    }

    #[test]
    fn render_human_all_passed_shows_positive_verdict() {
        let body = serde_json::json!({
            "results": [{
                "message_id": "msg-001",
                "binding": { "name": "orders", "topic": "orders.v1", "scope": { "kind": "global", "key": "default" } },
                "config": { "set_id": "s", "key": "k", "version_id": "v", "version": 1, "definition_checksum": "c" },
                "status": "passed",
                "processed_at": "2026-03-14T10:00:00Z"
            }]
        });
        let records = parse_results(&body).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("All validations passed"));
    }

    // ── render_json ───────────────────────────────────────────────

    #[test]
    fn render_json_is_valid() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let json_str = render_json(&report).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["summary"]["total"], 3);
        assert_eq!(parsed["summary"]["passed"], 2);
        assert_eq!(parsed["summary"]["failed"], 1);
        assert!(parsed["results"].is_array());
        assert_eq!(parsed["results"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn render_json_empty_results() {
        let report = InspectReport {
            summary: build_summary(&[]),
            results: vec![],
            filters_applied: None,
        };
        let json_str = render_json(&report).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["summary"]["total"], 0);
        assert!(parsed["results"].as_array().unwrap().is_empty());
    }

    #[test]
    fn render_json_includes_violations() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let json_str = render_json(&report).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        let violations = &parsed["results"][1]["violations"];
        assert_eq!(violations.as_array().unwrap().len(), 2);
        assert_eq!(violations[0]["rule"], "field-presence");
    }

    // ── str_field / str_nested helpers ────────────────────────────

    #[test]
    fn str_field_returns_empty_for_missing_key() {
        let v = serde_json::json!({"a": "b"});
        assert_eq!(str_field(&v, "missing"), "");
    }

    #[test]
    fn str_nested_returns_empty_for_missing_path() {
        let v = serde_json::json!({"a": {"b": "c"}});
        assert_eq!(str_nested(&v, &["a", "x"]), "");
        assert_eq!(str_nested(&v, &["x", "y"]), "");
    }

    #[test]
    fn str_nested_extracts_value() {
        let v = serde_json::json!({"a": {"b": "hello"}});
        assert_eq!(str_nested(&v, &["a", "b"]), "hello");
    }

    // ── InspectReport serialization ───────────────────────────────

    #[test]
    fn inspect_report_json_omits_null_filters() {
        let report = InspectReport {
            summary: build_summary(&[]),
            results: vec![],
            filters_applied: None,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("filters_applied"));
    }

    #[test]
    fn inspect_report_json_includes_filters_when_set() {
        let report = InspectReport {
            summary: build_summary(&[]),
            results: vec![],
            filters_applied: Some(FiltersApplied {
                scope_kind: "global".into(),
                scope_key: "default".into(),
                binding_name: Some("orders".into()),
                topic: None,
                failed_only: true,
                limit: 10,
            }),
        };
        let json_str = serde_json::to_string(&report).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["filters_applied"]["scope_kind"], "global");
        assert_eq!(parsed["filters_applied"]["binding_name"], "orders");
        assert_eq!(parsed["filters_applied"]["failed_only"], true);
        assert!(parsed["filters_applied"]["topic"].is_null());
    }

    // ── render_human with filters ─────────────────────────────────

    #[test]
    fn render_human_empty_with_binding_filter_shows_filter() {
        let report = InspectReport {
            summary: build_summary(&[]),
            results: vec![],
            filters_applied: Some(FiltersApplied {
                scope_kind: "global".into(),
                scope_key: "default".into(),
                binding_name: Some("orders".into()),
                topic: None,
                failed_only: false,
                limit: 20,
            }),
        };
        let out = render_human(&report, false);
        assert!(out.contains("binding=orders"));
    }

    #[test]
    fn render_human_verdict_pluralizes_failures() {
        let body = serde_json::json!({
            "results": [
                {
                    "message_id": "msg-1",
                    "binding": { "name": "b", "topic": "t", "scope": { "kind": "global", "key": "default" } },
                    "config": { "set_id": "s", "key": "k", "version_id": "v", "version": 1, "definition_checksum": "c" },
                    "status": "failed",
                    "violations": [{ "rule": "r", "field": "f", "operator": "required", "severity": "error", "message": "m" }],
                    "processed_at": "2026-03-14T10:00:00Z"
                },
                {
                    "message_id": "msg-2",
                    "binding": { "name": "b", "topic": "t", "scope": { "kind": "global", "key": "default" } },
                    "config": { "set_id": "s", "key": "k", "version_id": "v", "version": 1, "definition_checksum": "c" },
                    "status": "failed",
                    "violations": [{ "rule": "r", "field": "f", "operator": "required", "severity": "error", "message": "m" }],
                    "processed_at": "2026-03-14T10:01:00Z"
                }
            ]
        });
        let records = parse_results(&body).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, false);
        assert!(out.contains("2 results failed"));
    }

    #[test]
    fn render_human_shows_correlation_id() {
        let records = parse_results(&sample_api_response()).unwrap();
        let report = InspectReport {
            summary: build_summary(&records),
            results: records,
            filters_applied: None,
        };
        let out = render_human(&report, true);
        assert!(out.contains("correlation_id=corr-1"));
    }
}
