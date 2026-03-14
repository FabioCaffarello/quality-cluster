//! Automatic smoke/TDD recommendations from diff or baseline comparison.
//!
//! Composes signals from impact-map, tdd, and optional baseline-drift to generate
//! prioritized, actionable recommendations for what to validate after a change.
//!
//! ## Signals used
//!
//! 1. **Changed files** (from git status or explicit list)
//!    → affected areas, sensitive dimensions, nearby tests
//! 2. **Impact analysis** (codeintel AST index)
//!    → exported symbols, dependents, contract surface, risks
//! 3. **Baseline drift** (optional snapshot comparison)
//!    → contract surface drift, breaking changes, isolation loss
//!
//! ## Output structure
//!
//! Every item is tagged with provenance:
//! - `[fact]`           — observed from AST, config, or source
//! - `[inference]`      — derived from structural patterns or heuristics
//! - `[recommendation]` — actionable suggestion based on facts/inferences
//!
//! Recommendations are grouped into:
//! - **Smoke scenarios** to run (with why)
//! - **Quality-gate profile** (fast/ci/deep)
//! - **Priority test areas** (with coverage status)
//! - **Architectural/contract risks** to review

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::analyzers::{impact_map, tdd};

// ── Public API ─────────────────────────────────────────────────────────────

/// Analyze changed files and produce prioritized recommendations.
pub fn analyze(project_root: &Path, changed_files: &[String]) -> RecommendReport {
    if changed_files.is_empty() {
        return RecommendReport::empty();
    }

    let tdd_report = tdd::analyze(project_root, changed_files);
    let impact_report = impact_map::analyze(project_root, changed_files);

    build_report(changed_files, &tdd_report, &impact_report, None)
}

/// Analyze with baseline drift context from a snapshot file.
pub fn analyze_with_baseline(
    project_root: &Path,
    changed_files: &[String],
    baseline_path: &Path,
) -> RecommendReport {
    let tdd_report = if changed_files.is_empty() {
        tdd::TddReport::empty_report()
    } else {
        tdd::analyze(project_root, changed_files)
    };

    let impact_report = if changed_files.is_empty() {
        impact_map::ImpactReport::empty()
    } else {
        impact_map::analyze(project_root, changed_files)
    };

    let drift_context =
        match super::baseline_drift::analyze(baseline_path, project_root) {
            Ok(drift_report) => Some(DriftContext::from_drift_report(&drift_report)),
            Err(_) => None,
        };

    build_report(changed_files, &tdd_report, &impact_report, drift_context)
}

// ── Report types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RecommendReport {
    /// Input summary.
    pub input: InputSummary,
    /// Observed facts from static analysis.
    pub facts: Vec<TaggedItem>,
    /// Inferences derived from combining facts.
    pub inferences: Vec<TaggedItem>,
    /// Actionable recommendations.
    pub recommendations: Vec<Recommendation>,
    /// Smoke scenarios to run, with priority and rationale.
    pub smoke_scenarios: Vec<SmokeRecommendation>,
    /// Recommended quality-gate profile.
    pub gate_profile: GateRecommendation,
    /// Priority test areas.
    pub priority_areas: Vec<PriorityArea>,
    /// Architectural and contract risks to review.
    pub risks: Vec<RiskItem>,
    /// Commands to run (ordered).
    pub commands: CommandPlan,
    /// Methodology note.
    pub scope_note: String,
}

