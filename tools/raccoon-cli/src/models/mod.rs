use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub check: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Why this finding matters — the consequence if ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// Recommended next step to resolve this finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

impl Finding {
    pub fn info(check: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            check: check.into(),
            message: message.into(),
            location: None,
            why: None,
            help: None,
        }
    }

    pub fn warning(check: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            check: check.into(),
            message: message.into(),
            location: None,
            why: None,
            help: None,
        }
    }

    pub fn error(check: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            check: check.into(),
            message: message.into(),
            location: None,
            why: None,
            help: None,
        }
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn with_why(mut self, why: impl Into<String>) -> Self {
        self.why = Some(why.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.check, self.message)?;
        if let Some(loc) = &self.location {
            write!(f, " ({loc})")?;
        }
        if let Some(why) = &self.why {
            write!(f, " -- {why}")?;
        }
        if let Some(help) = &self.help {
            write!(f, ". Fix: {help}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
    Skip,
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckStatus::Pass => write!(f, "PASS"),
            CheckStatus::Fail => write!(f, "FAIL"),
            CheckStatus::Skip => write!(f, "SKIP"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub findings: Vec<Finding>,
}

impl CheckResult {
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pass,
            findings: Vec::new(),
        }
    }

    pub fn from_findings(name: impl Into<String>, findings: Vec<Finding>) -> Self {
        let has_errors = findings.iter().any(|f| f.severity == Severity::Error);
        Self {
            name: name.into(),
            status: if has_errors {
                CheckStatus::Fail
            } else {
                CheckStatus::Pass
            },
            findings,
        }
    }

    pub fn skip(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Skip,
            findings: vec![Finding::info("skip", reason)],
        }
    }
}

