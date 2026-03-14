//! Rename safety checks — evaluate the structural and semantic risk of renaming a symbol.
//!
//! Uses the codeintel AST index to locate definitions, structural references,
//! contract connections, and sensitive areas. Optionally enriches with gopls.
//!
//! ## What it does
//!
//! Given a symbol name (and optional new name), the checker:
//! 1. Resolves the symbol to definitions (types, functions, constants, variables)
//! 2. Collects all structural references (struct fields, params, returns, receivers, etc.)
//! 3. Optionally collects LSP references (function body call sites, cross-package)
//! 4. Identifies sensitive areas touched (domain, ports, contracts, adapters, actors)
//! 5. Detects contract surface involvement (interfaces, message types, ports)
//! 6. Assesses overall risk level (low / medium / high / critical)
//! 7. Suggests smoke scenarios and a quality-gate profile
//!
//! ## What it does NOT do
//!
//! - Does NOT perform the rename — this is assessment only
//! - No runtime or reflection analysis
//! - AST-only mode misses function body references (use --lsp for full coverage)
//!
//! All observations are labeled with provenance: "observed" (AST), "lsp" (gopls),
//! or "inferred" (heuristic recommendation).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::codeintel::{self, GoFunc, ProjectIndex, TypeKind, Visibility};
use crate::lsp::bridge::GoplsBridge;
use crate::lsp::types::{LspReference, LspStatus};

// ── Public API ─────────────────────────────────────────────────────────────

/// Run rename safety checks (AST only).
pub fn check(project_root: &Path, symbol: &str, new_name: Option<&str>) -> RenameSafetyReport {
    let index = codeintel::build_index(project_root);
    check_in_index(&index, symbol, new_name)
}

/// Run rename safety checks with optional LSP enrichment.
pub fn check_with_lsp(
    project_root: &Path,
    symbol: &str,
    new_name: Option<&str>,
    bridge: &mut GoplsBridge,
) -> RenameSafetyReport {
    let index = codeintel::build_index(project_root);
    let mut report = check_in_index(&index, symbol, new_name);

    let enriched = bridge.enrich_symbol_with_index(&index, project_root, symbol);
    let lsp_status = enriched.lsp_status.clone();

    // Collect LSP references not already in AST references.
    let lsp_only_refs: Vec<LspReference> = enriched
        .lsp_references
        .into_iter()
        .filter(|lr| {
            !report.affected_references.iter().any(|r| {
                r.file == lr.location.file && r.line == lr.location.line
            })
        })
        .collect();

    // Add LSP-discovered packages.
    for lr in &lsp_only_refs {
        let pkg = file_to_package(&lr.location.file);
        if !pkg.is_empty() {
            report.affected_packages.insert(pkg);
        }
    }

    let lsp_ref_count = lsp_only_refs.len();

    report.lsp_enrichment = Some(LspEnrichment {
        status: lsp_status,
        additional_references: lsp_only_refs,
    });

    // Recalculate risk with LSP data.
    let total_refs = report.affected_references.len() + lsp_ref_count;
    report.risk_assessment = assess_risk(
        &report.definitions,
        total_refs,
        &report.sensitive_areas,
        &report.contract_surface,
    );
    report.recommended_gate_profile = recommend_gate_profile(&report.risk_assessment);
    report.suggested_smoke_scenarios = suggest_smoke_scenarios(
        &report.sensitive_areas,
        &report.contract_surface,
    );

    // Update scope note.
    match &report.lsp_enrichment.as_ref().unwrap().status {
        LspStatus::Enriched => {
            report.scope_note = format!(
                "Analysis combines structural AST indexing with gopls semantic references. \
                 {} additional call-site references found via LSP. Each fact is tagged with \
                 its source.",
                lsp_ref_count
            );
        }
        LspStatus::NoResults => {
            report.scope_note = "Analysis is based on structural AST indexing. gopls was \
                available but returned no additional references.".to_string();
        }
        LspStatus::Unavailable { reason } => {
            report.scope_note = format!(
                "Analysis is based on structural AST indexing (declarations, signatures, \
                 struct fields). LSP enrichment unavailable: {reason}. Function body call \
                 sites and cross-package type resolution are not visible — actual blast \
                 radius may be larger."
            );
        }
    }

    report
}

/// Check rename safety using an existing index (useful for testing).
fn check_in_index(
    index: &ProjectIndex,
    symbol: &str,
    new_name: Option<&str>,
) -> RenameSafetyReport {
    let definitions = find_definitions(index, symbol);
    let affected_references = find_references(index, symbol);

    // Collect packages.
    let mut pkg_set: BTreeSet<String> = BTreeSet::new();
    for def in &definitions {
        pkg_set.insert(def.package.clone());
    }
    for r in &affected_references {
        pkg_set.insert(r.package.clone());
    }

    // Detect sensitive areas.
    let sensitive_areas = detect_sensitive_areas(&definitions, &affected_references);

    // Detect contract surface.
    let contract_surface = find_contract_surface(index, symbol, &definitions);

    // Check new-name conflicts.
    let new_name_conflicts = if let Some(nn) = new_name {
        find_conflicts(index, nn)
    } else {
        vec![]
    };

    // Assess risk.
    let risk_assessment = assess_risk(
        &definitions,
        affected_references.len(),
        &sensitive_areas,
        &contract_surface,
    );

    let recommended_gate_profile = recommend_gate_profile(&risk_assessment);
    let suggested_smoke_scenarios = suggest_smoke_scenarios(&sensitive_areas, &contract_surface);
    let recommended_checks = build_recommended_checks(&sensitive_areas, &contract_surface);

    // Resolution status.
    let status = if definitions.is_empty() {
        ResolutionStatus::NotFound
    } else if definitions.len() > 1 {
        ResolutionStatus::Ambiguous
    } else {
        ResolutionStatus::Resolved
    };

    let scope_note = "Analysis is based on structural AST indexing (declarations, signatures, \
        struct fields, type expressions). Function bodies are not analyzed — call sites, \
        assignments, and runtime usage are not visible. Use --lsp for deeper coverage."
        .to_string();

    RenameSafetyReport {
        symbol: symbol.to_string(),
        new_name: new_name.map(|s| s.to_string()),
        status,
        definitions,
        affected_references,
        affected_packages: pkg_set,
        sensitive_areas,
        contract_surface,
        new_name_conflicts,
        risk_assessment,
        recommended_gate_profile,
        suggested_smoke_scenarios,
        recommended_checks,
        scope_note,
        lsp_enrichment: None,
    }
}

