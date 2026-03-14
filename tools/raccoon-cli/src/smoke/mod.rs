pub mod api;
pub mod compose;
pub mod scenarios;
pub mod stages;

use crate::error::Result;
use crate::models::{CheckResult, CheckStatus, Report};

/// Configuration for the runtime smoke test.
#[derive(Debug, Clone)]
pub struct SmokeConfig {
    #[allow(dead_code)]
    pub project_root: std::path::PathBuf,
    pub base_url: String,
    pub compose_file: std::path::PathBuf,
    pub readiness_timeout_secs: u64,
    pub poll_interval_ms: u64,
    pub results_timeout_secs: u64,
}

impl SmokeConfig {
    pub fn new(project_root: &std::path::Path, base_url: Option<&str>) -> Self {
        let compose_file = project_root.join("deploy/compose/docker-compose.yaml");
        Self {
            project_root: project_root.to_path_buf(),
            base_url: base_url.unwrap_or("http://127.0.0.1:8080").to_string(),
            compose_file,
            readiness_timeout_secs: 60,
            poll_interval_ms: 500,
            results_timeout_secs: 30,
        }
    }
}

/// Stage definitions: name and runner function.
const STAGE_NAMES: &[&str] = &["bootstrap", "readiness", "inject", "route", "consume", "validate"];

/// Run the full runtime smoke test pipeline.
/// Stages execute sequentially; if any stage fails, remaining stages are skipped.
pub fn run(config: &SmokeConfig) -> Result<Report> {
    let mut report = Report::new("runtime-smoke");

    let stage_fns: Vec<Box<dyn Fn(&SmokeConfig) -> CheckResult>> = vec![
        Box::new(stages::bootstrap),
        Box::new(stages::readiness),
        Box::new(stages::inject),
        Box::new(stages::route),
        Box::new(stages::consume),
        Box::new(stages::validate),
    ];

    let mut failed_at: Option<&str> = None;

    for (i, stage_fn) in stage_fns.iter().enumerate() {
        let name = STAGE_NAMES[i];
        if let Some(blocker) = failed_at {
            report.add(CheckResult::skip(
                name,
                format!("skipped: {blocker} failed"),
            ));
            continue;
        }

        let result = stage_fn(config);
        let ok = result.status == CheckStatus::Pass;
        report.add(result);
        if !ok {
            failed_at = Some(name);
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CheckStatus;

    #[test]
    fn smoke_config_defaults() {
        let cfg = SmokeConfig::new(std::path::Path::new("/tmp/proj"), None);
        assert_eq!(cfg.base_url, "http://127.0.0.1:8080");
        assert_eq!(
            cfg.compose_file,
            std::path::PathBuf::from("/tmp/proj/deploy/compose/docker-compose.yaml")
        );
        assert_eq!(cfg.readiness_timeout_secs, 60);
    }

    #[test]
    fn smoke_config_custom_url() {
        let cfg = SmokeConfig::new(
            std::path::Path::new("/tmp"),
            Some("http://localhost:9090"),
        );
        assert_eq!(cfg.base_url, "http://localhost:9090");
    }

    #[test]
    fn run_fails_gracefully_when_compose_missing() {
        let cfg = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run(&cfg).unwrap();
        assert!(!report.passed());
        // Should have bootstrap fail and remaining stages skipped
        assert_eq!(report.checks.len(), 6);
        assert_eq!(report.checks[0].name, "bootstrap");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
        for check in &report.checks[1..] {
            assert_eq!(check.status, CheckStatus::Skip);
        }
    }

    #[test]
    fn smoke_config_defaults_all_fields() {
        let cfg = SmokeConfig::new(std::path::Path::new("/tmp/proj"), None);
        assert_eq!(cfg.readiness_timeout_secs, 60);
        assert_eq!(cfg.poll_interval_ms, 500);
        assert_eq!(cfg.results_timeout_secs, 30);
    }

    #[test]
    fn smoke_report_title_is_runtime_smoke() {
        let cfg = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run(&cfg).unwrap();
        assert_eq!(report.title, "runtime-smoke");
    }

    #[test]
    fn smoke_report_has_all_six_stage_names() {
        let cfg = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run(&cfg).unwrap();
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["bootstrap", "readiness", "inject", "route", "consume", "validate"]
        );
    }

    #[test]
    fn smoke_skipped_stages_reference_blocker() {
        let cfg = SmokeConfig::new(std::path::Path::new("/nonexistent"), None);
        let report = run(&cfg).unwrap();
        for check in &report.checks[1..] {
            assert_eq!(check.status, CheckStatus::Skip);
            assert!(
                check.findings[0].message.contains("bootstrap"),
                "skip message should reference the failed stage"
            );
        }
    }
}
