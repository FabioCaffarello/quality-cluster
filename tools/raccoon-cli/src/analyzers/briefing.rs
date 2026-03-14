//! Briefing — concise, auditable context for agents and developers.
//!
//! Composes data from existing analyzers (impact-map, arch-guard, contract-audit,
//! tdd, symbol-trace) into a short, dense briefing suitable for pasting into
//! agent context or reading during development.
//!
//! ## Design
//!
//! The briefing answers: "What do I need to know about this area before acting?"
//!
//! Every item is tagged with provenance:
//! - `[fact]`           — observed from AST, config, or source
//! - `[inferred]`       — derived from heuristics or structural patterns
//! - `[recommendation]` — actionable suggestion based on facts/inferences
//! - `[lsp]`            — enriched via gopls (when available)
//!
//! ## Composition
//!
//! 1. Resolve targets → files/packages/symbols
//! 2. Run impact-map on resolved targets
//! 3. Run arch-guard scoped to affected packages
//! 4. Run contract-audit scoped to affected contracts
//! 5. Aggregate TDD guidance for recommended checks
//! 6. (Optional) symbol-trace for symbol targets
//!
//! The briefing intentionally excludes runtime checks (smoke tests, results-inspect)
//! since those require live infrastructure.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::analyzers::{arch_guard, contracts, impact_map, symbol_trace, tdd};
use crate::lsp::bridge::GoplsBridge;

// ── Public API ─────────────────────────────────────────────────────────────

/// Generate a briefing for the given targets (AST only).
pub fn analyze(project_root: &Path, targets: &[String]) -> BriefingReport {
    if targets.is_empty() {
        return BriefingReport::empty();
    }

    let impact = impact_map::analyze(project_root, targets);
    let tdd_report = tdd::analyze(project_root, targets);

    // Run arch-guard and contract-audit for project-wide context, then scope findings.
    let arch_findings = collect_arch_findings(project_root, &impact);
    let contract_findings = collect_contract_findings(project_root, &impact);

    // If any target looks like a symbol (PascalCase, no path separators), trace it.
    let symbol_summaries = collect_symbol_summaries(project_root, targets);

    build_report(targets, &impact, &tdd_report, arch_findings, contract_findings, symbol_summaries, None)
}

/// Generate a briefing with optional LSP enrichment.
pub fn analyze_with_lsp(
    project_root: &Path,
    targets: &[String],
    bridge: &mut GoplsBridge,
) -> BriefingReport {
    if targets.is_empty() {
        return BriefingReport::empty();
    }

    let impact = impact_map::analyze_with_lsp(project_root, targets, bridge);
    let tdd_report = tdd::analyze(project_root, targets);

    let arch_findings = collect_arch_findings(project_root, &impact);
    let contract_findings = collect_contract_findings(project_root, &impact);
    let symbol_summaries = collect_symbol_summaries_with_lsp(project_root, targets, bridge);

    let lsp_note = if impact.lsp_enrichment.is_some() {
        Some("LSP enrichment active — references include function body call sites".to_string())
    } else {
        None
    };

    build_report(targets, &impact, &tdd_report, arch_findings, contract_findings, symbol_summaries, lsp_note)
}

// ── Report types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BriefingReport {
    pub targets: Vec<String>,
    pub facts: Vec<BriefingItem>,
    pub inferences: Vec<BriefingItem>,
    pub recommendations: Vec<BriefingItem>,
    pub checks_to_run: Vec<String>,
    pub sensitive_areas: Vec<String>,
    pub scope_note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp_note: Option<String>,
}

impl BriefingReport {
    fn empty() -> Self {
        BriefingReport {
            targets: vec![],
            facts: vec![],
            inferences: vec![],
            recommendations: vec!["Provide targets: file paths, package dirs, or symbol names.".into()].into_iter().map(|m| BriefingItem { category: "usage".into(), message: m, location: None }).collect(),
            checks_to_run: vec![],
            sensitive_areas: vec![],
            scope_note: "No targets provided.".into(),
            lsp_note: None,
        }
    }
}

