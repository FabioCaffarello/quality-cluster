//! Golden snapshot generator for code intelligence.
//!
//! Captures a stable, deterministic representation of the repository's structural
//! and semantic state. The snapshot is designed for:
//!
//!   - **Comparison**: diff two snapshots to detect structural drift over time.
//!   - **Auditing**: every fact is tagged with its provenance (ast, lsp, inferred).
//!   - **Debugging**: understand what the code intelligence layer actually sees.
//!
//! ## What enters the snapshot
//!
//! | Section          | Source   | Description                                     |
//! |------------------|----------|-------------------------------------------------|
//! | packages         | ast      | Package names, directories, file lists           |
//! | imports          | ast      | Deduplicated import paths with classification    |
//! | types            | ast      | Structs, interfaces, aliases with fields/methods |
//! | functions        | ast      | Exported function/method signatures              |
//! | constants        | ast      | Typed constants with values                      |
//! | interfaces       | ast      | Interface contracts with method sets              |
//! | arch_layers      | inferred | Layer classification per package                 |
//! | contracts        | ast      | Detected contract types and families              |
//! | stats            | ast      | Aggregate counters                                |
//! | metadata         | runtime  | Timestamp, version, project root                  |
//!
//! ## Stability guarantees
//!
//! - All collections are sorted by canonical key (name, path, etc.).
//! - No non-deterministic data (process IDs, absolute paths, etc.) unless
//!   explicitly opted in.
//! - Same source tree → same snapshot (modulo metadata.generated_at).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::codeintel;
use crate::codeintel::{GoFunc, ImportKind, ProjectIndex, TypeKind, Visibility};

// ── Snapshot model ─────────────────────────────────────────────────────

/// Provenance tag for a snapshot fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Deterministic structural fact from AST parsing.
    Ast,
    /// Semantic fact from gopls LSP enrichment.
    Lsp,
    /// Derived by heuristic or cross-reference analysis.
    Inferred,
    /// Runtime metadata (timestamp, version).
    Runtime,
}

/// Top-level golden snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: String,
    pub metadata: SnapshotMetadata,
    pub packages: Vec<PackageEntry>,
    pub imports: Vec<ImportEntry>,
    pub types: Vec<TypeEntry>,
    pub functions: Vec<FunctionEntry>,
    pub constants: Vec<ConstantEntry>,
    pub interfaces: Vec<InterfaceEntry>,
    pub arch_layers: Vec<ArchLayerEntry>,
    pub contracts: Vec<ContractEntry>,
    pub stats: SnapshotStats,
}

/// Non-structural metadata about the snapshot generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub generated_at: String,
    pub project_root: String,
    pub raccoon_version: String,
    pub provenance: Provenance,
}

/// A package in the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    pub name: String,
    pub dir: String,
    pub files: Vec<String>,
    pub file_count: usize,
    pub provenance: Provenance,
}

/// A deduplicated import path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEntry {
    pub path: String,
    pub kind: String,
    pub used_by: Vec<String>,
    pub provenance: Provenance,
}

/// A type definition in the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeEntry {
    pub name: String,
    pub package: String,
    pub kind: String,
    pub visibility: String,
    pub file: String,
    pub line: usize,
    pub fields: Vec<FieldEntry>,
    pub provenance: Provenance,
}

/// A struct field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldEntry {
    pub name: String,
    pub type_expr: String,
    pub embedded: bool,
}

/// A function or method signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionEntry {
    pub name: String,
    pub package: String,
    pub receiver: Option<String>,
    pub signature: String,
    pub visibility: String,
    pub file: String,
    pub line: usize,
    pub provenance: Provenance,
}

/// A constant declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantEntry {
    pub name: String,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub visibility: String,
    pub provenance: Provenance,
}

/// An interface contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceEntry {
    pub name: String,
    pub package: String,
    pub methods: Vec<String>,
    pub embeds: Vec<String>,
    pub file: String,
    pub line: usize,
    pub provenance: Provenance,
}

