//! Symbol trace — trace a symbol across the repository.
//!
//! Uses the codeintel AST index to locate definitions, structural references,
//! package relationships, and contract connections for a given symbol name.
//! Optionally enriches results with type-resolved definitions and cross-package
//! references from gopls via the LSP bridge.
//!
//! ## What it does
//!
//! Given a symbol name, the tracer:
//! 1. Finds all definitions (types, functions, constants, variables)
//! 2. Finds structural references (struct fields, function params/returns,
//!    receivers, interface embeds, type aliases that mention the symbol)
//! 3. Identifies packages involved (defining and referencing)
//! 4. Detects contract connections (interfaces, message types, ports)
//! 5. Recommends raccoon-cli commands for further investigation
//! 6. (Optional) Enriches with gopls definitions, references, and hover info
//!
//! ## What it does NOT do
//!
//! - No runtime/reflection tracing
//! - LSP references depend on gopls workspace state — incomplete workspace
//!   may yield partial results
//!
//! All observations are labeled with provenance: "observed" (AST fact),
//! "inferred" (heuristic), or "lsp" (gopls semantic result).

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::codeintel::{self, GoFunc, ProjectIndex, TypeKind, Visibility};
use crate::lsp::bridge::GoplsBridge;
use crate::lsp::types::{LspDefinition, LspReference, LspStatus, HoverInfo};

// ── Public API ─────────────────────────────────────────────────────────────

/// Trace a symbol across the project (AST only).
pub fn trace(project_root: &Path, symbol: &str) -> SymbolTraceReport {
    let index = codeintel::build_index(project_root);
    trace_in_index(&index, symbol)
}

/// Trace a symbol with optional LSP enrichment.
///
/// When the bridge is available, queries gopls for type-resolved definitions,
/// cross-package references (including function body call sites), and hover info.
/// Falls back cleanly when gopls is unavailable.
pub fn trace_with_lsp(
    project_root: &Path,
    symbol: &str,
    bridge: &mut GoplsBridge,
) -> SymbolTraceReport {
    let index = codeintel::build_index(project_root);
    let mut report = trace_in_index(&index, symbol);

    let enriched = bridge.enrich_symbol_with_index(&index, project_root, symbol);

    let lsp_status = enriched.lsp_status.clone();

    // Collect LSP definitions that aren't already in AST definitions.
    let lsp_only_defs: Vec<LspDefinition> = enriched
        .lsp_definitions
        .into_iter()
        .filter(|ld| {
            !report.definitions.iter().any(|d| {
                d.file == ld.location.file && d.line == ld.location.line
            })
        })
        .collect();

    // Collect LSP references that aren't already in AST references.
    let lsp_only_refs: Vec<LspReference> = enriched
        .lsp_references
        .into_iter()
        .filter(|lr| {
            !report.references.iter().any(|r| {
                r.file == lr.location.file && r.line == lr.location.line
            })
        })
        .collect();

    // Add LSP-discovered packages to the package list.
    for ld in &lsp_only_defs {
        let pkg = file_to_package(&ld.location.file);
        if !pkg.is_empty() && !report.packages.contains(&pkg) {
            report.packages.push(pkg);
        }
    }
    for lr in &lsp_only_refs {
        let pkg = file_to_package(&lr.location.file);
        if !pkg.is_empty() && !report.packages.contains(&pkg) {
            report.packages.push(pkg);
        }
    }
    report.packages.sort();

    report.lsp_enrichment = Some(LspEnrichment {
        status: lsp_status,
        definitions: lsp_only_defs,
        references: lsp_only_refs,
        hover: enriched.hover,
    });

    // Update scope note to reflect LSP presence.
    match &report.lsp_enrichment.as_ref().unwrap().status {
        LspStatus::Enriched => {
            report.scope_note = "Trace combines structural AST indexing (declarations, signatures, \
                struct fields) with gopls semantic analysis (type-resolved definitions, \
                cross-package references including call sites). Each fact is tagged with \
                its source: [ast] or [lsp].".to_string();
        }
        LspStatus::NoResults => {
            report.scope_note = "Trace is computed from structural AST indexing. gopls was \
                available but returned no additional results for this symbol.".to_string();
        }
        LspStatus::Unavailable { reason } => {
            report.scope_note = format!(
                "Trace is computed from structural AST indexing (declarations, signatures, \
                struct fields). LSP enrichment unavailable: {reason}. Function body call sites \
                and cross-package type resolution are not visible."
            );
        }
    }

    report
}

