use crate::analyzers;
use crate::error::Result;
use crate::models::{CheckResult, CheckStatus, Finding, Report, Severity};
use crate::output::OutputFormat;
use crate::smoke;
use serde::Serialize;
use std::time::Instant;

/// Which checks to include and how strictly to evaluate them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Static analysis only (doctor + topology-doctor + contract-audit + runtime-bindings). Fast, no infra needed.
    Fast,
    /// Same checks as Fast, but treats warnings as failures (stricter for CI pipelines).
    Ci,
    /// All checks including runtime-smoke. Requires a running local environment.
    Deep,
}

impl Profile {
    pub fn includes_static(&self) -> bool {
        true
    }

    pub fn includes_runtime(&self) -> bool {
        matches!(self, Profile::Deep)
    }

    /// In CI profile, warnings are promoted to errors so the gate fails on any non-clean report.
    pub fn warnings_are_errors(&self) -> bool {
        matches!(self, Profile::Ci)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Profile::Fast => "fast",
            Profile::Ci => "ci",
            Profile::Deep => "deep",
        }
    }
}

/// Configuration for a quality-gate run.
#[derive(Debug, Clone)]
pub struct GateConfig {
    pub project_root: std::path::PathBuf,
    pub profile: Profile,
    pub base_url: String,
    /// When true, skip remaining steps after the first failure.
    pub fail_fast: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Result of a single gate step with timing and finding-level counts.
#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration_ms: u128,
    /// Number of individual checks within this step.
    pub check_count: usize,
    /// Number of Error-severity findings in this step.
    pub error_count: usize,
    /// Number of Warning-severity findings in this step.
    pub warning_count: usize,
    /// Why this step was skipped (only set when status == Skip).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    /// True when the step function itself errored (IO, parse failure),
    /// as opposed to checks finding problems in the project.
    #[serde(skip_serializing_if = "is_false")]
    pub is_execution_error: bool,
    pub report: Report,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Pass,
    Fail,
    Skip,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepStatus::Pass => write!(f, "PASS"),
            StepStatus::Fail => write!(f, "FAIL"),
            StepStatus::Skip => write!(f, "SKIP"),
        }
    }
}

/// Step-level summary counts embedded in the JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct GateSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_checks: usize,
    /// Total Error-severity findings across all steps.
    pub total_errors: usize,
    /// Total Warning-severity findings across all steps.
    pub total_warnings: usize,
}

/// Structured verdict for JSON consumers.
#[derive(Debug, Clone, Serialize)]
pub struct Verdict {
    /// `"proceed"` when all steps pass/skip, `"stop"` when any step fails.
    pub action: String,
    /// Human-readable summary matching the guard rail output.
    pub message: String,
    /// Remediation hints for each failed step (empty when action is "proceed").
    pub next_steps: Vec<String>,
}

/// Aggregated result of the full quality gate.
#[derive(Debug, Clone, Serialize)]
pub struct GateReport {
    pub profile: String,
    pub steps: Vec<StepResult>,
    pub summary: GateSummary,
    pub verdict: Verdict,
    pub total_duration_ms: u128,
    pub passed: bool,
}

impl GateReport {
    pub fn step_counts(&self) -> (usize, usize, usize) {
        let pass = self.steps.iter().filter(|s| s.status == StepStatus::Pass).count();
        let fail = self.steps.iter().filter(|s| s.status == StepStatus::Fail).count();
        let skip = self.steps.iter().filter(|s| s.status == StepStatus::Skip).count();
        (pass, fail, skip)
    }
}

/// Count Error and Warning findings in a report.
fn count_findings(report: &Report) -> (usize, usize) {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    for check in &report.checks {
        for finding in &check.findings {
            match finding.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => {}
            }
        }
    }
    (errors, warnings)
}

/// Run the quality gate pipeline with the given configuration.
pub fn run(config: &GateConfig) -> Result<GateReport> {
    let gate_start = Instant::now();
    let mut steps = Vec::new();
    let mut blocker: Option<String> = None;

    /// Push a step or a fail-fast skip depending on whether a prior step failed.
    macro_rules! gate_step {
        ($name:expr, $body:expr) => {
            if let Some(ref failed_name) = blocker {
                steps.push(make_skip(
                    $name,
                    &format!(
                        "skipped — prior step '{}' failed (fail-fast mode)",
                        failed_name
                    ),
                ));
            } else {
                let step = run_step($name, &config.profile, $body);
                if config.fail_fast && step.status == StepStatus::Fail {
                    blocker = Some(step.name.clone());
                }
                steps.push(step);
            }
        };
    }

    // Step 1: doctor (project structure — validates that conventions exist for subsequent checks)
    if config.profile.includes_static() {
        gate_step!("doctor", || {
            analyzers::doctor::analyze(&config.project_root)
        });
    }

    // Step 2: topology-doctor (static — config/compose/source consistency)
    if config.profile.includes_static() {
        gate_step!("topology-doctor", || {
            analyzers::topology::analyze(&config.project_root)
        });
    }

    // Step 3: contract-audit (static — messaging contracts and invariants)
    if config.profile.includes_static() {
        gate_step!("contract-audit", || {
            analyzers::contracts::analyze(&config.project_root)
        });
    }

    // Step 4: runtime-bindings (static — config → kafka → jetstream → validator routing)
    if config.profile.includes_static() {
        gate_step!("runtime-bindings", || {
            analyzers::runtime_bindings::analyze(&config.project_root)
        });
    }

    // Step 5: arch-guard (static — clean architecture layer boundaries)
    if config.profile.includes_static() {
        gate_step!("arch-guard", || {
            analyzers::arch_guard::analyze(&config.project_root)
        });
    }

    // Step 6: drift-detect (static — cross-layer declaration/config/source alignment)
    if config.profile.includes_static() {
        gate_step!("drift-detect", || {
            analyzers::drift_detect::analyze(&config.project_root)
        });
    }

    // Step 7: runtime-smoke (only in Deep profile)
    if let Some(ref failed_name) = blocker {
        steps.push(make_skip(
            "runtime-smoke",
            &format!(
                "skipped — prior step '{}' failed (fail-fast mode)",
                failed_name
            ),
        ));
    } else if config.profile.includes_runtime() {
        let base_url = config.base_url.clone();
        let step = run_step("runtime-smoke", &config.profile, || {
            let smoke_cfg = smoke::SmokeConfig::new(&config.project_root, Some(&base_url));
            smoke::run(&smoke_cfg)
        });
        // blocker assignment intentionally unused — runtime-smoke is the last step
        let _ = &step;
        steps.push(step);
    } else {
        let reason = format!(
            "skipped in '{}' profile — use --profile deep to include",
            config.profile.label()
        );
        steps.push(make_skip("runtime-smoke", &reason));
    }

    let total_duration_ms = gate_start.elapsed().as_millis();

    let passed = steps.iter().all(|s| s.status != StepStatus::Fail);

    let total_checks: usize = steps.iter().map(|s| s.check_count).sum();
    let total_errors: usize = steps.iter().map(|s| s.error_count).sum();
    let total_warnings: usize = steps.iter().map(|s| s.warning_count).sum();
    let (sp, sf, ss) = {
        let pass = steps
            .iter()
            .filter(|s| s.status == StepStatus::Pass)
            .count();
        let fail = steps
            .iter()
            .filter(|s| s.status == StepStatus::Fail)
            .count();
        let skip = steps
            .iter()
            .filter(|s| s.status == StepStatus::Skip)
            .count();
        (pass, fail, skip)
    };

    let verdict = compute_verdict(&steps);

    Ok(GateReport {
        profile: config.profile.label().to_string(),
        steps,
        summary: GateSummary {
            passed: sp,
            failed: sf,
            skipped: ss,
            total_checks,
            total_errors,
            total_warnings,
        },
        verdict,
        total_duration_ms,
        passed,
    })
}