impl fmt::Display for CheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "--- {}: {} ---", self.name, self.status)?;
        for finding in &self.findings {
            writeln!(f, "  {finding}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub title: String,
    pub checks: Vec<CheckResult>,
    pub passed: bool,
}

impl Report {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            checks: Vec::new(),
            passed: true,
        }
    }

    pub fn add(&mut self, result: CheckResult) {
        if result.status == CheckStatus::Fail {
            self.passed = false;
        }
        self.checks.push(result);
    }

    pub fn passed(&self) -> bool {
        self.passed
    }

    pub fn summary(&self) -> (usize, usize, usize) {
        let pass = self
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Pass)
            .count();
        let fail = self
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count();
        let skip = self
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Skip)
            .count();
        (pass, fail, skip)
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== {} ===", self.title)?;
        writeln!(f)?;
        for check in &self.checks {
            writeln!(f, "--- {}: {} ---", check.name, check.status)?;
            for finding in &check.findings {
                writeln!(f, "  {finding}")?;
            }
        }
        let (pass, fail, skip) = self.summary();
        let verdict = if self.passed() { "PASSED" } else { "FAILED" };
        write!(
            f,
            "Result: {verdict} | {pass} passed, {fail} failed, {skip} skipped"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finding_display_without_location() {
        let f = Finding::warning("test-check", "something is off");
        assert_eq!(f.to_string(), "[warning] test-check: something is off");
    }

    #[test]
    fn finding_display_with_location() {
        let f = Finding::error("test-check", "bad thing").with_location("file.go:42");
        assert_eq!(f.to_string(), "[error] test-check: bad thing (file.go:42)");
    }

    #[test]
    fn check_result_from_findings_pass_when_no_errors() {
        let findings = vec![Finding::info("a", "ok"), Finding::warning("b", "meh")];
        let result = CheckResult::from_findings("my-check", findings);
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.findings.len(), 2);
    }

    #[test]
    fn check_result_from_findings_fail_when_error_present() {
        let findings = vec![Finding::info("a", "ok"), Finding::error("b", "boom")];
        let result = CheckResult::from_findings("my-check", findings);
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn report_passed_when_all_pass() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        report.add(CheckResult::pass("b"));
        assert!(report.passed());
    }

    #[test]
    fn report_failed_when_any_fail() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        report.add(CheckResult::from_findings(
            "b",
            vec![Finding::error("x", "fail")],
        ));
        assert!(!report.passed());
    }

    #[test]
    fn report_summary_counts() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        report.add(CheckResult::pass("b"));
        report.add(CheckResult::from_findings(
            "c",
            vec![Finding::error("x", "fail")],
        ));
        report.add(CheckResult::skip("d", "not applicable"));
        assert_eq!(report.summary(), (2, 1, 1));
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn finding_json_omits_null_location() {
        let f = Finding::info("check", "msg");
        let json = serde_json::to_string(&f).unwrap();
        assert!(!json.contains("location"));
    }

    #[test]
    fn report_json_roundtrip() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"title\":\"test\""));
    }

    #[test]
    fn report_json_includes_passed_field() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"passed\":true"));

        let mut failing = Report::new("test");
        failing.add(CheckResult::from_findings(
            "b",
            vec![Finding::error("x", "fail")],
        ));
        let json = serde_json::to_string(&failing).unwrap();
        assert!(json.contains("\"passed\":false"));
    }

    #[test]
    fn report_human_output_shows_verdict() {
        let mut passing = Report::new("test");
        passing.add(CheckResult::pass("a"));
        assert!(passing.to_string().contains("PASSED"));

        let mut failing = Report::new("test");
        failing.add(CheckResult::from_findings(
            "b",
            vec![Finding::error("x", "boom")],
        ));
        assert!(failing.to_string().contains("FAILED"));
    }

    #[test]
    fn report_passed_tracks_incrementally() {
        let mut report = Report::new("test");
        assert!(report.passed());
        report.add(CheckResult::pass("a"));
        assert!(report.passed());
        report.add(CheckResult::from_findings(
            "b",
            vec![Finding::error("x", "fail")],
        ));
        assert!(!report.passed());
        report.add(CheckResult::pass("c"));
        assert!(!report.passed()); // still false after adding more passes
    }

    #[test]
    fn report_with_only_skips_still_passes() {
        let mut report = Report::new("test");
        report.add(CheckResult::skip("a", "not applicable"));
        report.add(CheckResult::skip("b", "also skipped"));
        assert!(report.passed(), "skip-only report should pass");
        assert_eq!(report.summary(), (0, 0, 2));
    }

    #[test]
    fn check_result_from_empty_findings_passes() {
        let result = CheckResult::from_findings("empty", Vec::new());
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn check_result_skip_has_info_finding() {
        let result = CheckResult::skip("skipped", "reason here");
        assert_eq!(result.status, CheckStatus::Skip);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].severity, Severity::Info);
        assert_eq!(result.findings[0].check, "skip");
        assert_eq!(result.findings[0].message, "reason here");
    }

    #[test]
    fn finding_location_serialized_in_json() {
        let f = Finding::error("check", "msg").with_location("file.go:10");
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"location\":\"file.go:10\""));
    }

    #[test]
    fn severity_display_matches_serde() {
        for (sev, expected) in [
            (Severity::Info, "info"),
            (Severity::Warning, "warning"),
            (Severity::Error, "error"),
        ] {
            assert_eq!(sev.to_string(), expected);
            let json = serde_json::to_string(&sev).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn check_status_display_and_serde() {
        assert_eq!(CheckStatus::Pass.to_string(), "PASS");
        assert_eq!(CheckStatus::Fail.to_string(), "FAIL");
        assert_eq!(CheckStatus::Skip.to_string(), "SKIP");

        let json = serde_json::to_string(&CheckStatus::Pass).unwrap();
        assert_eq!(json, "\"pass\"");
    }

    #[test]
    fn report_empty_is_passed() {
        let report = Report::new("empty");
        assert!(report.passed());
        assert_eq!(report.summary(), (0, 0, 0));
    }

    #[test]
    fn report_display_includes_all_checks() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        report.add(CheckResult::skip("b", "skipped"));
        report.add(CheckResult::from_findings(
            "c",
            vec![Finding::error("x", "boom")],
        ));
        let display = report.to_string();
        assert!(display.contains("=== test ==="));
        assert!(display.contains("a: PASS"));
        assert!(display.contains("b: SKIP"));
        assert!(display.contains("c: FAIL"));
        assert!(display.contains("FAILED"));
        assert!(display.contains("1 passed, 1 failed, 1 skipped"));
    }

    #[test]
    fn check_result_pass_has_no_findings() {
        let result = CheckResult::pass("clean");
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn warnings_only_does_not_fail() {
        let findings = vec![
            Finding::warning("a", "minor issue"),
            Finding::warning("b", "another minor"),
        ];
        let result = CheckResult::from_findings("soft", findings);
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn report_json_has_correct_structure() {
        let mut report = Report::new("test");
        report.add(CheckResult::pass("a"));
        report.add(CheckResult::from_findings(
            "b",
            vec![Finding::warning("w", "watch out").with_location("here.go:1")],
        ));
        let json: serde_json::Value = serde_json::to_value(&report).unwrap();
        assert_eq!(json["title"], "test");
        assert_eq!(json["passed"], true); // only warnings
        assert_eq!(json["checks"].as_array().unwrap().len(), 2);

        let check_b = &json["checks"][1];
        assert_eq!(check_b["name"], "b");
        assert_eq!(check_b["status"], "pass");
        assert_eq!(check_b["findings"][0]["severity"], "warning");
        assert_eq!(check_b["findings"][0]["location"], "here.go:1");
    }

    // ── Finding why/help fields ─────────────────────────────────────

    #[test]
    fn finding_with_why_display() {
        let f = Finding::error("check", "something broke").with_why("this causes data loss");
        assert_eq!(
            f.to_string(),
            "[error] check: something broke -- this causes data loss"
        );
    }

    #[test]
    fn finding_with_help_display() {
        let f = Finding::error("check", "something broke").with_help("run fix command");
        assert_eq!(
            f.to_string(),
            "[error] check: something broke. Fix: run fix command"
        );
    }

    #[test]
    fn finding_with_why_and_help_display() {
        let f = Finding::error("check", "missing config")
            .with_why("pipeline will not start")
            .with_help("add the config file");
        assert_eq!(
            f.to_string(),
            "[error] check: missing config -- pipeline will not start. Fix: add the config file"
        );
    }

    #[test]
    fn finding_with_all_fields_display() {
        let f = Finding::error("check", "bad value")
            .with_location("file.go:10")
            .with_why("breaks routing")
            .with_help("fix the value");
        let display = f.to_string();
        assert!(display.contains("(file.go:10)"));
        assert!(display.contains("-- breaks routing"));
        assert!(display.contains("Fix: fix the value"));
    }

    #[test]
    fn finding_json_omits_null_why_and_help() {
        let f = Finding::info("check", "msg");
        let json = serde_json::to_string(&f).unwrap();
        assert!(!json.contains("why"));
        assert!(!json.contains("help"));
    }

    #[test]
    fn finding_json_includes_why_and_help_when_set() {
        let f = Finding::error("check", "msg")
            .with_why("reason")
            .with_help("suggestion");
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"why\":\"reason\""));
        assert!(json.contains("\"help\":\"suggestion\""));
    }

    #[test]
    fn finding_with_why_only_no_help() {
        let f = Finding::warning("check", "something").with_why("context");
        assert!(f.why.is_some());
        assert!(f.help.is_none());
        assert!(f.to_string().contains("-- context"));
        assert!(!f.to_string().contains("Fix:"));
    }
}