/// Architectural layer assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchLayerEntry {
    pub package_dir: String,
    pub layer: String,
    pub provenance: Provenance,
}

/// A detected contract type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEntry {
    pub name: String,
    pub family: String,
    pub file: String,
    pub line: usize,
    pub provenance: Provenance,
}

/// Aggregate counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotStats {
    pub total_files: usize,
    pub total_packages: usize,
    pub total_types: usize,
    pub total_functions: usize,
    pub total_constants: usize,
    pub total_imports: usize,
    pub total_lines: usize,
    pub structs: usize,
    pub interfaces: usize,
    pub type_aliases: usize,
    pub exported_types: usize,
    pub exported_functions: usize,
    pub test_files: usize,
    pub arch_layers_detected: usize,
    pub contracts_detected: usize,
}

// ── Layer detection (mirrors arch_guard layer model) ───────────────────

const LAYERS: &[&str] = &["domain", "application", "adapters", "actors", "interfaces"];

fn detect_layer(dir: &str) -> Option<&'static str> {
    for &layer in LAYERS {
        if dir.contains(&format!("internal/{layer}")) || dir.contains(&format!("internal\\{layer}"))
        {
            return Some(layer);
        }
    }
    if dir.starts_with("cmd") || dir == "cmd" {
        return Some("cmd");
    }
    if dir.starts_with("tools") || dir == "tools" {
        return Some("tools");
    }
    if dir.contains("shared") {
        return Some("shared");
    }
    None
}

// ── Contract detection heuristics ──────────────────────────────────────

const CONTRACT_SUFFIXES: &[(&str, &str)] = &[
    ("Command", "command"),
    ("Query", "query"),
    ("Reply", "reply"),
    ("Event", "event"),
    ("Envelope", "envelope"),
    ("Record", "record"),
    ("Binding", "binding"),
    ("Gateway", "port"),
    ("Repository", "port"),
    ("Port", "port"),
    ("Service", "port"),
];

fn detect_contract_family(type_name: &str) -> Option<&'static str> {
    for &(suffix, family) in CONTRACT_SUFFIXES {
        if type_name.ends_with(suffix) && type_name != suffix {
            return Some(family);
        }
    }
    None
}

// ── Snapshot generation ────────────────────────────────────────────────

/// Generate a golden snapshot of the project's code intelligence.
pub fn generate(project_root: &Path) -> Snapshot {
    let index = codeintel::index::build_index(project_root);
    build_snapshot_from_index(&index, project_root)
}

/// Build a snapshot from an existing ProjectIndex (useful for testing).
pub fn build_snapshot_from_index(index: &ProjectIndex, project_root: &Path) -> Snapshot {
    let now = chrono_now();
    let root_display = project_root.display().to_string();

    let metadata = SnapshotMetadata {
        generated_at: now,
        project_root: root_display,
        raccoon_version: env!("CARGO_PKG_VERSION").to_string(),
        provenance: Provenance::Runtime,
    };

    let packages = build_packages(index);
    let imports = build_imports(index);
    let types = build_types(index);
    let functions = build_functions(index);
    let constants = build_constants(index);
    let interfaces = build_interfaces(index);
    let arch_layers = build_arch_layers(index);
    let contracts = build_contracts(index);

    let stats = SnapshotStats {
        total_files: index.stats.total_files,
        total_packages: index.stats.total_packages,
        total_types: index.stats.total_types,
        total_functions: index.stats.total_functions,
        total_constants: index.stats.total_constants,
        total_imports: index.stats.total_imports,
        total_lines: index.stats.total_lines,
        structs: index.stats.structs,
        interfaces: index.stats.interfaces,
        type_aliases: index.stats.type_aliases,
        exported_types: index.stats.exported_types,
        exported_functions: index.stats.exported_functions,
        test_files: index.stats.test_files,
        arch_layers_detected: arch_layers.len(),
        contracts_detected: contracts.len(),
    };

    Snapshot {
        version: "1".to_string(),
        metadata,
        packages,
        imports,
        types,
        functions,
        constants,
        interfaces,
        arch_layers,
        contracts,
        stats,
    }
}