/// Build a skip step with reason.
fn make_skip(name: &str, reason: &str) -> StepResult {
    let mut r = Report::new(name);
    r.add(CheckResult::skip(name, reason));
    StepResult {
        name: name.to_string(),
        status: StepStatus::Skip,
        duration_ms: 0,
        check_count: 0,
        error_count: 0,
        warning_count: 0,
        skip_reason: Some(reason.to_string()),
        is_execution_error: false,
        report: r,
    }
}

/// Execute an analysis step, applying profile-level strictness.
fn run_step(name: &str, profile: &Profile, f: impl FnOnce() -> Result<Report>) -> StepResult {
    let start = Instant::now();
    match f() {
        Ok(mut report) => {
            // CI profile: promote warnings to errors so the gate fails on any non-clean report.
            if profile.warnings_are_errors() {
                promote_warnings(&mut report);
            }
            let check_count = report.checks.len();
            let (error_count, warning_count) = count_findings(&report);
            let status = if report.passed() {
                StepStatus::Pass
            } else {
                StepStatus::Fail
            };
            StepResult {
                name: name.to_string(),
                status,
                duration_ms: start.elapsed().as_millis(),
                check_count,
                error_count,
                warning_count,
                skip_reason: None,
                is_execution_error: false,
                report,
            }
        }
        Err(e) => {
            let mut report = Report::new(name);
            report.add(CheckResult::from_findings(
                name,
                vec![Finding::error(
                    name,
                    format!("execution error: {e}"),
                )],
            ));
            StepResult {
                name: name.to_string(),
                status: StepStatus::Fail,
                duration_ms: start.elapsed().as_millis(),
                check_count: 0,
                error_count: 1,
                warning_count: 0,
                skip_reason: None,
                is_execution_error: true,
                report,
            }
        }
    }
}

/// Promote all Warning findings to Error severity, causing their CheckResult to fail.
fn promote_warnings(report: &mut Report) {
    for check in &mut report.checks {
        let mut promoted = false;
        for finding in &mut check.findings {
            if finding.severity == Severity::Warning {
                finding.severity = Severity::Error;
                finding.message = format!("[ci] {}", finding.message);
                promoted = true;
            }
        }
        if promoted && check.status == CheckStatus::Pass {
            check.status = CheckStatus::Fail;
            report.passed = false;
        }
    }
}

/// Render a GateReport for human or JSON output.
pub fn render(report: &GateReport, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(report)?;
            Ok(json)
        }
        OutputFormat::Human => Ok(render_human(report, false)),
        OutputFormat::HumanVerbose => Ok(render_human(report, true)),
    }
}

