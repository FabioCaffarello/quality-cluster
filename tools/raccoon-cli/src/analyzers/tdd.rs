//! TDD guidance — impact-driven test discipline for changed files.
//!
//! Uses the codeintel AST index and impact analysis to recommend what to
//! validate before and after a change. Goes beyond file-pattern matching:
//! traces exported symbols, dependents, contract surface, and coverage gaps
//! to produce actionable, evidence-based guidance.
//!
//! ## What it answers
//!
//! - Which symbols/packages were affected?
//! - Which existing tests cover the affected code?
//! - Which coverage gaps exist (no tests, no scenario)?
//! - Which smoke scenarios and quality-gate profile should be run?
//! - What to do BEFORE and AFTER the change?

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::codeintel;
use crate::analyzers::impact_map;

// ── Public API ─────────────────────────────────────────────────────────────

/// Analyze changed files and produce TDD guidance driven by structural impact.
pub fn analyze(project_root: &Path, changed_files: &[String]) -> TddReport {
    if changed_files.is_empty() {
        return TddReport::empty();
    }

    let index = codeintel::build_index(project_root);
    let impact_report = impact_map::analyze(project_root, changed_files);

    let file_impacts = build_file_impacts(&impact_report);
    let affected_areas = build_affected_areas(changed_files);
    let existing_tests = find_nearby_tests(&index, changed_files);
    let coverage_gaps = find_coverage_gaps(&affected_areas, &existing_tests, project_root);
    let (recommended_scenarios, needs_infra) = recommend_scenarios(&affected_areas);
    let recommended_profile = recommend_gate_profile(&affected_areas, needs_infra);
    let before_commands = build_before_commands(&affected_areas, &recommended_scenarios);
    let after_commands = build_after_commands(&affected_areas, &recommended_scenarios);

    TddReport {
        changed_files: changed_files.to_vec(),
        file_impacts,
        affected_areas,
        existing_tests,
        coverage_gaps,
        recommended_profile,
        recommended_scenarios,
        before_commands,
        after_commands,
        needs_infra,
        scope_note: "Impact is computed from static AST analysis and file-pattern matching. \
            No call graph or runtime tracing is available.".into(),
    }
}

// ── Report types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TddReport {
    pub changed_files: Vec<String>,
    pub file_impacts: Vec<FileImpact>,
    pub affected_areas: Vec<AffectedArea>,
    pub existing_tests: Vec<NearbyTest>,
    pub coverage_gaps: Vec<CoverageGap>,
    pub recommended_profile: String,
    pub recommended_scenarios: Vec<RecommendedScenario>,
    pub before_commands: Vec<String>,
    pub after_commands: Vec<String>,
    pub needs_infra: bool,
    pub scope_note: String,
}

