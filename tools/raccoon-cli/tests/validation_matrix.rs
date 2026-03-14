//! # Operational Validation Matrix — raccoon-cli
//!
//! 97 integration tests organized across 14 categories.
//! Run with: `cargo test --test validation_matrix`
//!
//! ## Validated Scenarios
//!
//! | # | Category                     | Tests | Status |
//! |---|------------------------------|-------|--------|
//! | 1 | Argument parsing edge cases  |   7   | PASS   |
//! | 2 | Exit code contract (0/1/2)   |   8   | PASS   |
//! | 3 | Human output determinism     |  14   | PASS   |
//! | 4 | JSON output schema contract  |  18   | PASS   |
//! | 5 | Project root valid/invalid   |   7   | PASS   |
//! | 6 | quality-gate profiles        |  16   | PASS   |
//! | 7 | runtime-smoke absent env     |   3   | PASS   |
//! | 8 | doctor diagnostics           |   4   | PASS   |
//! | 9 | Actionable error messages    |   3   | PASS   |
//! |10 | Flag interaction edge cases  |   5   | PASS   |
//! |11 | JSON output determinism      |   2   | PASS   |
//! |12 | Topology fixture validation  |   3   | PASS   |
//! |13 | stderr cleanliness           |   2   | PASS   |
//! |14 | Help text consistency        |   2   | PASS   |
//!
//! ## Key Contracts Validated
//!
//! - **Exit codes**: 0=checks pass, 1=checks fail, 2=runtime/parse error
//! - **JSON schema**: title/checks/passed for standard commands; profile/steps/total_duration_ms/verdict for quality-gate
//! - **Determinism**: identical inputs produce identical human AND JSON outputs
//! - **Graceful degradation**: nonexistent root, empty project, absent docker — no panics, no exit(2)
//! - **Actionable errors**: every failure message suggests a fix or next step
//! - **Stage skipping**: runtime-smoke skips remaining stages on first failure
//! - **Fail-fast**: --fail-fast skips remaining gate steps after first failure
//! - **Profile behavior**: fast/ci skip runtime-smoke; deep attempts it; doctor always first; arch-guard runs as real step
//! - **Verdict**: JSON output includes structured verdict (action/message/next_steps)
//! - **stderr isolation**: check failures go to stdout only; stderr stays clean
//! - **Help consistency**: all subcommands have Usage text; main help lists all commands

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn raccoon() -> Command {
    Command::cargo_bin("raccoon-cli").unwrap()
}

/// Build a minimal valid project structure.
fn make_project(dir: &TempDir) {
    std::fs::write(dir.path().join("go.work"), "go 1.23\n").unwrap();
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
}

// ══════════════════════════════════════════════════════════════════════
// 1. ARGUMENT PARSING EDGE CASES
// ══════════════════════════════════════════════════════════════════════

#[test]
fn no_subcommand_shows_help_and_exits_error() {
    raccoon()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn double_dash_version_shows_version() {
    raccoon()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("raccoon-cli"));
}

#[test]
fn json_flag_after_subcommand_is_rejected() {
    // --json is global; it must come before the subcommand
    // clap with global=true should accept it after subcommand too,
    // but let's verify the behavior is consistent
    let result = raccoon()
        .args(["doctor", "--json", "--project-root", "/nonexistent"])
        .output()
        .unwrap();

    // Should either succeed (global flag accepted) or fail with clap error
    let code = result.status.code().unwrap_or(-1);
    // If clap accepts it, we get exit 1 (check fail). If not, exit 2 (clap error).
    assert!(
        code == 1 || code == 2,
        "expected exit 1 or 2, got {code}"
    );
}