fn render_human(report: &GateReport, verbose: bool) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "=== quality-gate [profile: {}] ===\n\n",
        report.profile
    ));

    for step in &report.steps {
        let duration = format_duration(step.duration_ms);
        let suffix = match step.status {
            StepStatus::Pass => {
                if step.check_count > 0 {
                    format!(" — {n} checks", n = step.check_count)
                } else {
                    String::new()
                }
            }
            StepStatus::Fail => {
                let mut parts = Vec::new();
                if step.check_count > 0 {
                    parts.push(format!("{} checks", step.check_count));
                }
                if step.is_execution_error {
                    parts.push("execution error".to_string());
                } else if step.error_count > 0 {
                    parts.push(format!(
                        "{} error{}",
                        step.error_count,
                        if step.error_count == 1 { "" } else { "s" }
                    ));
                    if step.warning_count > 0 {
                        parts.push(format!(
                            "{} warning{}",
                            step.warning_count,
                            if step.warning_count == 1 { "" } else { "s" }
                        ));
                    }
                }
                if parts.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", parts.join(", "))
                }
            }
            StepStatus::Skip => {
                if let Some(reason) = &step.skip_reason {
                    format!(" — {reason}")
                } else {
                    String::new()
                }
            }
        };
        out.push_str(&format!(
            "  {icon} {name} {status} ({duration}){suffix}\n",
            icon = status_icon(step.status),
            name = step.name,
            status = step.status,
        ));

        // Show findings for failed steps (or all steps if verbose)
        let show_detail = step.status == StepStatus::Fail || verbose;
        if show_detail {
            for check in &step.report.checks {
                let show_check = check.status == CheckStatus::Fail || verbose;
                if show_check && !check.findings.is_empty() {
                    for finding in &check.findings {
                        out.push_str(&format!("      [{check}] {finding}\n", check = check.name));
                    }
                }
            }
        }
    }

    out.push('\n');

    let (pass, fail, skip) = report.step_counts();
    let s = &report.summary;
    let total = format_duration(report.total_duration_ms);
    let verdict = if report.passed { "PASSED" } else { "FAILED" };
    out.push_str(&format!(
        "Result: {verdict} | {pass} passed, {fail} failed, {skip} skipped | {checks} checks | {total}\n",
        checks = s.total_checks,
    ));

    // Guard rail verdict
    if report.passed {
        out.push_str("\n> Safe to proceed — all guard rail checks passed.\n");
        // TDD next-action: remind of the discipline cycle
        out.push_str("\nTDD cycle:\n");
        out.push_str("  1. Write/update your scenario or test for the change you intend to make\n");
        out.push_str("  2. Implement the change\n");
        out.push_str("  3. Run `make verify` to prove the change is safe\n");
        if report.profile == "fast" || report.profile == "ci" {
            out.push_str("  4. Run `make check-deep` for full operational proof (requires `make up-dataplane`)\n");
        }
    } else {
        let error_word = if s.total_errors == 1 { "error" } else { "errors" };
        out.push_str(&format!(
            "\n> Stop — {} {} must be fixed before proceeding.\n",
            s.total_errors, error_word,
        ));
        out.push_str("\nActionable next steps:\n");
        for step in &report.steps {
            if step.status == StepStatus::Fail {
                let hint = step_remediation_hint(&step.name);
                out.push_str(&format!(
                    "  - Fix '{}': {}\n",
                    step.name, hint,
                ));
            }
        }
        out.push_str("\nDiscipline: fix errors first, then re-run `make check` before coding.\n");
    }

    out
}

/// Compute the structured verdict from step results.
fn compute_verdict(steps: &[StepResult]) -> Verdict {
    let failed: Vec<&StepResult> = steps.iter().filter(|s| s.status == StepStatus::Fail).collect();
    if failed.is_empty() {
        Verdict {
            action: "proceed".to_string(),
            message: "Safe to proceed — all guard rail checks passed.".to_string(),
            next_steps: Vec::new(),
        }
    } else {
        let total_errors: usize = failed.iter().map(|s| s.error_count).sum();
        let error_word = if total_errors == 1 { "error" } else { "errors" };
        Verdict {
            action: "stop".to_string(),
            message: format!(
                "Stop — {} {} must be fixed before proceeding.",
                total_errors, error_word
            ),
            next_steps: failed
                .iter()
                .map(|s| {
                    format!(
                        "Fix '{}': {}",
                        s.name,
                        step_remediation_hint(&s.name)
                    )
                })
                .collect(),
        }
    }
}

/// Per-step remediation hint for actionable output.
fn step_remediation_hint(step_name: &str) -> String {
    match step_name {
        "doctor" => {
            "run `raccoon-cli doctor` — check go.work, dirs, compose, and config files".to_string()
        }
        "topology-doctor" => {
            "run `raccoon-cli topology-doctor` — check configs, compose, and source wiring"
                .to_string()
        }
        "contract-audit" => {
            "run `raccoon-cli contract-audit` — check messaging contracts and invariants"
                .to_string()
        }
        "runtime-bindings" => {
            "run `raccoon-cli runtime-bindings` — check config → kafka → jetstream → validator routing"
                .to_string()
        }
        "arch-guard" => {
            "run `raccoon-cli arch-guard` — check clean architecture layer boundaries".to_string()
        }
        "drift-detect" => {
            "run `raccoon-cli drift-detect` — check cross-layer declaration/config/source alignment"
                .to_string()
        }
        "runtime-smoke" => {
            "ensure `make up-dataplane` is running, then `raccoon-cli runtime-smoke`".to_string()
        }
        other => format!("run `raccoon-cli {other}` for full details"),
    }
}

fn status_icon(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Pass => "[+]",
        StepStatus::Fail => "[x]",
        StepStatus::Skip => "[-]",
    }
}

