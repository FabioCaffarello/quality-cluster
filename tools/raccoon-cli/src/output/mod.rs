use crate::error::Result;
use crate::models::{CheckStatus, Report, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    HumanVerbose,
    Json,
}

pub fn render(report: &Report, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Human => Ok(render_human(report, false)),
        OutputFormat::HumanVerbose => Ok(render_human(report, true)),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(report)?;
            Ok(json)
        }
    }
}

/// Render a Report as human-readable text.
/// When `verbose` is true, all findings are shown.
/// When false, only failed checks show their findings.
pub fn render_human(report: &Report, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== {} ===", report.title).unwrap();
    writeln!(out).unwrap();

    for check in &report.checks {
        writeln!(out, "--- {}: {} ---", check.name, check.status).unwrap();
        if verbose || check.status == CheckStatus::Fail {
            for finding in &check.findings {
                writeln!(out, "  {finding}").unwrap();
            }
        }
    }

    let (pass, fail, skip) = report.summary();
    let verdict = if report.passed() { "PASSED" } else { "FAILED" };
    writeln!(
        out,
        "Result: {verdict} | {pass} passed, {fail} failed, {skip} skipped"
    )
    .unwrap();

    // Guard rail verdict
    if report.passed() {
        writeln!(out).unwrap();
        writeln!(out, "> Safe to proceed — all checks passed.").unwrap();
    } else {
        let error_count = report
            .checks
            .iter()
            .flat_map(|c| &c.findings)
            .filter(|f| f.severity == Severity::Error)
            .count();
        let error_word = if error_count == 1 { "error" } else { "errors" };
        writeln!(out).unwrap();
        writeln!(
            out,
            "> Stop — {error_count} {error_word} must be fixed before proceeding."
        )
        .unwrap();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CheckResult, Finding};

    fn sample_report() -> Report {
        let mut report = Report::new("test report");
        report.add(CheckResult::pass("check-a"));
        report.add(CheckResult::from_findings(
            "check-b",
            vec![Finding::warning("rule", "watch out")],
        ));
        report
    }

    #[test]
    fn human_output_contains_title() {
        let out = render(&sample_report(), OutputFormat::Human).unwrap();
        assert!(out.contains("test report"));
        assert!(out.contains("PASS"));
    }

    #[test]
    fn verbose_output_shows_all_findings() {
        let out = render(&sample_report(), OutputFormat::HumanVerbose).unwrap();
        assert!(out.contains("test report"));
        assert!(out.contains("watch out"));
    }

    #[test]
    fn non_verbose_hides_passing_findings() {
        let out = render(&sample_report(), OutputFormat::Human).unwrap();
        // check-b passes (only warning, no error), so its findings should be hidden
        assert!(!out.contains("watch out"));
    }

    #[test]
    fn json_output_is_valid() {
        let out = render(&sample_report(), OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["title"], "test report");
        assert_eq!(parsed["checks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn render_human_shows_verdict() {
        let mut passing = Report::new("test");
        passing.add(CheckResult::pass("a"));
        assert!(render_human(&passing, false).contains("PASSED"));

        let mut failing = Report::new("test");
        failing.add(CheckResult::from_findings(
            "b",
            vec![Finding::error("x", "boom")],
        ));
        assert!(render_human(&failing, false).contains("FAILED"));
    }

    #[test]
    fn render_human_verbose_shows_findings_for_passing_checks() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "info-check",
            vec![Finding::info("detail", "some detail info")],
        ));
        let default = render_human(&report, false);
        let verbose = render_human(&report, true);
        assert!(!default.contains("some detail info"));
        assert!(verbose.contains("some detail info"));
    }

    #[test]
    fn render_human_failed_check_shows_findings() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "bad",
            vec![Finding::error("bad", "this is broken")],
        ));
        let out = render_human(&report, false);
        assert!(
            out.contains("this is broken"),
            "failed check findings should show in non-verbose mode"
        );
    }

    #[test]
    fn render_json_pretty_printed() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        let out = render(&report, OutputFormat::Json).unwrap();
        assert!(out.contains('\n'), "JSON should be pretty-printed");
    }

    #[test]
    fn render_empty_report() {
        let report = Report::new("empty");
        let human = render(&report, OutputFormat::Human).unwrap();
        assert!(human.contains("=== empty ==="));
        assert!(human.contains("PASSED"));
        assert!(human.contains("0 passed, 0 failed, 0 skipped"));
    }

    #[test]
    fn render_report_with_skip() {
        let mut report = Report::new("test");
        report.add(CheckResult::skip("skipped-check", "not applicable"));
        let human = render(&report, OutputFormat::Human).unwrap();
        assert!(human.contains("SKIP"));
        assert!(human.contains("PASSED"));

        let json = render(&report, OutputFormat::Json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["passed"], true);
        assert_eq!(parsed["checks"][0]["status"], "skip");
    }

    #[test]
    fn render_human_shows_location_for_failed_findings() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "located",
            vec![Finding::error("check", "problem here").with_location("file.go:42")],
        ));
        let out = render_human(&report, false);
        assert!(out.contains("file.go:42"));
    }

    // ── Guard rail verdict ──────────────────────────────────────────

    #[test]
    fn render_human_shows_safe_to_proceed_on_pass() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        let out = render_human(&report, false);
        assert!(
            out.contains("Safe to proceed"),
            "passing report should show safe-to-proceed verdict, got:\n{out}"
        );
    }

    #[test]
    fn render_human_shows_stop_on_failure() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "bad",
            vec![Finding::error("x", "broke")],
        ));
        let out = render_human(&report, false);
        assert!(
            out.contains("Stop"),
            "failing report should show stop verdict, got:\n{out}"
        );
        assert!(
            out.contains("1 error must be fixed"),
            "should count errors, got:\n{out}"
        );
    }

    #[test]
    fn render_human_stop_pluralizes_errors() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "bad",
            vec![
                Finding::error("x", "err1"),
                Finding::error("y", "err2"),
                Finding::error("z", "err3"),
            ],
        ));
        let out = render_human(&report, false);
        assert!(
            out.contains("3 errors must be fixed"),
            "should pluralize, got:\n{out}"
        );
    }

    #[test]
    fn render_human_shows_why_and_help_for_failed_findings() {
        let mut report = Report::new("test");
        report.add(CheckResult::from_findings(
            "bad",
            vec![Finding::error("x", "something broke")
                .with_why("causes data loss")
                .with_help("run fix command")],
        ));
        let out = render_human(&report, false);
        assert!(
            out.contains("causes data loss"),
            "should show why, got:\n{out}"
        );
        assert!(
            out.contains("Fix: run fix command"),
            "should show help, got:\n{out}"
        );
    }
}