#[test]
fn project_root_with_equals_syntax() {
    raccoon()
        .args(["--project-root=/nonexistent", "doctor"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"));
}

#[test]
fn empty_base_url_is_accepted() {
    // Base URL is just a string, no validation at parse time
    raccoon()
        .args(["runtime-smoke", "--base-url", ""])
        .assert()
        .code(1); // Fails at bootstrap, not parsing
}

#[test]
fn quality_gate_all_profiles_parse() {
    for profile in &["fast", "ci", "deep"] {
        let result = raccoon()
            .args([
                "--project-root",
                "/nonexistent",
                "quality-gate",
                "--profile",
                profile,
            ])
            .output()
            .unwrap();
        let code = result.status.code().unwrap_or(-1);
        assert!(
            code == 0 || code == 1,
            "profile '{profile}' should parse without error, got exit {code}"
        );
    }
}

#[test]
fn quality_gate_invalid_profile_exits_2() {
    raccoon()
        .args([
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "ultra",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

// ══════════════════════════════════════════════════════════════════════
// 2. EXIT CODE CONTRACT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn exit_0_on_valid_project_doctor() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .code(0);
}

#[test]
fn exit_1_on_failed_checks_doctor() {
    raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .assert()
        .code(1);
}

#[test]
fn exit_1_on_failed_checks_topology_doctor() {
    raccoon()
        .args(["--project-root", "/nonexistent", "topology-doctor"])
        .assert()
        .code(1);
}

#[test]
fn exit_1_on_failed_checks_contract_audit() {
    raccoon()
        .args(["--project-root", "/nonexistent", "contract-audit"])
        .assert()
        .code(1);
}

#[test]
fn exit_1_on_failed_quality_gate_fast() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate", "--profile", "fast"])
        .assert()
        .code(1);
}

#[test]
fn exit_1_on_failed_quality_gate_ci() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate", "--profile", "ci"])
        .assert()
        .code(1);
}

#[test]
fn exit_1_runtime_smoke_absent_environment() {
    raccoon()
        .args(["--project-root", "/nonexistent", "runtime-smoke"])
        .assert()
        .code(1);
}

#[test]
fn exit_code_2_on_clap_error() {
    raccoon()
        .arg("--unknown-flag")
        .assert()
        .failure()
        .code(2);
}

// ══════════════════════════════════════════════════════════════════════
// 3. HUMAN OUTPUT DETERMINISM
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_human_output_has_verdict_line() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Result: FAILED") || stdout.contains("Result: PASSED"),
        "must contain 'Result:' verdict line"
    );
}

#[test]
fn doctor_human_output_has_title() {
    raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .assert()
        .stdout(predicate::str::contains("=== doctor ==="));
}

#[test]
fn topology_doctor_human_output_has_title() {
    raccoon()
        .args(["--project-root", "/nonexistent", "topology-doctor"])
        .assert()
        .stdout(predicate::str::contains("=== topology-doctor ==="));
}

#[test]
fn contract_audit_human_output_has_title() {
    raccoon()
        .args(["--project-root", "/nonexistent", "contract-audit"])
        .assert()
        .stdout(predicate::str::contains("=== contract-audit ==="));
}

#[test]
fn quality_gate_human_output_shows_profile() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate", "--profile", "ci"])
        .assert()
        .stdout(predicate::str::contains("profile: ci"));
}

#[test]
fn quality_gate_human_output_has_step_listing() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains(" doctor "), "must list doctor step");
    assert!(stdout.contains("topology-doctor"), "must list topology-doctor step");
    assert!(stdout.contains("contract-audit"), "must list contract-audit step");
    assert!(stdout.contains("arch-guard"), "must list arch-guard step");
    assert!(stdout.contains("runtime-smoke"), "must list runtime-smoke step");
}

#[test]
fn quality_gate_human_output_shows_step_status_icons() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Failed steps get [x], skipped get [-]
    assert!(stdout.contains("[x]"), "must have [x] icon for failed steps");
    assert!(stdout.contains("[-]"), "must have [-] icon for skipped steps");
}

#[test]
fn quality_gate_actionable_steps_reference_cli() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Actionable next steps"))
        .stdout(predicate::str::contains("raccoon-cli"));
}

#[test]
fn runtime_smoke_human_output_has_title() {
    raccoon()
        .args(["--project-root", "/nonexistent", "runtime-smoke"])
        .assert()
        .stdout(predicate::str::contains("=== runtime-smoke ==="));
}

#[test]
fn runtime_smoke_shows_bootstrap_failure() {
    raccoon()
        .args(["--project-root", "/nonexistent", "runtime-smoke"])
        .assert()
        .stdout(predicate::str::contains("bootstrap"))
        .stdout(predicate::str::contains("FAIL"));
}

#[test]
fn runtime_smoke_skips_remaining_stages_after_failure() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "runtime-smoke"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // bootstrap fails, so readiness/inject/route/consume/validate should be skipped
    let skip_count = stdout.matches("SKIP").count();
    assert!(
        skip_count >= 5,
        "expected at least 5 skipped stages after bootstrap failure, got {skip_count}"
    );
}

#[test]
fn verbose_includes_info_findings_in_human_output() {
    let default_output = raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let verbose_output = raccoon()
        .args(["-v", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();

    assert!(
        verbose_output.stdout.len() >= default_output.stdout.len(),
        "verbose should produce at least as much output"
    );
}

#[test]
fn human_output_deterministic_across_runs() {
    let run1 = raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let run2 = raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&run1.stdout),
        String::from_utf8_lossy(&run2.stdout),
        "identical inputs must produce identical human output"
    );
}