fn build_packages(index: &ProjectIndex) -> Vec<PackageEntry> {
    let mut entries: Vec<PackageEntry> = index
        .packages
        .iter()
        .map(|pkg| {
            let mut files = pkg.files.clone();
            files.sort();
            let file_count = files.len();
            PackageEntry {
                name: pkg.name.clone(),
                dir: pkg.dir.clone(),
                files,
                file_count,
                provenance: Provenance::Ast,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.dir.cmp(&b.dir).then_with(|| a.name.cmp(&b.name)));
    entries
}

fn build_imports(index: &ProjectIndex) -> Vec<ImportEntry> {
    let mut import_map: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();

    for file in &index.files {
        for imp in &file.imports {
            let kind = match imp.kind {
                ImportKind::Stdlib => "stdlib",
                ImportKind::Internal => "internal",
                ImportKind::External => "external",
            };
            let entry = import_map
                .entry(imp.path.clone())
                .or_insert_with(|| (kind.to_string(), Vec::new()));
            let pkg_dir = file_dir(&file.path);
            if !entry.1.contains(&pkg_dir) {
                entry.1.push(pkg_dir);
            }
        }
    }

    import_map
        .into_iter()
        .map(|(path, (kind, mut used_by))| {
            used_by.sort();
            ImportEntry {
                path,
                kind,
                used_by,
                provenance: Provenance::Ast,
            }
        })
        .collect()
}

fn build_types(index: &ProjectIndex) -> Vec<TypeEntry> {
    let mut entries: Vec<TypeEntry> = Vec::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }
        for t in &file.types {
            let (kind_str, fields) = match &t.kind {
                TypeKind::Struct { fields } => {
                    let fs: Vec<FieldEntry> = fields
                        .iter()
                        .map(|f| FieldEntry {
                            name: f.name.clone(),
                            type_expr: f.type_expr.clone(),
                            embedded: f.embedded,
                        })
                        .collect();
                    ("struct", fs)
                }
                TypeKind::Interface { .. } => ("interface", Vec::new()),
                TypeKind::Alias { underlying } => (
                    if underlying.contains("=") {
                        "alias"
                    } else {
                        "alias"
                    },
                    Vec::new(),
                ),
            };

            entries.push(TypeEntry {
                name: t.name.clone(),
                package: file.package.clone(),
                kind: kind_str.to_string(),
                visibility: vis_str(t.visibility),
                file: file.path.clone(),
                line: t.location.line,
                fields,
                provenance: Provenance::Ast,
            });
        }
    }

    entries.sort_by(|a, b| a.package.cmp(&b.package).then_with(|| a.name.cmp(&b.name)));
    entries
}

fn build_functions(index: &ProjectIndex) -> Vec<FunctionEntry> {
    let mut entries: Vec<FunctionEntry> = Vec::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }
        for f in &file.functions {
            if f.visibility != Visibility::Exported {
                continue;
            }
            entries.push(func_entry(f, &file.package, &file.path));
        }
    }

    entries.sort_by(|a, b| {
        a.package
            .cmp(&b.package)
            .then_with(|| a.receiver.cmp(&b.receiver))
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

fn func_entry(f: &GoFunc, package: &str, file_path: &str) -> FunctionEntry {
    let receiver = f.receiver.as_ref().map(|r| {
        if r.pointer {
            format!("*{}", r.type_name)
        } else {
            r.type_name.clone()
        }
    });

    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            if p.name.is_empty() {
                p.type_expr.clone()
            } else {
                format!("{} {}", p.name, p.type_expr)
            }
        })
        .collect();

    let returns: Vec<String> = f
        .returns
        .iter()
        .map(|p| {
            if p.name.is_empty() {
                p.type_expr.clone()
            } else {
                format!("{} {}", p.name, p.type_expr)
            }
        })
        .collect();

    let ret_str = if returns.is_empty() {
        String::new()
    } else if returns.len() == 1 {
        format!(" {}", returns[0])
    } else {
        format!(" ({})", returns.join(", "))
    };

    let signature = format!("({}){}", params.join(", "), ret_str);

    FunctionEntry {
        name: f.name.clone(),
        package: package.to_string(),
        receiver,
        signature,
        visibility: vis_str(f.visibility),
        file: file_path.to_string(),
        line: f.location.line,
        provenance: Provenance::Ast,
    }
}