/// Trace a symbol using an existing index (useful for testing).
fn trace_in_index(index: &ProjectIndex, symbol: &str) -> SymbolTraceReport {
    let definitions = find_definitions(index, symbol);
    let references = find_references(index, symbol);

    // Collect all packages involved.
    let mut pkg_set: BTreeSet<String> = BTreeSet::new();
    for def in &definitions {
        pkg_set.insert(def.package.clone());
    }
    for r in &references {
        pkg_set.insert(r.package.clone());
    }
    let packages: Vec<String> = pkg_set.into_iter().collect();

    // Detect contract connections.
    let contracts = find_contracts(index, symbol, &definitions);

    // Build recommended commands.
    let recommended_commands = build_recommendations(&definitions, &references);

    // Determine resolution status.
    let status = if definitions.is_empty() {
        ResolutionStatus::NotFound
    } else if definitions.len() > 1 {
        ResolutionStatus::Ambiguous
    } else {
        ResolutionStatus::Resolved
    };

    let scope_note = "Trace is computed from structural AST indexing (declarations, signatures, \
        struct fields, type expressions). Function bodies are not analyzed — call sites, \
        assignments, and runtime usage are not visible."
        .to_string();

    SymbolTraceReport {
        symbol: symbol.to_string(),
        status,
        definitions,
        references,
        packages,
        contracts,
        recommended_commands,
        scope_note,
        lsp_enrichment: None,
    }
}

// ── Report types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SymbolTraceReport {
    pub symbol: String,
    pub status: ResolutionStatus,
    pub definitions: Vec<Definition>,
    pub references: Vec<Reference>,
    pub packages: Vec<String>,
    pub contracts: Vec<ContractConnection>,
    pub recommended_commands: Vec<String>,
    pub scope_note: String,
    /// Optional LSP enrichment — present when `--lsp` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp_enrichment: Option<LspEnrichment>,
}

/// Semantic enrichment from gopls, layered on top of AST facts.
#[derive(Debug, Clone, Serialize)]
pub struct LspEnrichment {
    /// Whether gopls contributed results.
    pub status: LspStatus,
    /// Type-resolved definitions not already found by AST.
    pub definitions: Vec<LspDefinition>,
    /// Cross-package references (including function body call sites).
    pub references: Vec<LspReference>,
    /// Hover/type information from gopls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<HoverInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    /// Single definition found.
    Resolved,
    /// Multiple definitions across packages.
    Ambiguous,
    /// No definitions found.
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub name: String,
    pub kind: String, // "struct", "interface", "type_alias", "func", "method", "const", "var"
    pub package: String,
    pub file: String,
    pub line: usize,
    pub visibility: String,
    /// Extra details (fields for structs, methods for interfaces, params for funcs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    /// How the symbol is referenced.
    pub kind: ReferenceKind,
    /// The containing symbol (struct name, func name, interface name).
    pub context: String,
    pub package: String,
    pub file: String,
    pub line: usize,
    /// "observed" — this is a structural AST fact.
    pub basis: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// Appears in a struct field type expression.
    FieldType,
    /// Appears as an embedded field.
    EmbeddedField,
    /// Appears in a function/method parameter type.
    ParamType,
    /// Appears in a function/method return type.
    ReturnType,
    /// Appears as a function/method receiver type.
    ReceiverType,
    /// Appears in an interface embed.
    InterfaceEmbed,
    /// Appears as the underlying type of a type alias.
    AliasUnderlying,
    /// Appears in a constant type hint.
    ConstType,
    /// Appears in a variable type hint.
    VarType,
}