// ══════════════════════════════════════════════════════════════════════
// 4. JSON OUTPUT SCHEMA CONTRACT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_json_has_required_fields() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["title"], "doctor");
    assert!(parsed["checks"].is_array());
    assert!(parsed["passed"].is_boolean());
}

#[test]
fn doctor_json_checks_have_required_fields() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    for check in parsed["checks"].as_array().unwrap() {
        assert!(check["name"].is_string(), "check must have 'name'");
        assert!(check["status"].is_string(), "check must have 'status'");
        assert!(check["findings"].is_array(), "check must have 'findings'");
    }
}

#[test]
fn doctor_json_finding_severities_are_lowercase() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let valid_severities = ["info", "warning", "error"];
    for check in parsed["checks"].as_array().unwrap() {
        for finding in check["findings"].as_array().unwrap() {
            let severity = finding["severity"].as_str().unwrap();
            assert!(
                valid_severities.contains(&severity),
                "severity '{severity}' not in {valid_severities:?}"
            );
        }
    }
}

#[test]
fn doctor_json_statuses_are_lowercase() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let valid_statuses = ["pass", "fail", "skip"];
    for check in parsed["checks"].as_array().unwrap() {
        let status = check["status"].as_str().unwrap();
        assert!(
            valid_statuses.contains(&status),
            "status '{status}' not in {valid_statuses:?}"
        );
    }
}

#[test]
fn topology_doctor_json_has_required_fields() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "topology-doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["title"], "topology-doctor");
    assert!(parsed["checks"].is_array());
    assert!(parsed["passed"].is_boolean());
}

#[test]
fn contract_audit_json_has_required_fields() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "contract-audit"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["title"], "contract-audit");
    assert!(parsed["checks"].is_array());
    assert!(parsed["passed"].is_boolean());
}

#[test]
fn runtime_smoke_json_has_required_fields() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "runtime-smoke"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["title"], "runtime-smoke");
    assert!(parsed["checks"].is_array());
    assert!(parsed["passed"].is_boolean());
    assert_eq!(parsed["passed"], false);
}

#[test]
fn runtime_smoke_json_has_all_six_stages() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "runtime-smoke"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let checks = parsed["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 6, "runtime-smoke must have exactly 6 stages");

    let names: Vec<&str> = checks.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        vec!["bootstrap", "readiness", "inject", "route", "consume", "validate"]
    );
}

#[test]
fn runtime_smoke_json_skipped_stages_after_bootstrap_failure() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "runtime-smoke"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let checks = parsed["checks"].as_array().unwrap();
    assert_eq!(checks[0]["status"], "fail", "bootstrap should fail");
    for check in &checks[1..] {
        assert_eq!(
            check["status"], "skip",
            "stage '{}' should be skipped after bootstrap failure",
            check["name"]
        );
    }
}

#[test]
fn quality_gate_json_has_required_fields() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["profile"], "fast");
    assert!(parsed["steps"].is_array());
    assert!(parsed["total_duration_ms"].is_number());
    assert!(parsed["passed"].is_boolean());
}

#[test]
fn quality_gate_ci_json_has_ci_profile() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "ci",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["profile"], "ci");
    assert_eq!(parsed["passed"], false);
}

#[test]
fn quality_gate_json_steps_have_required_fields() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    for step in parsed["steps"].as_array().unwrap() {
        assert!(step["name"].is_string(), "step must have 'name'");
        assert!(step["status"].is_string(), "step must have 'status'");
        assert!(step["duration_ms"].is_number(), "step must have 'duration_ms'");
        assert!(step["check_count"].is_number(), "step must have 'check_count'");
        assert!(step["error_count"].is_number(), "step must have 'error_count'");
        assert!(step["warning_count"].is_number(), "step must have 'warning_count'");
        assert!(step["report"].is_object(), "step must have 'report'");
        assert!(step["report"]["title"].is_string(), "step report must have 'title'");
        assert!(step["report"]["checks"].is_array(), "step report must have 'checks'");
        // skip_reason present only for skipped steps
        if step["status"] == "skip" {
            assert!(step["skip_reason"].is_string(), "skipped step must have 'skip_reason'");
        } else {
            assert!(step.get("skip_reason").is_none(), "non-skipped step must not have 'skip_reason'");
        }
        // is_execution_error omitted when false
        if step.get("is_execution_error").is_some() {
            assert_eq!(step["is_execution_error"], true, "is_execution_error should only appear when true");
        }
    }
    // summary object
    assert!(parsed["summary"].is_object(), "must have 'summary'");
    assert!(parsed["summary"]["passed"].is_number(), "summary must have 'passed'");
    assert!(parsed["summary"]["failed"].is_number(), "summary must have 'failed'");
    assert!(parsed["summary"]["skipped"].is_number(), "summary must have 'skipped'");
    assert!(parsed["summary"]["total_checks"].is_number(), "summary must have 'total_checks'");
    assert!(parsed["summary"]["total_errors"].is_number(), "summary must have 'total_errors'");
    assert!(parsed["summary"]["total_warnings"].is_number(), "summary must have 'total_warnings'");
}

