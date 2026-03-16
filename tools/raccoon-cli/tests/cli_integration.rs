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
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "topology-doctor",
        ])
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
    for cmd in &[
        "doctor",
        "topology-doctor",
        "contract-audit",
        "runtime-bindings",
    ] {
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

    for cmd in &[
        "doctor",
        "topology-doctor",
        "contract-audit",
        "runtime-bindings",
    ] {
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
    for cmd in &[
        "doctor",
        "topology-doctor",
        "contract-audit",
        "runtime-bindings",
    ] {
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
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "runtime-bindings",
        ])
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
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "runtime-bindings",
        ])
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
    assert!(
        check_names.contains(&"subject-pattern"),
        "missing subject-pattern check"
    );
    assert!(
        check_names.contains(&"routing-constants"),
        "missing routing-constants check"
    );
    assert!(
        check_names.contains(&"lifecycle-events"),
        "missing lifecycle-events check"
    );
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
        .args(["results-inspect", "--base-url", "http://127.0.0.1:19999"])
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
            "--base-url",
            "http://127.0.0.1:19999",
            "--scope-kind",
            "global",
            "--scope-key",
            "default",
            "--binding",
            "orders",
            "--topic",
            "orders.v1",
            "--limit",
            "50",
            "--failed-only",
            "--latest",
            "5",
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
        .args([
            "--project-root",
            "/nonexistent",
            "scenario-smoke",
            "happy-path",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"))
        .stdout(predicate::str::contains("bootstrap"));
}

#[test]
fn scenario_smoke_readiness_probe_fails_without_compose() {
    raccoon()
        .args([
            "--project-root",
            "/nonexistent",
            "scenario-smoke",
            "readiness-probe",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAILED"));
}

#[test]
fn scenario_smoke_json_output_is_valid() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "scenario-smoke",
            "happy-path",
        ])
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
    raccoon().args(["scenario-smoke"]).assert().failure();
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
    raccoon().args(["lsp-enrich"]).assert().failure();
}

// ── Contract Usage Map ──────────────────────────────────────────────

#[test]
fn contract_usage_map_json_output_is_valid() {
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            "/nonexistent",
            "contract-usage-map",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["contracts"].is_array());
    assert!(parsed["families"].is_array());
    assert!(parsed["statistics"]["total_contracts"].is_number());
    assert!(parsed["scope_note"].is_string());
}

#[test]
fn contract_usage_map_human_output_has_header() {
    raccoon()
        .args(["--project-root", "/nonexistent", "contract-usage-map"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Contract Usage Map"))
        .stdout(predicate::str::contains("Families"));
}

#[test]
fn contract_usage_map_verbose_shows_details() {
    raccoon()
        .args(["-v", "--project-root", "/nonexistent", "contract-usage-map"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Contract Usage Map"));
}

// ── Snapshot ──────────────────────────────────────────────────────────

#[test]
fn snapshot_help_shows_examples() {
    raccoon()
        .args(["snapshot", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("snapshot"));
}

#[test]
fn snapshot_human_output_on_empty_project() {
    let dir = TempDir::new().unwrap();

    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "snapshot"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Golden Snapshot"))
        .stdout(predicate::str::contains("Stats:"))
        .stdout(predicate::str::contains("Packages (0)"));
}

#[test]
fn snapshot_json_output_is_valid() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype ConfigSet struct { ID string }\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["version"], "1");
    assert!(parsed["packages"].is_array());
    assert!(parsed["types"].is_array());
    assert!(parsed["functions"].is_array());
    assert!(parsed["stats"]["total_files"].is_number());
    assert!(parsed["metadata"]["generated_at"].is_string());
    assert!(parsed["metadata"]["raccoon_version"].is_string());
}

#[test]
fn snapshot_json_has_provenance_tags() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype ConfigGateway interface { Get() string }\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Packages should have provenance: ast
    let pkg_prov = &parsed["packages"][0]["provenance"];
    assert_eq!(pkg_prov, "ast");

    // Metadata should have provenance: runtime
    assert_eq!(parsed["metadata"]["provenance"], "runtime");
}

#[test]
fn snapshot_output_flag_writes_file() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    let out_path = dir.path().join("snapshot.json");

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
            "--output",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out_path.exists(), "snapshot file should be created");
    let content = std::fs::read_to_string(&out_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["version"], "1");
}

