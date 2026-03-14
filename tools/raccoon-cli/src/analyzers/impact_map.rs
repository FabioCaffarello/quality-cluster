//! Impact map — structural impact analysis for changed files, packages, or symbols.
//!
//! Uses the codeintel AST index to trace import relationships, symbol references,
//! and contract connections. Differentiates observed facts from inferred risks.
//!
//! ## What it does
//!
//! Given a set of targets (files, packages, or symbols), the impact map:
//! 1. Resolves each target to concrete packages/files in the index
//! 2. Finds direct dependents (packages that import the target package)
//! 3. Identifies exported symbols defined in the target
//! 4. Detects contract surface (interfaces, message types, ports)
//! 5. Maps to sensitive areas and recommends raccoon-cli checks
//!
//! ## What it does NOT do
//!
//! - No call graph (function body analysis is out of scope)
//! - No type resolution across packages
//! - No runtime/reflection tracing
//! All limitations are stated in the output.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::codeintel::{self, GoType, ImportKind, ProjectIndex, TypeKind, Visibility};
use crate::lsp::bridge::GoplsBridge;
use crate::lsp::types::{LspReference, LspStatus};

// ── Public API ─────────────────────────────────────────────────────────────

/// Run impact analysis for the given targets (AST only).
pub fn analyze(project_root: &Path, targets: &[String]) -> ImpactReport {
    let index = codeintel::build_index(project_root);
    let mut impacts = Vec::new();

    for target in targets {
        let impact = analyze_target(&index, target);
        impacts.push(impact);
    }

    // Aggregate recommended commands across all impacts (deduplicated, ordered).
    let mut all_commands: BTreeSet<String> = BTreeSet::new();
    let mut all_areas: BTreeSet<String> = BTreeSet::new();
    for impact in &impacts {
        for cmd in &impact.recommended_commands {
            all_commands.insert(cmd.clone());
        }
        for area in &impact.sensitive_areas {
            all_areas.insert(area.name.clone());
        }
    }

    let scope_note = "Impact is computed from static import graphs and exported symbol analysis. \
        No call graph, type resolution, or runtime tracing is available."
        .to_string();

    ImpactReport {
        targets: targets.to_vec(),
        impacts,
        recommended_commands: all_commands.into_iter().collect(),
        sensitive_areas_touched: all_areas.into_iter().collect(),
        scope_note,
        lsp_enrichment: None,
    }
}

/// Run impact analysis with optional LSP enrichment.
///
/// For each exported symbol in affected targets, queries gopls for references
/// to discover actual callers (including function body call sites) beyond
/// what import-level analysis can show.
pub fn analyze_with_lsp(
    project_root: &Path,
    targets: &[String],
    bridge: &mut GoplsBridge,
) -> ImpactReport {
    let mut report = analyze(project_root, targets);
    let index = codeintel::build_index(project_root);

    // Collect exported symbols from all resolved targets to query LSP.
    let mut lsp_refs: Vec<LspReference> = Vec::new();
    let mut queried_symbols: BTreeSet<String> = BTreeSet::new();
    let mut lsp_status = if bridge.is_available() {
        LspStatus::NoResults
    } else {
        LspStatus::Unavailable {
            reason: bridge.unavailable_reason().unwrap_or("gopls not available").to_string(),
        }
    };

    for impact in &report.impacts {
        // Only enrich the first few symbols to avoid slowdown.
        let limit = 10;
        for sym in impact.exported_symbols.iter().take(limit) {
            if queried_symbols.contains(&sym.name) {
                continue;
            }
            queried_symbols.insert(sym.name.clone());

            let enriched = bridge.enrich_symbol_with_index(&index, project_root, &sym.name);

            if matches!(enriched.lsp_status, LspStatus::Enriched) {
                lsp_status = LspStatus::Enriched;
            }

            for lr in enriched.lsp_references {
                // Deduplicate by location.
                if !lsp_refs.iter().any(|r| {
                    r.location.file == lr.location.file
                        && r.location.line == lr.location.line
                }) {
                    lsp_refs.push(lr);
                }
            }
        }
    }

    report.lsp_enrichment = Some(ImpactLspEnrichment {
        status: lsp_status,
        queried_symbols: queried_symbols.into_iter().collect(),
        additional_references: lsp_refs,
    });

    // Update scope note.
    match &report.lsp_enrichment.as_ref().unwrap().status {
        LspStatus::Enriched => {
            report.scope_note = "Impact combines static import graphs and exported symbol analysis \
                with gopls semantic references (cross-package call sites). Each source is tagged \
                [ast] or [lsp].".to_string();
        }
        LspStatus::NoResults => {
            report.scope_note = "Impact is computed from static import graphs and exported symbol \
                analysis. gopls was available but returned no additional references.".to_string();
        }
        LspStatus::Unavailable { reason } => {
            report.scope_note = format!(
                "Impact is computed from static import graphs and exported symbol analysis. \
                LSP enrichment unavailable: {reason}."
            );
        }
    }

    report
}