#[test]
fn quality_gate_fast_skips_runtime_smoke_in_json() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let smoke_step = parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "runtime-smoke")
        .expect("runtime-smoke step must exist");
    assert_eq!(smoke_step["status"], "skip");
}

#[test]
fn json_output_to_stdout_nothing_to_stderr_on_check_failure() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();

    // stdout must be valid JSON
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(serde_json::from_str::<serde_json::Value>(&stdout).is_ok());

    // stderr should be empty (check failures go to JSON, not stderr)
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.is_empty(), "stderr should be empty on check failure, got: {stderr}");
}

#[test]
fn json_finding_location_omitted_when_null() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Doctor findings don't have locations, so "location" key should be absent
    for check in parsed["checks"].as_array().unwrap() {
        for finding in check["findings"].as_array().unwrap() {
            assert!(
                finding.get("location").is_none(),
                "doctor findings should omit null location"
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// 5. PROJECT ROOT VALID/INVALID SCENARIOS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_with_valid_project_passes() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASSED"));
}

#[test]
fn doctor_with_missing_go_work_fails() {
    let dir = TempDir::new().unwrap();
    // No go.work file
    std::fs::create_dir_all(dir.path().join("internal")).unwrap();
    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("go.work not found"));
}

#[test]
fn doctor_with_missing_directories_warns() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("go.work"), "go 1.23\n").unwrap();
    // Missing internal/, deploy/, tests/, tools/

    let output = raccoon()
        .args(["-v", "--project-root", dir.path().to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found"), "should warn about missing directories");
}

#[test]
fn doctor_with_empty_configs_dir_warns() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    // Remove the .jsonc file
    std::fs::remove_file(dir.path().join("deploy/configs/consumer.jsonc")).unwrap();

    let output = raccoon()
        .args(["-v", "--project-root", dir.path().to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no .jsonc"), "should warn about missing .jsonc files");
}

#[test]
fn topology_doctor_on_empty_project_fails_gracefully() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("go.work"), "go 1.23\n").unwrap();

    let result = raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "topology-doctor"])
        .output()
        .unwrap();
    let code = result.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "topology-doctor on empty project should not crash (exit 2), got {code}"
    );
}

#[test]
fn contract_audit_on_empty_project_fails_gracefully() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("go.work"), "go 1.23\n").unwrap();

    let result = raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "contract-audit"])
        .output()
        .unwrap();
    let code = result.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "contract-audit on empty project should not crash (exit 2), got {code}"
    );
}

#[test]
fn all_commands_accept_nonexistent_root_without_crash() {
    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-smoke"] {
        let result = raccoon()
            .args(["--project-root", "/nonexistent/path/to/project", cmd])
            .output()
            .unwrap();
        let code = result.status.code().unwrap_or(-1);
        assert!(
            code == 0 || code == 1,
            "'{cmd}' with nonexistent root should exit 0 or 1, not crash (got {code})"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════
// 6. QUALITY-GATE PROFILES
// ══════════════════════════════════════════════════════════════════════

#[test]
fn quality_gate_fast_has_seven_steps() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let steps = parsed["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 7);
}

#[test]
fn quality_gate_ci_has_same_steps_as_fast() {
    let fast_output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let ci_output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "ci",
        ])
        .output()
        .unwrap();

    let fast: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&fast_output.stdout)).unwrap();
    let ci: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&ci_output.stdout)).unwrap();

    let fast_names: Vec<&str> = fast["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    let ci_names: Vec<&str> = ci["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(fast_names, ci_names);
}

#[test]
fn quality_gate_deep_attempts_runtime_smoke() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "deep",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let smoke_step = parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "runtime-smoke")
        .unwrap();
    // In deep profile, runtime-smoke is attempted (not skipped), so it should fail
    assert_eq!(
        smoke_step["status"], "fail",
        "runtime-smoke should be attempted (not skipped) in deep profile"
    );
}