/// A single briefing item with provenance baked into the section it belongs to.
#[derive(Debug, Clone, Serialize)]
pub struct BriefingItem {
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

// ── Internals ──────────────────────────────────────────────────────────────

fn build_report(
    targets: &[String],
    impact: &impact_map::ImpactReport,
    tdd_report: &tdd::TddReport,
    arch_findings: Vec<BriefingItem>,
    contract_findings: Vec<BriefingItem>,
    symbol_summaries: Vec<BriefingItem>,
    lsp_note: Option<String>,
) -> BriefingReport {
    let mut facts: Vec<BriefingItem> = Vec::new();
    let mut inferences: Vec<BriefingItem> = Vec::new();
    let mut recommendations: Vec<BriefingItem> = Vec::new();

    // ── Facts from impact analysis ──

    for imp in &impact.impacts {
        // Package resolution
        if let Some(ref pkg) = imp.resolved_package {
            facts.push(BriefingItem {
                category: "scope".into(),
                message: format!("{} resolves to package {}", imp.target, pkg),
                location: None,
            });
        }

        // Exported symbols count
        if !imp.exported_symbols.is_empty() {
            let names: Vec<&str> = imp.exported_symbols.iter().take(5).map(|s| s.name.as_str()).collect();
            let suffix = if imp.exported_symbols.len() > 5 {
                format!(" (+{} more)", imp.exported_symbols.len() - 5)
            } else {
                String::new()
            };
            facts.push(BriefingItem {
                category: "symbols".into(),
                message: format!("{} exports: {}{}", imp.target, names.join(", "), suffix),
                location: None,
            });
        }

        // Direct dependents
        if !imp.direct_dependents.is_empty() {
            let dep_names: Vec<&str> = imp.direct_dependents.iter().take(5).map(|d| d.package_dir.as_str()).collect();
            let suffix = if imp.direct_dependents.len() > 5 {
                format!(" (+{} more)", imp.direct_dependents.len() - 5)
            } else {
                String::new()
            };
            facts.push(BriefingItem {
                category: "dependents".into(),
                message: format!("{} has {} direct dependents: {}{}", imp.target, imp.direct_dependents.len(), dep_names.join(", "), suffix),
                location: None,
            });
        }

        // Contract surface
        for item in &imp.contract_surface {
            facts.push(BriefingItem {
                category: "contracts".into(),
                message: format!("{} [{}] — {}", item.name, item.kind, item.why),
                location: Some(item.location.clone()),
            });
        }

        // Sensitive areas
        for area in &imp.sensitive_areas {
            // These are inferences (pattern matching on file paths)
            inferences.push(BriefingItem {
                category: "sensitive-area".into(),
                message: format!("{} — {}", area.name, area.description),
                location: None,
            });
        }

        // Risks from impact analysis
        for risk in &imp.risks {
            inferences.push(BriefingItem {
                category: "risk".into(),
                message: format!("[{}] {}", risk.basis, risk.description),
                location: None,
            });
        }
    }

    // ── Symbol summaries ──
    for item in symbol_summaries {
        facts.push(item);
    }

    // ── Arch findings ──
    for item in arch_findings {
        // Arch violations are facts (observed from AST)
        facts.push(item);
    }

    // ── Contract findings ──
    for item in contract_findings {
        facts.push(item);
    }

    // ── Recommendations from TDD ──

    // Scenarios
    for scenario in &tdd_report.recommended_scenarios {
        recommendations.push(BriefingItem {
            category: "scenario".into(),
            message: format!("{} — {}", scenario.name, scenario.why),
            location: None,
        });
    }

    // Coverage gaps
    for gap in &tdd_report.coverage_gaps {
        recommendations.push(BriefingItem {
            category: "coverage-gap".into(),
            message: gap.description.clone(),
            location: None,
        });
    }

    // Before/after commands
    for cmd in &tdd_report.before_commands {
        recommendations.push(BriefingItem {
            category: "before-change".into(),
            message: cmd.clone(),
            location: None,
        });
    }
    for cmd in &tdd_report.after_commands {
        recommendations.push(BriefingItem {
            category: "after-change".into(),
            message: cmd.clone(),
            location: None,
        });
    }

    // Gate profile recommendation
    if tdd_report.recommended_profile != "fast" {
        recommendations.push(BriefingItem {
            category: "quality-gate".into(),
            message: format!("Use profile '{}' (elevated due to scope)", tdd_report.recommended_profile),
            location: None,
        });
    }

    // ── Aggregate checks and areas ──

    let mut checks: BTreeSet<String> = BTreeSet::new();
    for cmd in &impact.recommended_commands {
        checks.insert(cmd.clone());
    }
    for cmd in &tdd_report.before_commands {
        if cmd.starts_with("raccoon-cli") || cmd.starts_with("make") {
            checks.insert(cmd.clone());
        }
    }
    for cmd in &tdd_report.after_commands {
        if cmd.starts_with("raccoon-cli") || cmd.starts_with("make") {
            checks.insert(cmd.clone());
        }
    }

    let scope_note = format!(
        "Briefing from static analysis (AST + file patterns). {}. \
         No call graph, type resolution, or runtime tracing unless --lsp is used.",
        if lsp_note.is_some() { "LSP enrichment active" } else { "No LSP enrichment" }
    );

    // Deduplicate inferences
    dedup_items(&mut facts);
    dedup_items(&mut inferences);
    dedup_items(&mut recommendations);

    BriefingReport {
        targets: targets.to_vec(),
        facts,
        inferences,
        recommendations,
        checks_to_run: checks.into_iter().collect(),
        sensitive_areas: impact.sensitive_areas_touched.clone(),
        scope_note,
        lsp_note,
    }
}

fn dedup_items(items: &mut Vec<BriefingItem>) {
    let mut seen = BTreeSet::new();
    items.retain(|item| seen.insert(format!("{}:{}", item.category, item.message)));
}

fn collect_arch_findings(
    project_root: &Path,
    impact: &impact_map::ImpactReport,
) -> Vec<BriefingItem> {
    let mut items = Vec::new();

    // Only run arch-guard if there are resolved packages
    let has_packages = impact.impacts.iter().any(|i| i.resolved_package.is_some());
    if !has_packages {
        return items;
    }

    let affected_packages: BTreeSet<&str> = impact
        .impacts
        .iter()
        .filter_map(|i| i.resolved_package.as_deref())
        .collect();

    match arch_guard::analyze(project_root) {
        Ok(report) => {
            for check in &report.checks {
                for finding in &check.findings {
                    if finding.severity == crate::models::Severity::Error {
                        // Scope: only include findings that mention an affected package
                        let loc = finding.location.as_deref().unwrap_or("");
                        let is_scoped = affected_packages.iter().any(|pkg| loc.contains(pkg));
                        if is_scoped {
                            items.push(BriefingItem {
                                category: "arch-violation".into(),
                                message: format!("{}: {}", finding.check, finding.message),
                                location: finding.location.clone(),
                            });
                        }
                    }
                }
            }
        }
        Err(_) => {
            // arch-guard couldn't run; skip silently
        }
    }

    items
}

fn collect_contract_findings(
    project_root: &Path,
    impact: &impact_map::ImpactReport,
) -> Vec<BriefingItem> {
    let mut items = Vec::new();

    // Only run contract-audit if there are contract surface items
    let has_contracts = impact
        .impacts
        .iter()
        .any(|i| !i.contract_surface.is_empty());
    if !has_contracts {
        return items;
    }

    match contracts::analyze(project_root) {
        Ok(report) => {
            for check in &report.checks {
                if check.status == crate::models::CheckStatus::Fail {
                    for finding in &check.findings {
                        if finding.severity == crate::models::Severity::Error {
                            items.push(BriefingItem {
                                category: "contract-issue".into(),
                                message: format!("{}: {}", finding.check, finding.message),
                                location: finding.location.clone(),
                            });
                        }
                    }
                }
            }
        }
        Err(_) => {
            // contract-audit couldn't run; skip silently
        }
    }

    items
}

fn looks_like_symbol(target: &str) -> bool {
    // A symbol target: no path separators, starts with uppercase (PascalCase), no dots
    !target.contains('/')
        && !target.contains('.')
        && !target.is_empty()
        && target.chars().next().map_or(false, |c| c.is_uppercase())
}

fn collect_symbol_summaries(project_root: &Path, targets: &[String]) -> Vec<BriefingItem> {
    let mut items = Vec::new();

    for target in targets {
        if !looks_like_symbol(target) {
            continue;
        }

        let report = symbol_trace::trace(project_root, target);
        if report.status == symbol_trace::ResolutionStatus::NotFound {
            continue;
        }

        // Definitions
        for def in &report.definitions {
            items.push(BriefingItem {
                category: "symbol-definition".into(),
                message: format!(
                    "{} defined as {} [{}] ({})",
                    target, def.kind, def.name, def.visibility
                ),
                location: Some(format!("{}:{}", def.file, def.line)),
            });
        }

        // Contract connections
        for conn in &report.contracts {
            items.push(BriefingItem {
                category: "symbol-contract".into(),
                message: format!("{}: {}", conn.kind, conn.why),
                location: Some(format!("{}:{}", conn.file, conn.line)),
            });
        }
    }

    items
}

fn collect_symbol_summaries_with_lsp(
    project_root: &Path,
    targets: &[String],
    bridge: &mut GoplsBridge,
) -> Vec<BriefingItem> {
    let mut items = Vec::new();

    for target in targets {
        if !looks_like_symbol(target) {
            continue;
        }

        let report = symbol_trace::trace_with_lsp(project_root, target, bridge);
        if report.status == symbol_trace::ResolutionStatus::NotFound {
            continue;
        }

        for def in &report.definitions {
            items.push(BriefingItem {
                category: "symbol-definition".into(),
                message: format!(
                    "{} defined as {} [{}] ({})",
                    target, def.kind, def.name, def.visibility
                ),
                location: Some(format!("{}:{}", def.file, def.line)),
            });
        }

        for conn in &report.contracts {
            items.push(BriefingItem {
                category: "symbol-contract".into(),
                message: format!("{}: {}", conn.kind, conn.why),
                location: Some(format!("{}:{}", conn.file, conn.line)),
            });
        }

        // LSP-enriched reference count
        if let Some(ref lsp) = report.lsp_enrichment {
            if !lsp.references.is_empty() {
                items.push(BriefingItem {
                    category: "symbol-refs".into(),
                    message: format!(
                        "{} has {} cross-package references [lsp]",
                        target,
                        lsp.references.len()
                    ),
                    location: None,
                });
            }
        }
    }

    items
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_json(report: &BriefingReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_human(report: &BriefingReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== Briefing ===\n").unwrap();

    if report.targets.is_empty() {
        writeln!(out, "No targets provided.\n").unwrap();
        writeln!(out, "Usage: raccoon-cli briefing <target> [target2] ...").unwrap();
        writeln!(out, "  Targets: file paths, package dirs, or symbol names (PascalCase).\n").unwrap();
        return out;
    }

    writeln!(out, "Targets: {}\n", report.targets.join(", ")).unwrap();

    // Facts
    if !report.facts.is_empty() {
        writeln!(out, "Facts:").unwrap();
        let limit = if verbose { report.facts.len() } else { 15 };
        for item in report.facts.iter().take(limit) {
            if let Some(ref loc) = item.location {
                writeln!(out, "  [fact][{}] {} ({})", item.category, item.message, loc).unwrap();
            } else {
                writeln!(out, "  [fact][{}] {}", item.category, item.message).unwrap();
            }
        }
        if !verbose && report.facts.len() > 15 {
            writeln!(out, "  ... +{} more (use --verbose)", report.facts.len() - 15).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Inferences
    if !report.inferences.is_empty() {
        writeln!(out, "Inferences:").unwrap();
        let limit = if verbose { report.inferences.len() } else { 10 };
        for item in report.inferences.iter().take(limit) {
            writeln!(out, "  [inferred][{}] {}", item.category, item.message).unwrap();
        }
        if !verbose && report.inferences.len() > 10 {
            writeln!(out, "  ... +{} more (use --verbose)", report.inferences.len() - 10).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Recommendations
    if !report.recommendations.is_empty() {
        writeln!(out, "Recommendations:").unwrap();
        for item in &report.recommendations {
            if let Some(ref loc) = item.location {
                writeln!(out, "  [recommendation][{}] {} ({})", item.category, item.message, loc).unwrap();
            } else {
                writeln!(out, "  [recommendation][{}] {}", item.category, item.message).unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    // Sensitive areas
    if !report.sensitive_areas.is_empty() {
        writeln!(out, "Sensitive areas: {}\n", report.sensitive_areas.join(", ")).unwrap();
    }

    // Checks to run
    if !report.checks_to_run.is_empty() {
        writeln!(out, "Checks to run:").unwrap();
        for cmd in &report.checks_to_run {
            writeln!(out, "  {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // LSP note
    if let Some(ref note) = report.lsp_note {
        writeln!(out, "LSP: {note}\n").unwrap();
    }

    // Scope note
    writeln!(out, "Scope: {}", report.scope_note).unwrap();

    out
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_go_project(dir: &Path) {
        std::fs::write(dir.join("go.work"), "go 1.23\n").unwrap();
        std::fs::create_dir_all(dir.join("internal/domain/configctl")).unwrap();
        std::fs::create_dir_all(dir.join("internal/application/ports")).unwrap();
        std::fs::create_dir_all(dir.join("deploy/configs")).unwrap();
        std::fs::create_dir_all(dir.join("deploy/compose")).unwrap();
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        std::fs::create_dir_all(dir.join("tools")).unwrap();

        // Minimal Go file for codeintel to index
        std::fs::write(
            dir.join("go.mod"),
            "module github.com/example/quality-service\n\ngo 1.23\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("internal/domain/configctl/config.go"),
            r#"package configctl

// ConfigSet is the root aggregate for configuration management.
type ConfigSet struct {
    ID   string
    Name string
}

// Validate checks invariants.
func (c *ConfigSet) Validate() error {
    return nil
}
"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("internal/application/ports/configctl.go"),
            r#"package ports

import "github.com/example/quality-service/internal/domain/configctl"

// ConfigctlGateway is the port for config management.
type ConfigctlGateway interface {
    Get(id string) (*configctl.ConfigSet, error)
}
"#,
        )
        .unwrap();
    }

    #[test]
    fn empty_targets_returns_empty_report() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(dir.path(), &[]);
        assert!(report.targets.is_empty());
        assert!(!report.recommendations.is_empty()); // usage hint
        assert_eq!(report.scope_note, "No targets provided.");
    }

    #[test]
    fn briefing_for_file_target() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(
            dir.path(),
            &["internal/domain/configctl/config.go".into()],
        );
        assert_eq!(report.targets.len(), 1);
        // Should have at least some facts (package resolution, exported symbols)
        assert!(!report.facts.is_empty(), "file target should produce facts");
    }

    #[test]
    fn briefing_for_symbol_target() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(dir.path(), &["ConfigSet".into()]);
        assert_eq!(report.targets.len(), 1);
        // Should find the symbol definition
        let has_def = report
            .facts
            .iter()
            .any(|f| f.category == "symbol-definition" && f.message.contains("ConfigSet"));
        assert!(has_def, "should find ConfigSet definition in facts");
    }

    #[test]
    fn briefing_for_package_target() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(
            dir.path(),
            &["internal/domain/configctl".into()],
        );
        assert_eq!(report.targets.len(), 1);
    }

    #[test]
    fn briefing_multiple_targets() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(
            dir.path(),
            &[
                "internal/domain/configctl/config.go".into(),
                "ConfigSet".into(),
            ],
        );
        assert_eq!(report.targets.len(), 2);
    }

    #[test]
    fn briefing_unresolved_target_still_works() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(dir.path(), &["nonexistent/path.go".into()]);
        // Should not panic, should produce a report
        assert_eq!(report.targets.len(), 1);
    }

    #[test]
    fn briefing_json_roundtrip() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(
            dir.path(),
            &["internal/domain/configctl/config.go".into()],
        );
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["targets"].is_array());
        assert!(parsed["facts"].is_array());
        assert!(parsed["inferences"].is_array());
        assert!(parsed["recommendations"].is_array());
        assert!(parsed["scope_note"].is_string());
    }

    #[test]
    fn human_output_contains_sections() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(
            dir.path(),
            &["internal/domain/configctl/config.go".into()],
        );
        let human = render_human(&report, false);
        assert!(human.contains("=== Briefing ==="));
        assert!(human.contains("Targets:"));
        assert!(human.contains("[fact]"));
        assert!(human.contains("Scope:"));
    }

    #[test]
    fn empty_report_human_output() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(dir.path(), &[]);
        let human = render_human(&report, false);
        assert!(human.contains("No targets provided"));
    }

    #[test]
    fn looks_like_symbol_detection() {
        assert!(looks_like_symbol("ConfigSet"));
        assert!(looks_like_symbol("Validate"));
        assert!(!looks_like_symbol("internal/domain/configctl"));
        assert!(!looks_like_symbol("config.go"));
        assert!(!looks_like_symbol("lowercase"));
        assert!(!looks_like_symbol(""));
    }

    #[test]
    fn dedup_removes_duplicates() {
        let mut items = vec![
            BriefingItem { category: "a".into(), message: "same".into(), location: None },
            BriefingItem { category: "a".into(), message: "same".into(), location: Some("loc".into()) },
            BriefingItem { category: "a".into(), message: "different".into(), location: None },
        ];
        dedup_items(&mut items);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn verbose_shows_more_facts() {
        let dir = TempDir::new().unwrap();
        make_go_project(dir.path());
        let report = analyze(
            dir.path(),
            &["internal/domain/configctl/config.go".into()],
        );
        let terse = render_human(&report, false);
        let verbose = render_human(&report, true);
        // verbose should be at least as long
        assert!(verbose.len() >= terse.len());
    }
}