#[test]
fn snapshot_verbose_shows_more() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Model struct { ID string }\n\nfunc NewModel() Model { return Model{} }\n",
    )
    .unwrap();

    let terse = raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "snapshot"])
        .output()
        .unwrap();

    let verbose = raccoon()
        .args([
            "-v",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();

    assert!(
        verbose.stdout.len() > terse.stdout.len(),
        "verbose output should be longer"
    );
}

#[test]
fn snapshot_is_deterministic_across_runs() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string }\n",
    )
    .unwrap();

    let out1 = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();
    let out2 = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();

    let j1: serde_json::Value = serde_json::from_slice(&out1.stdout).unwrap();
    let j2: serde_json::Value = serde_json::from_slice(&out2.stdout).unwrap();

    // Structural sections must be identical
    assert_eq!(j1["packages"], j2["packages"]);
    assert_eq!(j1["types"], j2["types"]);
    assert_eq!(j1["stats"], j2["stats"]);
}

// ── Snapshot Diff ─────────────────────────────────────────────────────

#[test]
fn snapshot_diff_help_shows_examples() {
    raccoon()
        .args(["snapshot-diff", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("structured diff"));
}

#[test]
fn snapshot_diff_identical_files_no_changes() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    // Generate a snapshot
    let snap_path = dir.path().join("snap.json");
    raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
            "-o",
            snap_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Diff the snapshot against itself
    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot-diff",
            snap_path.to_str().unwrap(),
            snap_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No structural changes detected."));
}

#[test]
fn snapshot_diff_json_output_is_valid() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let snap_path = dir.path().join("snap.json");
    raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
            "-o",
            snap_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot-diff",
            snap_path.to_str().unwrap(),
            snap_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["has_changes"], false);
    assert!(parsed["sections"].is_object());
    assert!(parsed["stats_delta"].is_object());
}

#[test]
fn snapshot_diff_missing_file_fails() {
    raccoon()
        .args([
            "snapshot-diff",
            "/nonexistent/before.json",
            "/nonexistent/after.json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to load"));
}

#[test]
fn snapshot_diff_with_after_live() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let snap_path = dir.path().join("snap.json");
    raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
            "-o",
            snap_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot-diff",
            snap_path.to_str().unwrap(),
            "--after-live",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No structural changes detected."));
}

// ── Briefing ──────────────────────────────────────────────────────────

#[test]
fn briefing_help_shows_examples() {
    raccoon()
        .args(["briefing", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("[fact]"));
}

#[test]
fn briefing_no_targets_shows_usage() {
    // With a nonexistent project root and no git, targets will be empty
    raccoon()
        .args(["--project-root", "/nonexistent", "briefing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No targets provided"));
}

#[test]
fn briefing_json_no_targets_is_valid() {
    let output = raccoon()
        .args(["--json", "--project-root", "/nonexistent", "briefing"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["targets"].is_array());
    assert!(parsed["facts"].is_array());
    assert!(parsed["inferences"].is_array());
    assert!(parsed["recommendations"].is_array());
    assert!(parsed["scope_note"].is_string());
}

#[test]
fn briefing_with_file_target() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    // Create a minimal Go file
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Model struct { ID string }\n",
    )
    .unwrap();

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "internal/domain/model.go",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Briefing"))
        .stdout(predicate::str::contains("Targets:"));
}

#[test]
fn briefing_json_with_file_target() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Model struct { ID string }\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "internal/domain/model.go",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["targets"][0], "internal/domain/model.go");
    assert!(parsed["facts"].is_array());
    assert!(parsed["scope_note"].is_string());
}