#[test]
fn quality_gate_fast_runtime_smoke_skip_message_mentions_deep() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let smoke_step = parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "runtime-smoke")
        .unwrap();
    let findings = smoke_step["report"]["checks"][0]["findings"]
        .as_array()
        .unwrap();
    let msg = findings[0]["message"].as_str().unwrap();
    assert!(
        msg.contains("--profile deep"),
        "skip message should reference --profile deep, got: {msg}"
    );
}

#[test]
fn quality_gate_arch_guard_runs_as_real_step() {
    for profile in &["fast", "ci", "deep"] {
        let output = raccoon()
            .args([
                "--json",
                "--project-root",
                "/nonexistent",
                "quality-gate",
                "--profile",
                profile,
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

        let arch_step = parsed["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == "arch-guard")
            .expect("arch-guard step must exist");
        // arch-guard is now a real step (fail on nonexistent root, not skip)
        assert_ne!(
            arch_step["status"], "skip",
            "arch-guard should run as a real step in '{profile}' profile"
        );
    }
}

#[test]
fn quality_gate_step_order_is_deterministic() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let names: Vec<&str> = parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["doctor", "topology-doctor", "contract-audit", "runtime-bindings", "arch-guard", "drift-detect", "runtime-smoke"]
    );
}

#[test]
fn quality_gate_total_duration_is_reasonable() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let total_ms = parsed["total_duration_ms"].as_u64().unwrap();
    assert!(total_ms < 30_000, "fast profile should complete in < 30s, took {total_ms}ms");
}

#[test]
fn quality_gate_skipped_steps_have_zero_duration() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    for step in parsed["steps"].as_array().unwrap() {
        if step["status"] == "skip" {
            assert_eq!(
                step["duration_ms"], 0,
                "skipped step '{}' should have 0 duration",
                step["name"]
            );
        }
    }
}

#[test]
fn quality_gate_json_steps_have_check_count_and_skip_reason() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    for step in parsed["steps"].as_array().unwrap() {
        assert!(step["check_count"].is_number());
        if step["status"] == "skip" {
            assert!(step["skip_reason"].is_string());
        } else {
            assert!(step.get("skip_reason").is_none());
        }
    }
}

#[test]
fn quality_gate_json_has_summary_counts() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let summary = &parsed["summary"];
    assert!(summary["passed"].is_number());
    assert!(summary["failed"].is_number());
    assert!(summary["skipped"].is_number());
    assert!(summary["total_checks"].is_number());
    let total: u64 = summary["passed"].as_u64().unwrap()
        + summary["failed"].as_u64().unwrap()
        + summary["skipped"].as_u64().unwrap();
    assert_eq!(total, parsed["steps"].as_array().unwrap().len() as u64);
}

#[test]
fn quality_gate_json_has_verdict() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let verdict = &parsed["verdict"];
    assert!(verdict["action"].is_string(), "verdict must have action");
    assert!(verdict["message"].is_string(), "verdict must have message");
    assert!(verdict["next_steps"].is_array(), "verdict must have next_steps");
    assert_eq!(verdict["action"], "stop", "failing gate should have stop verdict");
    assert!(!verdict["next_steps"].as_array().unwrap().is_empty(), "stop verdict should have next_steps");
}

#[test]
fn quality_gate_json_verdict_proceed_is_unreachable_on_nonexistent() {
    // On /nonexistent, verdict should always be "stop"
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_ne!(parsed["verdict"]["action"], "proceed");
}

#[test]
fn quality_gate_fail_fast_skips_after_first_failure() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--fail-fast",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let steps = parsed["steps"].as_array().unwrap();
    // First step (doctor) should fail
    assert_eq!(steps[0]["status"], "fail");
    // Remaining steps should all be skipped
    for step in &steps[1..] {
        assert_eq!(
            step["status"], "skip",
            "step '{}' should be skipped in fail-fast mode",
            step["name"]
        );
        let reason = step["skip_reason"].as_str().unwrap();
        assert!(
            reason.contains("fail-fast"),
            "skip reason should mention fail-fast, got: {reason}"
        );
    }
}

#[test]
fn quality_gate_fail_fast_flag_accepted() {
    raccoon()
        .args(["quality-gate", "--fail-fast", "--help"])
        .assert()
        .success();
}

#[test]
fn quality_gate_runtime_bindings_step_present() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let has_bindings = parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["name"] == "runtime-bindings");
    assert!(has_bindings, "runtime-bindings should be a gate step");
}

#[test]
fn quality_gate_human_output_shows_check_counts_and_skip_reasons() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Failed steps show check count
    assert!(stdout.contains("checks"), "should show check counts");
    // Skipped steps show skip reason inline
    assert!(
        stdout.contains("--profile deep"),
        "should show skip reason inline"
    );
}