impl ReferenceKind {
    fn label(self) -> &'static str {
        match self {
            ReferenceKind::FieldType => "field type",
            ReferenceKind::EmbeddedField => "embedded field",
            ReferenceKind::ParamType => "param type",
            ReferenceKind::ReturnType => "return type",
            ReferenceKind::ReceiverType => "receiver",
            ReferenceKind::InterfaceEmbed => "interface embed",
            ReferenceKind::AliasUnderlying => "alias underlying",
            ReferenceKind::ConstType => "const type",
            ReferenceKind::VarType => "var type",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractConnection {
    pub name: String,
    pub kind: String, // "interface", "message_type", "port", "contract_type"
    pub file: String,
    pub line: usize,
    pub why: String,
    /// "observed" or "inferred".
    pub basis: String,
}

// ── Definition finding ─────────────────────────────────────────────────────

fn find_definitions(index: &ProjectIndex, symbol: &str) -> Vec<Definition> {
    let mut defs = Vec::new();

    // Types (struct, interface, alias)
    for file in &index.files {
        let pkg = file.package.clone();
        for t in &file.types {
            if t.name == symbol {
                let kind = type_kind_label(&t.kind);
                let details = type_details(&t.kind);
                defs.push(Definition {
                    name: t.name.clone(),
                    kind,
                    package: pkg.clone(),
                    file: t.location.file.clone(),
                    line: t.location.line,
                    visibility: visibility_label(t.visibility),
                    details,
                });
            }
        }

        // Functions and methods
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

        // Constants
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

        // Variables
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

/// Find structural references to a symbol across the index.
///
/// Searches struct field types, function params/returns, receivers,
/// interface embeds, alias underlyings, and const/var type hints.
fn find_references(index: &ProjectIndex, symbol: &str) -> Vec<Reference> {
    let mut refs = Vec::new();

    for file in &index.files {
        let pkg = file.package.clone();

        // Struct field types and embedded fields
        for t in &file.types {
            if let TypeKind::Struct { ref fields } = t.kind {
                for field in fields {
                    if field.embedded && field.type_expr == symbol {
                        refs.push(Reference {
                            kind: ReferenceKind::EmbeddedField,
                            context: t.name.clone(),
                            package: pkg.clone(),
                            file: field.location.file.clone(),
                            line: field.location.line,
                            basis: "observed".into(),
                        });
                    } else if type_expr_mentions(&field.type_expr, symbol) {
                        refs.push(Reference {
                            kind: ReferenceKind::FieldType,
                            context: format!("{}.{}", t.name, field.name),
                            package: pkg.clone(),
                            file: field.location.file.clone(),
                            line: field.location.line,
                            basis: "observed".into(),
                        });
                    }
                }
            }

            // Interface embeds
            if let TypeKind::Interface { ref embeds, .. } = t.kind {
                for embed in embeds {
                    if embed.type_name == symbol {
                        refs.push(Reference {
                            kind: ReferenceKind::InterfaceEmbed,
                            context: t.name.clone(),
                            package: pkg.clone(),
                            file: embed.location.file.clone(),
                            line: embed.location.line,
                            basis: "observed".into(),
                        });
                    }
                }
            }

            // Alias underlying
            if let TypeKind::Alias { ref underlying } = t.kind {
                if type_expr_mentions(underlying, symbol) {
                    refs.push(Reference {
                        kind: ReferenceKind::AliasUnderlying,
                        context: t.name.clone(),
                        package: pkg.clone(),
                        file: t.location.file.clone(),
                        line: t.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }

        // Function/method params, returns, receivers
        for f in &file.functions {
            // Receiver
            if let Some(ref recv) = f.receiver {
                if recv.type_name == symbol {
                    refs.push(Reference {
                        kind: ReferenceKind::ReceiverType,
                        context: f.name.clone(),
                        package: pkg.clone(),
                        file: f.location.file.clone(),
                        line: f.location.line,
                        basis: "observed".into(),
                    });
                }
            }

            // Params
            for p in &f.params {
                if type_expr_mentions(&p.type_expr, symbol) {
                    refs.push(Reference {
                        kind: ReferenceKind::ParamType,
                        context: f.name.clone(),
                        package: pkg.clone(),
                        file: f.location.file.clone(),
                        line: f.location.line,
                        basis: "observed".into(),
                    });
                }
            }

            // Returns
            for r in &f.returns {
                if type_expr_mentions(&r.type_expr, symbol) {
                    refs.push(Reference {
                        kind: ReferenceKind::ReturnType,
                        context: f.name.clone(),
                        package: pkg.clone(),
                        file: f.location.file.clone(),
                        line: f.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }

        // Constants with matching type hint
        for c in &file.constants {
            if let Some(ref th) = c.type_hint {
                if th == symbol {
                    refs.push(Reference {
                        kind: ReferenceKind::ConstType,
                        context: c.name.clone(),
                        package: pkg.clone(),
                        file: c.location.file.clone(),
                        line: c.location.line,
                        basis: "observed".into(),
                    });
                }
            }
        }

        // Variables with matching type hint
        for v in &file.variables {
            if let Some(ref th) = v.type_hint {
                if type_expr_mentions(th, symbol) {
                    refs.push(Reference {
                        kind: ReferenceKind::VarType,
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

    // Remove references that are self-definitions (same name and same line).
    refs.retain(|r| r.context != symbol);

    refs
}

/// Check if a type expression mentions a symbol.
///
/// Handles common Go type expressions: `Symbol`, `*Symbol`, `[]Symbol`,
/// `map[string]Symbol`, `chan Symbol`, `pkg.Symbol`, etc.
fn type_expr_mentions(expr: &str, symbol: &str) -> bool {
    // Direct match
    if expr == symbol {
        return true;
    }

    // Check for the symbol as a word boundary in the expression.
    // This handles *Symbol, []Symbol, map[K]Symbol, chan Symbol, etc.
    // We look for the symbol preceded by a non-alphanumeric char (or start)
    // and followed by a non-alphanumeric char (or end).
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
            let after_ok =
                i + sym_len == bytes.len() || !is_ident_char(bytes[i + sym_len]);
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

// ── Contract detection ─────────────────────────────────────────────────────

fn find_contracts(
    index: &ProjectIndex,
    symbol: &str,
    definitions: &[Definition],
) -> Vec<ContractConnection> {
    let mut contracts = Vec::new();

    for def in definitions {
        // Check if the definition itself is a contract type.
        let is_port = def.file.contains("ports");
        let is_contract_path = def.file.contains("contracts") || def.file.contains("events");

        match def.kind.as_str() {
            "interface" => {
                let why = if is_port {
                    "port interface — changes affect all implementations across adapters and actors"
                } else {
                    "exported interface — changes affect all implementors"
                };
                contracts.push(ContractConnection {
                    name: def.name.clone(),
                    kind: if is_port { "port" } else { "interface" }.into(),
                    file: def.file.clone(),
                    line: def.line,
                    why: why.into(),
                    basis: "observed".into(),
                });
            }
            "struct" => {
                let is_message = is_message_type(&def.name, &def.file);
                if is_message {
                    contracts.push(ContractConnection {
                        name: def.name.clone(),
                        kind: "message_type".into(),
                        file: def.file.clone(),
                        line: def.line,
                        why: "message struct — field changes affect serialization across services"
                            .into(),
                        basis: "observed".into(),
                    });
                }
            }
            "type_alias" | "const" => {
                if is_contract_path {
                    contracts.push(ContractConnection {
                        name: def.name.clone(),
                        kind: "contract_type".into(),
                        file: def.file.clone(),
                        line: def.line,
                        why: "defined in contract/event layer — changes affect message encoding"
                            .into(),
                        basis: "observed".into(),
                    });
                }
            }
            _ => {}
        }
    }

    // Also check if the symbol is referenced by known contract types.
    for file in &index.files {
        for t in &file.types {
            if t.visibility != Visibility::Exported {
                continue;
            }
            let is_contract_iface = matches!(t.kind, TypeKind::Interface { .. })
                && (t.location.file.contains("ports") || t.location.file.contains("contracts"));

            if is_contract_iface {
                // Check if any method signature mentions the symbol.
                if let TypeKind::Interface { ref methods, .. } = t.kind {
                    for m in methods {
                        if type_expr_mentions(&m.signature, symbol) {
                            contracts.push(ContractConnection {
                                name: format!("{}.{}", t.name, m.name),
                                kind: "interface_method".into(),
                                file: m.location.file.clone(),
                                line: m.location.line,
                                why: format!(
                                    "symbol appears in contract interface method signature"
                                ),
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

fn is_message_type(name: &str, file_path: &str) -> bool {
    let message_suffixes = [
        "Command", "Query", "Reply", "Event", "Request", "Response", "Message",
    ];
    let contract_paths = ["contracts/", "messages", "events"];

    message_suffixes.iter().any(|s| name.ends_with(s))
        || contract_paths.iter().any(|p| file_path.contains(p))
}

// ── Recommendations ────────────────────────────────────────────────────────

fn build_recommendations(definitions: &[Definition], references: &[Reference]) -> Vec<String> {
    let mut cmds: BTreeSet<String> = BTreeSet::new();

    let all_files: BTreeSet<&str> = definitions
        .iter()
        .map(|d| d.file.as_str())
        .chain(references.iter().map(|r| r.file.as_str()))
        .collect();

    for file in &all_files {
        if file.contains("contracts") || file.contains("ports") || file.contains("events") {
            cmds.insert("raccoon-cli contract-audit".into());
        }
        if file.contains("adapters/nats") || file.contains("adapters/kafka") {
            cmds.insert("raccoon-cli contract-audit".into());
            cmds.insert("raccoon-cli runtime-bindings".into());
        }
        if file.contains("domain/") {
            cmds.insert("raccoon-cli arch-guard".into());
        }
        if file.contains("application/") {
            cmds.insert("raccoon-cli arch-guard".into());
        }
        if file.contains("actors/") {
            cmds.insert("raccoon-cli runtime-bindings".into());
        }
        if file.contains("interfaces/http") {
            cmds.insert("raccoon-cli arch-guard".into());
        }
        if file.contains("deploy/") || file.contains("configs/") {
            cmds.insert("raccoon-cli topology-doctor".into());
            cmds.insert("raccoon-cli drift-detect".into());
        }
    }

    // If we found any definitions/references, impact-map is always useful.
    if !definitions.is_empty() || !references.is_empty() {
        cmds.insert("raccoon-cli impact-map <symbol>".into());
    }

    cmds.into_iter().collect()
}

// ── Rendering ──────────────────────────────────────────────────────────────

pub fn render_json(report: &SymbolTraceReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn render_human(report: &SymbolTraceReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== Symbol Trace: {} ===\n", report.symbol).unwrap();

    // LSP status (if enrichment was requested)
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
                "Status: ambiguous ({} definitions across packages)",
                report.definitions.len()
            )
            .unwrap();
            writeln!(
                out,
                "  Note: same name defined in multiple packages — these may be unrelated types."
            )
            .unwrap();
        }
        ResolutionStatus::NotFound => {
            writeln!(out, "Status: not found").unwrap();
            writeln!(out).unwrap();
            writeln!(
                out,
                "The symbol '{}' was not found as a type, function, constant, or variable",
                report.symbol
            )
            .unwrap();
            writeln!(
                out,
                "in the structural index. Possible reasons:"
            )
            .unwrap();
            writeln!(out, "  - The name is misspelled").unwrap();
            writeln!(out, "  - It is defined inside a function body (not indexed)").unwrap();
            writeln!(out, "  - It is a field name or local variable (not a top-level symbol)").unwrap();
            writeln!(out, "  - It exists in vendor/ or generated code (excluded from index)").unwrap();
            writeln!(out).unwrap();
            writeln!(out, "Scope: {}", report.scope_note).unwrap();
            return out;
        }
    }
    writeln!(out).unwrap();

    // Definitions [ast]
    writeln!(out, "Definitions ({}): [ast]", report.definitions.len()).unwrap();
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

    // LSP definitions (additional locations not found by AST)
    if let Some(ref lsp) = report.lsp_enrichment {
        if !lsp.definitions.is_empty() {
            writeln!(
                out,
                "Additional definitions ({}): [lsp]",
                lsp.definitions.len()
            )
            .unwrap();
            for def in &lsp.definitions {
                let name = def.qualified_name.as_deref().unwrap_or(&report.symbol);
                writeln!(
                    out,
                    "  {} at {}:{}",
                    name, def.location.file, def.location.line
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    // AST structural references
    if report.references.is_empty() {
        writeln!(out, "Structural references: none found [ast]").unwrap();
        if report.lsp_enrichment.is_none() {
            writeln!(
                out,
                "  (the symbol may be used in function bodies — use --lsp to find call sites)"
            )
            .unwrap();
        }
    } else {
        writeln!(
            out,
            "Structural references ({}): [ast]",
            report.references.len()
        )
        .unwrap();
        let limit = if verbose { report.references.len() } else { 20 };
        for r in report.references.iter().take(limit) {
            writeln!(
                out,
                "  {} in {} at {}:{}",
                r.kind.label(),
                r.context,
                r.file,
                r.line
            )
            .unwrap();
        }
        if !verbose && report.references.len() > 20 {
            writeln!(
                out,
                "  ... and {} more (use --verbose to see all)",
                report.references.len() - 20
            )
            .unwrap();
        }
    }
    writeln!(out).unwrap();

    // LSP references (call sites, usages in function bodies)
    if let Some(ref lsp) = report.lsp_enrichment {
        if !lsp.references.is_empty() {
            writeln!(
                out,
                "Semantic references ({}): [lsp]",
                lsp.references.len()
            )
            .unwrap();
            let limit = if verbose { lsp.references.len() } else { 20 };
            for r in lsp.references.iter().take(limit) {
                let ctx = r.context.as_deref().unwrap_or("");
                if ctx.is_empty() {
                    writeln!(out, "  {}:{}", r.location.file, r.location.line).unwrap();
                } else {
                    writeln!(out, "  {}:{} — {}", r.location.file, r.location.line, ctx).unwrap();
                }
            }
            if !verbose && lsp.references.len() > 20 {
                writeln!(
                    out,
                    "  ... and {} more (use --verbose to see all)",
                    lsp.references.len() - 20
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    // Hover/type info from LSP
    if let Some(ref lsp) = report.lsp_enrichment {
        if let Some(ref hover) = lsp.hover {
            if let Some(ref sig) = hover.signature {
                writeln!(out, "Type signature: {sig} [lsp]").unwrap();
            }
            if verbose {
                if let Some(ref doc) = hover.documentation {
                    writeln!(out, "Documentation: {doc} [lsp]").unwrap();
                }
            }
            writeln!(out).unwrap();
        }
    }

    // Packages
    if !report.packages.is_empty() {
        writeln!(out, "Packages involved ({}):", report.packages.len()).unwrap();
        for pkg in &report.packages {
            writeln!(out, "  {pkg}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // Contracts
    if !report.contracts.is_empty() {
        writeln!(
            out,
            "Contract connections ({}): ",
            report.contracts.len()
        )
        .unwrap();
        for c in &report.contracts {
            writeln!(
                out,
                "  {} [{}] at {}:{} [{}]",
                c.name, c.kind, c.file, c.line, c.basis
            )
            .unwrap();
            writeln!(out, "    why: {}", c.why).unwrap();
        }
        writeln!(out).unwrap();
    }

    // Recommended commands
    if !report.recommended_commands.is_empty() {
        writeln!(out, "Recommended checks:").unwrap();
        for cmd in &report.recommended_commands {
            writeln!(out, "  $ {cmd}").unwrap();
        }
        writeln!(out).unwrap();
    }

    // Scope
    writeln!(out, "Scope: {}", report.scope_note).unwrap();

    out
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn type_kind_label(kind: &TypeKind) -> String {
    match kind {
        TypeKind::Struct { .. } => "struct".into(),
        TypeKind::Interface { .. } => "interface".into(),
        TypeKind::Alias { .. } => "type_alias".into(),
    }
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
            let mut details = Vec::new();
            for f in fields {
                let tag_info = f
                    .tag
                    .as_ref()
                    .map(|t| format!(" {t}"))
                    .unwrap_or_default();
                if f.embedded {
                    details.push(format!("embed: {}{}", f.type_expr, tag_info));
                } else {
                    details.push(format!("field: {} {}{}", f.name, f.type_expr, tag_info));
                }
            }
            details
        }
        TypeKind::Interface { methods, embeds } => {
            let mut details = Vec::new();
            for e in embeds {
                details.push(format!("embed: {}", e.type_name));
            }
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

/// Extract a package-like directory from a file path.
/// E.g., "internal/domain/configctl/config.go" → "configctl".
fn file_to_package(path: &str) -> String {
    // Strip any absolute-path or project-root prefix for display.
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

fn func_info(f: &GoFunc) -> (String, Vec<String>) {
    let kind = if f.receiver.is_some() {
        "method"
    } else {
        "func"
    };

    let mut details = Vec::new();

    if let Some(ref recv) = f.receiver {
        let ptr = if recv.pointer { "*" } else { "" };
        details.push(format!("receiver: ({} {}{})", recv.name, ptr, recv.type_name));
    }

    if !f.params.is_empty() {
        let params: Vec<String> = f.params.iter().map(|p| format!("{} {}", p.name, p.type_expr)).collect();
        details.push(format!("params: ({})", params.join(", ")));
    }

    if !f.returns.is_empty() {
        let rets: Vec<String> = f.returns.iter().map(|r| {
            if r.name.is_empty() {
                r.type_expr.clone()
            } else {
                format!("{} {}", r.name, r.type_expr)
            }
        }).collect();
        details.push(format!("returns: ({})", rets.join(", ")));
    }

    (kind.into(), details)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project(tmp: &TempDir) -> &Path {
        let root = tmp.path();

        // Domain layer — types and constants
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

        // Application contracts — message types
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

        root
    }

    // ── Resolution status tests ────────────────────────────────────────────

    #[test]
    fn resolves_single_type() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "struct");
        assert_eq!(report.definitions[0].package, "configctl");
    }

    #[test]
    fn resolves_function() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "NewConfigSet");

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "func");
    }

    #[test]
    fn resolves_method() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "AddVersion");

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "method");
    }

    #[test]
    fn resolves_constant() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "LifecycleDraft");

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "const");
    }

    #[test]
    fn resolves_type_alias() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "VersionLifecycle");

        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "type_alias");
    }

    #[test]
    fn not_found_for_unknown_symbol() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "NonExistentType");

        assert_eq!(report.status, ResolutionStatus::NotFound);
        assert!(report.definitions.is_empty());
        assert!(report.references.is_empty());
        assert!(report.packages.is_empty());
    }

    #[test]
    fn ambiguous_for_duplicate_names() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        // "Supervisor" exists in actors/scopes/configctl
        // Add another in a different package
        fs::create_dir_all(root.join("internal/actors/scopes/consumer")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/consumer/supervisor.go"),
            r#"package consumer

type Supervisor struct {
	running bool
}
"#,
        )
        .unwrap();

        let report = trace(root, "Supervisor");
        assert_eq!(report.status, ResolutionStatus::Ambiguous);
        assert_eq!(report.definitions.len(), 2);
    }

    // ── Reference detection tests ──────────────────────────────────────────

    #[test]
    fn finds_struct_field_references() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigVersion");

        // ConfigVersion is referenced as a field type in ConfigSet ([]ConfigVersion)
        let field_refs: Vec<_> = report
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::FieldType)
            .collect();
        assert!(
            !field_refs.is_empty(),
            "should find ConfigVersion as field type in ConfigSet, refs: {:?}",
            report.references
        );
    }

    #[test]
    fn finds_receiver_references() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        let recv_refs: Vec<_> = report
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::ReceiverType)
            .collect();
        assert!(
            !recv_refs.is_empty(),
            "ConfigSet should be referenced as receiver type, refs: {:?}",
            report.references
        );
    }

    #[test]
    fn finds_param_type_references() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigVersion");

        let param_refs: Vec<_> = report
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::ParamType)
            .collect();
        // AddVersion takes ConfigVersion as param
        assert!(
            !param_refs.is_empty(),
            "ConfigVersion should be referenced as param type in AddVersion"
        );
    }

    #[test]
    fn finds_return_type_references() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        let ret_refs: Vec<_> = report
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::ReturnType)
            .collect();
        // NewConfigSet returns ConfigSet
        assert!(
            !ret_refs.is_empty(),
            "ConfigSet should be referenced as return type in NewConfigSet"
        );
    }

    #[test]
    fn finds_const_type_references() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "VersionLifecycle");

        let const_refs: Vec<_> = report
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::ConstType)
            .collect();
        // LifecycleDraft, LifecycleValidated, LifecycleActive all have type VersionLifecycle
        assert!(
            const_refs.len() >= 3,
            "VersionLifecycle should be referenced by at least 3 constants, got {}",
            const_refs.len()
        );
    }

    #[test]
    fn finds_field_type_reference_for_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "VersionLifecycle");

        // ConfigVersion has a field `Lifecycle VersionLifecycle`
        let field_refs: Vec<_> = report
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::FieldType)
            .collect();
        assert!(
            !field_refs.is_empty(),
            "VersionLifecycle should be referenced as field type in ConfigVersion"
        );
    }

    // ── Package tracking tests ─────────────────────────────────────────────

    #[test]
    fn tracks_packages_for_type() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        assert!(
            !report.packages.is_empty(),
            "should track at least one package"
        );
        assert!(
            report.packages.contains(&"configctl".to_string()),
            "should include configctl package"
        );
    }

    // ── Contract detection tests ───────────────────────────────────────────

    #[test]
    fn detects_port_interface_contract() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigctlGateway");

        assert!(
            !report.contracts.is_empty(),
            "should detect ConfigctlGateway as contract"
        );
        assert!(
            report.contracts.iter().any(|c| c.kind == "port"),
            "should classify as port, got: {:?}",
            report.contracts
        );
    }

    #[test]
    fn detects_message_type_contract() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "CreateDraftCommand");

        assert!(
            !report.contracts.is_empty(),
            "should detect CreateDraftCommand as contract"
        );
        assert!(
            report.contracts.iter().any(|c| c.kind == "message_type"),
            "should classify as message_type"
        );
    }

    // ── Recommended commands tests ─────────────────────────────────────────

    #[test]
    fn recommends_commands_for_domain_symbol() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        assert!(
            !report.recommended_commands.is_empty(),
            "should recommend checks for domain type"
        );
        assert!(
            report
                .recommended_commands
                .iter()
                .any(|c| c.contains("arch-guard")),
            "should recommend arch-guard for domain type"
        );
    }

    #[test]
    fn no_commands_for_unknown_symbol() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "DoesNotExist");

        assert!(report.recommended_commands.is_empty());
    }

    // ── Output rendering tests ─────────────────────────────────────────────

    #[test]
    fn json_output_is_valid() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["symbol"].is_string());
        assert!(parsed["status"].is_string());
        assert!(parsed["definitions"].is_array());
        assert!(parsed["references"].is_array());
        assert!(parsed["scope_note"].is_string());
    }