impl RecommendReport {
    fn empty() -> Self {
        RecommendReport {
            input: InputSummary {
                changed_files: vec![],
                has_baseline: false,
                change_scope: "none".into(),
            },
            facts: vec![],
            inferences: vec![],
            recommendations: vec![Recommendation {
                action: "Provide changed files or use git status for auto-detection".into(),
                why: "No input files to analyze".into(),
                priority: "info".into(),
            }],
            smoke_scenarios: vec![],
            gate_profile: GateRecommendation {
                profile: "fast".into(),
                why: "No changes detected — default profile".into(),
            },
            priority_areas: vec![],
            risks: vec![],
            commands: CommandPlan {
                before: vec![],
                after: vec!["make verify".into()],
            },
            scope_note: "No changed files provided. Pass file paths or let raccoon-cli \
                         auto-detect from git status."
                .into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InputSummary {
    pub changed_files: Vec<String>,
    pub has_baseline: bool,
    pub change_scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaggedItem {
    pub tag: String,
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub action: String,
    pub why: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SmokeRecommendation {
    pub scenario: String,
    pub description: String,
    pub why: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateRecommendation {
    pub profile: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriorityArea {
    pub area: String,
    pub description: String,
    pub has_unit_tests: bool,
    pub has_scenario: bool,
    pub coverage_status: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskItem {
    pub category: String,
    pub severity: String,
    pub evidence_basis: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub review_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandPlan {
    pub before: Vec<String>,
    pub after: Vec<String>,
}

// ── Drift context extraction ───────────────────────────────────────────────

#[allow(dead_code)]
struct DriftContext {
    verdict: String,
    critical_findings: Vec<DriftFinding>,
    warning_findings: Vec<DriftFinding>,
    info_findings: Vec<DriftFinding>,
}

struct DriftFinding {
    class: String,
    message: String,
    evidence_basis: String,
    recommendation: String,
}

impl DriftContext {
    fn from_drift_report(report: &super::baseline_drift::BaselineDriftReport) -> Self {
        let mut critical = Vec::new();
        let mut warning = Vec::new();
        let mut info = Vec::new();

        for f in &report.findings {
            let df = DriftFinding {
                class: f.class.clone(),
                message: f.message.clone(),
                evidence_basis: f.evidence_basis.clone(),
                recommendation: f.recommendation.clone(),
            };
            match f.severity.as_str() {
                "critical" => critical.push(df),
                "warning" => warning.push(df),
                _ => info.push(df),
            }
        }

        let verdict = match report.verdict {
            super::baseline_drift::Verdict::Clean => "clean",
            super::baseline_drift::Verdict::Mild => "mild",
            super::baseline_drift::Verdict::Drifted => "drifted",
        };

        DriftContext {
            verdict: verdict.to_string(),
            critical_findings: critical,
            warning_findings: warning,
            info_findings: info,
        }
    }

    fn has_critical(&self) -> bool {
        !self.critical_findings.is_empty()
    }

    fn has_contract_drift(&self) -> bool {
        self.critical_findings
            .iter()
            .chain(self.warning_findings.iter())
            .any(|f| f.class.contains("contract"))
    }

    fn has_breaking_changes(&self) -> bool {
        self.critical_findings
            .iter()
            .chain(self.warning_findings.iter())
            .any(|f| f.class.contains("breaking"))
    }

    fn has_isolation_loss(&self) -> bool {
        self.critical_findings
            .iter()
            .chain(self.warning_findings.iter())
            .any(|f| f.class.contains("isolation") || f.class.contains("coupling"))
    }
}

// ── Change scope classification ────────────────────────────────────────────

fn classify_change_scope(changed_files: &[String]) -> &'static str {
    if changed_files.is_empty() {
        return "none";
    }
    if changed_files.len() == 1 {
        return "single-file";
    }

    let mut areas: BTreeSet<&str> = BTreeSet::new();
    for f in changed_files {
        if f.contains("internal/domain/") {
            areas.insert("domain");
        } else if f.contains("internal/application/") {
            areas.insert("application");
        } else if f.contains("internal/adapters/") {
            areas.insert("adapters");
        } else if f.contains("internal/actors/") {
            areas.insert("actors");
        } else if f.contains("internal/interfaces/") {
            areas.insert("interfaces");
        } else if f.contains("deploy/") {
            areas.insert("deploy");
        } else if f.contains("tools/") {
            areas.insert("tools");
        } else {
            areas.insert("other");
        }
    }

    if changed_files.len() > 15 || areas.len() >= 4 {
        "large"
    } else if areas.len() >= 2 {
        "cross-layer"
    } else {
        "localized"
    }
}

// ── Core report builder ────────────────────────────────────────────────────

fn build_report(
    changed_files: &[String],
    tdd: &tdd::TddReport,
    impact: &impact_map::ImpactReport,
    drift: Option<DriftContext>,
) -> RecommendReport {
    let change_scope = classify_change_scope(changed_files);
    let has_baseline = drift.is_some();

    let mut facts: Vec<TaggedItem> = Vec::new();
    let mut inferences: Vec<TaggedItem> = Vec::new();
    let mut recommendations: Vec<Recommendation> = Vec::new();
    let mut risks: Vec<RiskItem> = Vec::new();

    // ── Facts from impact analysis ──

    for imp in &impact.impacts {
        if let Some(ref pkg) = imp.resolved_package {
            facts.push(TaggedItem {
                tag: "fact".into(),
                category: "scope".into(),
                message: format!("{} → package {}", imp.target, pkg),
                location: None,
            });
        }

        if !imp.exported_symbols.is_empty() {
            facts.push(TaggedItem {
                tag: "fact".into(),
                category: "symbols".into(),
                message: format!(
                    "{} exports {} symbols",
                    imp.target,
                    imp.exported_symbols.len()
                ),
                location: None,
            });
        }

        if !imp.direct_dependents.is_empty() {
            facts.push(TaggedItem {
                tag: "fact".into(),
                category: "dependents".into(),
                message: format!(
                    "{} has {} direct dependents: {}",
                    imp.target,
                    imp.direct_dependents.len(),
                    imp.direct_dependents
                        .iter()
                        .take(5)
                        .map(|d| d.package_dir.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                location: None,
            });
        }

        for item in &imp.contract_surface {
            facts.push(TaggedItem {
                tag: "fact".into(),
                category: "contract-surface".into(),
                message: format!("{} [{}] — {}", item.name, item.kind, item.why),
                location: Some(item.location.clone()),
            });
        }

        // Risks from impact
        for risk in &imp.risks {
            risks.push(RiskItem {
                category: "impact".into(),
                severity: if risk.basis == "observed" {
                    "warning".into()
                } else {
                    "info".into()
                },
                evidence_basis: risk.basis.clone(),
                message: risk.description.clone(),
                location: None,
                review_action: format!(
                    "Review impact in context of {} change",
                    imp.target
                ),
            });
        }
    }

    // ── Inferences from affected areas ──

    for area in &tdd.affected_areas {
        inferences.push(TaggedItem {
            tag: "inference".into(),
            category: "affected-area".into(),
            message: format!("{} — {}", area.name, area.description),
            location: None,
        });
    }

    if change_scope == "cross-layer" {
        inferences.push(TaggedItem {
            tag: "inference".into(),
            category: "scope".into(),
            message: "Changes span multiple architecture layers — higher integration risk".into(),
            location: None,
        });
    }

    if change_scope == "large" {
        inferences.push(TaggedItem {
            tag: "inference".into(),
            category: "scope".into(),
            message: "Large change set — full regression recommended".into(),
            location: None,
        });
    }

    // Coverage gaps as inferences
    for gap in &tdd.coverage_gaps {
        inferences.push(TaggedItem {
            tag: "inference".into(),
            category: "coverage-gap".into(),
            message: format!("{} — {}", gap.area, gap.description),
            location: None,
        });
    }

    // ── Drift-derived signals ──

    if let Some(ref drift_ctx) = drift {
        facts.push(TaggedItem {
            tag: "fact".into(),
            category: "baseline".into(),
            message: format!("Baseline drift verdict: {}", drift_ctx.verdict),
            location: None,
        });

        for finding in &drift_ctx.critical_findings {
            risks.push(RiskItem {
                category: finding.class.clone(),
                severity: "critical".into(),
                evidence_basis: finding.evidence_basis.clone(),
                message: finding.message.clone(),
                location: None,
                review_action: finding.recommendation.clone(),
            });
        }

        for finding in &drift_ctx.warning_findings {
            risks.push(RiskItem {
                category: finding.class.clone(),
                severity: "warning".into(),
                evidence_basis: finding.evidence_basis.clone(),
                message: finding.message.clone(),
                location: None,
                review_action: finding.recommendation.clone(),
            });
        }

        if drift_ctx.has_contract_drift() {
            inferences.push(TaggedItem {
                tag: "inference".into(),
                category: "drift".into(),
                message:
                    "Contract surface drift detected — consumers may be affected".into(),
                location: None,
            });
        }

        if drift_ctx.has_isolation_loss() {
            inferences.push(TaggedItem {
                tag: "inference".into(),
                category: "drift".into(),
                message:
                    "Architecture isolation loss detected — layer boundaries weakened".into(),
                location: None,
            });
        }
    }

    // ── Smoke scenario recommendations ──

    let mut smoke_scenarios: Vec<SmokeRecommendation> = Vec::new();
    let mut seen_scenarios: BTreeSet<String> = BTreeSet::new();

    for s in &tdd.recommended_scenarios {
        if seen_scenarios.insert(s.name.clone()) {
            smoke_scenarios.push(SmokeRecommendation {
                scenario: s.name.clone(),
                description: s.description.clone(),
                why: s.why.clone(),
                priority: "high".into(),
            });
        }
    }

    // Drift-driven scenario escalation
    if let Some(ref drift_ctx) = drift {
        if drift_ctx.has_contract_drift() && !seen_scenarios.contains("happy-path") {
            seen_scenarios.insert("happy-path".into());
            smoke_scenarios.push(SmokeRecommendation {
                scenario: "happy-path".into(),
                description: "full E2E: config lifecycle + data plane + validation results"
                    .into(),
                why: "contract surface drift detected — verify E2E integration".into(),
                priority: "high".into(),
            });
        }

        if drift_ctx.has_breaking_changes() && !seen_scenarios.contains("invalid-payload") {
            seen_scenarios.insert("invalid-payload".into());
            smoke_scenarios.push(SmokeRecommendation {
                scenario: "invalid-payload".into(),
                description: "validator catches invalid payloads from emulator".into(),
                why: "breaking changes detected — verify error handling still works".into(),
                priority: "medium".into(),
            });
        }

        if drift_ctx.has_critical() && !seen_scenarios.contains("readiness-probe") {
            seen_scenarios.insert("readiness-probe".into());
            smoke_scenarios.push(SmokeRecommendation {
                scenario: "readiness-probe".into(),
                description: "cluster bootstrap and readiness verification".into(),
                why: "critical drift detected — verify cluster still healthy".into(),
                priority: "medium".into(),
            });
        }
    }

    // Large changes: recommend full suite if not already covered
    if change_scope == "large" {
        for (name, desc) in &[
            ("happy-path", "full E2E: config lifecycle + data plane + validation results"),
            ("readiness-probe", "cluster bootstrap and readiness verification"),
        ] {
            if seen_scenarios.insert(name.to_string()) {
                smoke_scenarios.push(SmokeRecommendation {
                    scenario: name.to_string(),
                    description: desc.to_string(),
                    why: "large change set — full regression recommended".into(),
                    priority: "medium".into(),
                });
            }
        }
    }

    // ── Gate profile recommendation ──

    let gate_profile = determine_gate_profile(
        &tdd,
        &drift,
        change_scope,
        &smoke_scenarios,
    );

    // ── Priority areas ──

    let priority_areas = build_priority_areas(&tdd);

    // ── Actionable recommendations ──

    if !tdd.coverage_gaps.is_empty() {
        for gap in &tdd.coverage_gaps {
            recommendations.push(Recommendation {
                action: gap.suggestion.clone(),
                why: format!("Coverage gap in {}: {}", gap.area, gap.description),
                priority: if !gap.has_go_tests && !gap.has_scenario {
                    "high".into()
                } else {
                    "medium".into()
                },
            });
        }
    }

    if let Some(ref drift_ctx) = drift {
        if drift_ctx.verdict == "drifted" {
            recommendations.push(Recommendation {
                action: "Update baseline snapshot after verifying all checks pass".into(),
                why: "Significant baseline drift — current snapshot is stale".into(),
                priority: "high".into(),
            });
        }
    }

    if change_scope == "cross-layer" {
        recommendations.push(Recommendation {
            action: "Run arch-guard to verify layer boundaries are intact".into(),
            why: "Cross-layer changes have higher risk of boundary violations".into(),
            priority: "high".into(),
        });
    }

    // If there are contract surface items, recommend contract-audit
    let has_contracts = impact
        .impacts
        .iter()
        .any(|imp| !imp.contract_surface.is_empty());
    if has_contracts {
        recommendations.push(Recommendation {
            action: "Run contract-audit to verify messaging contract invariants".into(),
            why: "Changed files touch contract surface types".into(),
            priority: "high".into(),
        });
    }

    // ── Command plan ──

    let commands = CommandPlan {
        before: tdd.before_commands.clone(),
        after: tdd.after_commands.clone(),
    };

    // ── Scope note ──

    let mut scope_parts: Vec<&str> = vec![
        "Recommendations are computed from AST structural analysis and file-pattern matching.",
    ];
    if has_baseline {
        scope_parts.push("Baseline drift signals were incorporated from snapshot comparison.");
    }
    scope_parts.push("No call graph or runtime tracing is available.");

    RecommendReport {
        input: InputSummary {
            changed_files: changed_files.to_vec(),
            has_baseline,
            change_scope: change_scope.to_string(),
        },
        facts,
        inferences,
        recommendations,
        smoke_scenarios,
        gate_profile,
        priority_areas,
        risks,
        commands,
        scope_note: scope_parts.join(" "),
    }
}

// ── Gate profile logic ─────────────────────────────────────────────────────

fn determine_gate_profile(
    tdd: &tdd::TddReport,
    drift: &Option<DriftContext>,
    change_scope: &str,
    smoke_scenarios: &[SmokeRecommendation],
) -> GateRecommendation {
    // Deep: any runtime scenario needed, or drift has critical findings, or large changes
    if tdd.needs_infra {
        return GateRecommendation {
            profile: "deep".into(),
            why: "Runtime scenarios required — affected areas need live infrastructure".into(),
        };
    }

    if let Some(ref drift_ctx) = drift {
        if drift_ctx.has_critical() {
            return GateRecommendation {
                profile: "deep".into(),
                why: "Critical baseline drift detected — full validation needed".into(),
            };
        }
    }

    if change_scope == "large" {
        return GateRecommendation {
            profile: "ci".into(),
            why: "Large change set — strict CI profile recommended".into(),
        };
    }

    if change_scope == "cross-layer" {
        return GateRecommendation {
            profile: "ci".into(),
            why: "Cross-layer changes — CI-strict checks catch boundary violations".into(),
        };
    }

    if !smoke_scenarios.is_empty() {
        return GateRecommendation {
            profile: "deep".into(),
            why: "Smoke scenarios recommended — deep profile includes runtime checks".into(),
        };
    }

    GateRecommendation {
        profile: "fast".into(),
        why: "Localized changes with no runtime impact — fast profile sufficient".into(),
    }
}

// ── Priority areas ─────────────────────────────────────────────────────────

fn build_priority_areas(tdd: &tdd::TddReport) -> Vec<PriorityArea> {
    let mut areas: Vec<PriorityArea> = Vec::new();

    for area in &tdd.affected_areas {
        let gap = tdd.coverage_gaps.iter().find(|g| g.area == area.name);
        let has_tests = tdd
            .existing_tests
            .iter()
            .any(|t| t.for_area == area.name);

        let (coverage_status, suggestion) = match gap {
            Some(g) => {
                let status = if !g.has_go_tests && !g.has_scenario {
                    "uncovered"
                } else if !g.has_go_tests {
                    "scenario-only"
                } else {
                    "unit-only"
                };
                (status.to_string(), g.suggestion.clone())
            }
            None => {
                if has_tests {
                    ("covered".to_string(), "Tests exist — verify they pass".to_string())
                } else {
                    ("unknown".to_string(), "Verify coverage manually".to_string())
                }
            }
        };

        let has_scenario = gap.map(|g| g.has_scenario).unwrap_or(false);

        areas.push(PriorityArea {
            area: area.name.clone(),
            description: area.description.clone(),
            has_unit_tests: has_tests || gap.map(|g| g.has_go_tests).unwrap_or(false),
            has_scenario,
            coverage_status,
            suggestion,
        });
    }

    areas
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_json(report: &RecommendReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_human(report: &RecommendReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== Recommend: Smoke/TDD Priorities ===\n").unwrap();

    if report.input.changed_files.is_empty() && !report.input.has_baseline {
        writeln!(out, "No changed files detected.\n").unwrap();
        writeln!(out, "Usage:").unwrap();
        writeln!(out, "  raccoon-cli recommend <file1> [file2] ...").unwrap();
        writeln!(out, "  raccoon-cli recommend --baseline snapshot.json").unwrap();
        writeln!(
            out,
            "  raccoon-cli recommend   # auto-detect from git status"
        )
        .unwrap();
        return out;
    }

    // ── Input summary ──
    writeln!(
        out,
        "Input: {} file(s), scope: {}{}",
        report.input.changed_files.len(),
        report.input.change_scope,
        if report.input.has_baseline {
            " (with baseline)"
        } else {
            ""
        }
    )
    .unwrap();

    if verbose {
        for f in &report.input.changed_files {
            writeln!(out, "  {f}").unwrap();
        }
    }
    writeln!(out).unwrap();

    // ── Facts ──
    if !report.facts.is_empty() {
        writeln!(out, "Facts:").unwrap();
        for item in &report.facts {
            write!(out, "  [fact] ").unwrap();
            if let Some(ref loc) = item.location {
                writeln!(out, "{} ({})", item.message, loc).unwrap();
            } else {
                writeln!(out, "{}", item.message).unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    // ── Inferences ──
    if !report.inferences.is_empty() {
        writeln!(out, "Inferences:").unwrap();
        for item in &report.inferences {
            writeln!(out, "  [inference] {}", item.message).unwrap();
        }
        writeln!(out).unwrap();
    }

    // ── Risks ──
    if !report.risks.is_empty() {
        writeln!(out, "Risks:").unwrap();
        for risk in &report.risks {
            writeln!(
                out,
                "  [{}] [{}] {} — {}",
                risk.severity, risk.evidence_basis, risk.category, risk.message
            )
            .unwrap();
            if verbose {
                writeln!(out, "    action: {}", risk.review_action).unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    // ── Smoke scenarios ──
    if !report.smoke_scenarios.is_empty() {
        writeln!(out, "Smoke scenarios to run:").unwrap();
        for s in &report.smoke_scenarios {
            writeln!(
                out,
                "  [{}] raccoon-cli scenario-smoke {}",
                s.priority, s.scenario
            )
            .unwrap();
            writeln!(out, "       {} — {}", s.description, s.why).unwrap();
        }
        writeln!(out).unwrap();
    }

    // ── Gate profile ──
    writeln!(
        out,
        "Quality-gate: raccoon-cli quality-gate --profile {}",
        report.gate_profile.profile
    )
    .unwrap();
    writeln!(out, "  why: {}", report.gate_profile.why).unwrap();
    writeln!(out).unwrap();

    // ── Priority areas ──
    if !report.priority_areas.is_empty() {
        writeln!(out, "Priority test areas:").unwrap();
        for area in &report.priority_areas {
            let icon = match area.coverage_status.as_str() {
                "covered" => "ok",
                "uncovered" => "GAP",
                "scenario-only" => "partial",
                "unit-only" => "partial",
                _ => "?",
            };
            writeln!(
                out,
                "  [{}] {} — {}",
                icon, area.area, area.description
            )
            .unwrap();
            if area.coverage_status != "covered" {
                writeln!(out, "       {}", area.suggestion).unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    // ── Recommendations ──
    if !report.recommendations.is_empty() {
        writeln!(out, "Recommendations:").unwrap();
        for rec in &report.recommendations {
            writeln!(
                out,
                "  [{}] {} — {}",
                rec.priority, rec.action, rec.why
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    // ── Command plan ──
    if !report.commands.before.is_empty() {
        writeln!(out, "BEFORE (confirm baseline):").unwrap();
        for cmd in &report.commands.before {
            writeln!(out, "  $ {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    if !report.commands.after.is_empty() {
        writeln!(out, "AFTER (prove safety):").unwrap();
        for cmd in &report.commands.after {
            writeln!(out, "  $ {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // ── Scope note ──
    writeln!(out, "Scope: {}", report.scope_note).unwrap();

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

        // Domain layer with tests
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
        )
        .unwrap();

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
        )
        .unwrap();

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
        )
        .unwrap();

        // Application configctl
        fs::create_dir_all(root.join("internal/application/configctl/contracts")).unwrap();
        fs::write(
            root.join("internal/application/configctl/contracts/commands.go"),
            r#"package contracts

type CreateDraftCommand struct {
	SetID string
	Name  string
}
"#,
        )
        .unwrap();

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
        )
        .unwrap();

        // NATS adapter (no tests)
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
        )
        .unwrap();

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
        )
        .unwrap();

        // Validator logic (no tests)
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
        )
        .unwrap();

        // HTTP handlers (no tests)
        fs::create_dir_all(root.join("internal/interfaces/http/handlers")).unwrap();
        fs::write(
            root.join("internal/interfaces/http/handlers/configctl.go"),
            r#"package handlers

func HandleConfig() {}
"#,
        )
        .unwrap();

        // Config files
        fs::create_dir_all(root.join("deploy/configs")).unwrap();
        fs::write(
            root.join("deploy/configs/consumer.jsonc"),
            r#"{ "service": "consumer" }"#,
        )
        .unwrap();

        // Compose
        fs::create_dir_all(root.join("deploy/compose")).unwrap();
        fs::write(
            root.join("deploy/compose/docker-compose.yaml"),
            "services: {}",
        )
        .unwrap();

        root
    }

    // ── Contract changes ──────────────────────────────────────────────────

    #[test]
    fn contract_change_recommends_contract_audit() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &["internal/application/configctl/contracts/commands.go".into()],
        );

        assert!(
            report
                .recommendations
                .iter()
                .any(|r| r.action.contains("contract-audit")),
            "contract changes should recommend contract-audit, got: {:?}",
            report.recommendations
        );
    }

    #[test]
    fn contract_change_reports_facts() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &["internal/application/configctl/contracts/commands.go".into()],
        );

        assert!(!report.facts.is_empty(), "should have facts from impact analysis");
    }

    // ── Adapter changes (no tests, needs infra) ──────────────────────────

    #[test]
    fn adapter_change_recommends_smoke() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert!(
            !report.smoke_scenarios.is_empty(),
            "adapter change should recommend smoke scenarios"
        );
        assert!(
            report.smoke_scenarios.iter().any(|s| s.scenario == "happy-path"),
            "should recommend happy-path, got: {:?}",
            report.smoke_scenarios
        );
    }

    #[test]
    fn adapter_change_reports_coverage_gap() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert!(
            report
                .priority_areas
                .iter()
                .any(|a| a.area == "nats-adapters" && a.coverage_status != "covered"),
            "nats adapter should report coverage gap, got: {:?}",
            report.priority_areas
        );
    }

    #[test]
    fn adapter_change_recommends_deep_profile() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        assert_eq!(
            report.gate_profile.profile, "deep",
            "adapter changes needing infra should recommend deep profile"
        );
    }

    // ── Actor changes ─────────────────────────────────────────────────────

    #[test]
    fn actor_change_detects_affected_area() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &["internal/actors/scopes/configctl/supervisor.go".into()],
        );

        assert!(
            report.inferences.iter().any(|i| i.category == "affected-area"),
            "should detect affected area inference"
        );
    }

    #[test]
    fn validator_change_recommends_invalid_payload() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &["internal/actors/scopes/validator/supervisor.go".into()],
        );

        assert!(
            report
                .smoke_scenarios
                .iter()
                .any(|s| s.scenario == "invalid-payload"),
            "validator changes should recommend invalid-payload scenario"
        );
    }

    // ── Domain changes (well-covered) ─────────────────────────────────────

    #[test]
    fn domain_change_recommends_fast_profile() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert_eq!(
            report.gate_profile.profile, "fast",
            "domain-only changes should recommend fast profile"
        );
    }

    #[test]
    fn domain_change_has_no_smoke_scenarios() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert!(
            report.smoke_scenarios.is_empty(),
            "domain changes shouldn't need smoke scenarios"
        );
    }

    // ── Cross-layer changes ───────────────────────────────────────────────

    #[test]
    fn cross_layer_change_escalates_profile() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &[
                "internal/domain/configctl/config.go".into(),
                "internal/adapters/nats/codec.go".into(),
            ],
        );

        // Should be deep because nats needs infra
        assert!(
            report.gate_profile.profile == "deep" || report.gate_profile.profile == "ci",
            "cross-layer changes should escalate profile, got: {}",
            report.gate_profile.profile
        );
    }

    #[test]
    fn cross_layer_change_warns_about_boundaries() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &[
                "internal/domain/configctl/config.go".into(),
                "internal/adapters/nats/codec.go".into(),
            ],
        );

        let has_arch_rec = report
            .recommendations
            .iter()
            .any(|r| r.action.contains("arch-guard"));
        assert!(has_arch_rec, "cross-layer should recommend arch-guard");
    }

    #[test]
    fn cross_layer_merges_smoke_scenarios() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &[
                "internal/domain/configctl/config.go".into(),
                "internal/adapters/nats/codec.go".into(),
            ],
        );

        // No duplicates
        let names: Vec<&str> = report.smoke_scenarios.iter().map(|s| s.scenario.as_str()).collect();
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "should not have duplicate scenario recommendations"
        );
    }

    // ── Tooling changes (low risk) ────────────────────────────────────────

    #[test]
    fn tooling_change_minimal_recommendations() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["tools/raccoon-cli/src/main.rs".into()]);