impl TddReport {
    /// Create an empty report (no changed files detected).
    pub fn empty_report() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        TddReport {
            changed_files: vec![],
            file_impacts: vec![],
            affected_areas: vec![],
            existing_tests: vec![],
            coverage_gaps: vec![],
            recommended_profile: "fast".into(),
            recommended_scenarios: vec![],
            before_commands: vec![],
            after_commands: vec!["make verify".into()],
            needs_infra: false,
            scope_note: "No changed files detected.".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FileImpact {
    pub file: String,
    pub package: Option<String>,
    pub exported_symbols: Vec<String>,
    pub direct_dependents: Vec<String>,
    pub contract_items: Vec<String>,
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedArea {
    pub name: String,
    pub description: String,
    pub files_touched: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NearbyTest {
    pub test_file: String,
    pub package: String,
    pub for_area: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageGap {
    pub area: String,
    pub description: String,
    pub has_go_tests: bool,
    pub has_scenario: bool,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendedScenario {
    pub name: String,
    pub description: String,
    pub why: String,
}

// ── Sensitive areas (canonical definitions) ────────────────────────────────

struct AreaDef {
    name: &'static str,
    description: &'static str,
    patterns: &'static [&'static str],
    dimensions: &'static [&'static str],
    scenarios: &'static [&'static str],
}

const AREA_DEFS: &[AreaDef] = &[
    AreaDef {
        name: "config-files",
        description: "deploy/configs — service configuration",
        patterns: &["deploy/configs/"],
        dimensions: &["project-structure", "topology", "drift"],
        scenarios: &[],
    },
    AreaDef {
        name: "compose",
        description: "docker-compose — service orchestration",
        patterns: &["deploy/compose/"],
        dimensions: &["project-structure", "topology", "drift"],
        scenarios: &["readiness-probe"],
    },
    AreaDef {
        name: "nats-adapters",
        description: "NATS/JetStream adapter layer — transport wiring",
        patterns: &["internal/adapters/nats/"],
        dimensions: &["contracts", "runtime-bindings", "architecture"],
        scenarios: &["happy-path"],
    },
    AreaDef {
        name: "kafka-adapters",
        description: "Kafka adapter layer — data plane transport",
        patterns: &["internal/adapters/kafka/"],
        dimensions: &["topology", "runtime-bindings", "architecture"],
        scenarios: &["happy-path"],
    },
    AreaDef {
        name: "domain",
        description: "domain layer — business rules (must be pure)",
        patterns: &["internal/domain/"],
        dimensions: &["architecture", "contracts"],
        scenarios: &[],
    },
    AreaDef {
        name: "application",
        description: "application layer — use cases and ports",
        patterns: &["internal/application/"],
        dimensions: &["architecture", "contracts"],
        scenarios: &[],
    },
    AreaDef {
        name: "http-handlers",
        description: "HTTP interface layer — API endpoints",
        patterns: &["internal/interfaces/http/"],
        dimensions: &["architecture"],
        scenarios: &["readiness-probe"],
    },
    AreaDef {
        name: "actors",
        description: "actor supervision trees — runtime wiring",
        patterns: &["internal/actors/"],
        dimensions: &["architecture", "runtime-bindings"],
        scenarios: &["readiness-probe"],
    },
    AreaDef {
        name: "validator-logic",
        description: "validator — validation rules and results",
        patterns: &["internal/actors/scopes/validator/", "internal/application/validatorresults/"],
        dimensions: &["contracts", "runtime-bindings"],
        scenarios: &["happy-path", "invalid-payload"],
    },
    AreaDef {
        name: "consumer-pipeline",
        description: "consumer — kafka-to-jetstream bridging",
        patterns: &["internal/actors/scopes/consumer/", "internal/application/dataplane/"],
        dimensions: &["topology", "runtime-bindings"],
        scenarios: &["happy-path"],
    },
    AreaDef {
        name: "config-lifecycle",
        description: "configctl — config draft/validate/compile/activate",
        patterns: &["internal/actors/scopes/configctl/", "internal/application/configctl/"],
        dimensions: &["contracts"],
        scenarios: &["config-lifecycle"],
    },
];

/// Map dimension name to the raccoon-cli command that validates it.
fn dimension_command(dim: &str) -> Option<&'static str> {
    match dim {
        "project-structure" => Some("raccoon-cli doctor"),
        "topology" => Some("raccoon-cli topology-doctor"),
        "contracts" => Some("raccoon-cli contract-audit"),
        "runtime-bindings" => Some("raccoon-cli runtime-bindings"),
        "architecture" => Some("raccoon-cli arch-guard"),
        "drift" => Some("raccoon-cli drift-detect"),
        _ => None,
    }
}

/// Scenario metadata.
fn scenario_description(name: &str) -> &'static str {
    match name {
        "happy-path" => "full E2E: config lifecycle + data plane + validation results",
        "config-lifecycle" => "control plane lifecycle: draft -> validate -> compile -> activate",
        "invalid-payload" => "validator catches invalid payloads from emulator",
        "missing-binding" => "query non-existent scope — verifies graceful degradation",
        "readiness-probe" => "cluster bootstrap and readiness verification",
        _ => "runtime validation scenario",
    }
}

// ── Build functions ────────────────────────────────────────────────────────

fn build_file_impacts(impact_report: &impact_map::ImpactReport) -> Vec<FileImpact> {
    impact_report
        .impacts
        .iter()
        .map(|imp| FileImpact {
            file: imp.target.clone(),
            package: imp.resolved_package.clone(),
            exported_symbols: imp
                .exported_symbols
                .iter()
                .map(|s| format!("{} [{}]", s.name, s.kind))
                .collect(),
            direct_dependents: imp
                .direct_dependents
                .iter()
                .map(|d| d.package_dir.clone())
                .collect(),
            contract_items: imp
                .contract_surface
                .iter()
                .map(|c| format!("{} [{}]", c.name, c.kind))
                .collect(),
            risks: imp
                .risks
                .iter()
                .map(|r| format!("[{}] {}", r.basis, r.description))
                .collect(),
        })
        .collect()
}

fn build_affected_areas(changed_files: &[String]) -> Vec<AffectedArea> {
    let mut areas: BTreeMap<&str, Vec<String>> = BTreeMap::new();

    for file in changed_files {
        for area_def in AREA_DEFS {
            if area_def.patterns.iter().any(|p| file.contains(p)) {
                areas
                    .entry(area_def.name)
                    .or_default()
                    .push(file.clone());
            }
        }
    }

    areas
        .into_iter()
        .map(|(name, files)| {
            let def = AREA_DEFS.iter().find(|a| a.name == name).unwrap();
            AffectedArea {
                name: name.to_string(),
                description: def.description.to_string(),
                files_touched: files,
            }
        })
        .collect()
}

fn find_nearby_tests(
    index: &codeintel::ProjectIndex,
    changed_files: &[String],
) -> Vec<NearbyTest> {
    let mut tests = Vec::new();
    let mut seen_test_files: BTreeSet<String> = BTreeSet::new();

    for file in changed_files {
        let normalized = file.trim_start_matches("./");

        // Find the directory of this file.
        let dir = match normalized.rfind('/') {
            Some(pos) => &normalized[..pos],
            None => continue,
        };

        // Look for test files in the same package.
        for go_file in index.files_in_dir(dir) {
            if go_file.is_test && !seen_test_files.contains(&go_file.path) {
                seen_test_files.insert(go_file.path.clone());
                let area = area_for_path(&go_file.path);
                tests.push(NearbyTest {
                    test_file: go_file.path.clone(),
                    package: go_file.package.clone(),
                    for_area: area,
                });
            }
        }
    }

    tests
}

fn area_for_path(path: &str) -> String {
    for area_def in AREA_DEFS {
        if area_def.patterns.iter().any(|p| path.contains(p)) {
            return area_def.name.to_string();
        }
    }
    "general".to_string()
}

fn find_coverage_gaps(
    affected_areas: &[AffectedArea],
    existing_tests: &[NearbyTest],
    project_root: &Path,
) -> Vec<CoverageGap> {
    let mut gaps = Vec::new();

    for area in affected_areas {
        let area_def = match AREA_DEFS.iter().find(|a| a.name == area.name) {
            Some(d) => d,
            None => continue,
        };

        // Check if there are Go tests covering this area.
        let has_go_tests = existing_tests.iter().any(|t| t.for_area == area.name);

        // Check if there is a scenario for this area.
        let has_scenario = !area_def.scenarios.is_empty();

        // Also check if the area directories actually have _test.go files on disk.
        let has_tests_on_disk = area_def.patterns.iter().any(|pattern| {
            let dir = project_root.join(pattern);
            dir.is_dir() && has_test_files_in_dir(&dir)
        });

        let effective_has_tests = has_go_tests || has_tests_on_disk;

        if !effective_has_tests && !has_scenario {
            gaps.push(CoverageGap {
                area: area.name.clone(),
                description: format!(
                    "{} — no Go tests and no runtime scenario covering this area",
                    area_def.description
                ),
                has_go_tests: false,
                has_scenario: false,
                suggestion: format!(
                    "add _test.go files in {} or create a scenario-smoke covering {}",
                    area_def.patterns.first().unwrap_or(&"this area"),
                    area.name,
                ),
            });
        } else if !effective_has_tests {
            gaps.push(CoverageGap {
                area: area.name.clone(),
                description: format!(
                    "{} — no Go tests (runtime scenario exists: {})",
                    area_def.description,
                    area_def.scenarios.join(", "),
                ),
                has_go_tests: false,
                has_scenario: true,
                suggestion: format!(
                    "add _test.go files in {} to catch regressions without cluster",
                    area_def.patterns.first().unwrap_or(&"this area"),
                ),
            });
        } else if !has_scenario {
            gaps.push(CoverageGap {
                area: area.name.clone(),
                description: format!(
                    "{} — Go tests exist but no runtime scenario",
                    area_def.description,
                ),
                has_go_tests: true,
                has_scenario: false,
                suggestion: "unit tests cover logic but not runtime integration — \
                    consider adding a scenario-smoke if this area interacts with infra"
                    .into(),
            });
        }
        // If both exist, no gap.
    }

    gaps
}

fn has_test_files_in_dir(dir: &Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if has_test_files_in_dir(&path) {
                return true;
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with("_test.go") {
                return true;
            }
        }
    }
    false
}