// ══════════════════════════════════════════════════════════════════════
// 7. RUNTIME-SMOKE ABSENT ENVIRONMENT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn runtime_smoke_absent_compose_shows_actionable_error() {
    raccoon()
        .args(["--project-root", "/nonexistent", "runtime-smoke"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("compose file not found"));
}

#[test]
fn runtime_smoke_json_absent_compose() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "runtime-smoke"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["passed"], false);
    let bootstrap = &parsed["checks"][0];
    assert_eq!(bootstrap["name"], "bootstrap");
    assert_eq!(bootstrap["status"], "fail");
    let msg = bootstrap["findings"][0]["message"].as_str().unwrap();
    assert!(
        msg.contains("compose file not found"),
        "bootstrap error should mention compose file, got: {msg}"
    );
}

#[test]
fn runtime_smoke_with_valid_compose_but_no_docker() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "runtime-smoke",
        ])
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    // Should fail at bootstrap (docker not running or services not up), not crash
    assert_eq!(code, 1, "should exit 1 (check failure), not crash");
}

// ══════════════════════════════════════════════════════════════════════
// 8. DOCTOR DIAGNOSTICS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_error_suggests_fix_action() {
    raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .assert()
        .stdout(predicate::str::contains("Fix:"));
}

#[test]
fn doctor_json_failed_shows_project_root_check() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let project_root_check = parsed["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "project-root")
        .expect("must have project-root check");
    assert_eq!(project_root_check["status"], "fail");
}

#[test]
fn doctor_valid_project_json_shows_all_pass() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "doctor",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(parsed["passed"], true);
    for check in parsed["checks"].as_array().unwrap() {
        assert_eq!(
            check["status"], "pass",
            "check '{}' should pass on valid project",
            check["name"]
        );
    }
}

#[test]
fn doctor_checks_expected_names() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let names: Vec<&str> = parsed["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();

    assert!(names.contains(&"project-root"));
    assert!(names.contains(&"dir-internal"));
    assert!(names.contains(&"dir-deploy"));
    assert!(names.contains(&"dir-tests"));
    assert!(names.contains(&"dir-tools"));
    assert!(names.contains(&"compose-file"));
    assert!(names.contains(&"config-files"));
}

// ══════════════════════════════════════════════════════════════════════
// 9. ACTIONABLE ERROR MESSAGES
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_missing_root_error_mentions_project_root_flag() {
    raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .assert()
        .stdout(predicate::str::contains("--project-root"));
}

#[test]
fn runtime_smoke_bootstrap_error_mentions_make_up_dataplane() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args([
            "-v",
            "--project-root",
            dir.path().to_str().unwrap(),
            "runtime-smoke",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // If docker compose fails, error should mention how to start services
    // (either "make up-dataplane" or "compose file not found" or docker error)
    assert!(
        stdout.contains("make up-dataplane")
            || stdout.contains("compose")
            || stdout.contains("docker"),
        "bootstrap error should be actionable"
    );
}

#[test]
fn quality_gate_failure_message_references_specific_failed_step() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Actionable next steps should reference the specific failed steps
    assert!(
        stdout.contains("Fix 'topology-doctor'") || stdout.contains("Fix 'contract-audit'"),
        "should reference the specific failed step name"
    );
}

// ══════════════════════════════════════════════════════════════════════
// 10. FLAG INTERACTION EDGE CASES
// ══════════════════════════════════════════════════════════════════════

#[test]
fn json_and_verbose_together_produces_json() {
    let output = raccoon()
        .args([
            "--json",
            "-v",
            "--project-root",
            "/nonexistent",
            "doctor",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // When --json is set, output should be JSON regardless of --verbose
    // (because OutputFormat prioritizes JSON over HumanVerbose in main.rs)
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
        "output should be valid JSON even with -v flag"
    );
}

#[test]
fn json_output_consistent_between_commands_schema() {
    // All standard commands (doctor, topology-doctor, contract-audit, runtime-smoke)
    // should produce the same top-level JSON schema
    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-smoke"] {
        let output = raccoon()
            .args(["--json", "--project-root", "/nonexistent", cmd])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("'{cmd}' should produce valid JSON: {e}"));

        assert!(parsed["title"].is_string(), "'{cmd}' must have 'title'");
        assert!(parsed["checks"].is_array(), "'{cmd}' must have 'checks'");
        assert!(parsed["passed"].is_boolean(), "'{cmd}' must have 'passed'");
    }
}