// ── Report types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RenameSafetyReport {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>,
    pub status: ResolutionStatus,
    pub definitions: Vec<Definition>,
    pub affected_references: Vec<AffectedReference>,
    pub affected_packages: BTreeSet<String>,
    pub sensitive_areas: Vec<SensitiveArea>,
    pub contract_surface: Vec<ContractInvolvement>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_name_conflicts: Vec<Conflict>,
    pub risk_assessment: RiskAssessment,
    pub recommended_gate_profile: String,
    pub suggested_smoke_scenarios: Vec<String>,
    pub recommended_checks: Vec<String>,
    pub scope_note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp_enrichment: Option<LspEnrichment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Resolved,
    Ambiguous,
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub name: String,
    pub kind: String,
    pub package: String,
    pub file: String,
    pub line: usize,
    pub visibility: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedReference {
    pub kind: String,
    pub context: String,
    pub package: String,
    pub file: String,
    pub line: usize,
    /// "observed" (AST fact).
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SensitiveArea {
    pub area: String,
    pub files: Vec<String>,
    /// "observed" — derived from file paths of definitions/references.
    pub basis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractInvolvement {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub why: String,
    /// "observed" or "inferred".
    pub basis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub name: String,
    pub kind: String,
    pub package: String,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub reasons: Vec<RiskReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskReason {
    pub factor: String,
    pub detail: String,
    /// "observed" or "inferred".
    pub basis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspEnrichment {
    pub status: LspStatus,
    pub additional_references: Vec<LspReference>,
}

// ── Definition finding ─────────────────────────────────────────────────────

fn find_definitions(index: &ProjectIndex, symbol: &str) -> Vec<Definition> {
    let mut defs = Vec::new();

    for file in &index.files {
        let pkg = file.package.clone();

        for t in &file.types {
            if t.name == symbol {
                let kind = match &t.kind {
                    TypeKind::Struct { .. } => "struct",
                    TypeKind::Interface { .. } => "interface",
                    TypeKind::Alias { .. } => "type_alias",
                };
                let details = type_details(&t.kind);
                defs.push(Definition {
                    name: t.name.clone(),
                    kind: kind.into(),
                    package: pkg.clone(),
                    file: t.location.file.clone(),
                    line: t.location.line,
                    visibility: visibility_label(t.visibility),
                    details,
                });
            }
        }

        for f in &file.functions {
            if f.name == symbol {
                let (kind, details) = func_info(f);
                defs.push(Definition {
                    name: f.name.clone(),
                    kind,
                    package: pkg.clone(),
                    file: f.location.file.clone(),
                    line: f.location.line,
                    visibility: visibility_label(f.visibility),
                    details,
                });
            }
        }

        for c in &file.constants {
            if c.name == symbol {
                let mut details = Vec::new();
                if let Some(ref th) = c.type_hint {
                    details.push(format!("type: {th}"));
                }
                if let Some(ref v) = c.value {
                    details.push(format!("value: {v}"));
                }
                defs.push(Definition {
                    name: c.name.clone(),
                    kind: "const".into(),
                    package: pkg.clone(),
                    file: c.location.file.clone(),
                    line: c.location.line,
                    visibility: visibility_label(c.visibility),
                    details,
                });
            }
        }

        for v in &file.variables {
            if v.name == symbol {
                let mut details = Vec::new();
                if let Some(ref th) = v.type_hint {
                    details.push(format!("type: {th}"));
                }
                defs.push(Definition {
                    name: v.name.clone(),
                    kind: "var".into(),
                    package: pkg.clone(),
                    file: v.location.file.clone(),
                    line: v.location.line,
                    visibility: visibility_label(v.visibility),
                    details,
                });
            }
        }
    }

    defs
}

// ── Reference finding ──────────────────────────────────────────────────────

fn find_references(index: &ProjectIndex, symbol: &str) -> Vec<AffectedReference> {
    let mut refs = Vec::new();

    for file in &index.files {
        let pkg = file.package.clone();

        for t in &file.types {
            if let TypeKind::Struct { ref fields } = t.kind {
                for field in fields {
                    if field.embedded && field.type_expr == symbol {
                        refs.push(AffectedReference {
                            kind: "embedded_field".into(),
                            context: t.name.clone(),
                            package: pkg.clone(),
                            file: field.location.file.clone(),
                            line: field.location.line,
                            basis: "observed".into(),
                        });
                    } else if type_expr_mentions(&field.type_expr, symbol) {
                        refs.push(AffectedReference {
                            kind: "field_type".into(),
                            context: format!("{}.{}", t.name, field.name),
                            package: pkg.clone(),
                            file: field.location.file.clone(),
                            line: field.location.line,
                            basis: "observed".into(),
                        });
                    }
                }
            }

            if let TypeKind::Interface { ref embeds, .. } = t.kind {
                for embed in embeds {
                    if embed.type_name == symbol {
                        refs.push(AffectedReference {
                            kind: "interface_embed".into(),
                            context: t.name.clone(),
                            package: pkg.clone(),
                            file: embed.location.file.clone(),
                            line: embed.location.line,
                            basis: "observed".into(),
                        });
                    }
                }
            }

            if let TypeKind::Alias { ref underlying } = t.kind {
                if type_expr_mentions(underlying, symbol) {
                    refs.push(AffectedReference {
                        kind: "alias_underlying".into(),
                        context: t.name.clone(),
                        package: pkg.clone(),
                        file: t.location.file.clone(),
                        line: t.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }

        for f in &file.functions {
            if let Some(ref recv) = f.receiver {
                if recv.type_name == symbol {
                    refs.push(AffectedReference {
                        kind: "receiver".into(),
                        context: f.name.clone(),
                        package: pkg.clone(),
                        file: f.location.file.clone(),
                        line: f.location.line,
                        basis: "observed".into(),
                    });
                }
            }

            for p in &f.params {
                if type_expr_mentions(&p.type_expr, symbol) {
                    refs.push(AffectedReference {
                        kind: "param_type".into(),
                        context: f.name.clone(),
                        package: pkg.clone(),
                        file: f.location.file.clone(),
                        line: f.location.line,
                        basis: "observed".into(),
                    });
                }
            }

            for r in &f.returns {
                if type_expr_mentions(&r.type_expr, symbol) {
                    refs.push(AffectedReference {
                        kind: "return_type".into(),
                        context: f.name.clone(),
                        package: pkg.clone(),
                        file: f.location.file.clone(),
                        line: f.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }

        for c in &file.constants {
            if let Some(ref th) = c.type_hint {
                if th == symbol {
                    refs.push(AffectedReference {
                        kind: "const_type".into(),
                        context: c.name.clone(),
                        package: pkg.clone(),
                        file: c.location.file.clone(),
                        line: c.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }

        for v in &file.variables {
            if let Some(ref th) = v.type_hint {
                if type_expr_mentions(th, symbol) {
                    refs.push(AffectedReference {
                        kind: "var_type".into(),
                        context: v.name.clone(),
                        package: pkg.clone(),
                        file: v.location.file.clone(),
                        line: v.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }
    }

    // Remove self-definitions.
    refs.retain(|r| r.context != symbol);

    refs
}

// ── Sensitive area detection ───────────────────────────────────────────────

fn detect_sensitive_areas(
    definitions: &[Definition],
    references: &[AffectedReference],
) -> Vec<SensitiveArea> {
    let mut area_files: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();

    let all_files: Vec<&str> = definitions
        .iter()
        .map(|d| d.file.as_str())
        .chain(references.iter().map(|r| r.file.as_str()))
        .collect();

    for file in &all_files {
        if file.contains("domain/") {
            area_files.entry("domain").or_default().insert(file.to_string());
        }
        if file.contains("ports") {
            area_files.entry("ports").or_default().insert(file.to_string());
        }
        if file.contains("contracts") || file.contains("events") {
            area_files
                .entry("contracts/events")
                .or_default()
                .insert(file.to_string());
        }
        if file.contains("adapters/") {
            area_files.entry("adapters").or_default().insert(file.to_string());
        }
        if file.contains("actors/") {
            area_files.entry("actors").or_default().insert(file.to_string());
        }
        if file.contains("interfaces/http") {
            area_files
                .entry("http_interfaces")
                .or_default()
                .insert(file.to_string());
        }
        if file.contains("application/") && !file.contains("contracts") && !file.contains("ports")
        {
            area_files.entry("application").or_default().insert(file.to_string());
        }
    }

    area_files
        .into_iter()
        .map(|(area, files)| SensitiveArea {
            area: area.to_string(),
            files: files.into_iter().collect(),
            basis: "observed".into(),
        })
        .collect()
}

// ── Contract surface detection ─────────────────────────────────────────────

fn find_contract_surface(
    index: &ProjectIndex,
    symbol: &str,
    definitions: &[Definition],
) -> Vec<ContractInvolvement> {
    let mut contracts = Vec::new();

    for def in definitions {
        let is_port = def.file.contains("ports");
        let is_contract_path = def.file.contains("contracts") || def.file.contains("events");

        match def.kind.as_str() {
            "interface" => {
                let why = if is_port {
                    "port interface — renaming affects all implementations across adapters and actors"
                } else {
                    "exported interface — renaming affects all implementors"
                };
                contracts.push(ContractInvolvement {
                    name: def.name.clone(),
                    kind: if is_port { "port" } else { "interface" }.into(),
                    file: def.file.clone(),
                    line: def.line,
                    why: why.into(),
                    basis: "observed".into(),
                });
            }
            "struct" => {
                if is_message_type(&def.name, &def.file) {
                    contracts.push(ContractInvolvement {
                        name: def.name.clone(),
                        kind: "message_type".into(),
                        file: def.file.clone(),
                        line: def.line,
                        why: "message struct — renaming may break serialization across services"
                            .into(),
                        basis: "observed".into(),
                    });
                }
            }
            "type_alias" | "const" => {
                if is_contract_path {
                    contracts.push(ContractInvolvement {
                        name: def.name.clone(),
                        kind: "contract_type".into(),
                        file: def.file.clone(),
                        line: def.line,
                        why: "defined in contract/event layer — renaming affects message encoding"
                            .into(),
                        basis: "observed".into(),
                    });
                }
            }
            _ => {}
        }
    }

    // Check if the symbol is referenced by known contract interfaces.
    for file in &index.files {
        for t in &file.types {
            if t.visibility != Visibility::Exported {
                continue;
            }
            let is_contract_iface = matches!(t.kind, TypeKind::Interface { .. })
                && (t.location.file.contains("ports") || t.location.file.contains("contracts"));

            if is_contract_iface {
                if let TypeKind::Interface { ref methods, .. } = t.kind {
                    for m in methods {
                        if type_expr_mentions(&m.signature, symbol) {
                            contracts.push(ContractInvolvement {
                                name: format!("{}.{}", t.name, m.name),
                                kind: "interface_method".into(),
                                file: m.location.file.clone(),
                                line: m.location.line,
                                why: "symbol appears in contract interface method signature"
                                    .into(),
                                basis: "observed".into(),
                            });
                        }
                    }
                }
            }
        }
    }

    contracts
}

// ── Conflict detection ─────────────────────────────────────────────────────

fn find_conflicts(index: &ProjectIndex, new_name: &str) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    for file in &index.files {
        let pkg = file.package.clone();

        for t in &file.types {
            if t.name == new_name {
                conflicts.push(Conflict {
                    name: t.name.clone(),
                    kind: match &t.kind {
                        TypeKind::Struct { .. } => "struct",
                        TypeKind::Interface { .. } => "interface",
                        TypeKind::Alias { .. } => "type_alias",
                    }
                    .into(),
                    package: pkg.clone(),
                    file: t.location.file.clone(),
                    line: t.location.line,
                });
            }
        }

        for f in &file.functions {
            if f.name == new_name {
                conflicts.push(Conflict {
                    name: f.name.clone(),
                    kind: if f.receiver.is_some() { "method" } else { "func" }.into(),
                    package: pkg.clone(),
                    file: f.location.file.clone(),
                    line: f.location.line,
                });
            }
        }

        for c in &file.constants {
            if c.name == new_name {
                conflicts.push(Conflict {
                    name: c.name.clone(),
                    kind: "const".into(),
                    package: pkg.clone(),
                    file: c.location.file.clone(),
                    line: c.location.line,
                });
            }
        }

        for v in &file.variables {
            if v.name == new_name {
                conflicts.push(Conflict {
                    name: v.name.clone(),
                    kind: "var".into(),
                    package: pkg.clone(),
                    file: v.location.file.clone(),
                    line: v.location.line,
                });
            }
        }
    }

    conflicts
}

// ── Risk assessment ────────────────────────────────────────────────────────

fn assess_risk(
    definitions: &[Definition],
    total_ref_count: usize,
    sensitive_areas: &[SensitiveArea],
    contracts: &[ContractInvolvement],
) -> RiskAssessment {
    let mut reasons = Vec::new();
    let mut max_level = RiskLevel::Low;

    // Factor: visibility
    let is_exported = definitions.iter().any(|d| d.visibility == "exported");
    if is_exported {
        reasons.push(RiskReason {
            factor: "exported_symbol".into(),
            detail: "symbol is exported — external packages may depend on it".into(),
            basis: "observed".into(),
        });
        max_level = max_level.max(RiskLevel::Medium);
    }

    // Factor: reference count
    if total_ref_count > 20 {
        reasons.push(RiskReason {
            factor: "high_reference_count".into(),
            detail: format!("{total_ref_count} references found — broad blast radius"),
            basis: "observed".into(),
        });
        max_level = max_level.max(RiskLevel::High);
    } else if total_ref_count > 5 {
        reasons.push(RiskReason {
            factor: "moderate_reference_count".into(),
            detail: format!("{total_ref_count} references found"),
            basis: "observed".into(),
        });
        max_level = max_level.max(RiskLevel::Medium);
    }

    // Factor: multiple definitions (ambiguous)
    if definitions.len() > 1 {
        reasons.push(RiskReason {
            factor: "ambiguous_definitions".into(),
            detail: format!(
                "{} definitions across packages — rename may be partial or unintended",
                definitions.len()
            ),
            basis: "observed".into(),
        });
        max_level = max_level.max(RiskLevel::High);
    }

    // Factor: contract surface
    if !contracts.is_empty() {
        let has_port = contracts.iter().any(|c| c.kind == "port");
        let has_message = contracts.iter().any(|c| c.kind == "message_type");

        if has_port {
            reasons.push(RiskReason {
                factor: "port_interface".into(),
                detail: "symbol is or touches a port interface — all adapter implementations must be updated".into(),
                basis: "observed".into(),
            });
            max_level = max_level.max(RiskLevel::Critical);
        }

        if has_message {
            reasons.push(RiskReason {
                factor: "message_type".into(),
                detail: "symbol is a message type — renaming may break serialization compatibility".into(),
                basis: "observed".into(),
            });
            max_level = max_level.max(RiskLevel::Critical);
        }

        if !has_port && !has_message {
            reasons.push(RiskReason {
                factor: "contract_involvement".into(),
                detail: format!(
                    "{} contract connections — changes propagate to dependent services",
                    contracts.len()
                ),
                basis: "observed".into(),
            });
            max_level = max_level.max(RiskLevel::High);
        }
    }

    // Factor: sensitive areas
    let area_names: Vec<&str> = sensitive_areas.iter().map(|a| a.area.as_str()).collect();
    if area_names.contains(&"domain") && area_names.contains(&"adapters") {
        reasons.push(RiskReason {
            factor: "cross_layer_impact".into(),
            detail: "rename spans domain and adapter layers — high coupling risk".into(),
            basis: "inferred".into(),
        });
        max_level = max_level.max(RiskLevel::High);
    }

    if area_names.contains(&"ports") {
        reasons.push(RiskReason {
            factor: "port_layer_touched".into(),
            detail: "rename touches port definitions — interface contract boundary".into(),
            basis: "observed".into(),
        });
        max_level = max_level.max(RiskLevel::High);
    }

    // Factor: multi-package spread
    let pkg_count = definitions
        .iter()
        .map(|d| &d.package)
        .collect::<BTreeSet<_>>()
        .len();
    if pkg_count > 2 {
        reasons.push(RiskReason {
            factor: "multi_package_spread".into(),
            detail: format!("symbol defined/referenced across {pkg_count} packages"),
            basis: "observed".into(),
        });
        max_level = max_level.max(RiskLevel::Medium);
    }

    if reasons.is_empty() {
        reasons.push(RiskReason {
            factor: "contained_change".into(),
            detail: "rename appears contained with limited blast radius".into(),
            basis: "inferred".into(),
        });
    }

    RiskAssessment {
        level: max_level,
        reasons,
    }
}

fn recommend_gate_profile(risk: &RiskAssessment) -> String {
    match risk.level {
        RiskLevel::Low => "fast".into(),
        RiskLevel::Medium => "ci".into(),
        RiskLevel::High | RiskLevel::Critical => "deep".into(),
    }
}

fn suggest_smoke_scenarios(
    sensitive_areas: &[SensitiveArea],
    contracts: &[ContractInvolvement],
) -> Vec<String> {
    let mut scenarios: BTreeSet<String> = BTreeSet::new();

    let area_names: BTreeSet<&str> = sensitive_areas.iter().map(|a| a.area.as_str()).collect();

    if !contracts.is_empty() || area_names.contains("contracts/events") {
        scenarios.insert("config-lifecycle".into());
    }

    if area_names.contains("domain") || area_names.contains("application") {
        scenarios.insert("config-lifecycle".into());
    }

    if area_names.contains("adapters") || area_names.contains("actors") {
        scenarios.insert("happy-path".into());
    }

    if area_names.contains("http_interfaces") {
        scenarios.insert("readiness-probe".into());
    }

    if contracts.iter().any(|c| c.kind == "message_type") {
        scenarios.insert("invalid-payload".into());
    }

    if contracts.iter().any(|c| c.kind == "port") {
        scenarios.insert("happy-path".into());
    }

    scenarios.into_iter().collect()
}

fn build_recommended_checks(
    sensitive_areas: &[SensitiveArea],
    contracts: &[ContractInvolvement],
) -> Vec<String> {
    let mut cmds: BTreeSet<String> = BTreeSet::new();

    let area_names: BTreeSet<&str> = sensitive_areas.iter().map(|a| a.area.as_str()).collect();

    if area_names.contains("domain") || area_names.contains("application") {
        cmds.insert("raccoon-cli arch-guard".into());
    }

    if area_names.contains("ports") || area_names.contains("contracts/events") || !contracts.is_empty() {
        cmds.insert("raccoon-cli contract-audit".into());
    }

    if area_names.contains("adapters") || area_names.contains("actors") {
        cmds.insert("raccoon-cli runtime-bindings".into());
    }

    if area_names.contains("http_interfaces") {
        cmds.insert("raccoon-cli arch-guard".into());
    }

    cmds.insert("raccoon-cli drift-detect".into());

    cmds.into_iter().collect()
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_json(report: &RenameSafetyReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_human(report: &RenameSafetyReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // Header
    let rename_label = if let Some(ref nn) = report.new_name {
        format!("{} -> {}", report.symbol, nn)
    } else {
        report.symbol.clone()
    };
    writeln!(out, "=== Rename Safety Check: {rename_label} ===\n").unwrap();

    // LSP status
    if let Some(ref lsp) = report.lsp_enrichment {
        match &lsp.status {
            LspStatus::Enriched => writeln!(out, "LSP: enriched (gopls connected)").unwrap(),
            LspStatus::NoResults => {
                writeln!(out, "LSP: connected but no additional results").unwrap()
            }
            LspStatus::Unavailable { reason } => {
                writeln!(out, "LSP: unavailable ({reason})").unwrap()
            }
        }
        writeln!(out).unwrap();
    }

    // Status
    match report.status {
        ResolutionStatus::Resolved => {
            writeln!(out, "Status: resolved (single definition)").unwrap();
        }
        ResolutionStatus::Ambiguous => {
            writeln!(
                out,
                "Status: AMBIGUOUS ({} definitions across packages)",
                report.definitions.len()
            )
            .unwrap();
            writeln!(
                out,
                "  Warning: same name in multiple packages — rename may be partial or unintended."
            )
            .unwrap();
        }
        ResolutionStatus::NotFound => {
            writeln!(out, "Status: NOT FOUND").unwrap();
            writeln!(out).unwrap();
            writeln!(
                out,
                "The symbol '{}' was not found in the structural index.",
                report.symbol
            )
            .unwrap();
            writeln!(out, "Possible reasons:").unwrap();
            writeln!(out, "  - The name is misspelled").unwrap();
            writeln!(out, "  - It is defined inside a function body (not indexed)").unwrap();
            writeln!(out, "  - It is a field name or local variable").unwrap();
            writeln!(out, "  - It exists in vendor/ or generated code (excluded)").unwrap();
            writeln!(out).unwrap();
            writeln!(out, "Scope: {}", report.scope_note).unwrap();
            return out;
        }
    }
    writeln!(out).unwrap();

    // Risk assessment (prominent)
    let risk_icon = match report.risk_assessment.level {
        RiskLevel::Low => "LOW",
        RiskLevel::Medium => "MEDIUM",
        RiskLevel::High => "HIGH",
        RiskLevel::Critical => "CRITICAL",
    };
    writeln!(out, "Risk level: {risk_icon}").unwrap();
    for reason in &report.risk_assessment.reasons {
        writeln!(
            out,
            "  [{basis}] {factor}: {detail}",
            basis = reason.basis,
            factor = reason.factor,
            detail = reason.detail,
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // Definitions
    writeln!(out, "Definitions ({}): [observed]", report.definitions.len()).unwrap();
    for def in &report.definitions {
        writeln!(
            out,
            "  {} [{}] ({}) at {}:{}",
            def.name, def.kind, def.visibility, def.file, def.line
        )
        .unwrap();
        writeln!(out, "    package: {}", def.package).unwrap();
        if verbose || report.definitions.len() == 1 {
            for detail in &def.details {
                writeln!(out, "    {detail}").unwrap();
            }
        }
    }
    writeln!(out).unwrap();

    // Affected references
    let total_refs = report.affected_references.len()
        + report
            .lsp_enrichment
            .as_ref()
            .map(|l| l.additional_references.len())
            .unwrap_or(0);

    if report.affected_references.is_empty() {
        writeln!(out, "Affected references: none found [observed]").unwrap();
        if report.lsp_enrichment.is_none() {
            writeln!(
                out,
                "  (use --lsp to find call sites in function bodies)"
            )
            .unwrap();
        }
    } else {
        writeln!(
            out,
            "Affected references ({total_refs} total, {} structural): [observed]",
            report.affected_references.len()
        )
        .unwrap();
        let limit = if verbose {
            report.affected_references.len()
        } else {
            20
        };
        for r in report.affected_references.iter().take(limit) {
            writeln!(out, "  {} in {} at {}:{}", r.kind, r.context, r.file, r.line).unwrap();
        }
        if !verbose && report.affected_references.len() > 20 {
            writeln!(
                out,
                "  ... and {} more (use --verbose to see all)",
                report.affected_references.len() - 20
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();

    // LSP additional references
    if let Some(ref lsp) = report.lsp_enrichment {
        if !lsp.additional_references.is_empty() {
            writeln!(
                out,
                "Additional semantic references ({}): [lsp]",
                lsp.additional_references.len()
            )
            .unwrap();
            let limit = if verbose {
                lsp.additional_references.len()
            } else {
                10
            };
            for r in lsp.additional_references.iter().take(limit) {
                writeln!(out, "  {}:{}", r.location.file, r.location.line).unwrap();
            }
            if !verbose && lsp.additional_references.len() > 10 {
                writeln!(
                    out,
                    "  ... and {} more (use --verbose to see all)",
                    lsp.additional_references.len() - 10
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    // Sensitive areas
    if !report.sensitive_areas.is_empty() {
        writeln!(
            out,
            "Sensitive areas touched ({}): [observed]",
            report.sensitive_areas.len()
        )
        .unwrap();
        for area in &report.sensitive_areas {
            writeln!(out, "  {} ({} files)", area.area, area.files.len()).unwrap();
            if verbose {
                for f in &area.files {
                    writeln!(out, "    {f}").unwrap();
                }
            }
        }
        writeln!(out).unwrap();
    }

    // Contract surface
    if !report.contract_surface.is_empty() {
        writeln!(
            out,
            "Contract surface ({}): [observed]",
            report.contract_surface.len()
        )
        .unwrap();
        for c in &report.contract_surface {
            writeln!(out, "  {} [{}] at {}:{}", c.name, c.kind, c.file, c.line).unwrap();
            writeln!(out, "    why: {}", c.why).unwrap();
        }
        writeln!(out).unwrap();
    }

    // New name conflicts
    if !report.new_name_conflicts.is_empty() {
        writeln!(
            out,
            "Name conflicts with '{}' ({}):",
            report.new_name.as_deref().unwrap_or("?"),
            report.new_name_conflicts.len()
        )
        .unwrap();
        for c in &report.new_name_conflicts {
            writeln!(
                out,
                "  {} [{}] in {} at {}:{}",
                c.name, c.kind, c.package, c.file, c.line
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    // Packages affected
    if !report.affected_packages.is_empty() {
        writeln!(
            out,
            "Packages affected ({}):",
            report.affected_packages.len()
        )
        .unwrap();
        for pkg in &report.affected_packages {
            writeln!(out, "  {pkg}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // Recommendations section
    writeln!(out, "--- Recommendations ---\n").unwrap();

    writeln!(
        out,
        "Quality gate profile: {}",
        report.recommended_gate_profile
    )
    .unwrap();

    if !report.suggested_smoke_scenarios.is_empty() {
        writeln!(out, "Suggested smoke scenarios:").unwrap();
        for s in &report.suggested_smoke_scenarios {
            writeln!(out, "  $ raccoon-cli scenario-smoke {s}").unwrap();
        }
    }

    if !report.recommended_checks.is_empty() {
        writeln!(out, "Recommended checks:").unwrap();
        for cmd in &report.recommended_checks {
            writeln!(out, "  $ {cmd}").unwrap();
        }
    }
    writeln!(out).unwrap();

    // Scope
    writeln!(out, "Scope: {}", report.scope_note).unwrap();

    out
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn type_expr_mentions(expr: &str, symbol: &str) -> bool {
    if expr == symbol {
        return true;
    }

    let bytes = expr.as_bytes();
    let sym_bytes = symbol.as_bytes();
    let sym_len = sym_bytes.len();

    if sym_len == 0 || sym_len > bytes.len() {
        return false;
    }

    let mut i = 0;
    while i + sym_len <= bytes.len() {
        if &bytes[i..i + sym_len] == sym_bytes {
            let before_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            let after_ok = i + sym_len == bytes.len() || !is_ident_char(bytes[i + sym_len]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }

    false
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_message_type(name: &str, file_path: &str) -> bool {
    let message_suffixes = [
        "Command", "Query", "Reply", "Event", "Request", "Response", "Message",
    ];
    let contract_paths = ["contracts/", "messages", "events"];

    message_suffixes.iter().any(|s| name.ends_with(s))
        || contract_paths.iter().any(|p| file_path.contains(p))
}

fn visibility_label(vis: Visibility) -> String {
    match vis {
        Visibility::Exported => "exported".into(),
        Visibility::Unexported => "unexported".into(),
    }
}

fn type_details(kind: &TypeKind) -> Vec<String> {
    match kind {
        TypeKind::Struct { fields } => {
            fields
                .iter()
                .map(|f| {
                    let tag_info = f
                        .tag
                        .as_ref()
                        .map(|t| format!(" {t}"))
                        .unwrap_or_default();
                    if f.embedded {
                        format!("embed: {}{}", f.type_expr, tag_info)
                    } else {
                        format!("field: {} {}{}", f.name, f.type_expr, tag_info)
                    }
                })
                .collect()
        }
        TypeKind::Interface { methods, embeds } => {
            let mut details: Vec<String> = embeds
                .iter()
                .map(|e| format!("embed: {}", e.type_name))
                .collect();
            for m in methods {
                details.push(format!("method: {}{}", m.name, m.signature));
            }
            details
        }
        TypeKind::Alias { underlying } => {
            vec![format!("underlying: {underlying}")]
        }
    }
}

fn func_info(f: &GoFunc) -> (String, Vec<String>) {
    let kind = if f.receiver.is_some() {
        "method"
    } else {
        "func"
    };

    let mut details = Vec::new();
    if let Some(ref recv) = f.receiver {
        let ptr = if recv.pointer { "*" } else { "" };
        details.push(format!(
            "receiver: ({} {}{})",
            recv.name, ptr, recv.type_name
        ));
    }
    if !f.params.is_empty() {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} {}", p.name, p.type_expr))
            .collect();
        details.push(format!("params: ({})", params.join(", ")));
    }
    if !f.returns.is_empty() {
        let rets: Vec<String> = f
            .returns
            .iter()
            .map(|r| {
                if r.name.is_empty() {
                    r.type_expr.clone()
                } else {
                    format!("{} {}", r.name, r.type_expr)
                }
            })
            .collect();
        details.push(format!("returns: ({})", rets.join(", ")));
    }

    (kind.into(), details)
}

fn file_to_package(path: &str) -> String {
    let rel = if let Some(pos) = path.find("internal/") {
        &path[pos..]
    } else {
        path
    };
    match rel.rfind('/') {
        Some(pos) => {
            let dir = &rel[..pos];
            dir.rsplit('/').next().unwrap_or(dir).to_string()
        }
        None => String::new(),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project(tmp: &TempDir) -> &Path {
        let root = tmp.path();

        // Domain layer
        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::write(
            root.join("internal/domain/configctl/lifecycle.go"),
            r#"package configctl

type VersionLifecycle string

const (
	LifecycleDraft     VersionLifecycle = "draft"
	LifecycleValidated VersionLifecycle = "validated"
	LifecycleActive    VersionLifecycle = "active"
)
"#,
        )
        .unwrap();

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
	Lifecycle VersionLifecycle
	CreatedAt time.Time
}

func NewConfigSet(id string) ConfigSet {
	return ConfigSet{SetID: id}
}

func (s *ConfigSet) AddVersion(v ConfigVersion) {
}

func (s ConfigSet) VersionCount() int {
	return len(s.Versions)
}
"#,
        )
        .unwrap();

        // Application ports
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

        // Application contracts
        fs::create_dir_all(root.join("internal/application/configctl/contracts")).unwrap();
        fs::write(
            root.join("internal/application/configctl/contracts/commands.go"),
            r#"package contracts

type CreateDraftCommand struct {
	SetID string
	Name  string
}

type ActivateConfigCommand struct {
	SetID     string
	VersionID string
}
"#,
        )
        .unwrap();

        // Application use case
        fs::create_dir_all(root.join("internal/application/configctl")).unwrap();
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

        // Adapter layer
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

func Decode(data []byte) (domain.ConfigSet, error) {
	return domain.ConfigSet{}, nil
}
"#,
        )
        .unwrap();

        // Actors
        fs::create_dir_all(root.join("internal/actors/scopes/configctl")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/configctl/supervisor.go"),
            r#"package configctl

import (
	domain "example.com/quality-service/internal/domain/configctl"
)

type Supervisor struct {
	sets []domain.ConfigSet
}

func New() Supervisor {
	return Supervisor{}
}
"#,
        )
        .unwrap();

        // HTTP interfaces
        fs::create_dir_all(root.join("internal/interfaces/http/handlers")).unwrap();
        fs::write(
            root.join("internal/interfaces/http/handlers/configctl.go"),
            r#"package handlers

func HandleGetConfig(id string) string {
	return id
}
"#,
        )
        .unwrap();

        root
    }

    // ── Resolution status ──────────────────────────────────────────────

    #[test]
    fn resolves_single_definition() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "struct");
    }

    #[test]
    fn not_found_for_unknown_symbol() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "NonExistentType", None);

        assert_eq!(report.status, ResolutionStatus::NotFound);
        assert!(report.definitions.is_empty());
        assert!(report.affected_references.is_empty());
        assert_eq!(report.risk_assessment.level, RiskLevel::Low);
    }

    #[test]
    fn ambiguous_for_duplicate_names() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        fs::create_dir_all(root.join("internal/actors/scopes/consumer")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/consumer/supervisor.go"),
            "package consumer\n\ntype Supervisor struct {\n\trunning bool\n}\n",
        )
        .unwrap();

        let report = check(root, "Supervisor", None);
        assert_eq!(report.status, ResolutionStatus::Ambiguous);
        assert!(report.definitions.len() > 1);
        // Ambiguous should raise risk.
        assert!(report.risk_assessment.level >= RiskLevel::High);
        assert!(report
            .risk_assessment
            .reasons
            .iter()
            .any(|r| r.factor == "ambiguous_definitions"));
    }

    // ── Risk assessment ────────────────────────────────────────────────

    #[test]
    fn exported_symbol_raises_risk() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert!(report.risk_assessment.level >= RiskLevel::Medium);
        assert!(report
            .risk_assessment
            .reasons
            .iter()
            .any(|r| r.factor == "exported_symbol"));
    }

    #[test]
    fn unexported_symbol_low_risk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::write(
            root.join("internal/domain/configctl/helpers.go"),
            "package configctl\n\nfunc helperFunc() {}\n",
        )
        .unwrap();

        let report = check(root, "helperFunc", None);
        assert_eq!(report.risk_assessment.level, RiskLevel::Low);
    }

    #[test]
    fn port_interface_is_critical() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigctlGateway", None);

        assert_eq!(report.risk_assessment.level, RiskLevel::Critical);
        assert!(report
            .risk_assessment
            .reasons
            .iter()
            .any(|r| r.factor == "port_interface"));
    }

    #[test]
    fn message_type_is_critical() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "CreateDraftCommand", None);

        assert_eq!(report.risk_assessment.level, RiskLevel::Critical);
        assert!(report
            .risk_assessment
            .reasons
            .iter()
            .any(|r| r.factor == "message_type"));
    }

    // ── Affected references ────────────────────────────────────────────

    #[test]
    fn finds_affected_references() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        // ConfigSet is referenced as receiver, return type, param type, field type
        assert!(
            !report.affected_references.is_empty(),
            "should find references to ConfigSet"
        );

        let kinds: BTreeSet<&str> = report
            .affected_references
            .iter()
            .map(|r| r.kind.as_str())
            .collect();
        assert!(
            kinds.contains("receiver"),
            "should find receiver references"
        );
        assert!(
            kinds.contains("return_type"),
            "should find return type references"
        );
    }

    #[test]
    fn all_references_have_observed_basis() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        for r in &report.affected_references {
            assert_eq!(r.basis, "observed");
        }
    }

    // ── Sensitive areas ────────────────────────────────────────────────

    #[test]
    fn detects_sensitive_areas() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        let area_names: Vec<&str> = report
            .sensitive_areas
            .iter()
            .map(|a| a.area.as_str())
            .collect();
        assert!(
            area_names.contains(&"domain"),
            "should detect domain area, got: {:?}",
            area_names
        );
    }

    #[test]
    fn cross_layer_impact_detected() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        // ConfigSet is in domain and referenced from adapters
        let area_names: Vec<&str> = report
            .sensitive_areas
            .iter()
            .map(|a| a.area.as_str())
            .collect();

        // Should have both domain and adapter areas
        if area_names.contains(&"domain") && area_names.contains(&"adapters") {
            assert!(report
                .risk_assessment
                .reasons
                .iter()
                .any(|r| r.factor == "cross_layer_impact"));
        }
    }

    // ── Contract surface ───────────────────────────────────────────────

    #[test]
    fn detects_port_contract() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigctlGateway", None);

        assert!(
            !report.contract_surface.is_empty(),
            "should detect contract surface for port interface"
        );
        assert!(report.contract_surface.iter().any(|c| c.kind == "port"));
    }

    #[test]
    fn detects_message_type_contract() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "CreateDraftCommand", None);

        assert!(report
            .contract_surface
            .iter()
            .any(|c| c.kind == "message_type"));
    }

    // ── New name conflicts ─────────────────────────────────────────────

    #[test]
    fn detects_new_name_conflict() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        // Rename ConfigSet to ConfigVersion — ConfigVersion already exists
        let report = check(root, "ConfigSet", Some("ConfigVersion"));
        assert!(
            !report.new_name_conflicts.is_empty(),
            "should detect conflict with existing ConfigVersion"
        );
        assert_eq!(report.new_name_conflicts[0].name, "ConfigVersion");
    }

    #[test]
    fn no_conflict_for_fresh_name() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let report = check(root, "ConfigSet", Some("QualityConfigSet"));
        assert!(
            report.new_name_conflicts.is_empty(),
            "should not detect conflict for fresh name"
        );
    }

    // ── Recommendations ────────────────────────────────────────────────

    #[test]
    fn gate_profile_for_low_risk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::write(
            root.join("internal/domain/configctl/helpers.go"),
            "package configctl\n\nfunc helperFunc() {}\n",
        )
        .unwrap();

        let report = check(root, "helperFunc", None);
        assert_eq!(report.recommended_gate_profile, "fast");
    }

    #[test]
    fn gate_profile_for_critical_risk() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigctlGateway", None);

        assert_eq!(report.recommended_gate_profile, "deep");
    }

    #[test]
    fn smoke_scenarios_for_domain_type() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert!(
            !report.suggested_smoke_scenarios.is_empty(),
            "should suggest smoke scenarios"
        );
    }

    #[test]
    fn smoke_scenarios_for_message_type() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "CreateDraftCommand", None);

        assert!(
            report
                .suggested_smoke_scenarios
                .contains(&"invalid-payload".to_string()),
            "message types should suggest invalid-payload scenario, got: {:?}",
            report.suggested_smoke_scenarios
        );
    }

    #[test]
    fn recommended_checks_include_arch_guard_for_domain() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert!(report
            .recommended_checks
            .iter()
            .any(|c| c.contains("arch-guard")));
    }

    #[test]
    fn recommended_checks_include_contract_audit_for_port() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigctlGateway", None);

        assert!(report
            .recommended_checks
            .iter()
            .any(|c| c.contains("contract-audit")));
    }

    // ── Output rendering ───────────────────────────────────────────────

    #[test]
    fn json_output_is_valid() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["symbol"].is_string());
        assert!(parsed["status"].is_string());
        assert!(parsed["definitions"].is_array());
        assert!(parsed["affected_references"].is_array());
        assert!(parsed["risk_assessment"].is_object());
        assert!(parsed["risk_assessment"]["level"].is_string());
        assert!(parsed["risk_assessment"]["reasons"].is_array());
        assert!(parsed["recommended_gate_profile"].is_string());
        assert!(parsed["scope_note"].is_string());
    }

    #[test]
    fn json_output_with_new_name() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", Some("QualityConfigSet"));

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["new_name"], "QualityConfigSet");
    }

    #[test]
    fn json_omits_new_name_when_absent() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("new_name").is_none());
    }

    #[test]
    fn human_output_shows_key_sections() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        let human = render_human(&report, false);
        assert!(human.contains("Rename Safety Check: ConfigSet"));
        assert!(human.contains("Risk level:"));
        assert!(human.contains("Definitions"));
        assert!(human.contains("Affected references"));
        assert!(human.contains("Scope:"));
        assert!(human.contains("Recommendations"));
    }

    #[test]
    fn human_output_shows_rename_arrow() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", Some("QualityConfigSet"));

        let human = render_human(&report, false);
        assert!(human.contains("ConfigSet -> QualityConfigSet"));
    }

    #[test]
    fn human_output_for_not_found() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "Nonexistent", None);

        let human = render_human(&report, false);
        assert!(human.contains("NOT FOUND"));
        assert!(human.contains("misspelled"));
    }

    #[test]
    fn human_output_for_ambiguous() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        fs::create_dir_all(root.join("internal/actors/scopes/consumer")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/consumer/supervisor.go"),
            "package consumer\n\ntype Supervisor struct {\n\trunning bool\n}\n",
        )
        .unwrap();

        let report = check(root, "Supervisor", None);
        let human = render_human(&report, false);
        assert!(human.contains("AMBIGUOUS"));
    }

    // ── LSP enrichment ─────────────────────────────────────────────────

    #[test]
    fn lsp_fallback_when_unavailable() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = check_with_lsp(root, "ConfigSet", None, &mut bridge);

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert!(!report.definitions.is_empty());

        let lsp = report.lsp_enrichment.as_ref().expect("should have lsp_enrichment");
        assert!(matches!(lsp.status, LspStatus::Unavailable { .. }));
        assert!(lsp.additional_references.is_empty());

        assert!(report.scope_note.contains("unavailable"));
    }

    #[test]
    fn lsp_enrichment_absent_without_flag() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert!(report.lsp_enrichment.is_none());
    }

    // ── Edge cases ─────────────────────────────────────────────────────

    #[test]
    fn empty_project_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let report = check(tmp.path(), "Anything", None);

        assert_eq!(report.status, ResolutionStatus::NotFound);
        assert!(report.definitions.is_empty());
    }

    #[test]
    fn scope_note_always_present() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let found = check(root, "ConfigSet", None);
        assert!(!found.scope_note.is_empty());

        let not_found = check(root, "Nope", None);
        assert!(!not_found.scope_note.is_empty());
    }

    #[test]
    fn risk_reasons_have_valid_basis() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        for reason in &report.risk_assessment.reasons {
            assert!(
                reason.basis == "observed" || reason.basis == "inferred",
                "invalid basis: {}",
                reason.basis
            );
        }
    }

    #[test]
    fn sensitive_areas_have_observed_basis() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        for area in &report.sensitive_areas {
            assert_eq!(area.basis, "observed");
        }
    }

    #[test]
    fn packages_tracked_correctly() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert!(
            !report.affected_packages.is_empty(),
            "should track at least one package"
        );
        assert!(
            report.affected_packages.contains("configctl"),
            "should include configctl package"
        );
    }

    #[test]
    fn drift_detect_always_recommended() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = check(root, "ConfigSet", None);

        assert!(report
            .recommended_checks
            .iter()
            .any(|c| c.contains("drift-detect")));
    }
}