// ── Report types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub targets: Vec<String>,
    pub impacts: Vec<TargetImpact>,
    pub recommended_commands: Vec<String>,
    pub sensitive_areas_touched: Vec<String>,
    pub scope_note: String,
    /// Optional LSP enrichment — present when `--lsp` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp_enrichment: Option<ImpactLspEnrichment>,
}

/// Semantic enrichment from gopls for impact analysis.
#[derive(Debug, Clone, Serialize)]
pub struct ImpactLspEnrichment {
    /// Whether gopls contributed results.
    pub status: LspStatus,
    /// Symbols that were queried via gopls.
    pub queried_symbols: Vec<String>,
    /// Cross-package references found by gopls (call sites not visible to AST).
    pub additional_references: Vec<LspReference>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetImpact {
    pub target: String,
    pub kind: TargetKind,
    pub resolved_package: Option<String>,
    pub resolved_files: Vec<String>,
    pub exported_symbols: Vec<ExportedSymbol>,
    pub direct_dependents: Vec<Dependent>,
    pub contract_surface: Vec<ContractItem>,
    pub sensitive_areas: Vec<AreaMatch>,
    pub risks: Vec<Risk>,
    pub recommended_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    File,
    Package,
    Symbol,
    Unresolved,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportedSymbol {
    pub name: String,
    pub kind: String, // "struct", "interface", "func", "const", "type_alias"
    pub location: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dependent {
    pub package_dir: String,
    pub import_path: String,
    /// Whether this is a fact (direct import) or inference (transitive).
    pub basis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractItem {
    pub name: String,
    pub kind: String, // "interface", "message_type", "port"
    pub location: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AreaMatch {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Risk {
    pub description: String,
    /// "observed" = fact from AST; "inferred" = heuristic guess.
    pub basis: String,
}

// ── Sensitive areas (shared with coverage_map) ─────────────────────────────

struct SensitiveAreaDef {
    name: &'static str,
    description: &'static str,
    patterns: &'static [&'static str],
    commands: &'static [&'static str],
}

const SENSITIVE_AREAS: &[SensitiveAreaDef] = &[
    SensitiveAreaDef {
        name: "config-files",
        description: "deploy/configs — service configuration",
        patterns: &["deploy/configs/"],
        commands: &["raccoon-cli doctor", "raccoon-cli topology-doctor", "raccoon-cli drift-detect"],
    },
    SensitiveAreaDef {
        name: "compose",
        description: "docker-compose — service orchestration",
        patterns: &["deploy/compose/"],
        commands: &["raccoon-cli doctor", "raccoon-cli topology-doctor", "raccoon-cli drift-detect"],
    },
    SensitiveAreaDef {
        name: "nats-adapters",
        description: "NATS/JetStream adapter layer",
        patterns: &["internal/adapters/nats/"],
        commands: &["raccoon-cli contract-audit", "raccoon-cli runtime-bindings", "raccoon-cli arch-guard"],
    },
    SensitiveAreaDef {
        name: "kafka-adapters",
        description: "Kafka adapter layer",
        patterns: &["internal/adapters/kafka/"],
        commands: &["raccoon-cli topology-doctor", "raccoon-cli runtime-bindings", "raccoon-cli arch-guard"],
    },
    SensitiveAreaDef {
        name: "domain",
        description: "domain layer — business rules (must be pure)",
        patterns: &["internal/domain/"],
        commands: &["raccoon-cli arch-guard", "raccoon-cli contract-audit"],
    },
    SensitiveAreaDef {
        name: "application",
        description: "application layer — use cases and ports",
        patterns: &["internal/application/"],
        commands: &["raccoon-cli arch-guard", "raccoon-cli contract-audit"],
    },
    SensitiveAreaDef {
        name: "http-handlers",
        description: "HTTP interface layer — API endpoints",
        patterns: &["internal/interfaces/http/"],
        commands: &["raccoon-cli arch-guard"],
    },
    SensitiveAreaDef {
        name: "actors",
        description: "actor supervision trees — runtime wiring",
        patterns: &["internal/actors/"],
        commands: &["raccoon-cli arch-guard", "raccoon-cli runtime-bindings"],
    },
    SensitiveAreaDef {
        name: "validator-logic",
        description: "validator — validation rules and results",
        patterns: &["internal/actors/scopes/validator/", "internal/application/validatorresults/"],
        commands: &["raccoon-cli contract-audit", "raccoon-cli runtime-bindings", "raccoon-cli scenario-smoke happy-path"],
    },
    SensitiveAreaDef {
        name: "consumer-pipeline",
        description: "consumer — kafka-to-jetstream bridging",
        patterns: &["internal/actors/scopes/consumer/", "internal/application/dataplane/"],
        commands: &["raccoon-cli topology-doctor", "raccoon-cli runtime-bindings", "raccoon-cli scenario-smoke happy-path"],
    },
    SensitiveAreaDef {
        name: "config-lifecycle",
        description: "configctl — config draft/validate/compile/activate",
        patterns: &["internal/actors/scopes/configctl/", "internal/application/configctl/"],
        commands: &["raccoon-cli contract-audit", "raccoon-cli scenario-smoke config-lifecycle"],
    },
];

// ── Target resolution ──────────────────────────────────────────────────────

fn analyze_target(index: &ProjectIndex, target: &str) -> TargetImpact {
    // Try to resolve as file first, then package, then symbol.
    if let Some(impact) = try_as_file(index, target) {
        return impact;
    }
    if let Some(impact) = try_as_package(index, target) {
        return impact;
    }
    if let Some(impact) = try_as_symbol(index, target) {
        return impact;
    }

    // Unresolved — still provide area matching if the path looks like a project path.
    let (areas, commands) = match_areas(target);
    let risks = if !areas.is_empty() {
        vec![Risk {
            description: format!("target '{}' not found in AST index but matches known project paths", target),
            basis: "inferred".into(),
        }]
    } else {
        vec![Risk {
            description: format!("target '{}' not found in AST index and does not match any known area", target),
            basis: "observed".into(),
        }]
    };

    TargetImpact {
        target: target.to_string(),
        kind: TargetKind::Unresolved,
        resolved_package: None,
        resolved_files: vec![],
        exported_symbols: vec![],
        direct_dependents: vec![],
        contract_surface: vec![],
        sensitive_areas: areas,
        risks,
        recommended_commands: commands,
    }
}

fn try_as_file(index: &ProjectIndex, target: &str) -> Option<TargetImpact> {
    // Match file path (with or without leading ./)
    let normalized = target.trim_start_matches("./");
    let file = index.files.iter().find(|f| f.path == normalized)?;

    let pkg_dir = file_dir(&file.path);
    let pkg = index.find_package(&pkg_dir);

    let exported_symbols = collect_exported_symbols_from_file(index, normalized);
    let dependents = find_dependents(index, &pkg_dir);
    let contract_surface = find_contracts_in_file(index, normalized);
    let (areas, commands) = match_areas(normalized);
    let risks = compute_risks(&exported_symbols, &contract_surface, &dependents);

    Some(TargetImpact {
        target: target.to_string(),
        kind: TargetKind::File,
        resolved_package: pkg.map(|p| p.dir.clone()),
        resolved_files: vec![file.path.clone()],
        exported_symbols,
        direct_dependents: dependents,
        contract_surface,
        sensitive_areas: areas,
        risks,
        recommended_commands: commands,
    })
}

fn try_as_package(index: &ProjectIndex, target: &str) -> Option<TargetImpact> {
    let normalized = target.trim_start_matches("./").trim_end_matches('/');
    let pkg = index.find_package(normalized)?;

    let files: Vec<String> = pkg.files.clone();
    let exported_symbols = collect_exported_symbols_from_package(index, normalized);
    let dependents = find_dependents(index, normalized);
    let contract_surface = find_contracts_in_package(index, normalized);
    let (areas, commands) = match_areas(normalized);
    let risks = compute_risks(&exported_symbols, &contract_surface, &dependents);

    Some(TargetImpact {
        target: target.to_string(),
        kind: TargetKind::Package,
        resolved_package: Some(pkg.dir.clone()),
        resolved_files: files,
        exported_symbols,
        direct_dependents: dependents,
        contract_surface,
        sensitive_areas: areas,
        risks,
        recommended_commands: commands,
    })
}

fn try_as_symbol(index: &ProjectIndex, target: &str) -> Option<TargetImpact> {
    // Find types or functions matching this name.
    let types = index.find_type(target);
    let funcs = index.find_func(target);

    if types.is_empty() && funcs.is_empty() {
        return None;
    }

    // Collect all files/packages containing this symbol.
    let mut files_set: BTreeSet<String> = BTreeSet::new();
    let mut pkg_dirs: BTreeSet<String> = BTreeSet::new();

    for t in &types {
        files_set.insert(t.location.file.clone());
        pkg_dirs.insert(file_dir(&t.location.file));
    }
    for f in &funcs {
        files_set.insert(f.location.file.clone());
        pkg_dirs.insert(file_dir(&f.location.file));
    }

    let files: Vec<String> = files_set.into_iter().collect();
    let primary_pkg = pkg_dirs.iter().next().cloned();

    // Exported symbols: just the matched ones
    let mut exported_symbols = Vec::new();
    for t in &types {
        if t.visibility == Visibility::Exported {
            exported_symbols.push(ExportedSymbol {
                name: t.name.clone(),
                kind: type_kind_label(&t.kind),
                location: format!("{}:{}", t.location.file, t.location.line),
            });
        }
    }
    for f in &funcs {
        if f.visibility == Visibility::Exported {
            exported_symbols.push(ExportedSymbol {
                name: f.name.clone(),
                kind: if f.receiver.is_some() { "method" } else { "func" }.into(),
                location: format!("{}:{}", f.location.file, f.location.line),
            });
        }
    }

    // Dependents: union across all packages containing this symbol
    let mut all_dependents: BTreeMap<String, Dependent> = BTreeMap::new();
    for dir in &pkg_dirs {
        for dep in find_dependents(index, dir) {
            all_dependents.entry(dep.package_dir.clone()).or_insert(dep);
        }
    }
    let dependents: Vec<Dependent> = all_dependents.into_values().collect();

    // Contract surface from all files
    let mut contract_surface = Vec::new();
    for file_path in &files {
        contract_surface.extend(find_contracts_in_file(index, file_path));
    }

    let first_file = files.first().map(|f| f.as_str()).unwrap_or("");
    let (areas, commands) = match_areas(first_file);
    let risks = compute_risks(&exported_symbols, &contract_surface, &dependents);

    Some(TargetImpact {
        target: target.to_string(),
        kind: TargetKind::Symbol,
        resolved_package: primary_pkg,
        resolved_files: files,
        exported_symbols,
        direct_dependents: dependents,
        contract_surface,
        sensitive_areas: areas,
        risks,
        recommended_commands: commands,
    })
}

// ── Dependency analysis ────────────────────────────────────────────────────

/// Find all packages that directly import the target package directory.
fn find_dependents(index: &ProjectIndex, target_dir: &str) -> Vec<Dependent> {
    let mut dependents = Vec::new();

    for pkg in &index.packages {
        if pkg.dir == target_dir {
            continue;
        }
        for imp in &pkg.imports {
            if imp.kind != ImportKind::Internal {
                continue;
            }
            // Internal imports contain the full module path; check if
            // the suffix matches the target directory.
            if import_matches_dir(&imp.path, target_dir) {
                dependents.push(Dependent {
                    package_dir: pkg.dir.clone(),
                    import_path: imp.path.clone(),
                    basis: "observed: direct import in AST".into(),
                });
                break; // one match per package is enough
            }
        }
    }

    dependents
}

/// Check if an import path like "github.com/org/repo/internal/domain/configctl"
/// ends with the given directory like "internal/domain/configctl".
fn import_matches_dir(import_path: &str, dir: &str) -> bool {
    import_path.ends_with(dir)
        || import_path.ends_with(&format!("/{dir}"))
}

// ── Symbol collection ──────────────────────────────────────────────────────

fn collect_exported_symbols_from_file(index: &ProjectIndex, file_path: &str) -> Vec<ExportedSymbol> {
    let mut symbols = Vec::new();
    for file in &index.files {
        if file.path != file_path {
            continue;
        }
        for t in &file.types {
            if t.visibility == Visibility::Exported {
                symbols.push(ExportedSymbol {
                    name: t.name.clone(),
                    kind: type_kind_label(&t.kind),
                    location: format!("{}:{}", t.location.file, t.location.line),
                });
            }
        }
        for f in &file.functions {
            if f.visibility == Visibility::Exported {
                let kind = if f.receiver.is_some() { "method" } else { "func" };
                symbols.push(ExportedSymbol {
                    name: f.name.clone(),
                    kind: kind.into(),
                    location: format!("{}:{}", f.location.file, f.location.line),
                });
            }
        }
        for c in &file.constants {
            if c.visibility == Visibility::Exported {
                symbols.push(ExportedSymbol {
                    name: c.name.clone(),
                    kind: "const".into(),
                    location: format!("{}:{}", c.location.file, c.location.line),
                });
            }
        }
    }
    symbols
}

fn collect_exported_symbols_from_package(index: &ProjectIndex, pkg_dir: &str) -> Vec<ExportedSymbol> {
    let mut symbols = Vec::new();
    for file in index.files_in_dir(pkg_dir) {
        if file.is_test {
            continue;
        }
        for t in &file.types {
            if t.visibility == Visibility::Exported {
                symbols.push(ExportedSymbol {
                    name: t.name.clone(),
                    kind: type_kind_label(&t.kind),
                    location: format!("{}:{}", t.location.file, t.location.line),
                });
            }
        }
        for f in &file.functions {
            if f.visibility == Visibility::Exported {
                let kind = if f.receiver.is_some() { "method" } else { "func" };
                symbols.push(ExportedSymbol {
                    name: f.name.clone(),
                    kind: kind.into(),
                    location: format!("{}:{}", f.location.file, f.location.line),
                });
            }
        }
        for c in &file.constants {
            if c.visibility == Visibility::Exported {
                symbols.push(ExportedSymbol {
                    name: c.name.clone(),
                    kind: "const".into(),
                    location: format!("{}:{}", c.location.file, c.location.line),
                });
            }
        }
    }
    symbols
}

// ── Contract surface detection ─────────────────────────────────────────────

fn find_contracts_in_file(index: &ProjectIndex, file_path: &str) -> Vec<ContractItem> {
    let mut contracts = Vec::new();
    for file in &index.files {
        if file.path != file_path {
            continue;
        }
        append_contracts_from_types(&file.types, &mut contracts);
    }
    contracts
}

fn find_contracts_in_package(index: &ProjectIndex, pkg_dir: &str) -> Vec<ContractItem> {
    let mut contracts = Vec::new();
    for file in index.files_in_dir(pkg_dir) {
        if file.is_test {
            continue;
        }
        append_contracts_from_types(&file.types, &mut contracts);
    }
    contracts
}

fn append_contracts_from_types(types: &[GoType], contracts: &mut Vec<ContractItem>) {
    for t in types {
        if t.visibility != Visibility::Exported {
            continue;
        }
        let loc = format!("{}:{}", t.location.file, t.location.line);

        match &t.kind {
            TypeKind::Interface { methods, .. } => {
                let why = if t.location.file.contains("ports") {
                    "port interface — changes affect all implementations"
                } else {
                    "exported interface — changes affect all implementors"
                };
                contracts.push(ContractItem {
                    name: t.name.clone(),
                    kind: "interface".into(),
                    location: loc,
                    why: why.into(),
                });
                // Also flag individual methods as contract surface
                for m in methods {
                    contracts.push(ContractItem {
                        name: format!("{}.{}", t.name, m.name),
                        kind: "interface_method".into(),
                        location: format!("{}:{}", m.location.file, m.location.line),
                        why: "changing interface method signature breaks all implementors".into(),
                    });
                }
            }
            TypeKind::Struct { fields } => {
                // Structs in contracts/, messages, or with Command/Query/Reply/Event in name
                let is_message = is_message_type(&t.name, &t.location.file);
                if is_message {
                    let field_names: Vec<&str> = fields.iter()
                        .filter(|f| f.visibility == Visibility::Exported)
                        .map(|f| f.name.as_str())
                        .collect();
                    contracts.push(ContractItem {
                        name: t.name.clone(),
                        kind: "message_type".into(),
                        location: loc,
                        why: format!(
                            "message struct — field changes affect serialization (fields: {})",
                            if field_names.is_empty() { "none".into() } else { field_names.join(", ") }
                        ),
                    });
                }
            }
            TypeKind::Alias { .. } => {
                // Type aliases in contract-heavy paths
                if t.location.file.contains("contracts") || t.location.file.contains("events") {
                    contracts.push(ContractItem {
                        name: t.name.clone(),
                        kind: "contract_type".into(),
                        location: loc,
                        why: "type alias in contract layer — changes affect message encoding".into(),
                    });
                }
            }
        }
    }
}

fn is_message_type(name: &str, file_path: &str) -> bool {
    let message_suffixes = ["Command", "Query", "Reply", "Event", "Request", "Response", "Message"];
    let contract_paths = ["contracts/", "messages", "events"];

    message_suffixes.iter().any(|s| name.ends_with(s))
        || contract_paths.iter().any(|p| file_path.contains(p))
}

// ── Area matching ──────────────────────────────────────────────────────────

fn match_areas(path: &str) -> (Vec<AreaMatch>, Vec<String>) {
    let mut areas = Vec::new();
    let mut commands: BTreeSet<String> = BTreeSet::new();

    for area in SENSITIVE_AREAS {
        if area.patterns.iter().any(|p| path.contains(p)) {
            areas.push(AreaMatch {
                name: area.name.to_string(),
                description: area.description.to_string(),
            });
            for cmd in area.commands {
                commands.insert(cmd.to_string());
            }
        }
    }

    // Always recommend quality-gate as a catch-all
    if !areas.is_empty() {
        commands.insert("raccoon-cli quality-gate".to_string());
    }

    (areas, commands.into_iter().collect())
}

// ── Risk computation ───────────────────────────────────────────────────────

fn compute_risks(
    symbols: &[ExportedSymbol],
    contracts: &[ContractItem],
    dependents: &[Dependent],
) -> Vec<Risk> {
    let mut risks = Vec::new();

    if !contracts.is_empty() {
        let iface_count = contracts.iter().filter(|c| c.kind == "interface").count();
        let msg_count = contracts.iter().filter(|c| c.kind == "message_type").count();

        if iface_count > 0 {
            risks.push(Risk {
                description: format!(
                    "{} interface(s) in contract surface — signature changes break implementors",
                    iface_count
                ),
                basis: "observed".into(),
            });
        }
        if msg_count > 0 {
            risks.push(Risk {
                description: format!(
                    "{} message type(s) — field changes affect serialization/deserialization across services",
                    msg_count
                ),
                basis: "observed".into(),
            });
        }
    }

    if dependents.len() > 3 {
        risks.push(Risk {
            description: format!(
                "high fan-out: {} packages depend on this — changes have wide blast radius",
                dependents.len()
            ),
            basis: "observed".into(),
        });
    } else if !dependents.is_empty() {
        risks.push(Risk {
            description: format!(
                "{} direct dependent package(s) — verify they still compile and behave correctly",
                dependents.len()
            ),
            basis: "observed".into(),
        });
    }

    let exported_count = symbols.len();
    if exported_count > 10 {
        risks.push(Risk {
            description: format!(
                "{} exported symbols — large API surface increases coupling risk",
                exported_count
            ),
            basis: "inferred".into(),
        });
    }

    risks
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_json(report: &ImpactReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_human(report: &ImpactReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== Impact Map ===").unwrap();
    writeln!(out).unwrap();

    // LSP status (if enrichment was requested)
    if let Some(ref lsp) = report.lsp_enrichment {
        match &lsp.status {
            LspStatus::Enriched => writeln!(out, "LSP: enriched (gopls connected)").unwrap(),
            LspStatus::NoResults => {
                writeln!(out, "LSP: connected but no additional references").unwrap()
            }
            LspStatus::Unavailable { reason } => {
                writeln!(out, "LSP: unavailable ({reason})").unwrap()
            }
        }
        writeln!(out).unwrap();
    }

    for impact in &report.impacts {
        writeln!(out, "Target: {} [{}]", impact.target, target_kind_label(impact.kind)).unwrap();

        if let Some(pkg) = &impact.resolved_package {
            writeln!(out, "  Package: {pkg}").unwrap();
        }

        if impact.kind == TargetKind::Unresolved {
            writeln!(out, "  (not found in AST index)").unwrap();
        }

        if !impact.resolved_files.is_empty() && verbose {
            writeln!(out, "  Files:").unwrap();
            for f in &impact.resolved_files {
                writeln!(out, "    {f}").unwrap();
            }
        }

        // Exported symbols
        if !impact.exported_symbols.is_empty() {
            writeln!(out, "  Exported symbols ({}): [ast]", impact.exported_symbols.len()).unwrap();
            let limit = if verbose { impact.exported_symbols.len() } else { 10 };
            for sym in impact.exported_symbols.iter().take(limit) {
                writeln!(out, "    {} [{}] at {}", sym.name, sym.kind, sym.location).unwrap();
            }
            if !verbose && impact.exported_symbols.len() > 10 {
                writeln!(out, "    ... and {} more (use --verbose to see all)", impact.exported_symbols.len() - 10).unwrap();
            }
        }

        // Contract surface
        if !impact.contract_surface.is_empty() {
            writeln!(out, "  Contract surface ({}): [ast]", impact.contract_surface.len()).unwrap();
            for item in &impact.contract_surface {
                writeln!(out, "    {} [{}] at {}", item.name, item.kind, item.location).unwrap();
                writeln!(out, "      why: {}", item.why).unwrap();
            }
        }

        // Dependents
        if !impact.direct_dependents.is_empty() {
            writeln!(out, "  Direct dependents ({}): [ast]", impact.direct_dependents.len()).unwrap();
            for dep in &impact.direct_dependents {
                writeln!(out, "    {} ({})", dep.package_dir, dep.basis).unwrap();
            }
        }

        // Sensitive areas
        if !impact.sensitive_areas.is_empty() {
            writeln!(out, "  Sensitive areas:").unwrap();
            for area in &impact.sensitive_areas {
                writeln!(out, "    {} — {}", area.name, area.description).unwrap();
            }
        }

        // Risks
        if !impact.risks.is_empty() {
            writeln!(out, "  Risks:").unwrap();
            for risk in &impact.risks {
                writeln!(out, "    [{}] {}", risk.basis, risk.description).unwrap();
            }
        }

        writeln!(out).unwrap();
    }

    // LSP semantic references (cross-package call sites)
    if let Some(ref lsp) = report.lsp_enrichment {
        if !lsp.additional_references.is_empty() {
            writeln!(
                out,
                "Semantic references ({}): [lsp]",
                lsp.additional_references.len()
            )
            .unwrap();
            writeln!(
                out,
                "  (cross-package call sites found by gopls for: {})",
                lsp.queried_symbols.join(", ")
            )
            .unwrap();
            let limit = if verbose { lsp.additional_references.len() } else { 20 };
            for r in lsp.additional_references.iter().take(limit) {
                let ctx = r.context.as_deref().unwrap_or("");
                if ctx.is_empty() {
                    writeln!(out, "    {}:{}", r.location.file, r.location.line).unwrap();
                } else {
                    writeln!(out, "    {}:{} — {}", r.location.file, r.location.line, ctx).unwrap();
                }
            }
            if !verbose && lsp.additional_references.len() > 20 {
                writeln!(
                    out,
                    "    ... and {} more (use --verbose to see all)",
                    lsp.additional_references.len() - 20
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    // Aggregated recommendations
    if !report.recommended_commands.is_empty() {
        writeln!(out, "Recommended checks:").unwrap();
        for cmd in &report.recommended_commands {
            writeln!(out, "  $ {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // Scope disclaimer
    writeln!(out, "Scope: {}", report.scope_note).unwrap();

    out
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn file_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

fn type_kind_label(kind: &TypeKind) -> String {
    match kind {
        TypeKind::Struct { .. } => "struct".into(),
        TypeKind::Interface { .. } => "interface".into(),
        TypeKind::Alias { .. } => "type_alias".into(),
    }
}

fn target_kind_label(kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::File => "file",
        TargetKind::Package => "package",
        TargetKind::Symbol => "symbol",
        TargetKind::Unresolved => "unresolved",
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
        ).unwrap();

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
"#,
        ).unwrap();

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
        ).unwrap();

        // Application use cases (depends on domain + ports)
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

        // Adapter layer (depends on application)
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

        // Actors (depends on application + adapters)
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

        root
    }

    #[test]
    fn resolves_file_target() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert_eq!(report.impacts.len(), 1);
        assert_eq!(report.impacts[0].kind, TargetKind::File);
        assert_eq!(
            report.impacts[0].resolved_package.as_deref(),
            Some("internal/domain/configctl")
        );
        assert!(!report.impacts[0].exported_symbols.is_empty());
    }

    #[test]
    fn resolves_package_target() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl".into()]);

        assert_eq!(report.impacts.len(), 1);
        assert_eq!(report.impacts[0].kind, TargetKind::Package);
        assert!(report.impacts[0].resolved_files.len() >= 2);
    }

    #[test]
    fn resolves_symbol_target() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["ConfigSet".into()]);

        assert_eq!(report.impacts.len(), 1);
        assert_eq!(report.impacts[0].kind, TargetKind::Symbol);
        assert!(report.impacts[0].exported_symbols.iter().any(|s| s.name == "ConfigSet"));
    }

    #[test]
    fn unresolved_target_produces_report() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["nonexistent/path.go".into()]);

        assert_eq!(report.impacts.len(), 1);
        assert_eq!(report.impacts[0].kind, TargetKind::Unresolved);
        assert!(!report.impacts[0].risks.is_empty());
    }

    #[test]
    fn finds_direct_dependents() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl".into()]);

        let deps = &report.impacts[0].direct_dependents;
        // application/configctl, adapters/nats, and actors/scopes/configctl all import domain
        assert!(
            !deps.is_empty(),
            "domain package should have dependents, got: {:?}",
            deps
        );
    }

    #[test]
    fn detects_contract_surface_interfaces() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/application/ports/configctl.go".into()]);

        let contracts = &report.impacts[0].contract_surface;
        assert!(
            contracts.iter().any(|c| c.name == "ConfigctlGateway" && c.kind == "interface"),
            "should detect ConfigctlGateway interface, got: {:?}",
            contracts
        );
    }

    #[test]
    fn detects_message_types_in_contracts() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/application/configctl/contracts".into()]);

        let contracts = &report.impacts[0].contract_surface;
        assert!(
            contracts.iter().any(|c| c.kind == "message_type"),
            "should detect message types in contracts/, got: {:?}",
            contracts
        );
    }

    #[test]
    fn matches_sensitive_areas() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        let areas = &report.impacts[0].sensitive_areas;
        assert!(
            areas.iter().any(|a| a.name == "domain"),
            "should match domain sensitive area, got: {:?}",
            areas
        );
    }