fn recommend_scenarios(affected_areas: &[AffectedArea]) -> (Vec<RecommendedScenario>, bool) {
    let mut scenario_names: BTreeSet<&str> = BTreeSet::new();
    let mut needs_infra = false;

    for area in affected_areas {
        if let Some(area_def) = AREA_DEFS.iter().find(|a| a.name == area.name) {
            for scenario_name in area_def.scenarios {
                scenario_names.insert(scenario_name);
                needs_infra = true;
            }
        }
    }

    let scenarios = scenario_names
        .into_iter()
        .map(|name| {
            let desc = scenario_description(name);
            let areas_needing: Vec<&str> = affected_areas
                .iter()
                .filter(|a| {
                    AREA_DEFS
                        .iter()
                        .find(|d| d.name == a.name)
                        .map(|d| d.scenarios.contains(&name))
                        .unwrap_or(false)
                })
                .map(|a| a.name.as_str())
                .collect();

            RecommendedScenario {
                name: name.to_string(),
                description: desc.to_string(),
                why: format!("covers affected area(s): {}", areas_needing.join(", ")),
            }
        })
        .collect();

    (scenarios, needs_infra)
}

fn recommend_gate_profile(affected_areas: &[AffectedArea], needs_infra: bool) -> String {
    if needs_infra {
        "deep".into()
    } else if affected_areas.is_empty() {
        "fast".into()
    } else {
        "fast".into()
    }
}