fn build_constants(index: &ProjectIndex) -> Vec<ConstantEntry> {
    let mut entries: Vec<ConstantEntry> = Vec::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }
        for c in &file.constants {
            if c.visibility != Visibility::Exported {
                continue;
            }
            entries.push(ConstantEntry {
                name: c.name.clone(),
                package: file.package.clone(),
                type_hint: c.type_hint.clone(),
                value: c.value.clone(),
                visibility: vis_str(c.visibility),
                provenance: Provenance::Ast,
            });
        }
    }

    entries.sort_by(|a, b| a.package.cmp(&b.package).then_with(|| a.name.cmp(&b.name)));
    entries
}

fn build_interfaces(index: &ProjectIndex) -> Vec<InterfaceEntry> {
    let mut entries: Vec<InterfaceEntry> = Vec::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }
        for t in &file.types {
            if let TypeKind::Interface { methods, embeds } = &t.kind {
                let mut method_names: Vec<String> =
                    methods.iter().map(|m| m.name.clone()).collect();
                method_names.sort();

                let mut embed_names: Vec<String> =
                    embeds.iter().map(|e| e.type_name.clone()).collect();
                embed_names.sort();

                entries.push(InterfaceEntry {
                    name: t.name.clone(),
                    package: file.package.clone(),
                    methods: method_names,
                    embeds: embed_names,
                    file: file.path.clone(),
                    line: t.location.line,
                    provenance: Provenance::Ast,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.package.cmp(&b.package).then_with(|| a.name.cmp(&b.name)));
    entries
}

fn build_arch_layers(index: &ProjectIndex) -> Vec<ArchLayerEntry> {
    let mut seen: BTreeMap<String, &'static str> = BTreeMap::new();

    for pkg in &index.packages {
        if let Some(layer) = detect_layer(&pkg.dir) {
            seen.entry(pkg.dir.clone()).or_insert(layer);
        }
    }

    seen.into_iter()
        .map(|(dir, layer)| ArchLayerEntry {
            package_dir: dir,
            layer: layer.to_string(),
            provenance: Provenance::Inferred,
        })
        .collect()
}

fn build_contracts(index: &ProjectIndex) -> Vec<ContractEntry> {
    let mut entries: Vec<ContractEntry> = Vec::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }
        for t in &file.types {
            if t.visibility != Visibility::Exported {
                continue;
            }
            if let Some(family) = detect_contract_family(&t.name) {
                entries.push(ContractEntry {
                    name: t.name.clone(),
                    family: family.to_string(),
                    file: file.path.clone(),
                    line: t.location.line,
                    provenance: Provenance::Inferred,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.family.cmp(&b.family).then_with(|| a.name.cmp(&b.name)));
    entries
}

// ── Rendering ──────────────────────────────────────────────────────────

/// Render the snapshot as pretty-printed JSON.
pub fn render_json(snapshot: &Snapshot) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(snapshot)
}

/// Render the snapshot in human-readable text.
pub fn render_human(snapshot: &Snapshot, verbose: bool) -> String {
    let mut out = String::new();

    out.push_str("Golden Snapshot\n");
    out.push_str(&format!("  Version:   {}\n", snapshot.version));
    out.push_str(&format!(
        "  Generated: {}\n",
        snapshot.metadata.generated_at
    ));
    out.push_str(&format!(
        "  Raccoon:   {}\n",
        snapshot.metadata.raccoon_version
    ));
    out.push('\n');

    // Stats summary
    out.push_str("Stats:\n");
    out.push_str(&format!("  Files:      {}\n", snapshot.stats.total_files));
    out.push_str(&format!(
        "  Packages:   {}\n",
        snapshot.stats.total_packages
    ));
    out.push_str(&format!(
        "  Types:      {} (structs: {}, interfaces: {}, aliases: {})\n",
        snapshot.stats.total_types,
        snapshot.stats.structs,
        snapshot.stats.interfaces,
        snapshot.stats.type_aliases,
    ));
    out.push_str(&format!(
        "  Functions:  {} (exported: {})\n",
        snapshot.stats.total_functions, snapshot.stats.exported_functions,
    ));
    out.push_str(&format!(
        "  Constants:  {}\n",
        snapshot.stats.total_constants
    ));
    out.push_str(&format!("  Imports:    {}\n", snapshot.stats.total_imports));
    out.push_str(&format!("  Lines:      {}\n", snapshot.stats.total_lines));
    out.push_str(&format!("  Test files: {}\n", snapshot.stats.test_files));
    out.push_str(&format!(
        "  Arch layers: {}\n",
        snapshot.stats.arch_layers_detected
    ));
    out.push_str(&format!(
        "  Contracts:  {}\n",
        snapshot.stats.contracts_detected
    ));
    out.push('\n');

    // Packages
    out.push_str(&format!("Packages ({}):\n", snapshot.packages.len()));
    for pkg in &snapshot.packages {
        out.push_str(&format!(
            "  {} ({}) — {} files [{}]\n",
            pkg.name,
            pkg.dir,
            pkg.file_count,
            pkg.provenance_tag()
        ));
    }
    out.push('\n');

    // Interfaces (always shown — contract surface)
    if !snapshot.interfaces.is_empty() {
        out.push_str(&format!("Interfaces ({}):\n", snapshot.interfaces.len()));
        for iface in &snapshot.interfaces {
            out.push_str(&format!(
                "  {} ({}) — {} methods [{}]\n",
                iface.name,
                iface.package,
                iface.methods.len(),
                iface.provenance_tag()
            ));
            if verbose {
                for m in &iface.methods {
                    out.push_str(&format!("    - {m}\n"));
                }
                for e in &iface.embeds {
                    out.push_str(&format!("    > embeds {e}\n"));
                }
            }
        }
        out.push('\n');
    }

    // Contracts
    if !snapshot.contracts.is_empty() {
        out.push_str(&format!("Contracts ({}):\n", snapshot.contracts.len()));
        for c in &snapshot.contracts {
            out.push_str(&format!(
                "  {} (family: {}) at {}:{} [{}]\n",
                c.name,
                c.family,
                c.file,
                c.line,
                c.provenance_tag()
            ));
        }
        out.push('\n');
    }

    // Arch layers
    if !snapshot.arch_layers.is_empty() {
        out.push_str(&format!(
            "Architecture layers ({}):\n",
            snapshot.arch_layers.len()
        ));
        for l in &snapshot.arch_layers {
            out.push_str(&format!(
                "  {} → {} [{}]\n",
                l.package_dir,
                l.layer,
                l.provenance_tag()
            ));
        }
        out.push('\n');
    }

    // Verbose: types, functions, constants, imports
    if verbose {
        if !snapshot.types.is_empty() {
            out.push_str(&format!("Types ({}):\n", snapshot.types.len()));
            for t in &snapshot.types {
                out.push_str(&format!(
                    "  {} {} ({}) at {}:{} [{}]\n",
                    t.kind,
                    t.name,
                    t.package,
                    t.file,
                    t.line,
                    t.provenance_tag()
                ));
                for f in &t.fields {
                    let embed = if f.embedded { " [embedded]" } else { "" };
                    out.push_str(&format!("    .{}: {}{}\n", f.name, f.type_expr, embed));
                }
            }
            out.push('\n');
        }

        if !snapshot.functions.is_empty() {
            out.push_str(&format!(
                "Exported functions ({}):\n",
                snapshot.functions.len()
            ));
            for f in &snapshot.functions {
                let recv = f
                    .receiver
                    .as_deref()
                    .map(|r| format!("({r}) "))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  {recv}{}{} [{}]\n",
                    f.name,
                    f.signature,
                    f.provenance_tag()
                ));
            }
            out.push('\n');
        }

        if !snapshot.constants.is_empty() {
            out.push_str(&format!(
                "Exported constants ({}):\n",
                snapshot.constants.len()
            ));
            for c in &snapshot.constants {
                let th = c.type_hint.as_deref().unwrap_or("?");
                let val = c.value.as_deref().unwrap_or("?");
                out.push_str(&format!(
                    "  {} {} = {} [{}]\n",
                    c.name,
                    th,
                    val,
                    c.provenance_tag()
                ));
            }
            out.push('\n');
        }

        if !snapshot.imports.is_empty() {
            out.push_str(&format!("Imports ({}):\n", snapshot.imports.len()));
            for imp in &snapshot.imports {
                out.push_str(&format!(
                    "  {} ({}) — used by: {} [{}]\n",
                    imp.path,
                    imp.kind,
                    imp.used_by.join(", "),
                    imp.provenance_tag()
                ));
            }
            out.push('\n');
        }
    }

    out
}

// ── Helpers ────────────────────────────────────────────────────────────

fn vis_str(v: Visibility) -> String {
    match v {
        Visibility::Exported => "exported".to_string(),
        Visibility::Unexported => "unexported".to_string(),
    }
}

fn file_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

fn chrono_now() -> String {
    // Use a simple UTC timestamp without external dependency.
    let output = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output();
    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

// ── Provenance display helpers ─────────────────────────────────────────

trait ProvenanceTag {
    fn provenance_tag(&self) -> &'static str;
}

impl ProvenanceTag for PackageEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for InterfaceEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for ContractEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for ArchLayerEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for TypeEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for FunctionEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for ConstantEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

impl ProvenanceTag for ImportEntry {
    fn provenance_tag(&self) -> &'static str {
        prov_str(&self.provenance)
    }
}

fn prov_str(p: &Provenance) -> &'static str {
    match p {
        Provenance::Ast => "ast",
        Provenance::Lsp => "lsp",
        Provenance::Inferred => "inferred",
        Provenance::Runtime => "runtime",
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_fixture(tmp: &TempDir) -> &Path {
        let root = tmp.path();

        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::create_dir_all(root.join("internal/application/ports")).unwrap();
        fs::create_dir_all(root.join("internal/adapters/nats")).unwrap();

        fs::write(
            root.join("internal/domain/configctl/lifecycle.go"),
            r#"package configctl

type VersionLifecycle string

const (
	LifecycleDraft     VersionLifecycle = "draft"
	LifecycleValidated VersionLifecycle = "validated"
	LifecycleCompiled  VersionLifecycle = "compiled"
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

        fs::write(
            root.join("internal/domain/configctl/config_test.go"),
            r#"package configctl

import "testing"

func TestNewConfigSet(t *testing.T) {
}
"#,
        )
        .unwrap();

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

        fs::write(
            root.join("internal/adapters/nats/publisher.go"),
            r#"package nats

type EventPublisher struct {
	conn string
}

func NewEventPublisher(conn string) *EventPublisher {
	return &EventPublisher{conn: conn}
}
"#,
        )
        .unwrap();

        root
    }

    #[test]
    fn generates_snapshot_from_project() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        assert_eq!(snap.version, "1");
        assert!(!snap.packages.is_empty());
        assert!(snap.stats.total_files > 0);
        assert!(snap.stats.total_types > 0);
    }

    #[test]
    fn snapshot_is_deterministic() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);

        let index = codeintel::index::build_index(root);
        let snap1 = build_snapshot_from_index(&index, root);
        let snap2 = build_snapshot_from_index(&index, root);

        // Compare structural sections (skip metadata.generated_at)
        let j1 = serde_json::to_value(&snap1).unwrap();
        let j2 = serde_json::to_value(&snap2).unwrap();

        assert_eq!(j1["packages"], j2["packages"]);
        assert_eq!(j1["imports"], j2["imports"]);
        assert_eq!(j1["types"], j2["types"]);
        assert_eq!(j1["functions"], j2["functions"]);
        assert_eq!(j1["constants"], j2["constants"]);
        assert_eq!(j1["interfaces"], j2["interfaces"]);
        assert_eq!(j1["arch_layers"], j2["arch_layers"]);
        assert_eq!(j1["contracts"], j2["contracts"]);
        assert_eq!(j1["stats"], j2["stats"]);
    }

    #[test]
    fn snapshot_packages_are_sorted() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let dirs: Vec<&str> = snap.packages.iter().map(|p| p.dir.as_str()).collect();
        let mut sorted = dirs.clone();
        sorted.sort();
        assert_eq!(dirs, sorted);
    }

    #[test]
    fn snapshot_excludes_test_files_from_types() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        // Test functions should not appear in types or functions
        for f in &snap.functions {
            assert!(
                !f.name.starts_with("Test"),
                "test func should not appear: {}",
                f.name
            );
        }
    }

    #[test]
    fn snapshot_detects_arch_layers() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let layer_names: Vec<&str> = snap.arch_layers.iter().map(|l| l.layer.as_str()).collect();
        assert!(
            layer_names.contains(&"domain"),
            "should detect domain layer"
        );
        assert!(
            layer_names.contains(&"application"),
            "should detect application layer"
        );
        assert!(
            layer_names.contains(&"adapters"),
            "should detect adapters layer"
        );
    }

    #[test]
    fn snapshot_detects_contracts() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let contract_names: Vec<&str> = snap.contracts.iter().map(|c| c.name.as_str()).collect();
        assert!(
            contract_names.contains(&"ConfigctlGateway"),
            "should detect gateway contract"
        );
    }

    #[test]
    fn snapshot_detects_interfaces() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let iface_names: Vec<&str> = snap.interfaces.iter().map(|i| i.name.as_str()).collect();
        assert!(iface_names.contains(&"ConfigctlGateway"));

        let gw = snap
            .interfaces
            .iter()
            .find(|i| i.name == "ConfigctlGateway")
            .unwrap();
        assert_eq!(gw.methods.len(), 2);
        assert!(gw.methods.contains(&"CreateDraft".to_string()));
        assert!(gw.methods.contains(&"GetConfig".to_string()));
    }

    #[test]
    fn snapshot_only_includes_exported_functions() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        for f in &snap.functions {
            assert_eq!(f.visibility, "exported");
        }
    }

    #[test]
    fn snapshot_only_includes_exported_constants() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        for c in &snap.constants {
            assert_eq!(c.visibility, "exported");
        }
    }

    #[test]
    fn snapshot_includes_struct_fields() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let config_set = snap.types.iter().find(|t| t.name == "ConfigSet").unwrap();
        assert_eq!(config_set.kind, "struct");
        assert!(!config_set.fields.is_empty());
        let field_names: Vec<&str> = config_set.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(field_names.contains(&"SetID"));
        assert!(field_names.contains(&"Versions"));
    }

    #[test]
    fn snapshot_provenance_tagging() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        // Packages should be ast
        for pkg in &snap.packages {
            assert_eq!(pkg.provenance, Provenance::Ast);
        }
        // Arch layers should be inferred
        for layer in &snap.arch_layers {
            assert_eq!(layer.provenance, Provenance::Inferred);
        }
        // Contracts should be inferred
        for c in &snap.contracts {
            assert_eq!(c.provenance, Provenance::Inferred);
        }
        // Metadata should be runtime
        assert_eq!(snap.metadata.provenance, Provenance::Runtime);
    }

    #[test]
    fn empty_project_produces_empty_snapshot() {
        let tmp = TempDir::new().unwrap();
        let snap = generate(tmp.path());

        assert_eq!(snap.stats.total_files, 0);
        assert!(snap.packages.is_empty());
        assert!(snap.types.is_empty());
        assert!(snap.functions.is_empty());
    }

    #[test]
    fn json_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let json = render_json(&snap).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["version"], "1");
        assert!(parsed["packages"].is_array());
        assert!(parsed["imports"].is_array());
        assert!(parsed["types"].is_array());
        assert!(parsed["functions"].is_array());
        assert!(parsed["constants"].is_array());
        assert!(parsed["interfaces"].is_array());
        assert!(parsed["arch_layers"].is_array());
        assert!(parsed["contracts"].is_array());
        assert!(parsed["stats"].is_object());
        assert!(parsed["metadata"].is_object());
        assert_eq!(parsed["metadata"]["provenance"], "runtime");
    }

    #[test]
    fn human_rendering_has_sections() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let text = render_human(&snap, false);
        assert!(text.contains("Golden Snapshot"));
        assert!(text.contains("Stats:"));
        assert!(text.contains("Packages ("));
        assert!(text.contains("Interfaces ("));
        assert!(text.contains("Contracts ("));
        assert!(text.contains("Architecture layers ("));
    }

    #[test]
    fn verbose_rendering_shows_types_and_functions() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let terse = render_human(&snap, false);
        let verbose = render_human(&snap, true);

        assert!(verbose.len() > terse.len());
        assert!(verbose.contains("Types ("));
        assert!(verbose.contains("Exported functions ("));
        assert!(verbose.contains("Exported constants ("));
        assert!(verbose.contains("Imports ("));
    }

    #[test]
    fn detect_layer_classification() {
        assert_eq!(detect_layer("internal/domain/configctl"), Some("domain"));
        assert_eq!(
            detect_layer("internal/application/ports"),
            Some("application")
        );
        assert_eq!(detect_layer("internal/adapters/nats"), Some("adapters"));
        assert_eq!(detect_layer("internal/actors/scopes"), Some("actors"));
        assert_eq!(detect_layer("internal/interfaces/http"), Some("interfaces"));
        assert_eq!(detect_layer("cmd/server"), Some("cmd"));
        assert_eq!(detect_layer("tools/raccoon-cli"), Some("tools"));
        assert_eq!(detect_layer("pkg/utils"), None);
    }

    #[test]
    fn detect_contract_families() {
        assert_eq!(
            detect_contract_family("CreateDraftCommand"),
            Some("command")
        );
        assert_eq!(detect_contract_family("GetConfigQuery"), Some("query"));
        assert_eq!(detect_contract_family("DraftCreatedEvent"), Some("event"));
        assert_eq!(detect_contract_family("ConfigctlGateway"), Some("port"));
        assert_eq!(detect_contract_family("DataPlaneRecord"), Some("record"));
        assert_eq!(detect_contract_family("ConfigSet"), None);
        // Edge case: suffix alone should not match
        assert_eq!(detect_contract_family("Command"), None);
        assert_eq!(detect_contract_family("Event"), None);
    }

    #[test]
    fn imports_deduplicated_with_usage() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        // "time" should appear exactly once
        let time_imports: Vec<&ImportEntry> =
            snap.imports.iter().filter(|i| i.path == "time").collect();
        assert_eq!(time_imports.len(), 1);
        assert_eq!(time_imports[0].kind, "stdlib");
    }

    #[test]
    fn function_signatures_are_correct() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture(&tmp);
        let snap = generate(root);

        let new_cs = snap
            .functions
            .iter()
            .find(|f| f.name == "NewConfigSet")
            .unwrap();
        assert!(new_cs.receiver.is_none());
        assert!(new_cs.signature.contains("id string"));
        assert!(new_cs.signature.contains("ConfigSet"));

        let add_ver = snap
            .functions
            .iter()
            .find(|f| f.name == "AddVersion")
            .unwrap();
        assert_eq!(add_ver.receiver.as_deref(), Some("*ConfigSet"));

        let count = snap
            .functions
            .iter()
            .find(|f| f.name == "VersionCount")
            .unwrap();
        assert_eq!(count.receiver.as_deref(), Some("ConfigSet"));
    }
}