    #[test]
    fn produces_recommended_commands() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/config.go".into()]);

        assert!(
            !report.recommended_commands.is_empty(),
            "should recommend at least one command"
        );
    }

    #[test]
    fn multiple_targets_aggregated() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(
            root,
            &[
                "internal/domain/configctl/config.go".into(),
                "internal/adapters/nats/codec.go".into(),
            ],
        );

        assert_eq!(report.impacts.len(), 2);
        assert!(report.sensitive_areas_touched.len() >= 2);
        assert!(report.recommended_commands.len() >= 2);
    }

    #[test]
    fn empty_targets_produces_empty_report() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &[]);

        assert!(report.impacts.is_empty());
        assert!(report.recommended_commands.is_empty());
    }

    #[test]
    fn json_output_is_valid() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl".into()]);

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["targets"].is_array());
        assert!(parsed["impacts"].is_array());
        assert!(parsed["scope_note"].is_string());
    }

    #[test]
    fn human_output_contains_key_sections() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl".into()]);

        let human = render_human(&report, false);
        assert!(human.contains("Impact Map"));
        assert!(human.contains("Target:"));
        assert!(human.contains("Scope:"));
    }

    #[test]
    fn human_verbose_shows_files() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl".into()]);

        let verbose = render_human(&report, true);
        let terse = render_human(&report, false);
        assert!(verbose.contains("Files:"));
        assert!(!terse.contains("Files:"));
    }

    #[test]
    fn risk_basis_is_always_set() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl".into()]);

        for impact in &report.impacts {
            for risk in &impact.risks {
                assert!(
                    risk.basis == "observed" || risk.basis == "inferred",
                    "risk basis must be 'observed' or 'inferred', got: {}",
                    risk.basis
                );
            }
        }
    }

    #[test]
    fn leading_dot_slash_normalized() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["./internal/domain/configctl/config.go".into()]);

        assert_eq!(report.impacts[0].kind, TargetKind::File);
    }

    #[test]
    fn trailing_slash_normalized_for_package() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = analyze(root, &["internal/domain/configctl/".into()]);

        assert_eq!(report.impacts[0].kind, TargetKind::Package);
    }

    // ── LSP enrichment tests ──────────────────────────────────────────────

    #[test]
    fn lsp_fallback_when_unavailable() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = analyze_with_lsp(root, &["internal/domain/configctl".into()], &mut bridge);

        // AST results should still be present.
        assert_eq!(report.impacts.len(), 1);
        assert_eq!(report.impacts[0].kind, TargetKind::Package);
        assert!(!report.impacts[0].exported_symbols.is_empty());

        // LSP enrichment should be present but unavailable.
        let lsp = report.lsp_enrichment.as_ref().expect("should have lsp_enrichment");
        assert!(
            matches!(lsp.status, LspStatus::Unavailable { .. }),
            "LSP status should be unavailable, got: {:?}",
            lsp.status
        );
        assert!(lsp.additional_references.is_empty());
    }

    #[test]
    fn lsp_enrichment_absent_without_flag() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let report = analyze(root, &["internal/domain/configctl".into()]);
        assert!(
            report.lsp_enrichment.is_none(),
            "lsp_enrichment should be None when analyze() is used"
        );
    }

    #[test]
    fn lsp_enrichment_json_includes_section() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = analyze_with_lsp(root, &["internal/domain/configctl".into()], &mut bridge);
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(
            parsed["lsp_enrichment"].is_object(),
            "JSON should include lsp_enrichment section"
        );
        assert!(parsed["lsp_enrichment"]["queried_symbols"].is_array());
    }

    #[test]
    fn lsp_enrichment_absent_in_json_without_flag() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let report = analyze(root, &["internal/domain/configctl".into()]);
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(
            parsed.get("lsp_enrichment").is_none(),
            "lsp_enrichment should not appear when --lsp not used"
        );
    }

    #[test]
    fn lsp_renders_provenance_tags() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = analyze_with_lsp(root, &["internal/domain/configctl".into()], &mut bridge);
        let human = render_human(&report, false);

        assert!(human.contains("[ast]"), "output should contain [ast] tags");
        assert!(
            human.contains("LSP: unavailable"),
            "output should show LSP status"
        );
    }

    #[test]
    fn lsp_scope_note_mentions_unavailability() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("gopls not installed");

        let report = analyze_with_lsp(root, &["internal/domain/configctl".into()], &mut bridge);
        assert!(
            report.scope_note.contains("unavailable"),
            "scope note should mention LSP unavailability"
        );
    }

    #[test]
    fn lsp_queries_exported_symbols() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = analyze_with_lsp(root, &["internal/domain/configctl".into()], &mut bridge);
        let lsp = report.lsp_enrichment.as_ref().unwrap();

        // Should have queried at least some exported symbols.
        assert!(
            !lsp.queried_symbols.is_empty(),
            "should have queried exported symbols via LSP"
        );
    }

    #[test]
    fn lsp_empty_targets_no_crash() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = analyze_with_lsp(root, &[], &mut bridge);
        assert!(report.impacts.is_empty());
        assert!(report.lsp_enrichment.is_some());
    }
}