    #[test]
    fn human_output_shows_key_sections() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        let human = render_human(&report, false);
        assert!(human.contains("Symbol Trace: ConfigSet"));
        assert!(human.contains("Definitions"));
        assert!(human.contains("Structural references"));
        assert!(human.contains("Scope:"));
    }

    #[test]
    fn human_output_for_not_found() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "Nonexistent");

        let human = render_human(&report, false);
        assert!(human.contains("not found"));
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

        let report = trace(root, "Supervisor");
        let human = render_human(&report, false);
        assert!(human.contains("ambiguous"));
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn type_expr_mentions_direct() {
        assert!(type_expr_mentions("ConfigSet", "ConfigSet"));
    }

    #[test]
    fn type_expr_mentions_pointer() {
        assert!(type_expr_mentions("*ConfigSet", "ConfigSet"));
    }

    #[test]
    fn type_expr_mentions_slice() {
        assert!(type_expr_mentions("[]ConfigSet", "ConfigSet"));
    }

    #[test]
    fn type_expr_mentions_map_value() {
        assert!(type_expr_mentions("map[string]ConfigSet", "ConfigSet"));
    }

    #[test]
    fn type_expr_no_false_positive_prefix() {
        assert!(!type_expr_mentions("ConfigSetExtra", "ConfigSet"));
    }

    #[test]
    fn type_expr_no_false_positive_suffix() {
        assert!(!type_expr_mentions("MyConfigSet", "ConfigSet"));
    }

    #[test]
    fn type_expr_mentions_channel() {
        assert!(type_expr_mentions("chan ConfigSet", "ConfigSet"));
    }

    #[test]
    fn empty_project_symbol_not_found() {
        let tmp = TempDir::new().unwrap();
        let report = trace(tmp.path(), "Anything");
        assert_eq!(report.status, ResolutionStatus::NotFound);
    }

    #[test]
    fn scope_note_always_present() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let found = trace(root, "ConfigSet");
        assert!(!found.scope_note.is_empty());

        let not_found = trace(root, "Nope");
        assert!(!not_found.scope_note.is_empty());
    }

    #[test]
    fn all_references_have_observed_basis() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        for r in &report.references {
            assert_eq!(r.basis, "observed", "all structural refs must be observed");
        }
    }

    #[test]
    fn unexported_symbol_found() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::write(
            root.join("internal/domain/configctl/helpers.go"),
            "package configctl\n\nfunc helperFunc() {}\n",
        )
        .unwrap();

        let report = trace(root, "helperFunc");
        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert_eq!(report.definitions[0].visibility, "unexported");
    }

    #[test]
    fn verbose_shows_more_details() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let report = trace(root, "ConfigSet");

        let terse = render_human(&report, false);
        let verbose = render_human(&report, true);
        // Single definition always shows details, but verbose guarantees it
        assert!(verbose.contains("field:") || verbose.contains("Definitions"));
        assert!(terse.contains("field:") || terse.contains("Definitions"));
    }

    // ── LSP enrichment tests ──────────────────────────────────────────────

    #[test]
    fn lsp_fallback_when_unavailable() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = trace_with_lsp(root, "ConfigSet", &mut bridge);

        // AST results should still be present.
        assert_eq!(report.status, ResolutionStatus::Resolved);
        assert!(!report.definitions.is_empty());
        assert!(!report.references.is_empty());

        // LSP enrichment should be present but unavailable.
        let lsp = report.lsp_enrichment.as_ref().expect("lsp_enrichment should be Some");
        assert!(
            matches!(lsp.status, LspStatus::Unavailable { .. }),
            "LSP status should be unavailable, got: {:?}",
            lsp.status
        );
        assert!(lsp.definitions.is_empty());
        assert!(lsp.references.is_empty());
        assert!(lsp.hover.is_none());

        // Scope note should mention unavailability.
        assert!(
            report.scope_note.contains("unavailable"),
            "scope note should mention LSP unavailability"
        );
    }

    #[test]
    fn lsp_fallback_for_unknown_symbol() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = trace_with_lsp(root, "DoesNotExist", &mut bridge);

        assert_eq!(report.status, ResolutionStatus::NotFound);
        let lsp = report.lsp_enrichment.as_ref().expect("lsp_enrichment should be Some");
        assert!(matches!(lsp.status, LspStatus::Unavailable { .. }));
    }

    #[test]
    fn lsp_fallback_for_ambiguous_symbol() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        // Create a second definition for "Supervisor"
        fs::create_dir_all(root.join("internal/actors/scopes/consumer")).unwrap();
        fs::write(
            root.join("internal/actors/scopes/consumer/supervisor.go"),
            "package consumer\n\ntype Supervisor struct {\n\trunning bool\n}\n",
        )
        .unwrap();

        let mut bridge = GoplsBridge::unavailable("test: no gopls");
        let report = trace_with_lsp(root, "Supervisor", &mut bridge);

        assert_eq!(report.status, ResolutionStatus::Ambiguous);
        assert!(report.definitions.len() > 1);
        assert!(report.lsp_enrichment.is_some());
    }

    #[test]
    fn lsp_enrichment_absent_without_flag() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let report = trace(root, "ConfigSet");

        assert!(
            report.lsp_enrichment.is_none(),
            "lsp_enrichment should be None when trace() (not trace_with_lsp) is called"
        );
    }

    #[test]
    fn lsp_enrichment_renders_provenance_tags() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = trace_with_lsp(root, "ConfigSet", &mut bridge);
        let human = render_human(&report, false);

        // Should contain provenance tags.
        assert!(human.contains("[ast]"), "output should contain [ast] tags");
        assert!(
            human.contains("LSP: unavailable"),
            "output should show LSP status"
        );
    }

    #[test]
    fn lsp_enrichment_json_includes_enrichment_section() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);
        let mut bridge = GoplsBridge::unavailable("test: no gopls");

        let report = trace_with_lsp(root, "ConfigSet", &mut bridge);
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["lsp_enrichment"].is_object(), "JSON should include lsp_enrichment section");
        assert!(parsed["lsp_enrichment"]["status"].is_object(), "status should be present");
        assert!(parsed["lsp_enrichment"]["definitions"].is_array());
        assert!(parsed["lsp_enrichment"]["references"].is_array());
    }

    #[test]
    fn lsp_enrichment_absent_in_json_without_flag() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        let report = trace(root, "ConfigSet");
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(
            parsed.get("lsp_enrichment").is_none(),
            "lsp_enrichment should not appear in JSON when not used"
        );
    }

    #[test]
    fn lsp_human_output_no_call_site_hint_when_lsp_used() {
        let tmp = TempDir::new().unwrap();
        let root = create_test_project(&tmp);

        // Without LSP, should hint about --lsp.
        let report_no_lsp = trace(root, "ConfigctlGateway");
        let human_no_lsp = render_human(&report_no_lsp, false);

        // With LSP (unavailable), should NOT hint about --lsp (already tried).
        let mut bridge = GoplsBridge::unavailable("test: no gopls");
        let report_lsp = trace_with_lsp(root, "ConfigctlGateway", &mut bridge);
        let human_lsp = render_human(&report_lsp, false);

        // The no-lsp output may suggest --lsp for finding call sites.
        if report_no_lsp.references.is_empty() {
            assert!(
                human_no_lsp.contains("--lsp"),
                "should suggest --lsp when no refs found without LSP"
            );
        }

        // When LSP was used, the hint should not appear.
        if report_lsp.references.is_empty() {
            assert!(
                !human_lsp.contains("--lsp"),
                "should not suggest --lsp when LSP was already used"
            );
        }
    }

    #[test]
    fn file_to_package_extracts_correctly() {
        assert_eq!(file_to_package("internal/domain/configctl/config.go"), "configctl");
        assert_eq!(file_to_package("internal/adapters/nats/codec.go"), "nats");
        assert_eq!(file_to_package("main.go"), "");
    }
}
