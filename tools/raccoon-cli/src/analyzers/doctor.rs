use crate::error::Result;
use crate::models::{CheckResult, Finding, Report};
use std::path::Path;

pub fn analyze(project_root: &Path) -> Result<Report> {
    let mut report = Report::new("doctor");

    // Check that project root looks like quality-service
    let go_work = project_root.join("go.work");
    if go_work.exists() {
        report.add(CheckResult::pass("project-root"));
    } else {
        report.add(CheckResult::from_findings(
            "project-root",
            vec![Finding::error(
                "project-root",
                format!("go.work not found at {}", project_root.display()),
            )
            .with_why("go.work is the workspace root marker; all other checks depend on it")
            .with_help("pass --project-root pointing to the quality-service root")],
        ));
    }

    // Check for expected directories
    let dir_reasons: &[(&str, &str)] = &[
        (
            "internal",
            "topology-doctor and contract-audit scan internal/ for Go source artifacts",
        ),
        (
            "deploy",
            "topology-doctor reads configs and compose from deploy/",
        ),
        ("tests", "test infrastructure lives in tests/"),
        ("tools", "raccoon-cli and other tooling live in tools/"),
    ];

    for (dir, why) in dir_reasons {
        let path = project_root.join(dir);
        if path.is_dir() {
            report.add(CheckResult::pass(format!("dir-{dir}")));
        } else {
            report.add(CheckResult::from_findings(
                format!("dir-{dir}"),
                vec![Finding::warning(
                    "project-structure",
                    format!("expected directory '{dir}/' not found"),
                )
                .with_why(*why)
                .with_help("verify --project-root or create the directory")],
            ));
        }
    }

    // Check for docker-compose file
    let compose = project_root.join("deploy/compose/docker-compose.yaml");
    if compose.is_file() {
        report.add(CheckResult::pass("compose-file"));
    } else {
        report.add(CheckResult::from_findings(
            "compose-file",
            vec![Finding::warning(
                "project-structure",
                "deploy/compose/docker-compose.yaml not found",
            )
            .with_why("topology-doctor validates service wiring against the compose file; runtime-smoke needs it to start the environment")
            .with_help("create the compose file or check --project-root")],
        ));
    }

    // Check for config files
    let configs_dir = project_root.join("deploy/configs");
    if configs_dir.is_dir() {
        let has_jsonc = std::fs::read_dir(&configs_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("jsonc"))
            })
            .unwrap_or(false);
        if has_jsonc {
            report.add(CheckResult::pass("config-files"));
        } else {
            report.add(CheckResult::from_findings(
                "config-files",
                vec![Finding::warning(
                    "project-structure",
                    "deploy/configs/ exists but contains no .jsonc files",
                )
                .with_why("topology-doctor reads .jsonc configs to validate transport consistency")
                .with_help(
                    "add service configs (consumer.jsonc, emulator.jsonc, validator.jsonc)",
                )],
            ));
        }
    } else {
        report.add(CheckResult::from_findings(
            "config-files",
            vec![Finding::warning(
                "project-structure",
                "deploy/configs/ not found",
            )
            .with_why("service configs are required for topology-doctor to validate transport wiring")
            .with_help("create deploy/configs/ with .jsonc service config files")],
        ));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CheckStatus;

    #[test]
    fn on_nonexistent_root_fails() {
        let report = analyze(std::path::Path::new("/nonexistent")).unwrap();
        assert!(!report.passed());
    }

    #[test]
    fn error_message_is_actionable() {
        let report = analyze(std::path::Path::new("/nonexistent")).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "project-root")
            .unwrap();
        let finding = &check.findings[0];
        assert!(
            finding.why.is_some(),
            "doctor error should explain why it matters"
        );
        assert!(finding.help.is_some(), "doctor error should suggest a fix");
    }

    #[test]
    fn checks_compose_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.23").unwrap();
        std::fs::create_dir_all(dir.path().join("internal")).unwrap();
        std::fs::create_dir_all(dir.path().join("deploy")).unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::create_dir_all(dir.path().join("tools")).unwrap();

        let report = analyze(dir.path()).unwrap();
        let compose_check = report.checks.iter().find(|c| c.name == "compose-file");
        assert!(
            compose_check.is_some(),
            "doctor should check for compose file"
        );
    }

    #[test]
    fn checks_config_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.23").unwrap();
        std::fs::create_dir_all(dir.path().join("internal")).unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/configs")).unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::create_dir_all(dir.path().join("tools")).unwrap();

        let report = analyze(dir.path()).unwrap();
        let config_check = report
            .checks
            .iter()
            .find(|c| c.name == "config-files")
            .unwrap();
        assert!(config_check
            .findings
            .iter()
            .any(|f| f.message.contains("no .jsonc")));
    }

    #[test]
    fn passes_on_valid_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.23").unwrap();
        std::fs::create_dir_all(dir.path().join("internal")).unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/configs")).unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/compose")).unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::create_dir_all(dir.path().join("tools")).unwrap();
        std::fs::write(
            dir.path().join("deploy/configs/consumer.jsonc"),
            r#"{"kafka": {"brokers": ["kafka:9092"]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("deploy/compose/docker-compose.yaml"),
            "services:\n  nats:\n    image: nats\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        assert!(report.passed());
    }

    #[test]
    fn report_title_is_doctor() {
        let report = analyze(std::path::Path::new("/nonexistent")).unwrap();
        assert_eq!(report.title, "doctor");
    }

    #[test]
    fn all_checks_have_unique_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.23").unwrap();
        std::fs::create_dir_all(dir.path().join("internal")).unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/configs")).unwrap();
        std::fs::create_dir_all(dir.path().join("deploy/compose")).unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::create_dir_all(dir.path().join("tools")).unwrap();
        std::fs::write(dir.path().join("deploy/configs/consumer.jsonc"), "{}").unwrap();
        std::fs::write(dir.path().join("deploy/compose/docker-compose.yaml"), "").unwrap();

        let report = analyze(dir.path()).unwrap();
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "all check names must be unique: {names:?}"
        );
    }

    #[test]
    fn missing_dirs_are_warnings_not_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.23").unwrap();
        // No internal/, deploy/, tests/, tools/

        let report = analyze(dir.path()).unwrap();
        let dir_checks: Vec<_> = report
            .checks
            .iter()
            .filter(|c| c.name.starts_with("dir-"))
            .collect();
        for check in &dir_checks {
            // Missing dirs should produce warnings, not errors → check still passes
            assert_eq!(
                check.status,
                CheckStatus::Pass,
                "missing dir '{}' should be a warning (pass), not error (fail)",
                check.name
            );
        }
    }

    // ── Guard rail: actionable findings ─────────────────────────────

    #[test]
    fn all_findings_have_why_and_help() {
        let report = analyze(std::path::Path::new("/nonexistent")).unwrap();
        for check in &report.checks {
            for finding in &check.findings {
                assert!(
                    finding.why.is_some(),
                    "finding in '{}' should have 'why': {:?}",
                    check.name,
                    finding.message,
                );
                assert!(
                    finding.help.is_some(),
                    "finding in '{}' should have 'help': {:?}",
                    check.name,
                    finding.message,
                );
            }
        }
    }

    #[test]
    fn warning_findings_explain_downstream_impact() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.work"), "go 1.23").unwrap();
        // Missing all dirs

        let report = analyze(dir.path()).unwrap();
        for check in &report.checks {
            for finding in &check.findings {
                if finding.severity == crate::models::Severity::Warning {
                    let why = finding.why.as_deref().unwrap_or("");
                    assert!(
                        !why.is_empty(),
                        "warning '{}' should explain why it matters",
                        finding.message,
                    );
                }
            }
        }
    }
}