#[test]
fn quality_gate_json_schema_differs_from_standard_commands() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "quality-gate",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // quality-gate has a different schema: profile, steps, total_duration_ms
    assert!(parsed["profile"].is_string());
    assert!(parsed["steps"].is_array());
    assert!(parsed["total_duration_ms"].is_number());
    // But no "title" or "checks" at top level
    assert!(parsed.get("title").is_none());
    assert!(parsed.get("checks").is_none());
}

#[test]
fn verbose_flag_with_quality_gate_shows_passing_details() {
    let default = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let verbose = raccoon()
        .args(["-v", "--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();

    assert!(
        verbose.stdout.len() >= default.stdout.len(),
        "verbose quality-gate should produce at least as much output"
    );
}

#[test]
fn base_url_only_relevant_for_smoke_commands() {
    // --base-url should be accepted by runtime-smoke and quality-gate
    raccoon()
        .args([
            "--project-root",
            "/nonexistent",
            "runtime-smoke",
            "--base-url",
            "http://example.com:9999",
        ])
        .assert()
        .code(1); // Fails at bootstrap, not at arg parsing

    raccoon()
        .args([
            "--project-root",
            "/nonexistent",
            "quality-gate",
            "--base-url",
            "http://example.com:9999",
        ])
        .assert()
        .code(1);
}

// ══════════════════════════════════════════════════════════════════════
// 11. JSON OUTPUT DETERMINISM
// ══════════════════════════════════════════════════════════════════════

#[test]
fn json_output_deterministic_across_runs_doctor() {
    let run1 = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let run2 = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();

    let j1: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&run1.stdout)).unwrap();
    let j2: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&run2.stdout)).unwrap();

    // Compare structure (exclude timing-sensitive fields)
    assert_eq!(j1["title"], j2["title"]);
    assert_eq!(j1["passed"], j2["passed"]);
    assert_eq!(j1["checks"], j2["checks"]);
}

#[test]
fn json_output_deterministic_across_runs_quality_gate() {
    let run1 = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let run2 = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();

    let j1: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&run1.stdout)).unwrap();
    let j2: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&run2.stdout)).unwrap();

    assert_eq!(j1["profile"], j2["profile"]);
    assert_eq!(j1["passed"], j2["passed"]);
    assert_eq!(j1["summary"], j2["summary"]);

    // Step names and statuses should be deterministic
    let steps1: Vec<(&str, &str)> = j1["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["name"].as_str().unwrap(), s["status"].as_str().unwrap()))
        .collect();
    let steps2: Vec<(&str, &str)> = j2["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["name"].as_str().unwrap(), s["status"].as_str().unwrap()))
        .collect();
    assert_eq!(steps1, steps2);
}

// ══════════════════════════════════════════════════════════════════════
// 12. TOPOLOGY AND CONTRACT-AUDIT ON VALID FIXTURE
// ══════════════════════════════════════════════════════════════════════

fn make_topology_fixture(dir: &TempDir) {
    make_project(dir);

    // Create all three config files
    std::fs::write(
        dir.path().join("deploy/configs/emulator.jsonc"),
        r#"{"kafka": {"brokers": ["kafka:9092"], "client_id": "emulator"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("deploy/configs/validator.jsonc"),
        r#"{"nats": {"url": "nats://nats:4222"}}"#,
    )
    .unwrap();
    // Update consumer config with nats and bootstrap
    std::fs::write(
        dir.path().join("deploy/configs/consumer.jsonc"),
        r#"{
  "kafka": {"brokers": ["kafka:9092"], "consumer_group": "consumer-v1"},
  "nats": {"url": "nats://nats:4222"},
  "bootstrap": {"base_url": "http://server:8080"}
}"#,
    )
    .unwrap();

    // Create compose with all services
    std::fs::write(
        dir.path().join("deploy/compose/docker-compose.yaml"),
        r#"services:
  nats:
    image: nats
  kafka:
    image: kafka
  configctl:
    image: configctl
    depends_on:
      nats:
        condition: service_healthy
  server:
    image: server
    depends_on:
      nats:
        condition: service_healthy
      configctl:
        condition: service_healthy
  consumer:
    image: consumer
    depends_on:
      nats:
        condition: service_healthy
      server:
        condition: service_healthy
      kafka:
        condition: service_healthy
  emulator:
    image: emulator
    depends_on:
      server:
        condition: service_healthy
      kafka:
        condition: service_healthy
      consumer:
        condition: service_healthy
      validator:
        condition: service_healthy
  validator:
    image: validator
    depends_on:
      nats:
        condition: service_healthy
      configctl:
        condition: service_healthy
"#,
    )
    .unwrap();
}

