use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn raccoon() -> Command {
    Command::cargo_bin("raccoon-cli").unwrap()
}

/// Create a minimal project structure for doctor to pass on.
fn make_project(dir: &TempDir) {
    std::fs::write(dir.path().join("go.work"), "go 1.23\n").unwrap();
    std::fs::create_dir_all(dir.path().join("internal")).unwrap();
    std::fs::create_dir_all(dir.path().join("deploy/configs")).unwrap();
    std::fs::create_dir_all(dir.path().join("deploy/compose")).unwrap();
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    std::fs::create_dir_all(dir.path().join("tools")).unwrap();
}

// ── Version / Help ────────────────────────────────────────────────────

#[test]
fn shows_version() {
    raccoon()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("raccoon-cli"));
}

#[test]
fn shows_help() {
    raccoon()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Engineering quality toolkit"))
        .stdout(predicate::str::contains("Quick start"));
}

#[test]
fn subcommand_help_shows_examples() {
    raccoon()
        .args(["quality-gate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"));
}

#[test]
fn doctor_help_shows_examples() {
    raccoon()
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"));
}

// ── Exit Codes ────────────────────────────────────────────────────────

#[test]
fn unknown_command_exits_with_error() {
    raccoon()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn invalid_profile_exits_with_error() {
    raccoon()
        .args(["quality-gate", "--profile", "turbo"])
        .assert()
        .failure();
}

#[test]
fn doctor_nonexistent_root_exits_1() {
    raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"));
}

#[test]
fn quality_gate_nonexistent_root_exits_1() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .assert()
        .code(1);
}

// ── JSON Output ───────────────────────────────────────────────────────

#[test]
fn doctor_json_output_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["title"], "doctor");
    assert_eq!(parsed["passed"], false);
    assert!(parsed["checks"].is_array());
}

#[test]
fn quality_gate_json_output_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["profile"], "fast");
    assert_eq!(parsed["passed"], false);
    assert!(parsed["steps"].is_array());
    assert!(parsed["total_duration_ms"].is_number());
}

#[test]
fn topology_doctor_json_output_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "topology-doctor"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["title"], "topology-doctor");
    assert!(parsed["checks"].is_array());
}

#[test]
fn contract_audit_json_output_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "contract-audit"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["title"], "contract-audit");
    assert!(parsed["checks"].is_array());
}

// ── Global Flags Consistency ──────────────────────────────────────────

#[test]
fn json_flag_works_with_all_commands() {
    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-bindings"] {
        let output = raccoon()
            .args(["--json", "--project-root", "/nonexistent", cmd])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
            "--json should produce valid JSON for '{cmd}'"
        );
    }
}

#[test]
fn project_root_flag_works_with_doctor() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    // Add minimal config files for doctor to fully pass
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

    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .success();
}

#[test]
fn project_root_flag_accepted_by_all_commands() {
    // Verify --project-root is accepted (even if checks fail on minimal structure)
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-bindings"] {
        let result = raccoon()
            .args(["--project-root", dir.path().to_str().unwrap(), cmd])
            .output()
            .unwrap();

        // Should not exit with code 2 (runtime error), only 0 or 1
        let code = result.status.code().unwrap_or(-1);
        assert!(
            code == 0 || code == 1,
            "{cmd} should exit 0 or 1, got {code}"
        );
    }
}

#[test]
fn verbose_flag_accepted_with_all_commands() {
    for cmd in &["doctor", "topology-doctor", "contract-audit", "runtime-bindings"] {
        raccoon()
            .args(["-v", "--project-root", "/nonexistent", cmd])
            .output()
            .expect(&format!("-v should be accepted for '{cmd}'"));
    }
}

// ── Doctor on Valid Project ───────────────────────────────────────────

#[test]
fn doctor_passes_on_valid_project() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
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

    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PASSED"));
}

// ── Human Output Consistency ──────────────────────────────────────────

#[test]
fn human_output_shows_verdict_line() {
    let output = raccoon()
        .args(["--project-root", "/nonexistent", "doctor"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Result: FAILED") || stdout.contains("Result: PASSED"),
        "human output should contain a Result verdict line"
    );
}

#[test]
fn quality_gate_human_output_shows_actionable_steps() {
    raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Actionable next steps"))
        .stdout(predicate::str::contains("raccoon-cli"));
}

// ── Verbose Output ────────────────────────────────────────────────────

#[test]
// ── Runtime Bindings ──────────────────────────────────────────────────

#[test]
fn runtime_bindings_json_output_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "runtime-bindings"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["title"], "runtime-bindings");
    assert!(parsed["checks"].is_array());
}

#[test]
fn runtime_bindings_fails_without_internal_dir() {
    raccoon()
        .args(["--project-root", "/nonexistent", "runtime-bindings"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"));
}

#[test]
fn runtime_bindings_help_shows_examples() {
    raccoon()
        .args(["runtime-bindings", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"));
}

#[test]
fn runtime_bindings_on_minimal_project() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "runtime-bindings"])
        .output()
        .unwrap();

    let code = output.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 1, "should exit 0 or 1, got {code}");
}

#[test]
fn runtime_bindings_json_has_all_check_names() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "runtime-bindings",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let check_names: Vec<&str> = parsed["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();

    // Core checks should always be present
    assert!(check_names.contains(&"subject-pattern"), "missing subject-pattern check");
    assert!(check_names.contains(&"routing-constants"), "missing routing-constants check");
    assert!(check_names.contains(&"lifecycle-events"), "missing lifecycle-events check");
}

// ── Results Inspect ───────────────────────────────────────────────────