fn build_before_commands(
    affected_areas: &[AffectedArea],
    recommended_scenarios: &[RecommendedScenario],
) -> Vec<String> {
    let mut commands: BTreeSet<String> = BTreeSet::new();

    for area in affected_areas {
        if let Some(area_def) = AREA_DEFS.iter().find(|a| a.name == area.name) {
            for dim in area_def.dimensions {
                if let Some(cmd) = dimension_command(dim) {
                    commands.insert(cmd.to_string());
                }
            }
        }
    }

    // Add scenario commands for before (confirm baseline).
    for scenario in recommended_scenarios {
        commands.insert(format!("raccoon-cli scenario-smoke {}", scenario.name));
    }

    commands.into_iter().collect()
}

fn build_after_commands(
    affected_areas: &[AffectedArea],
    recommended_scenarios: &[RecommendedScenario],
) -> Vec<String> {
    let mut commands: BTreeSet<String> = BTreeSet::new();

    // Same static checks as before.
    for area in affected_areas {
        if let Some(area_def) = AREA_DEFS.iter().find(|a| a.name == area.name) {
            for dim in area_def.dimensions {
                if let Some(cmd) = dimension_command(dim) {
                    commands.insert(cmd.to_string());
                }
            }
        }
    }

    for scenario in recommended_scenarios {
        commands.insert(format!("raccoon-cli scenario-smoke {}", scenario.name));
    }

    // Always include canonical verification.
    commands.insert("make verify".to_string());

    commands.into_iter().collect()
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_json(report: &TddReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_human(report: &TddReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== TDD Flow Guide ===\n").unwrap();

    if report.changed_files.is_empty() {
        writeln!(out, "No changed files detected.\n").unwrap();
        writeln!(out, "Usage: raccoon-cli tdd <file1> [file2] ...").unwrap();
        writeln!(out, "   or: make changes first, then run `raccoon-cli tdd` to auto-detect.\n").unwrap();
        writeln!(out, "Generic TDD cycle:").unwrap();
        writeln!(out, "  1. Run `make check` to confirm known-good baseline").unwrap();
        writeln!(out, "  2. Write/update test or scenario for your intended change").unwrap();
        writeln!(out, "  3. Implement the change").unwrap();
        writeln!(out, "  4. Run `make verify` to prove safety").unwrap();
        return out;
    }

    // Changed files
    writeln!(out, "Changed files ({}):", report.changed_files.len()).unwrap();
    for f in &report.changed_files {
        writeln!(out, "  {f}").unwrap();
    }
    writeln!(out).unwrap();

    // Impact summary
    if !report.file_impacts.is_empty() {
        let has_symbols = report.file_impacts.iter().any(|fi| !fi.exported_symbols.is_empty());
        let has_contracts = report.file_impacts.iter().any(|fi| !fi.contract_items.is_empty());
        let has_deps = report.file_impacts.iter().any(|fi| !fi.direct_dependents.is_empty());

        if has_symbols || has_contracts || has_deps {
            writeln!(out, "Structural impact:").unwrap();
            for fi in &report.file_impacts {
                if fi.exported_symbols.is_empty()
                    && fi.contract_items.is_empty()
                    && fi.direct_dependents.is_empty()
                {
                    continue;
                }

                let pkg_label = fi
                    .package
                    .as_deref()
                    .map(|p| format!(" ({})", p))
                    .unwrap_or_default();
                writeln!(out, "  {}{}:", fi.file, pkg_label).unwrap();

                if !fi.exported_symbols.is_empty() {
                    let limit = if verbose { fi.exported_symbols.len() } else { 5 };
                    writeln!(
                        out,
                        "    exported symbols ({}): {}{}",
                        fi.exported_symbols.len(),
                        fi.exported_symbols.iter().take(limit).cloned().collect::<Vec<_>>().join(", "),
                        if !verbose && fi.exported_symbols.len() > 5 {
                            format!(" ... +{} more", fi.exported_symbols.len() - 5)
                        } else {
                            String::new()
                        }
                    )
                    .unwrap();
                }

                if !fi.contract_items.is_empty() {
                    writeln!(
                        out,
                        "    contract surface ({}): {}",
                        fi.contract_items.len(),
                        fi.contract_items.join(", ")
                    )
                    .unwrap();
                }

                if !fi.direct_dependents.is_empty() {
                    writeln!(
                        out,
                        "    dependents ({}): {}",
                        fi.direct_dependents.len(),
                        fi.direct_dependents.join(", ")
                    )
                    .unwrap();
                }

                if verbose {
                    for risk in &fi.risks {
                        writeln!(out, "    risk: {risk}").unwrap();
                    }
                }
            }
            writeln!(out).unwrap();
        }
    }

    // Affected areas
    if report.affected_areas.is_empty() {
        writeln!(out, "No sensitive areas affected — standard TDD cycle applies:").unwrap();
        writeln!(out, "  1. Run `make check` before coding").unwrap();
        writeln!(out, "  2. Write/update tests").unwrap();
        writeln!(out, "  3. Implement changes").unwrap();
        writeln!(out, "  4. Run `make verify` after changes").unwrap();
        return out;
    }

    writeln!(out, "Affected sensitive areas:").unwrap();
    for area in &report.affected_areas {
        writeln!(out, "  {} — {}", area.name, area.description).unwrap();
        if verbose {
            for f in &area.files_touched {
                writeln!(out, "    touched: {f}").unwrap();
            }
        }
    }
    writeln!(out).unwrap();

    // Existing tests
    if !report.existing_tests.is_empty() {
        writeln!(out, "Existing tests nearby ({}):", report.existing_tests.len()).unwrap();
        for t in &report.existing_tests {
            writeln!(out, "  {} (package: {}, area: {})", t.test_file, t.package, t.for_area).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Coverage gaps
    if !report.coverage_gaps.is_empty() {
        writeln!(out, "Coverage gaps:").unwrap();
        for gap in &report.coverage_gaps {
            writeln!(out, "  {} — {}", gap.area, gap.description).unwrap();
            writeln!(out, "    suggestion: {}", gap.suggestion).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Before commands
    if !report.before_commands.is_empty() {
        writeln!(out, "BEFORE your change (confirm baseline passes):").unwrap();
        for cmd in &report.before_commands {
            writeln!(out, "  $ {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // After commands
    if !report.after_commands.is_empty() {
        writeln!(out, "AFTER your change (prove safety):").unwrap();
        for cmd in &report.after_commands {
            writeln!(out, "  $ {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // Recommended scenarios
    if !report.recommended_scenarios.is_empty() {
        writeln!(out, "Recommended scenarios:").unwrap();
        for s in &report.recommended_scenarios {
            writeln!(out, "  {} — {}", s.name, s.description).unwrap();
            writeln!(out, "    why: {}", s.why).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Infrastructure note
    if report.needs_infra {
        writeln!(out, "NOTE: Runtime scenarios require a running cluster.").unwrap();
        writeln!(out, "  Start with: make up-dataplane").unwrap();
        writeln!(out, "  Recommended gate: raccoon-cli quality-gate --profile {}", report.recommended_profile).unwrap();
        writeln!(out).unwrap();
    }

    // Discipline reminder
    writeln!(out, "Discipline:").unwrap();
    writeln!(out, "  - Without a passing baseline, you can't know your change is safe").unwrap();
    writeln!(out, "  - Without a test for the new behavior, success is a guess").unwrap();
    writeln!(out, "  - `make verify` is the canonical proof command").unwrap();

    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project(tmp: &TempDir) -> &Path {
        let root = tmp.path();

        // Domain layer with tests (good coverage)
        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::write(
            root.join("internal/domain/configctl/config.go"),
            r#"package configctl

import "time"

type ConfigSet struct {
	SetID    string
	Versions []ConfigVersion
}

type ConfigVersion struct {
	VersionID string
	CreatedAt time.Time
}

func NewConfigSet(id string) ConfigSet {
	return ConfigSet{SetID: id}
}
"#,
        ).unwrap();

        fs::write(
            root.join("internal/domain/configctl/config_test.go"),
            r#"package configctl

import "testing"

func TestNewConfigSet(t *testing.T) {
	s := NewConfigSet("test")
	if s.SetID != "test" {
		t.Fatal("expected test")
	}
}
"#,
        ).unwrap();

        // Application ports (contracts)
        fs::create_dir_all(root.join("internal/application/ports")).unwrap();
        fs::write(
            root.join("internal/application/ports/configctl.go"),
            r#"package ports

import "context"

type ConfigctlGateway interface {
	CreateDraft(ctx context.Context, cmd string) (string, error)
	GetConfig(ctx context.Context, id string) (string, error)
}
"#,
        ).unwrap();

        // Application configctl (with test)
        fs::create_dir_all(root.join("internal/application/configctl/contracts")).unwrap();
        fs::write(
            root.join("internal/application/configctl/contracts/commands.go"),
            r#"package contracts

type CreateDraftCommand struct {
	SetID string
	Name  string
}
"#,
        ).unwrap();

        fs::write(
            root.join("internal/application/configctl/create_draft.go"),
            r#"package configctl

import (
	domain "example.com/quality-service/internal/domain/configctl"
)

func CreateDraft(id string) domain.ConfigSet {
	return domain.NewConfigSet(id)
}
"#,
        ).unwrap();

        // NATS adapter (no tests — weak coverage)
        fs::create_dir_all(root.join("internal/adapters/nats")).unwrap();
        fs::write(
            root.join("internal/adapters/nats/codec.go"),
            r#"package nats

import (
	domain "example.com/quality-service/internal/domain/configctl"
)

func Encode(s domain.ConfigSet) ([]byte, error) {
	return nil, nil
}
"#,
        ).unwrap();

        // Actors — configctl supervisor
        fs::create_dir_all(root.join("internal/actors/scopes/configctl")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/configctl/supervisor.go"),
            r#"package configctl

import (
	app "example.com/quality-service/internal/application/configctl"
	domain "example.com/quality-service/internal/domain/configctl"
)

type Supervisor struct {
	sets []domain.ConfigSet
}

func New() Supervisor {
	s := app.CreateDraft("init")
	return Supervisor{sets: []domain.ConfigSet{s}}
}
"#,
        ).unwrap();

        // Validator logic (no tests — weak coverage)
        fs::create_dir_all(root.join("internal/actors/scopes/validator")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/validator/supervisor.go"),
            r#"package validator

type Supervisor struct {
	running bool
}

func New() Supervisor {
	return Supervisor{running: true}
}
"#,
        ).unwrap();

        // HTTP handlers (no tests)
        fs::create_dir_all(root.join("internal/interfaces/http/handlers")).unwrap();
        fs::write(
            root.join("internal/interfaces/http/handlers/configctl.go"),
            r#"package handlers

func HandleConfig() {}
"#,
        ).unwrap();

        // Config files
        fs::create_dir_all(root.join("deploy/configs")).unwrap();
        fs::write(
            root.join("deploy/configs/consumer.jsonc"),
            r#"{ "service": "consumer" }"#,
        ).unwrap();

        // Compose
        fs::create_dir_all(root.join("deploy/compose")).unwrap();
        fs::write(
            root.join("deploy/compose/docker-compose.yaml"),
            "services: {}",
        ).unwrap();

        root
    }

    // ── Well-covered area ──────────────────────────────────────────────

    #[test]
    fn domain_change_finds_nearby_tests() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert!(!report.existing_tests.is_empty(), "should find config_test.go");
        assert!(
            report.existing_tests.iter().any(|t| t.test_file.contains("config_test.go")),
            "should find config_test.go nearby, got: {:?}",
            report.existing_tests
        );
    }

    #[test]
    fn domain_change_detects_affected_area() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert!(
            report.affected_areas.iter().any(|a| a.name == "domain"),
            "should detect domain area, got: {:?}",
            report.affected_areas
        );
    }

    #[test]
    fn domain_change_has_structural_impact() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert!(!report.file_impacts.is_empty());
        let impact = &report.file_impacts[0];
        assert!(!impact.exported_symbols.is_empty(), "domain file exports ConfigSet etc");
    }

    #[test]
    fn domain_change_no_infra_needed() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert!(!report.needs_infra, "domain changes don't need infra");
    }

    // ── Weakly-covered area ────────────────────────────────────────────

    #[test]
    fn nats_adapter_has_no_tests_nearby() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert!(
            report.existing_tests.is_empty() || !report.existing_tests.iter().any(|t| t.for_area == "nats-adapters"),
            "nats adapter has no test files"
        );
    }

    #[test]
    fn nats_adapter_reports_coverage_gap() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert!(
            report.coverage_gaps.iter().any(|g| g.area == "nats-adapters"),
            "should report coverage gap for nats-adapters, got: {:?}",
            report.coverage_gaps
        );
    }

    #[test]
    fn nats_adapter_needs_infra() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert!(report.needs_infra, "nats adapter changes should recommend runtime scenarios");
    }

    #[test]
    fn nats_adapter_recommends_happy_path() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert!(
            report.recommended_scenarios.iter().any(|s| s.name == "happy-path"),
            "nats adapter should recommend happy-path scenario, got: {:?}",
            report.recommended_scenarios
        );
    }

    // ── Validator area (needs specific scenarios) ──────────────────────

    #[test]
    fn validator_recommends_invalid_payload_scenario() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/actors/scopes/validator/supervisor.go".into()]);

        assert!(
            report.recommended_scenarios.iter().any(|s| s.name == "invalid-payload"),
            "validator changes should recommend invalid-payload scenario"
        );
    }

    #[test]
    fn validator_recommends_happy_path_scenario() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/actors/scopes/validator/supervisor.go".into()]);

        assert!(
            report.recommended_scenarios.iter().any(|s| s.name == "happy-path"),
            "validator changes should recommend happy-path scenario"
        );
    }

    // ── Config lifecycle ───────────────────────────────────────────────

    #[test]
    fn configctl_change_recommends_config_lifecycle_scenario() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/application/configctl/create_draft.go".into()]);

        assert!(
            report.recommended_scenarios.iter().any(|s| s.name == "config-lifecycle"),
            "configctl changes should recommend config-lifecycle scenario, got: {:?}",
            report.recommended_scenarios
        );
    }

    // ── Mixed/ambiguous changes ────────────────────────────────────────

    #[test]
    fn mixed_changes_aggregate_areas() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &[
            "internal/domain/configctl/config.go".into(),
            "internal/adapters/nats/codec.go".into(),
        ]);

        assert!(report.affected_areas.len() >= 2, "should affect both domain and nats-adapters");
        assert!(
            report.affected_areas.iter().any(|a| a.name == "domain"),
            "should include domain area"
        );
        assert!(
            report.affected_areas.iter().any(|a| a.name == "nats-adapters"),
            "should include nats-adapters area"
        );
    }

    #[test]
    fn mixed_changes_merge_commands() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &[
            "internal/domain/configctl/config.go".into(),
            "internal/adapters/nats/codec.go".into(),
        ]);

        // Should have arch-guard (from both) and contract-audit (from both)
        assert!(
            report.before_commands.iter().any(|c| c.contains("arch-guard")),
            "should include arch-guard, got: {:?}",
            report.before_commands
        );
        assert!(
            report.before_commands.iter().any(|c| c.contains("contract-audit")),
            "should include contract-audit"
        );
    }

    #[test]
    fn mixed_changes_no_duplicate_commands() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &[
            "internal/domain/configctl/config.go".into(),
            "internal/adapters/nats/codec.go".into(),
        ]);

        let mut seen = BTreeSet::new();
        for cmd in &report.before_commands {
            assert!(seen.insert(cmd), "duplicate before command: {cmd}");
        }
    }

    // ── Edge cases ─────────────────────────────────────────────────────

    #[test]
    fn empty_files_returns_empty_report() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &[]);

        assert!(report.changed_files.is_empty());
        assert!(report.affected_areas.is_empty());
        assert!(report.after_commands.iter().any(|c| c.contains("verify")));
    }

    #[test]
    fn non_project_file_no_areas() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["README.md".into()]);

        assert!(report.affected_areas.is_empty());
    }

    #[test]
    fn config_file_change_affects_config_area() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["deploy/configs/consumer.jsonc".into()]);

        assert!(
            report.affected_areas.iter().any(|a| a.name == "config-files"),
            "should affect config-files area"
        );
    }

    #[test]
    fn compose_change_recommends_readiness_probe() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["deploy/compose/docker-compose.yaml".into()]);

        assert!(
            report.recommended_scenarios.iter().any(|s| s.name == "readiness-probe"),
            "compose changes should recommend readiness-probe scenario"
        );
    }

    // ── Rendering tests ────────────────────────────────────────────────

    #[test]
    fn json_output_is_valid() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["changed_files"].is_array());
        assert!(parsed["file_impacts"].is_array());
        assert!(parsed["affected_areas"].is_array());
        assert!(parsed["existing_tests"].is_array());
        assert!(parsed["coverage_gaps"].is_array());
        assert!(parsed["recommended_profile"].is_string());
        assert!(parsed["recommended_scenarios"].is_array());
        assert!(parsed["before_commands"].is_array());
        assert!(parsed["after_commands"].is_array());
        assert!(parsed["needs_infra"].is_boolean());
        assert!(parsed["scope_note"].is_string());
    }

    #[test]
    fn human_output_contains_key_sections() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        let human = render_human(&report, false);
        assert!(human.contains("TDD Flow Guide"));
        assert!(human.contains("Changed files"));
        assert!(human.contains("Affected sensitive areas"));
        assert!(human.contains("BEFORE"));
        assert!(human.contains("AFTER"));
        assert!(human.contains("Discipline"));
    }

    #[test]
    fn human_output_for_empty_is_helpful() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &[]);

        let human = render_human(&report, false);
        assert!(human.contains("No changed files"));
        assert!(human.contains("make verify"));
    }

    #[test]
    fn verbose_shows_more_detail() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        let verbose = render_human(&report, true);
        let terse = render_human(&report, false);
        // Verbose should include touched files under areas
        assert!(verbose.contains("touched:"));
        assert!(!terse.contains("touched:"));
    }

    // ── Gate profile recommendation ────────────────────────────────────

    #[test]
    fn domain_change_recommends_fast_profile() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert_eq!(report.recommended_profile, "fast");
    }

    #[test]
    fn validator_change_recommends_deep_profile() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/actors/scopes/validator/supervisor.go".into()]);

        assert_eq!(report.recommended_profile, "deep");
    }

    // ── Contracts and dependents ────────────────────────────────────────

    #[test]
    fn port_interface_change_detects_contracts() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/application/ports/configctl.go".into()]);

        let has_contract = report.file_impacts.iter().any(|fi| !fi.contract_items.is_empty());
        assert!(has_contract, "port interface should show contract surface");
    }

    #[test]
    fn domain_change_detects_dependents() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        let has_deps = report.file_impacts.iter().any(|fi| !fi.direct_dependents.is_empty());
        assert!(has_deps, "domain package should have dependents");
    }

    // ── Before/after command correctness ───────────────────────────────

    #[test]
    fn after_commands_always_include_make_verify() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        for file in &[
            "internal/domain/configctl/config.go",
            "internal/adapters/nats/codec.go",
            "deploy/configs/consumer.jsonc",
            "README.md",
        ] {
            let report = analyze(root, &[file.to_string()]);
            assert!(
                report.after_commands.iter().any(|c| c.contains("verify")),
                "after_commands should include make verify for file {}, got: {:?}",
                file,
                report.after_commands
            );
        }
    }

    #[test]
    fn before_commands_match_affected_dimensions() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        // Domain requires architecture + contracts
        assert!(
            report.before_commands.iter().any(|c| c.contains("arch-guard")),
            "domain changes require arch-guard"
        );
        assert!(
            report.before_commands.iter().any(|c| c.contains("contract-audit")),
            "domain changes require contract-audit"
        );
    }
}