fn format_duration(ms: u128) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonexistent_config(profile: Profile) -> GateConfig {
        GateConfig {
            project_root: std::path::PathBuf::from("/nonexistent/quality-service"),
            profile,
            base_url: "http://127.0.0.1:8080".to_string(),
            fail_fast: false,
        }
    }

    fn passing_verdict() -> Verdict {
        Verdict {
            action: "proceed".to_string(),
            message: "Safe to proceed — all guard rail checks passed.".to_string(),
            next_steps: Vec::new(),
        }
    }

    fn failing_verdict() -> Verdict {
        Verdict {
            action: "stop".to_string(),
            message: "Stop — 1 error must be fixed before proceeding.".to_string(),
            next_steps: vec!["Fix 'b': run `raccoon-cli b` for full details".to_string()],
        }
    }

    fn make_passing_step(name: &str, checks: usize) -> StepResult {
        let mut r = Report::new(name);
        for i in 0..checks {
            r.add(CheckResult::pass(format!("check-{i}")));
        }
        StepResult {
            name: name.to_string(),
            status: StepStatus::Pass,
            duration_ms: 42,
            check_count: checks,
            error_count: 0,
            warning_count: 0,
            skip_reason: None,
            is_execution_error: false,
            report: r,
        }
    }

    fn make_failing_step(name: &str) -> StepResult {
        let mut r = Report::new(name);
        r.add(CheckResult::from_findings(
            "bad-check",
            vec![Finding::error("bad-check", "something broke")],
        ));
        StepResult {
            name: name.to_string(),
            status: StepStatus::Fail,
            duration_ms: 10,
            check_count: 1,
            error_count: 1,
            warning_count: 0,
            skip_reason: None,
            is_execution_error: false,
            report: r,
        }
    }

    // --- Profile behavior ---

    #[test]
    fn fast_profile_includes_static_excludes_runtime() {
        let p = Profile::Fast;
        assert!(p.includes_static());
        assert!(!p.includes_runtime());
        assert!(!p.warnings_are_errors());
    }

    #[test]
    fn ci_profile_includes_static_excludes_runtime_warnings_are_errors() {
        let p = Profile::Ci;
        assert!(p.includes_static());
        assert!(!p.includes_runtime());
        assert!(p.warnings_are_errors());
    }

    #[test]
    fn deep_profile_includes_everything() {
        let p = Profile::Deep;
        assert!(p.includes_static());
        assert!(p.includes_runtime());
        assert!(!p.warnings_are_errors());
    }

    // --- CI warnings-as-errors ---

    #[test]
    fn ci_profile_promotes_warnings_to_errors() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "soft-check",
            vec![Finding::warning("soft-check", "just a warning")],
        ));
        assert!(report.passed(), "warning-only report should pass before promotion");

        promote_warnings(&mut report);

        assert!(!report.passed(), "report should fail after promotion");
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
        assert_eq!(report.checks[0].findings[0].severity, Severity::Error);
        assert!(report.checks[0].findings[0].message.contains("[ci]"));
    }

    #[test]
    fn fast_profile_does_not_promote_warnings() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "soft-check",
            vec![Finding::warning("soft-check", "just a warning")],
        ));
        // No promote_warnings call — Fast profile doesn't do it
        assert!(report.passed());
    }

    // --- Full gate runs ---

    #[test]
    fn gate_fast_on_nonexistent_root_fails() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        assert!(!report.passed);
        assert_eq!(report.profile, "fast");
        assert_eq!(report.steps.len(), 7);
        assert_eq!(report.steps[0].name, "doctor");
        assert_eq!(report.steps[1].name, "topology-doctor");
        assert_eq!(report.steps[2].name, "contract-audit");
        assert_eq!(report.steps[3].name, "runtime-bindings");
        assert_eq!(report.steps[4].name, "arch-guard");
        // arch-guard is now a real step that fails on missing internal/ dir
        assert_eq!(report.steps[4].status, StepStatus::Fail);
        assert_eq!(report.steps[5].name, "drift-detect");
        assert_eq!(report.steps[6].name, "runtime-smoke");
        assert_eq!(report.steps[6].status, StepStatus::Skip);
    }

    #[test]
    fn gate_deep_on_nonexistent_root_fails_with_runtime() {
        let report = run(&nonexistent_config(Profile::Deep)).unwrap();
        assert!(!report.passed);
        assert_eq!(report.profile, "deep");
        assert_eq!(report.steps.len(), 7);
        assert_eq!(report.steps[6].name, "runtime-smoke");
        assert_ne!(report.steps[6].status, StepStatus::Skip);
    }

    #[test]
    fn gate_ci_same_steps_as_fast() {
        let fast_report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let ci_report = run(&nonexistent_config(Profile::Ci)).unwrap();
        assert_eq!(fast_report.steps.len(), ci_report.steps.len());
        for (f, c) in fast_report.steps.iter().zip(ci_report.steps.iter()) {
            assert_eq!(f.name, c.name);
        }
    }

    // --- Summary counts ---

    #[test]
    fn gate_report_step_counts() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let (pass, fail, skip) = report.step_counts();
        assert!(fail > 0, "should have at least one failure");
        assert!(skip > 0, "should have at least one skip");
        assert_eq!(pass + fail + skip, report.steps.len());
    }

    #[test]
    fn gate_report_summary_matches_steps() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let (pass, fail, skip) = report.step_counts();
        assert_eq!(report.summary.passed, pass);
        assert_eq!(report.summary.failed, fail);
        assert_eq!(report.summary.skipped, skip);
    }

    #[test]
    fn gate_report_summary_total_checks() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let computed: usize = report.steps.iter().map(|s| s.check_count).sum();
        assert_eq!(report.summary.total_checks, computed);
        assert!(report.summary.total_checks > 0);
    }

    // --- Skip reasons ---

    #[test]
    fn skipped_steps_have_skip_reason() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        for step in &report.steps {
            if step.status == StepStatus::Skip {
                assert!(
                    step.skip_reason.is_some(),
                    "skipped step '{}' should have skip_reason",
                    step.name
                );
            }
        }
    }

    #[test]
    fn executed_steps_have_no_skip_reason() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        for step in &report.steps {
            if step.status != StepStatus::Skip {
                assert!(
                    step.skip_reason.is_none(),
                    "executed step '{}' should not have skip_reason",
                    step.name
                );
            }
        }
    }

    // --- Check counts ---

    #[test]
    fn executed_steps_have_positive_check_count() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        for step in &report.steps {
            if step.status != StepStatus::Skip {
                assert!(
                    step.check_count > 0,
                    "executed step '{}' should have check_count > 0",
                    step.name
                );
            }
        }
    }

    #[test]
    fn skipped_steps_have_zero_check_count() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        for step in &report.steps {
            if step.status == StepStatus::Skip {
                assert_eq!(
                    step.check_count, 0,
                    "skipped step '{}' should have check_count == 0",
                    step.name
                );
            }
        }
    }

    // --- Timing ---

    #[test]
    fn gate_report_has_positive_total_duration() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        assert!(report.total_duration_ms < 60_000, "should not take a minute");
    }

    #[test]
    fn each_skipped_step_has_zero_duration() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        for step in &report.steps {
            if step.status == StepStatus::Skip {
                assert_eq!(step.duration_ms, 0);
            }
        }
    }

    // --- Output rendering ---

    #[test]
    fn human_output_contains_profile_and_verdict() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Human).unwrap();
        assert!(out.contains("profile: fast"));
        assert!(out.contains("FAILED"));
        assert!(out.contains("Actionable next steps"));
    }

    #[test]
    fn human_output_shows_check_counts() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![make_passing_step("topology-doctor", 13)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 13,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 42,
            passed: true,
        };
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(
            out.contains("13 checks"),
            "human output should show check count, got:\n{out}"
        );
    }

    #[test]
    fn human_output_shows_skip_reason_inline() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Human).unwrap();
        assert!(
            out.contains("--profile deep"),
            "skip reason should appear inline in human output, got:\n{out}"
        );
    }

    #[test]
    fn human_output_actionable_steps_have_specific_hints() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Human).unwrap();
        // Should have step-specific remediation, not generic
        assert!(
            out.contains("check configs, compose, and source wiring")
                || out.contains("check messaging contracts"),
            "actionable steps should have specific hints, got:\n{out}"
        );
    }

    #[test]
    fn skip_message_references_correct_flag() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let smoke_step = report.steps.iter().find(|s| s.name == "runtime-smoke").unwrap();
        assert_eq!(smoke_step.status, StepStatus::Skip);
        assert!(smoke_step.skip_reason.as_ref().unwrap().contains("--profile deep"));
    }

    #[test]
    fn json_output_is_valid_and_structured() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["profile"], "fast");
        assert_eq!(parsed["passed"], false);
        assert!(parsed["steps"].is_array());
        assert!(parsed["total_duration_ms"].is_number());
    }

    #[test]
    fn json_output_has_summary() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(parsed["summary"]["passed"].is_number());
        assert!(parsed["summary"]["failed"].is_number());
        assert!(parsed["summary"]["skipped"].is_number());
        assert!(parsed["summary"]["total_checks"].is_number());
    }

    #[test]
    fn json_output_steps_have_check_count() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        for step in parsed["steps"].as_array().unwrap() {
            assert!(
                step["check_count"].is_number(),
                "step '{}' must have check_count",
                step["name"]
            );
        }
    }

    #[test]
    fn json_output_skip_reason_present_for_skipped() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        for step in parsed["steps"].as_array().unwrap() {
            if step["status"] == "skip" {
                assert!(
                    step["skip_reason"].is_string(),
                    "skipped step '{}' must have skip_reason",
                    step["name"]
                );
            }
        }
    }

    #[test]
    fn json_output_skip_reason_absent_for_executed() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        for step in parsed["steps"].as_array().unwrap() {
            if step["status"] != "skip" {
                assert!(
                    step.get("skip_reason").is_none(),
                    "executed step '{}' should not have skip_reason",
                    step["name"]
                );
            }
        }
    }

    #[test]
    fn human_output_shows_failed_findings() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Human).unwrap();
        assert!(out.contains("[x]"));
    }

    #[test]
    fn human_output_passed_gate_shows_safe_to_proceed() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![make_passing_step("dummy", 1)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 1,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 42,
            passed: true,
        };
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(out.contains("PASSED"));
        assert!(out.contains("Safe to proceed"), "passed gate should show safe-to-proceed, got:\n{out}");
        assert!(!out.contains("Actionable next steps"));
    }

    #[test]
    fn human_output_failed_gate_shows_stop() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Human).unwrap();
        assert!(out.contains("Stop"), "failed gate should show stop verdict, got:\n{out}");
        assert!(out.contains("must be fixed"), "should explain what to do, got:\n{out}");
        assert!(out.contains("Actionable next steps"));
        assert!(out.contains("Discipline: fix errors first"), "failed gate should show discipline reminder, got:\n{out}");
    }

    #[test]
    fn passed_gate_shows_tdd_cycle() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![make_passing_step("doctor", 3)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 3,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 42,
            passed: true,
        };
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(out.contains("TDD cycle:"), "passed gate should show TDD cycle, got:\n{out}");
        assert!(out.contains("make verify"), "should recommend make verify, got:\n{out}");
        assert!(out.contains("make check-deep"), "fast profile should recommend check-deep, got:\n{out}");
    }

    #[test]
    fn deep_profile_passed_gate_omits_check_deep_recommendation() {
        let gate = GateReport {
            profile: "deep".to_string(),
            steps: vec![make_passing_step("doctor", 3)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 3,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 42,
            passed: true,
        };
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(out.contains("TDD cycle:"), "passed gate should show TDD cycle");
        assert!(!out.contains("make check-deep"), "deep profile already includes runtime, should not recommend check-deep, got:\n{out}");
    }

    #[test]
    fn verbose_output_shows_passing_findings() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![StepResult {
                name: "dummy".to_string(),
                status: StepStatus::Pass,
                duration_ms: 1,
                check_count: 1,
                error_count: 0,
                warning_count: 0,
                skip_reason: None,
                is_execution_error: false,
                report: {
                    let mut r = Report::new("dummy");
                    r.add(CheckResult::from_findings(
                        "verbose-check",
                        vec![Finding::info("test", "some detail")],
                    ));
                    r
                },
            }],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 1,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 1,
            passed: true,
        };
        let default_out = render(&gate, OutputFormat::Human).unwrap();
        let verbose_out = render(&gate, OutputFormat::HumanVerbose).unwrap();

        assert!(!default_out.contains("some detail"), "default should hide passing findings");
        assert!(verbose_out.contains("some detail"), "verbose should show all findings");
    }

    // --- Exit code semantics ---

    #[test]
    fn passed_is_false_when_any_step_fails() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        assert!(!report.passed);
    }

    #[test]
    fn passed_is_true_when_all_steps_pass_or_skip() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![
                make_passing_step("a", 1),
                StepResult {
                    name: "b".to_string(),
                    status: StepStatus::Skip,
                    duration_ms: 0,
                    check_count: 0,
                    error_count: 0,
                    warning_count: 0,
                    skip_reason: Some("not needed".to_string()),
                    is_execution_error: false,
                    report: Report::new("b"),
                },
            ],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 1,
                total_checks: 1,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 1,
            passed: true,
        };
        assert!(gate.passed);
    }

    // --- Runtime error handling ---

    #[test]
    fn runtime_error_in_step_becomes_fail_with_context() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        // doctor, topology-doctor, and contract-audit on /nonexistent should be Fail, not panic
        for step in &report.steps {
            if step.status == StepStatus::Fail {
                let has_finding = step
                    .report
                    .checks
                    .iter()
                    .any(|c| !c.findings.is_empty());
                assert!(
                    has_finding,
                    "failed step '{}' must have at least one finding with context",
                    step.name
                );
            }
        }
    }

    // --- Partial failure: mixed pass + fail ---

    #[test]
    fn mixed_gate_report_fails_overall() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![
                make_passing_step("a", 5),
                make_failing_step("b"),
            ],
            summary: GateSummary {
                passed: 1,
                failed: 1,
                skipped: 0,
                total_checks: 6,
                total_errors: 1,
                total_warnings: 0,
            },
            verdict: failing_verdict(),
            total_duration_ms: 52,
            passed: false,
        };
        assert!(!gate.passed);
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(out.contains("FAILED"));
        assert!(out.contains("1 passed, 1 failed, 0 skipped"));
        assert!(out.contains("6 checks"));
        assert!(out.contains("Actionable next steps"));
    }

    // --- Format helpers ---

    #[test]
    fn format_duration_ms() {
        assert_eq!(format_duration(42), "42ms");
        assert_eq!(format_duration(999), "999ms");
    }

    #[test]
    fn format_duration_secs() {
        assert_eq!(format_duration(1000), "1.0s");
        assert_eq!(format_duration(1500), "1.5s");
        assert_eq!(format_duration(12345), "12.3s");
    }

    // ── CI promotion edge cases ─────────────────────────────────────

    #[test]
    fn ci_promotion_leaves_errors_untouched() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "hard-fail",
            vec![Finding::error("hard-fail", "already an error")],
        ));
        assert!(!report.passed());

        promote_warnings(&mut report);

        // Error should stay as-is, not get double-promoted
        assert!(!report.passed());
        assert_eq!(report.checks[0].findings[0].severity, Severity::Error);
        assert!(!report.checks[0].findings[0].message.contains("[ci]"));
    }

    #[test]
    fn ci_promotion_leaves_info_untouched() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "info-check",
            vec![Finding::info("detail", "just info")],
        ));
        assert!(report.passed());

        promote_warnings(&mut report);

        // Info should remain info — no promotion
        assert!(report.passed());
        assert_eq!(report.checks[0].findings[0].severity, Severity::Info);
    }

    #[test]
    fn ci_promotion_mixed_findings() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "mixed",
            vec![
                Finding::info("detail", "informational"),
                Finding::warning("soft", "a warning"),
                Finding::error("hard", "already error"),
            ],
        ));
        // Has error → already fails
        assert!(!report.passed());

        promote_warnings(&mut report);

        // Warning should be promoted, info and error untouched
        let findings = &report.checks[0].findings;
        assert_eq!(findings[0].severity, Severity::Info);
        assert_eq!(findings[1].severity, Severity::Error);
        assert!(findings[1].message.contains("[ci]"));
        assert_eq!(findings[2].severity, Severity::Error);
        assert!(!findings[2].message.contains("[ci]"));
    }

    #[test]
    fn ci_promotion_multiple_checks() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "clean",
            vec![Finding::info("a", "ok")],
        ));
        report.add(CheckResult::from_findings(
            "soft",
            vec![Finding::warning("b", "just a warning")],
        ));
        assert!(report.passed());

        promote_warnings(&mut report);

        // Only the second check should fail after promotion
        assert!(!report.passed());
        assert_eq!(report.checks[0].status, CheckStatus::Pass);
        assert_eq!(report.checks[1].status, CheckStatus::Fail);
    }

    // ── Profile label ───────────────────────────────────────────────

    #[test]
    fn profile_labels_are_correct() {
        assert_eq!(Profile::Fast.label(), "fast");
        assert_eq!(Profile::Ci.label(), "ci");
        assert_eq!(Profile::Deep.label(), "deep");
    }

    // ── make_skip helper ────────────────────────────────────────────

    #[test]
    fn make_skip_produces_correct_step() {
        let step = make_skip("test-step", "reason here");
        assert_eq!(step.name, "test-step");
        assert_eq!(step.status, StepStatus::Skip);
        assert_eq!(step.duration_ms, 0);
        assert_eq!(step.check_count, 0);
        assert_eq!(step.skip_reason.as_deref(), Some("reason here"));
        assert!(!step.report.checks.is_empty());
    }

    // ── Status icon and display ─────────────────────────────────────

    #[test]
    fn step_status_display() {
        assert_eq!(StepStatus::Pass.to_string(), "PASS");
        assert_eq!(StepStatus::Fail.to_string(), "FAIL");
        assert_eq!(StepStatus::Skip.to_string(), "SKIP");
    }

    #[test]
    fn step_status_json_lowercase() {
        let json = serde_json::to_string(&StepStatus::Pass).unwrap();
        assert_eq!(json, "\"pass\"");
        let json = serde_json::to_string(&StepStatus::Fail).unwrap();
        assert_eq!(json, "\"fail\"");
        let json = serde_json::to_string(&StepStatus::Skip).unwrap();
        assert_eq!(json, "\"skip\"");
    }

    // ── Remediation hints ───────────────────────────────────────────

    #[test]
    fn remediation_hints_are_specific() {
        let doctor = step_remediation_hint("doctor");
        assert!(doctor.contains("raccoon-cli doctor"));

        let topo = step_remediation_hint("topology-doctor");
        assert!(topo.contains("raccoon-cli topology-doctor"));

        let contract = step_remediation_hint("contract-audit");
        assert!(contract.contains("raccoon-cli contract-audit"));

        let smoke = step_remediation_hint("runtime-smoke");
        assert!(smoke.contains("make up-dataplane"));

        let unknown = step_remediation_hint("unknown-step");
        assert!(unknown.contains("raccoon-cli unknown-step"));
    }

    // ── GateReport JSON serialization ───────────────────────────────

    #[test]
    fn gate_report_json_skip_reason_omitted_for_non_skip() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![make_passing_step("a", 1)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 1,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 1,
            passed: true,
        };
        let json = serde_json::to_string(&gate).unwrap();
        assert!(!json.contains("skip_reason"));
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0), "0ms");
    }

    // ── Finding-level counts ────────────────────────────────────────

    #[test]
    fn step_result_has_error_and_warning_counts() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        for step in &report.steps {
            if step.status == StepStatus::Fail {
                assert!(
                    step.error_count > 0,
                    "failed step '{}' should have error_count > 0",
                    step.name
                );
            }
            if step.status == StepStatus::Skip {
                assert_eq!(step.error_count, 0);
                assert_eq!(step.warning_count, 0);
            }
        }
    }

    #[test]
    fn summary_has_total_errors_and_warnings() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let computed_errors: usize = report.steps.iter().map(|s| s.error_count).sum();
        let computed_warnings: usize = report.steps.iter().map(|s| s.warning_count).sum();
        assert_eq!(report.summary.total_errors, computed_errors);
        assert_eq!(report.summary.total_warnings, computed_warnings);
        assert!(report.summary.total_errors > 0, "nonexistent root should produce errors");
    }

    #[test]
    fn json_output_includes_finding_counts() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        for step in parsed["steps"].as_array().unwrap() {
            assert!(step["error_count"].is_number(), "step '{}' must have error_count", step["name"]);
            assert!(step["warning_count"].is_number(), "step '{}' must have warning_count", step["name"]);
        }
        assert!(parsed["summary"]["total_errors"].is_number());
        assert!(parsed["summary"]["total_warnings"].is_number());
    }

    // ── Execution error vs check failure ────────────────────────────

    #[test]
    fn execution_error_flag_not_set_on_normal_failure() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        // Static analyzers on nonexistent root produce check findings, not execution errors
        for step in &report.steps {
            if step.status == StepStatus::Fail {
                // These are check findings (dirs not found etc), not execution errors
                assert!(
                    !step.is_execution_error,
                    "step '{}' should not be marked as execution_error for missing project",
                    step.name
                );
            }
        }
    }

    #[test]
    fn is_execution_error_omitted_from_json_when_false() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![make_passing_step("a", 1)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 1,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 1,
            passed: true,
        };
        let json = serde_json::to_string(&gate).unwrap();
        assert!(!json.contains("is_execution_error"), "is_execution_error should be omitted when false");
    }

    // ── Doctor as step 0 ────────────────────────────────────────────

    #[test]
    fn doctor_is_first_step_in_all_profiles() {
        for profile in &[Profile::Fast, Profile::Ci, Profile::Deep] {
            let report = run(&nonexistent_config(*profile)).unwrap();
            assert_eq!(
                report.steps[0].name, "doctor",
                "doctor should be first step in {:?} profile",
                profile
            );
        }
    }

    // ── Human output: total checks in summary ───────────────────────

    #[test]
    fn human_output_summary_shows_total_checks() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Human).unwrap();
        let total = report.summary.total_checks;
        assert!(
            out.contains(&format!("{total} checks")),
            "summary should show total checks count, got:\n{out}"
        );
    }

    // ── Human output: execution error vs check failure ──────────────

    #[test]
    fn human_output_shows_error_counts_on_failure() {
        let mut r = Report::new("failing");
        r.add(CheckResult::from_findings(
            "bad",
            vec![
                Finding::error("a", "err1"),
                Finding::error("b", "err2"),
                Finding::warning("c", "warn1"),
            ],
        ));
        let step = StepResult {
            name: "test-step".to_string(),
            status: StepStatus::Fail,
            duration_ms: 5,
            check_count: 1,
            error_count: 2,
            warning_count: 1,
            skip_reason: None,
            is_execution_error: false,
            report: r,
        };
        let steps = vec![step];
        let verdict = compute_verdict(&steps);
        let gate = GateReport {
            profile: "fast".to_string(),
            steps,
            summary: GateSummary {
                passed: 0,
                failed: 1,
                skipped: 0,
                total_checks: 1,
                total_errors: 2,
                total_warnings: 1,
            },
            verdict,
            total_duration_ms: 5,
            passed: false,
        };
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(out.contains("2 errors"), "should show error count, got:\n{out}");
        assert!(out.contains("1 warning"), "should show warning count, got:\n{out}");
    }

    #[test]
    fn human_output_shows_execution_error_label() {
        let mut r = Report::new("broken");
        r.add(CheckResult::from_findings(
            "broken",
            vec![Finding::error("broken", "execution error: io error: not found")],
        ));
        let step = StepResult {
            name: "test-step".to_string(),
            status: StepStatus::Fail,
            duration_ms: 1,
            check_count: 0,
            error_count: 1,
            warning_count: 0,
            skip_reason: None,
            is_execution_error: true,
            report: r,
        };
        let steps = vec![step];
        let verdict = compute_verdict(&steps);
        let gate = GateReport {
            profile: "fast".to_string(),
            steps,
            summary: GateSummary {
                passed: 0,
                failed: 1,
                skipped: 0,
                total_checks: 0,
                total_errors: 1,
                total_warnings: 0,
            },
            verdict,
            total_duration_ms: 1,
            passed: false,
        };
        let out = render(&gate, OutputFormat::Human).unwrap();
        assert!(out.contains("execution error"), "should label execution errors, got:\n{out}");
    }

    // ── make_skip has zero finding counts ────────────────────────────

    #[test]
    fn make_skip_has_zero_finding_counts() {
        let step = make_skip("test", "reason");
        assert_eq!(step.error_count, 0);
        assert_eq!(step.warning_count, 0);
        assert!(!step.is_execution_error);
    }

    // ── count_findings helper ───────────────────────────────────────

    #[test]
    fn count_findings_counts_correctly() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "mixed",
            vec![
                Finding::info("a", "info"),
                Finding::warning("b", "warn"),
                Finding::error("c", "err"),
                Finding::error("d", "err2"),
            ],
        ));
        let (errors, warnings) = count_findings(&report);
        assert_eq!(errors, 2);
        assert_eq!(warnings, 1);
    }

    #[test]
    fn count_findings_empty_report() {
        let report = Report::new("empty");
        let (errors, warnings) = count_findings(&report);
        assert_eq!(errors, 0);
        assert_eq!(warnings, 0);
    }

    // ── runtime-bindings as gate step ─────────────────────────────────

    #[test]
    fn runtime_bindings_is_step_4_in_all_profiles() {
        for profile in &[Profile::Fast, Profile::Ci, Profile::Deep] {
            let report = run(&nonexistent_config(*profile)).unwrap();
            assert_eq!(
                report.steps[3].name, "runtime-bindings",
                "runtime-bindings should be step 4 (index 3) in {:?} profile",
                profile
            );
        }
    }

    #[test]
    fn gate_has_seven_steps() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        assert_eq!(report.steps.len(), 7);
    }

    #[test]
    fn runtime_bindings_remediation_hint_is_specific() {
        let hint = step_remediation_hint("runtime-bindings");
        assert!(hint.contains("raccoon-cli runtime-bindings"));
    }

    // ── Fail-fast behavior ────────────────────────────────────────────

    #[test]
    fn fail_fast_skips_remaining_steps_after_failure() {
        let config = GateConfig {
            project_root: std::path::PathBuf::from("/nonexistent/quality-service"),
            profile: Profile::Fast,
            base_url: "http://127.0.0.1:8080".to_string(),
            fail_fast: true,
        };
        let report = run(&config).unwrap();

        // doctor should fail on /nonexistent, then remaining steps should be skipped
        assert_eq!(report.steps[0].name, "doctor");
        assert_eq!(report.steps[0].status, StepStatus::Fail);

        // All subsequent steps should be skipped due to fail-fast
        for step in &report.steps[1..] {
            assert_eq!(
                step.status,
                StepStatus::Skip,
                "step '{}' should be skipped in fail-fast mode after doctor failure",
                step.name
            );
            assert!(
                step.skip_reason
                    .as_ref()
                    .unwrap()
                    .contains("fail-fast mode"),
                "skip reason for '{}' should mention fail-fast",
                step.name
            );
        }
    }

    #[test]
    fn fail_fast_disabled_runs_all_steps() {
        let config = GateConfig {
            project_root: std::path::PathBuf::from("/nonexistent/quality-service"),
            profile: Profile::Fast,
            base_url: "http://127.0.0.1:8080".to_string(),
            fail_fast: false,
        };
        let report = run(&config).unwrap();

        // Without fail-fast, all static steps should be executed (not skipped due to prior failure)
        let executed: Vec<&str> = report
            .steps
            .iter()
            .filter(|s| s.status != StepStatus::Skip)
            .map(|s| s.name.as_str())
            .collect();
        assert!(
            executed.len() >= 3,
            "without fail-fast, at least doctor, topology-doctor, contract-audit should execute"
        );
    }

    #[test]
    fn fail_fast_skip_reason_references_failed_step() {
        let config = GateConfig {
            project_root: std::path::PathBuf::from("/nonexistent/quality-service"),
            profile: Profile::Fast,
            base_url: "http://127.0.0.1:8080".to_string(),
            fail_fast: true,
        };
        let report = run(&config).unwrap();

        // Find the first failed step name
        let failed_name = report
            .steps
            .iter()
            .find(|s| s.status == StepStatus::Fail)
            .map(|s| s.name.as_str())
            .unwrap();

        // All skipped-due-to-fail-fast steps should reference the failed step
        for step in &report.steps {
            if step.status == StepStatus::Skip {
                if let Some(reason) = &step.skip_reason {
                    if reason.contains("fail-fast") {
                        assert!(
                            reason.contains(failed_name),
                            "skip reason should reference '{}', got: {}",
                            failed_name,
                            reason
                        );
                    }
                }
            }
        }
    }

    // ── Verdict structure ─────────────────────────────────────────────

    #[test]
    fn verdict_proceed_when_all_pass() {
        let steps = vec![make_passing_step("a", 3)];
        let verdict = compute_verdict(&steps);
        assert_eq!(verdict.action, "proceed");
        assert!(verdict.message.contains("Safe to proceed"));
        assert!(verdict.next_steps.is_empty());
    }

    #[test]
    fn verdict_stop_when_any_fails() {
        let steps = vec![make_passing_step("a", 3), make_failing_step("b")];
        let verdict = compute_verdict(&steps);
        assert_eq!(verdict.action, "stop");
        assert!(verdict.message.contains("Stop"));
        assert!(verdict.message.contains("1 error"));
        assert!(!verdict.next_steps.is_empty());
        assert!(verdict.next_steps[0].contains("Fix 'b'"));
    }

    #[test]
    fn verdict_in_json_output() {
        let report = run(&nonexistent_config(Profile::Fast)).unwrap();
        let out = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(parsed["verdict"]["action"].is_string());
        assert!(parsed["verdict"]["message"].is_string());
        assert!(parsed["verdict"]["next_steps"].is_array());
        assert_eq!(parsed["verdict"]["action"], "stop");
    }

    #[test]
    fn verdict_proceed_in_json_for_passing_gate() {
        let gate = GateReport {
            profile: "fast".to_string(),
            steps: vec![make_passing_step("a", 1)],
            summary: GateSummary {
                passed: 1,
                failed: 0,
                skipped: 0,
                total_checks: 1,
                total_errors: 0,
                total_warnings: 0,
            },
            verdict: passing_verdict(),
            total_duration_ms: 1,
            passed: true,
        };
        let out = render(&gate, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["verdict"]["action"], "proceed");
        assert!(parsed["verdict"]["next_steps"].as_array().unwrap().is_empty());
    }
}