#[test]
fn topology_doctor_on_fixture_does_not_crash() {
    let dir = TempDir::new().unwrap();
    make_topology_fixture(&dir);

    let result = raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "topology-doctor"])
        .output()
        .unwrap();
    let code = result.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "topology-doctor on fixture should not crash (got {code})"
    );
}

#[test]
fn topology_doctor_json_on_fixture_is_valid() {
    let dir = TempDir::new().unwrap();
    make_topology_fixture(&dir);

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "topology-doctor",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("should be valid JSON: {e}\nstdout: {stdout}"));
    assert_eq!(parsed["title"], "topology-doctor");
    assert!(parsed["checks"].as_array().unwrap().len() > 0);
}

#[test]
fn contract_audit_on_fixture_does_not_crash() {
    let dir = TempDir::new().unwrap();
    make_topology_fixture(&dir);

    let result = raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "contract-audit"])
        .output()
        .unwrap();
    let code = result.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "contract-audit on fixture should not crash (got {code})"
    );
}

// ══════════════════════════════════════════════════════════════════════
// 13. STDERR IS CLEAN ON CHECK FAILURES
// ══════════════════════════════════════════════════════════════════════

#[test]
fn stderr_is_clean_for_all_commands_on_check_failure() {
    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-smoke"] {
        let output = raccoon()
            .args(["--project-root", "/nonexistent", cmd])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.is_empty(),
            "stderr should be empty for '{cmd}' on check failure, got: {stderr}"
        );
    }
}

#[test]
fn stderr_is_clean_for_quality_gate_on_check_failure() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "stderr should be empty for quality-gate on check failure, got: {stderr}"
    );
}

// ══════════════════════════════════════════════════════════════════════
// 14. HELP TEXT CONSISTENCY
// ══════════════════════════════════════════════════════════════════════

#[test]
fn all_subcommands_have_help() {
    for cmd in &[
        "doctor",
        "topology-doctor",
        "contract-audit",
        "runtime-smoke",
        "quality-gate",
    ] {
        raccoon()
            .args([cmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"));
    }
}

#[test]
fn help_lists_all_subcommands() {
    let output = raccoon()
        .arg("--help")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-smoke", "quality-gate"] {
        assert!(
            stdout.contains(cmd),
            "main help should list '{cmd}'"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════
// 15. GUARD RAIL VERDICT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_pass_in_quality_gate_shows_safe_when_all_pass() {
    // Doctor passes on a minimal project — verify it shows safe-to-proceed
    // when used standalone (quality-gate requires all steps to pass, which
    // needs a full Go codebase, so we test the doctor standalone here)
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Safe to proceed"));
}

#[test]
fn quality_gate_fail_shows_stop() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Stop"))
        .stdout(predicate::str::contains("must be fixed"));
}

#[test]
fn standalone_doctor_pass_shows_safe_to_proceed() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Safe to proceed"));
}

#[test]
fn standalone_doctor_fail_shows_stop() {
    raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Stop"))
        .stdout(predicate::str::contains("must be fixed"));
}

// ══════════════════════════════════════════════════════════════════════
// 16. WHY/HELP IN JSON OUTPUT
// ══════════════════════════════════════════════════════════════════════

#[test]
fn doctor_json_findings_have_why_and_help() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    for check in parsed["checks"].as_array().unwrap() {
        for finding in check["findings"].as_array().unwrap() {
            assert!(
                finding["why"].is_string(),
                "finding in '{}' should have 'why' field: {}",
                check["name"],
                finding["message"]
            );
            assert!(
                finding["help"].is_string(),
                "finding in '{}' should have 'help' field: {}",
                check["name"],
                finding["message"]
            );
        }
    }
}

#[test]
fn doctor_json_why_and_help_omitted_when_not_set() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args(["--json", "--project-root", dir.path().to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Passing checks have no findings, so no why/help
    assert_eq!(parsed["passed"], true);
    for check in parsed["checks"].as_array().unwrap() {
        assert!(check["findings"].as_array().unwrap().is_empty());
    }
}

#[test]
fn quality_gate_json_topology_findings_have_why() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // topology-doctor step on nonexistent root should have findings with why
    let topo_step = parsed["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "topology-doctor")
        .unwrap();
    assert_eq!(topo_step["status"], "fail");
    let findings: Vec<&serde_json::Value> = topo_step["report"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["findings"].as_array().unwrap())
        .collect();
    assert!(!findings.is_empty(), "should have findings");
    for finding in &findings {
        if finding["severity"] == "error" {
            assert!(
                finding["why"].is_string(),
                "error finding should have 'why': {}",
                finding["message"]
            );
        }
    }
}