#[test]
fn briefing_with_symbol_target() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype ConfigSet struct { ID string }\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "ConfigSet",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["targets"][0], "ConfigSet");
    // Should have found the symbol definition
    let has_def = parsed["facts"].as_array().unwrap().iter().any(|f| {
        f["category"] == "symbol-definition"
            && f["message"].as_str().unwrap_or("").contains("ConfigSet")
    });
    assert!(has_def, "should find ConfigSet in facts");
}

#[test]
fn briefing_multiple_targets_json() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Foo struct { X int }\ntype Bar struct { Y int }\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "internal/domain/model.go",
            "Foo",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["targets"].as_array().unwrap().len(), 2);
}

#[test]
fn briefing_ambiguous_symbol_works() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::create_dir_all(dir.path().join("internal/adapters")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Handler struct { ID string }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/adapters/handler.go"),
        "package adapters\n\ntype Handler struct { Name string }\n",
    )
    .unwrap();

    // Should not panic, should include both definitions
    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "Handler",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Briefing"));
}

#[test]
fn briefing_verbose_shows_more() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Model struct { ID string }\n",
    )
    .unwrap();

    let terse = raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "internal/domain/model.go",
        ])
        .output()
        .unwrap();

    let verbose = raccoon()
        .args([
            "-v",
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "internal/domain/model.go",
        ])
        .output()
        .unwrap();

    // Verbose output should be >= terse
    assert!(verbose.stdout.len() >= terse.stdout.len());
}

#[test]
fn briefing_fallback_without_lsp() {
    // briefing without --lsp should work fine
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/example/svc\n\ngo 1.23\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string }\n",
    )
    .unwrap();

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "briefing",
            "Config",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Briefing"))
        .stdout(predicate::str::contains("No LSP enrichment"));
}

// ── Baseline Drift ────────────────────────────────────────────────────

#[test]
fn baseline_drift_help_shows_examples() {
    raccoon()
        .args(["baseline-drift", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains("baseline.json"));
}

#[test]
fn baseline_drift_missing_file_exits_2() {
    raccoon()
        .args(["baseline-drift", "/nonexistent/baseline.json"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn baseline_drift_invalid_json_exits_2() {
    let dir = TempDir::new().unwrap();
    let bad_file = dir.path().join("bad.json");
    std::fs::write(&bad_file, "not json").unwrap();

    raccoon()
        .args(["baseline-drift", bad_file.to_str().unwrap()])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn baseline_drift_clean_on_identical_project() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string }\n",
    )
    .unwrap();

    // Generate baseline snapshot
    let snap_output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();
    assert!(snap_output.status.success());

    let baseline_path = dir.path().join("baseline.json");
    std::fs::write(&baseline_path, &snap_output.stdout).unwrap();

    // Run baseline-drift against the same project
    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "baseline-drift",
            baseline_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("CLEAN"));
}

#[test]
fn baseline_drift_json_output_is_valid() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string }\n",
    )
    .unwrap();

    // Generate baseline
    let snap_output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();
    let baseline_path = dir.path().join("baseline.json");
    std::fs::write(&baseline_path, &snap_output.stdout).unwrap();

    // Run baseline-drift with --json
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "baseline-drift",
            baseline_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("should be valid JSON");
    assert_eq!(parsed["verdict"], "clean");
    assert!(parsed["findings"].is_array());
    assert!(parsed["summary"].is_object());
    assert!(parsed["baseline"].is_object());
    assert!(parsed["current"].is_object());
}