        assert!(
            report.smoke_scenarios.is_empty(),
            "tooling changes shouldn't trigger smoke scenarios"
        );
    }

    // ── Empty input ───────────────────────────────────────────────────────

    #[test]
    fn empty_input_returns_helpful_report() {
        let report = RecommendReport::empty();

        assert!(report.input.changed_files.is_empty());
        assert_eq!(report.input.change_scope, "none");
        assert!(!report.recommendations.is_empty(), "should have usage hint");
    }

    // ── Ambiguous changes ─────────────────────────────────────────────────

    #[test]
    fn non_project_file_produces_minimal_report() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["README.md".into()]);

        assert!(report.smoke_scenarios.is_empty());
        assert_eq!(report.gate_profile.profile, "fast");
    }

    #[test]
    fn config_file_change_detects_area() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["deploy/configs/consumer.jsonc".into()]);

        assert!(
            report
                .inferences
                .iter()
                .any(|i| i.category == "affected-area" && i.message.contains("config")),
            "config file changes should detect config area"
        );
    }

    // ── Large changes ─────────────────────────────────────────────────────

    #[test]
    fn large_change_set_scope_detected() {
        let files: Vec<String> = (0..20)
            .map(|i| format!("internal/domain/configctl/file{i}.go"))
            .chain((0..5).map(|i| format!("internal/adapters/nats/file{i}.go")))
            .chain((0..5).map(|i| format!("internal/actors/scopes/configctl/file{i}.go")))
            .chain((0..5).map(|i| format!("deploy/configs/config{i}.jsonc")))
            .collect();

        let scope = classify_change_scope(&files);
        assert_eq!(scope, "large", "35 files across 4 areas = large scope");
    }

    // ── Rendering ─────────────────────────────────────────────────────────

    #[test]
    fn json_output_is_valid() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["input"]["changed_files"].is_array());
        assert!(parsed["facts"].is_array());
        assert!(parsed["inferences"].is_array());
        assert!(parsed["recommendations"].is_array());
        assert!(parsed["smoke_scenarios"].is_array());
        assert!(parsed["gate_profile"]["profile"].is_string());
        assert!(parsed["priority_areas"].is_array());
        assert!(parsed["risks"].is_array());
        assert!(parsed["commands"]["before"].is_array());
        assert!(parsed["commands"]["after"].is_array());
        assert!(parsed["scope_note"].is_string());
    }

    #[test]
    fn human_output_contains_key_sections() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        let human = render_human(&report, false);
        assert!(human.contains("Recommend: Smoke/TDD Priorities"));
        assert!(human.contains("Smoke scenarios to run"));
        assert!(human.contains("Quality-gate"));
        assert!(human.contains("BEFORE"));
        assert!(human.contains("AFTER"));
        assert!(human.contains("Scope:"));
    }

    #[test]
    fn human_output_empty_is_helpful() {
        let report = RecommendReport::empty();
        let human = render_human(&report, false);
        assert!(human.contains("No changed files"));
        assert!(human.contains("Usage:"));
    }

    #[test]
    fn verbose_shows_file_list() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/adapters/nats/codec.go".into()]);

        let verbose = render_human(&report, true);
        let terse = render_human(&report, false);

        assert!(
            verbose.contains("internal/adapters/nats/codec.go"),
            "verbose should list files"
        );
        // Terse may or may not list files depending on section, that's ok
        let _ = terse;
    }

    // ── Provenance tagging ────────────────────────────────────────────────

    #[test]
    fn facts_are_tagged_fact() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        for fact in &report.facts {
            assert_eq!(fact.tag, "fact", "all facts should be tagged 'fact'");
        }
    }

    #[test]
    fn inferences_are_tagged_inference() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        for inf in &report.inferences {
            assert_eq!(
                inf.tag, "inference",
                "all inferences should be tagged 'inference'"
            );
        }
    }

    // ── Change scope classification ───────────────────────────────────────

    #[test]
    fn single_file_scope() {
        assert_eq!(
            classify_change_scope(&["internal/domain/configctl/config.go".into()]),
            "single-file"
        );
    }

    #[test]
    fn localized_scope() {
        assert_eq!(
            classify_change_scope(&[
                "internal/domain/configctl/config.go".into(),
                "internal/domain/configctl/version.go".into(),
            ]),
            "localized"
        );
    }

    #[test]
    fn cross_layer_scope() {
        assert_eq!(
            classify_change_scope(&[
                "internal/domain/configctl/config.go".into(),
                "internal/adapters/nats/codec.go".into(),
            ]),
            "cross-layer"
        );
    }

    #[test]
    fn empty_scope() {
        assert_eq!(classify_change_scope(&[]), "none");
    }
}