#[test]
fn results_inspect_help_shows_examples() {
    raccoon()
        .args(["results-inspect", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("--failed-only"))
        .stdout(predicate::str::contains("--latest"));
}

#[test]
fn results_inspect_unreachable_exits_2() {
    // When service is unreachable, should exit 2 (execution error)
    raccoon()
        .args([
            "results-inspect",
            "--base-url",
            "http://127.0.0.1:19999",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot reach quality-service"));
}

#[test]
fn results_inspect_unreachable_json_still_exits_2() {
    raccoon()
        .args([
            "--json",
            "results-inspect",
            "--base-url",
            "http://127.0.0.1:19999",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot reach quality-service"));
}

#[test]
fn results_inspect_accepts_all_filter_flags() {
    // Verify all flags are accepted by the parser even if service is unreachable
    let result = raccoon()
        .args([
            "results-inspect",
            "--base-url", "http://127.0.0.1:19999",
            "--scope-kind", "global",
            "--scope-key", "default",
            "--binding", "orders",
            "--topic", "orders.v1",
            "--limit", "50",
            "--failed-only",
            "--latest", "5",
        ])
        .output()
        .unwrap();

    let code = result.status.code().unwrap_or(-1);
    // Should exit 2 (connection error), not panic or crash
    assert_eq!(code, 2, "should exit 2 for connection error");
}

// ── Scenario Smoke ────────────────────────────────────────────────────

#[test]
fn scenario_smoke_help_shows_examples() {
    raccoon()
        .args(["scenario-smoke", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("happy-path"))
        .stdout(predicate::str::contains("config-lifecycle"));
}

#[test]
fn scenario_smoke_list_shows_all_scenarios() {
    raccoon()
        .args(["scenario-smoke", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("happy-path"))
        .stdout(predicate::str::contains("config-lifecycle"))
        .stdout(predicate::str::contains("invalid-payload"))
        .stdout(predicate::str::contains("missing-binding"))
        .stdout(predicate::str::contains("readiness-probe"));
}

#[test]
fn scenario_smoke_list_json_is_valid() {
    let output = raccoon()
        .args(["--json", "scenario-smoke", "--list"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["scenarios"].is_array());
    let scenarios = parsed["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 5);
    for s in scenarios {
        assert!(s["name"].is_string());
        assert!(s["description"].is_string());
        assert!(s["preconditions"].is_array());
    }
}

#[test]
fn scenario_smoke_unknown_scenario_exits_2() {
    raccoon()
        .args(["scenario-smoke", "nonexistent-scenario"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown scenario"));
}

#[test]
fn scenario_smoke_happy_path_fails_without_compose() {
    raccoon()
        .args(["--project-root", "/nonexistent", "scenario-smoke", "happy-path"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"))
        .stdout(predicate::str::contains("bootstrap"));
}

#[test]
fn scenario_smoke_readiness_probe_fails_without_compose() {
    raccoon()
        .args(["--project-root", "/nonexistent", "scenario-smoke", "readiness-probe"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"));
}

#[test]
fn scenario_smoke_json_output_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "scenario-smoke", "happy-path"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["title"].as_str().unwrap().contains("happy-path"));
    assert_eq!(parsed["passed"], false);
    assert!(parsed["checks"].is_array());
}

#[test]
fn scenario_smoke_requires_scenario_or_list() {
    raccoon()
        .args(["scenario-smoke"])
        .assert()
        .failure();
}

// ── Verbose Output ────────────────────────────────────────────────────

#[test]
fn verbose_shows_more_detail_than_default() {
    let default = raccoon()
        .args(["--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();
    let verbose = raccoon()
        .args(["-v", "--project-root", "/nonexistent", "quality-gate"])
        .output()
        .unwrap();

    let default_len = default.stdout.len();
    let verbose_len = verbose.stdout.len();
    assert!(
        verbose_len >= default_len,
        "verbose output ({verbose_len} bytes) should be >= default ({default_len} bytes)"
    );
}

// ── LSP Enrich ───────────────────────────────────────────────────────

#[test]
fn lsp_enrich_help_shows_examples() {
    raccoon()
        .args(["lsp-enrich", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gopls"))
        .stdout(predicate::str::contains("--no-lsp"));
}

#[test]
fn lsp_enrich_no_lsp_on_fixture() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    // Write a Go file for the index to find.
    let pkg = dir.path().join("internal/domain");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("config.go"),
        "package domain\n\ntype ConfigSet struct {\n\tName string\n}\n",
    )
    .unwrap();

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "lsp-enrich",
            "--no-lsp",
            "ConfigSet",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Symbol: ConfigSet"))
        .stdout(predicate::str::contains("AST definitions (1)"))
        .stdout(predicate::str::contains("LSP: unavailable"));
}

#[test]
fn lsp_enrich_no_lsp_json_output() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    let pkg = dir.path().join("internal/domain");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("config.go"),
        "package domain\n\ntype Foo struct{}\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "lsp-enrich",
            "--no-lsp",
            "Foo",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("output should be valid JSON");
    assert_eq!(json["symbol"], "Foo");
    assert!(json["ast_definitions"].is_array());
    assert!(json["lsp_status"].is_object() || json["lsp_status"].is_string());
}

#[test]
fn lsp_enrich_unknown_symbol_returns_empty() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "lsp-enrich",
            "--no-lsp",
            "DoesNotExist",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("AST definitions: none found"));
}

#[test]
fn lsp_enrich_requires_symbol_arg() {
    raccoon()
        .args(["lsp-enrich"])
        .assert()
        .failure();
}