#[test]
fn baseline_drift_detects_drift_after_change() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string }\n\nfunc NewConfig(name string) Config { return Config{Name: name} }\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("internal/application/ports")).unwrap();
    std::fs::write(
        dir.path().join("internal/application/ports/port.go"),
        "package ports\n\nimport \"context\"\n\ntype ConfigGateway interface {\n\tGet(ctx context.Context, id string) (string, error)\n}\n",
    )
    .unwrap();

    // Generate baseline
    let snap_output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();
    let baseline_path = dir.path().join("baseline.json");
    std::fs::write(&baseline_path, &snap_output.stdout).unwrap();

    // Modify the project: change function signature, modify interface
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string\n\tLabel string }\n\nfunc NewConfig(name string, label string) Config { return Config{Name: name, Label: label} }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("internal/application/ports/port.go"),
        "package ports\n\nimport \"context\"\n\ntype ConfigGateway interface {\n\tGet(ctx context.Context, id string) (string, error)\n\tDelete(ctx context.Context, id string) error\n}\n",
    )
    .unwrap();

    // Run baseline-drift — should detect drift
    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "baseline-drift",
            baseline_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Exit code 1 for drift (or 0 for mild)
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("should be valid JSON");
    let total = parsed["summary"]["total_findings"].as_u64().unwrap();
    assert!(total > 0, "Should have findings after changes");
    // Should detect api-signature-drift for NewConfig
    let findings = parsed["findings"].as_array().unwrap();
    let has_sig_drift = findings.iter().any(|f| f["class"] == "api-signature-drift");
    assert!(has_sig_drift, "Should detect NewConfig signature change");
}

#[test]
fn baseline_drift_verbose_shows_evidence() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    std::fs::create_dir_all(dir.path().join("internal/domain")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\ntype Config struct { Name string }\n",
    )
    .unwrap();

    // Generate baseline, then modify
    let snap_output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "snapshot",
        ])
        .output()
        .unwrap();
    let baseline_path = dir.path().join("baseline.json");
    std::fs::write(&baseline_path, &snap_output.stdout).unwrap();

    // Remove the type entirely
    std::fs::write(
        dir.path().join("internal/domain/model.go"),
        "package domain\n\n// empty\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "-v",
            "--project-root",
            dir.path().to_str().unwrap(),
            "baseline-drift",
            baseline_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("evidence:"),
        "Verbose output should show evidence"
    );
}

// ── Recommend ────────────────────────────────────────────────────────

#[test]
fn recommend_help_shows_examples() {
    raccoon()
        .args(["recommend", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"));
}

#[test]
fn recommend_no_files_succeeds() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);
    raccoon()
        .args(["--project-root", dir.path().to_str().unwrap(), "recommend"])
        .assert()
        .success();
}

#[test]
fn recommend_json_output_is_valid() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "recommend",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["input"].is_object());
    assert!(parsed["facts"].is_array());
    assert!(parsed["inferences"].is_array());
    assert!(parsed["recommendations"].is_array());
    assert!(parsed["smoke_scenarios"].is_array());
    assert!(parsed["gate_profile"].is_object());
    assert!(parsed["priority_areas"].is_array());
    assert!(parsed["risks"].is_array());
    assert!(parsed["commands"].is_object());
    assert!(parsed["scope_note"].is_string());
}

#[test]
fn recommend_with_file_targets() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    // Create a Go file in domain layer
    std::fs::create_dir_all(dir.path().join("internal/domain/configctl")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/configctl/config.go"),
        "package configctl\n\ntype ConfigSet struct {\n\tSetID string\n}\n",
    )
    .unwrap();

    raccoon()
        .args([
            "--project-root",
            dir.path().to_str().unwrap(),
            "recommend",
            "internal/domain/configctl/config.go",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recommend: Smoke/TDD Priorities"));
}

#[test]
fn recommend_json_with_file_targets_has_facts() {
    let dir = TempDir::new().unwrap();
    make_project(&dir);

    std::fs::create_dir_all(dir.path().join("internal/domain/configctl")).unwrap();
    std::fs::write(
        dir.path().join("internal/domain/configctl/config.go"),
        "package configctl\n\ntype ConfigSet struct {\n\tSetID string\n}\n",
    )
    .unwrap();

    let output = raccoon()
        .args([
            "--json",
            "--project-root",
            dir.path().to_str().unwrap(),
            "recommend",
            "internal/domain/configctl/config.go",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        parsed["input"]["changed_files"].as_array().unwrap().len() == 1,
        "should have 1 changed file"
    );
    assert!(
        !parsed["facts"].as_array().unwrap().is_empty(),
        "should have facts for a real Go file"
    );
}
