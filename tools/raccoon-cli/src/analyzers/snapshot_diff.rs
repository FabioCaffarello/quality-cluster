//! Semantic diff between two code intelligence snapshots.
//!
//! Compares two [`Snapshot`] instances and produces a structured report of
//! additions, removals, and modifications across every snapshot section.
//!
//! Design principles:
//!
//! - **Semantic, not textual**: diffs are at the level of types, functions,
//!   interfaces — not lines of JSON.
//! - **Observed vs. inferred**: the report separates directly observed changes
//!   (a struct gained a field) from derived inferences (a contract surface may
//!   have changed).
//! - **Noise reduction**: metadata changes (timestamps, project root) and
//!   trivially derived stat deltas are reported separately from structural
//!   changes.
//! - **Version-aware**: rejects comparison of incompatible snapshot versions.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::snapshot::*;

// ── Diff model ────────────────────────────────────────────────────────

/// Top-level diff report between two snapshots.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotDiff {
    /// Snapshot version (both must match).
    pub version: String,
    /// Left-side metadata (the "before" snapshot).
    pub before: DiffMeta,
    /// Right-side metadata (the "after" snapshot).
    pub after: DiffMeta,
    /// Whether any structural change was detected.
    pub has_changes: bool,
    /// Structural changes grouped by section.
    pub sections: DiffSections,
    /// High-level inferences derived from the observed changes.
    pub inferences: Vec<Inference>,
    /// Summary statistics delta.
    pub stats_delta: StatsDelta,
}

/// Minimal metadata extracted from a snapshot for labeling.
#[derive(Debug, Clone, Serialize)]
pub struct DiffMeta {
    pub generated_at: String,
    pub raccoon_version: String,
}

/// All section-level diffs.
#[derive(Debug, Clone, Serialize)]
pub struct DiffSections {
    pub packages: SectionDiff<PackageDelta>,
    pub imports: SectionDiff<ImportDelta>,
    pub types: SectionDiff<TypeDelta>,
    pub functions: SectionDiff<FunctionDelta>,
    pub constants: SectionDiff<ConstantDelta>,
    pub interfaces: SectionDiff<InterfaceDelta>,
    pub arch_layers: SectionDiff<ArchLayerDelta>,
    pub contracts: SectionDiff<ContractDelta>,
}

/// Generic section diff: added, removed, modified items.
#[derive(Debug, Clone, Serialize)]
pub struct SectionDiff<T: Serialize> {
    pub added: Vec<T>,
    pub removed: Vec<T>,
    pub modified: Vec<T>,
    pub total_changes: usize,
}

impl<T: Serialize> SectionDiff<T> {
    fn empty() -> Self {
        Self {
            added: Vec::new(),
            removed: Vec::new(),
            modified: Vec::new(),
            total_changes: 0,
        }
    }

    fn finalize(&mut self) {
        self.total_changes = self.added.len() + self.removed.len() + self.modified.len();
    }
}

// ── Per-section delta types ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PackageDelta {
    pub name: String,
    pub dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files_removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportDelta {
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub consumers_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub consumers_removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeDelta {
    pub name: String,
    pub package: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind_changed: Option<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility_changed: Option<(String, String)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields_removed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields_type_changed: Vec<FieldTypeChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldTypeChange {
    pub name: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDelta {
    pub name: String,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_changed: Option<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_changed: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConstantDelta {
    pub name: String,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_changed: Option<(Option<String>, Option<String>)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_changed: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceDelta {
    pub name: String,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub methods_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub methods_removed: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub embeds_added: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub embeds_removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchLayerDelta {
    pub package_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_changed: Option<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractDelta {
    pub name: String,
    pub family: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_changed: Option<(String, String)>,
}

/// A derived inference about the impact of observed changes.
#[derive(Debug, Clone, Serialize)]
pub struct Inference {
    pub category: String,
    pub severity: String,
    pub message: String,
}

/// Aggregate statistics delta.
#[derive(Debug, Clone, Serialize)]
pub struct StatsDelta {
    pub total_files: i64,
    pub total_packages: i64,
    pub total_types: i64,
    pub total_functions: i64,
    pub total_constants: i64,
    pub total_imports: i64,
    pub total_lines: i64,
    pub structs: i64,
    pub interfaces: i64,
    pub type_aliases: i64,
    pub exported_types: i64,
    pub exported_functions: i64,
    pub test_files: i64,
    pub arch_layers_detected: i64,
    pub contracts_detected: i64,
}

// ── Error type ────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum DiffError {
    Io(std::io::Error),
    Json(serde_json::Error),
    VersionMismatch { before: String, after: String },
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffError::Io(e) => write!(f, "I/O error: {e}"),
            DiffError::Json(e) => write!(f, "JSON parse error: {e}"),
            DiffError::VersionMismatch { before, after } => {
                write!(f, "snapshot version mismatch: before={before}, after={after}")
            }
        }
    }
}

impl From<std::io::Error> for DiffError {
    fn from(e: std::io::Error) -> Self {
        DiffError::Io(e)
    }
}

impl From<serde_json::Error> for DiffError {
    fn from(e: serde_json::Error) -> Self {
        DiffError::Json(e)
    }
}

// ── Loading ───────────────────────────────────────────────────────────

/// Load a snapshot from a JSON file.
pub fn load_snapshot(path: &Path) -> Result<Snapshot, DiffError> {
    let contents = std::fs::read_to_string(path)?;
    let snap: Snapshot = serde_json::from_str(&contents)?;
    Ok(snap)
}

// ── Core diff logic ───────────────────────────────────────────────────

/// Compare two snapshots and produce a structured diff report.
pub fn diff(before: &Snapshot, after: &Snapshot) -> Result<SnapshotDiff, DiffError> {
    if before.version != after.version {
        return Err(DiffError::VersionMismatch {
            before: before.version.to_string(),
            after: after.version.to_string(),
        });
    }

    let mut sections = DiffSections {
        packages: diff_packages(&before.packages, &after.packages),
        imports: diff_imports(&before.imports, &after.imports),
        types: diff_types(&before.types, &after.types),
        functions: diff_functions(&before.functions, &after.functions),
        constants: diff_constants(&before.constants, &after.constants),
        interfaces: diff_interfaces(&before.interfaces, &after.interfaces),
        arch_layers: diff_arch_layers(&before.arch_layers, &after.arch_layers),
        contracts: diff_contracts(&before.contracts, &after.contracts),
    };

    sections.packages.finalize();
    sections.imports.finalize();
    sections.types.finalize();
    sections.functions.finalize();
    sections.constants.finalize();
    sections.interfaces.finalize();
    sections.arch_layers.finalize();
    sections.contracts.finalize();

    let has_changes = sections.packages.total_changes > 0
        || sections.imports.total_changes > 0
        || sections.types.total_changes > 0
        || sections.functions.total_changes > 0
        || sections.constants.total_changes > 0
        || sections.interfaces.total_changes > 0
        || sections.arch_layers.total_changes > 0
        || sections.contracts.total_changes > 0;

    let stats_delta = diff_stats(&before.stats, &after.stats);
    let inferences = derive_inferences(&sections, &stats_delta);

    Ok(SnapshotDiff {
        version: before.version.to_string(),
        before: DiffMeta {
            generated_at: before.metadata.generated_at.clone(),
            raccoon_version: before.metadata.raccoon_version.clone(),
        },
        after: DiffMeta {
            generated_at: after.metadata.generated_at.clone(),
            raccoon_version: after.metadata.raccoon_version.clone(),
        },
        has_changes,
        sections,
        inferences,
        stats_delta,
    })
}

// ── Section diffing ───────────────────────────────────────────────────

fn diff_packages(before: &[PackageEntry], after: &[PackageEntry]) -> SectionDiff<PackageDelta> {
    let before_map: BTreeMap<&str, &PackageEntry> = before.iter().map(|p| (p.dir.as_str(), p)).collect();
    let after_map: BTreeMap<&str, &PackageEntry> = after.iter().map(|p| (p.dir.as_str(), p)).collect();

    let mut result = SectionDiff::empty();

    for (dir, pkg) in &after_map {
        if !before_map.contains_key(dir) {
            result.added.push(PackageDelta {
                name: pkg.name.clone(),
                dir: pkg.dir.clone(),
                change: Some("added".into()),
                files_added: pkg.files.clone(),
                files_removed: Vec::new(),
            });
        }
    }

    for (dir, pkg) in &before_map {
        if !after_map.contains_key(dir) {
            result.removed.push(PackageDelta {
                name: pkg.name.clone(),
                dir: pkg.dir.clone(),
                change: Some("removed".into()),
                files_added: Vec::new(),
                files_removed: pkg.files.clone(),
            });
        }
    }

    for (dir, b) in &before_map {
        if let Some(a) = after_map.get(dir) {
            let b_files: BTreeSet<&str> = b.files.iter().map(|f| f.as_str()).collect();
            let a_files: BTreeSet<&str> = a.files.iter().map(|f| f.as_str()).collect();

            let added: Vec<String> = a_files.difference(&b_files).map(|f| f.to_string()).collect();
            let removed: Vec<String> = b_files.difference(&a_files).map(|f| f.to_string()).collect();

            if !added.is_empty() || !removed.is_empty() || a.name != b.name {
                result.modified.push(PackageDelta {
                    name: a.name.clone(),
                    dir: a.dir.clone(),
                    change: Some("modified".into()),
                    files_added: added,
                    files_removed: removed,
                });
            }
        }
    }

    result
}

fn diff_imports(before: &[ImportEntry], after: &[ImportEntry]) -> SectionDiff<ImportDelta> {
    let before_map: BTreeMap<&str, &ImportEntry> = before.iter().map(|i| (i.path.as_str(), i)).collect();
    let after_map: BTreeMap<&str, &ImportEntry> = after.iter().map(|i| (i.path.as_str(), i)).collect();

    let mut result = SectionDiff::empty();

    for (path, imp) in &after_map {
        if !before_map.contains_key(path) {
            result.added.push(ImportDelta {
                path: imp.path.clone(),
                kind: imp.kind.clone(),
                change: Some("added".into()),
                consumers_added: imp.used_by.clone(),
                consumers_removed: Vec::new(),
            });
        }
    }

    for (path, imp) in &before_map {
        if !after_map.contains_key(path) {
            result.removed.push(ImportDelta {
                path: imp.path.clone(),
                kind: imp.kind.clone(),
                change: Some("removed".into()),
                consumers_added: Vec::new(),
                consumers_removed: imp.used_by.clone(),
            });
        }
    }

    for (path, b) in &before_map {
        if let Some(a) = after_map.get(path) {
            let b_users: BTreeSet<&str> = b.used_by.iter().map(|u| u.as_str()).collect();
            let a_users: BTreeSet<&str> = a.used_by.iter().map(|u| u.as_str()).collect();

            let added: Vec<String> = a_users.difference(&b_users).map(|u| u.to_string()).collect();
            let removed: Vec<String> = b_users.difference(&a_users).map(|u| u.to_string()).collect();

            if !added.is_empty() || !removed.is_empty() || a.kind != b.kind {
                result.modified.push(ImportDelta {
                    path: a.path.clone(),
                    kind: a.kind.clone(),
                    change: Some("modified".into()),
                    consumers_added: added,
                    consumers_removed: removed,
                });
            }
        }
    }

    result
}

fn diff_types(before: &[TypeEntry], after: &[TypeEntry]) -> SectionDiff<TypeDelta> {
    // Key: (package, name)
    let before_map: BTreeMap<(&str, &str), &TypeEntry> =
        before.iter().map(|t| ((t.package.as_str(), t.name.as_str()), t)).collect();
    let after_map: BTreeMap<(&str, &str), &TypeEntry> =
        after.iter().map(|t| ((t.package.as_str(), t.name.as_str()), t)).collect();

    let mut result = SectionDiff::empty();

    for (key, t) in &after_map {
        if !before_map.contains_key(key) {
            result.added.push(TypeDelta {
                name: t.name.clone(),
                package: t.package.clone(),
                kind: t.kind.clone(),
                change: Some("added".into()),
                kind_changed: None,
                visibility_changed: None,
                fields_added: t.fields.iter().map(|f| f.name.clone()).collect(),
                fields_removed: Vec::new(),
                fields_type_changed: Vec::new(),
            });
        }
    }

    for (key, t) in &before_map {
        if !after_map.contains_key(key) {
            result.removed.push(TypeDelta {
                name: t.name.clone(),
                package: t.package.clone(),
                kind: t.kind.clone(),
                change: Some("removed".into()),
                kind_changed: None,
                visibility_changed: None,
                fields_added: Vec::new(),
                fields_removed: t.fields.iter().map(|f| f.name.clone()).collect(),
                fields_type_changed: Vec::new(),
            });
        }
    }

    for (key, b) in &before_map {
        if let Some(a) = after_map.get(key) {
            let kind_changed = if a.kind != b.kind {
                Some((b.kind.clone(), a.kind.clone()))
            } else {
                None
            };

            let vis_changed = if a.visibility != b.visibility {
                Some((b.visibility.clone(), a.visibility.clone()))
            } else {
                None
            };

            // Field diff
            let b_fields: BTreeMap<&str, &FieldEntry> =
                b.fields.iter().map(|f| (f.name.as_str(), f)).collect();
            let a_fields: BTreeMap<&str, &FieldEntry> =
                a.fields.iter().map(|f| (f.name.as_str(), f)).collect();

            let fields_added: Vec<String> = a_fields.keys()
                .filter(|k| !b_fields.contains_key(*k))
                .map(|k| k.to_string())
                .collect();
            let fields_removed: Vec<String> = b_fields.keys()
                .filter(|k| !a_fields.contains_key(*k))
                .map(|k| k.to_string())
                .collect();

            let mut fields_type_changed = Vec::new();
            for (fname, bf) in &b_fields {
                if let Some(af) = a_fields.get(fname) {
                    if af.type_expr != bf.type_expr {
                        fields_type_changed.push(FieldTypeChange {
                            name: fname.to_string(),
                            before: bf.type_expr.clone(),
                            after: af.type_expr.clone(),
                        });
                    }
                }
            }

            if kind_changed.is_some()
                || vis_changed.is_some()
                || !fields_added.is_empty()
                || !fields_removed.is_empty()
                || !fields_type_changed.is_empty()
            {
                result.modified.push(TypeDelta {
                    name: a.name.clone(),
                    package: a.package.clone(),
                    kind: a.kind.clone(),
                    change: Some("modified".into()),
                    kind_changed,
                    visibility_changed: vis_changed,
                    fields_added,
                    fields_removed,
                    fields_type_changed,
                });
            }
        }
    }

    result
}

fn diff_functions(before: &[FunctionEntry], after: &[FunctionEntry]) -> SectionDiff<FunctionDelta> {
    // Key: (package, receiver, name)
    fn fkey(f: &FunctionEntry) -> (String, String, String) {
        (f.package.clone(), f.receiver.clone().unwrap_or_default(), f.name.clone())
    }

    let before_map: BTreeMap<_, &FunctionEntry> = before.iter().map(|f| (fkey(f), f)).collect();
    let after_map: BTreeMap<_, &FunctionEntry> = after.iter().map(|f| (fkey(f), f)).collect();

    let mut result = SectionDiff::empty();

    for (key, f) in &after_map {
        if !before_map.contains_key(key) {
            result.added.push(FunctionDelta {
                name: f.name.clone(),
                package: f.package.clone(),
                receiver: f.receiver.clone(),
                change: Some("added".into()),
                signature_changed: None,
                receiver_changed: None,
            });
        }
    }

    for (key, f) in &before_map {
        if !after_map.contains_key(key) {
            result.removed.push(FunctionDelta {
                name: f.name.clone(),
                package: f.package.clone(),
                receiver: f.receiver.clone(),
                change: Some("removed".into()),
                signature_changed: None,
                receiver_changed: None,
            });
        }
    }

    for (key, b) in &before_map {
        if let Some(a) = after_map.get(key) {
            let sig_changed = if a.signature != b.signature {
                Some((b.signature.clone(), a.signature.clone()))
            } else {
                None
            };

            if sig_changed.is_some() {
                result.modified.push(FunctionDelta {
                    name: a.name.clone(),
                    package: a.package.clone(),
                    receiver: a.receiver.clone(),
                    change: Some("modified".into()),
                    signature_changed: sig_changed,
                    receiver_changed: None,
                });
            }
        }
    }

    result
}

fn diff_constants(before: &[ConstantEntry], after: &[ConstantEntry]) -> SectionDiff<ConstantDelta> {
    let before_map: BTreeMap<(&str, &str), &ConstantEntry> =
        before.iter().map(|c| ((c.package.as_str(), c.name.as_str()), c)).collect();
    let after_map: BTreeMap<(&str, &str), &ConstantEntry> =
        after.iter().map(|c| ((c.package.as_str(), c.name.as_str()), c)).collect();

    let mut result = SectionDiff::empty();

    for (key, c) in &after_map {
        if !before_map.contains_key(key) {
            result.added.push(ConstantDelta {
                name: c.name.clone(),
                package: c.package.clone(),
                change: Some("added".into()),
                value_changed: None,
                type_changed: None,
            });
        }
    }

    for (key, c) in &before_map {
        if !after_map.contains_key(key) {
            result.removed.push(ConstantDelta {
                name: c.name.clone(),
                package: c.package.clone(),
                change: Some("removed".into()),
                value_changed: None,
                type_changed: None,
            });
        }
    }

    for (key, b) in &before_map {
        if let Some(a) = after_map.get(key) {
            let val_changed = if a.value != b.value {
                Some((b.value.clone(), a.value.clone()))
            } else {
                None
            };
            let type_changed = if a.type_hint != b.type_hint {
                Some((b.type_hint.clone(), a.type_hint.clone()))
            } else {
                None
            };

            if val_changed.is_some() || type_changed.is_some() {
                result.modified.push(ConstantDelta {
                    name: a.name.clone(),
                    package: a.package.clone(),
                    change: Some("modified".into()),
                    value_changed: val_changed,
                    type_changed,
                });
            }
        }
    }

    result
}

fn diff_interfaces(before: &[InterfaceEntry], after: &[InterfaceEntry]) -> SectionDiff<InterfaceDelta> {
    let before_map: BTreeMap<(&str, &str), &InterfaceEntry> =
        before.iter().map(|i| ((i.package.as_str(), i.name.as_str()), i)).collect();
    let after_map: BTreeMap<(&str, &str), &InterfaceEntry> =
        after.iter().map(|i| ((i.package.as_str(), i.name.as_str()), i)).collect();

    let mut result = SectionDiff::empty();

    for (key, iface) in &after_map {
        if !before_map.contains_key(key) {
            result.added.push(InterfaceDelta {
                name: iface.name.clone(),
                package: iface.package.clone(),
                change: Some("added".into()),
                methods_added: iface.methods.clone(),
                methods_removed: Vec::new(),
                embeds_added: iface.embeds.clone(),
                embeds_removed: Vec::new(),
            });
        }
    }

    for (key, iface) in &before_map {
        if !after_map.contains_key(key) {
            result.removed.push(InterfaceDelta {
                name: iface.name.clone(),
                package: iface.package.clone(),
                change: Some("removed".into()),
                methods_added: Vec::new(),
                methods_removed: iface.methods.clone(),
                embeds_added: Vec::new(),
                embeds_removed: iface.embeds.clone(),
            });
        }
    }

    for (key, b) in &before_map {
        if let Some(a) = after_map.get(key) {
            let b_methods: BTreeSet<&str> = b.methods.iter().map(|m| m.as_str()).collect();
            let a_methods: BTreeSet<&str> = a.methods.iter().map(|m| m.as_str()).collect();
            let b_embeds: BTreeSet<&str> = b.embeds.iter().map(|e| e.as_str()).collect();
            let a_embeds: BTreeSet<&str> = a.embeds.iter().map(|e| e.as_str()).collect();

            let methods_added: Vec<String> = a_methods.difference(&b_methods).map(|m| m.to_string()).collect();
            let methods_removed: Vec<String> = b_methods.difference(&a_methods).map(|m| m.to_string()).collect();
            let embeds_added: Vec<String> = a_embeds.difference(&b_embeds).map(|e| e.to_string()).collect();
            let embeds_removed: Vec<String> = b_embeds.difference(&a_embeds).map(|e| e.to_string()).collect();

            if !methods_added.is_empty()
                || !methods_removed.is_empty()
                || !embeds_added.is_empty()
                || !embeds_removed.is_empty()
            {
                result.modified.push(InterfaceDelta {
                    name: a.name.clone(),
                    package: a.package.clone(),
                    change: Some("modified".into()),
                    methods_added,
                    methods_removed,
                    embeds_added,
                    embeds_removed,
                });
            }
        }
    }

    result
}

fn diff_arch_layers(before: &[ArchLayerEntry], after: &[ArchLayerEntry]) -> SectionDiff<ArchLayerDelta> {
    let before_map: BTreeMap<&str, &ArchLayerEntry> =
        before.iter().map(|l| (l.package_dir.as_str(), l)).collect();
    let after_map: BTreeMap<&str, &ArchLayerEntry> =
        after.iter().map(|l| (l.package_dir.as_str(), l)).collect();

    let mut result = SectionDiff::empty();

    for (dir, l) in &after_map {
        if !before_map.contains_key(dir) {
            result.added.push(ArchLayerDelta {
                package_dir: l.package_dir.clone(),
                change: Some("added".into()),
                layer_changed: None,
            });
        }
    }

    for (dir, l) in &before_map {
        if !after_map.contains_key(dir) {
            result.removed.push(ArchLayerDelta {
                package_dir: l.package_dir.clone(),
                change: Some("removed".into()),
                layer_changed: None,
            });
        }
    }

    for (dir, b) in &before_map {
        if let Some(a) = after_map.get(dir) {
            if a.layer != b.layer {
                result.modified.push(ArchLayerDelta {
                    package_dir: a.package_dir.clone(),
                    change: Some("modified".into()),
                    layer_changed: Some((b.layer.clone(), a.layer.clone())),
                });
            }
        }
    }

    result
}

fn diff_contracts(before: &[ContractEntry], after: &[ContractEntry]) -> SectionDiff<ContractDelta> {
    let before_map: BTreeMap<&str, &ContractEntry> =
        before.iter().map(|c| (c.name.as_str(), c)).collect();
    let after_map: BTreeMap<&str, &ContractEntry> =
        after.iter().map(|c| (c.name.as_str(), c)).collect();

    let mut result = SectionDiff::empty();

    for (name, c) in &after_map {
        if !before_map.contains_key(name) {
            result.added.push(ContractDelta {
                name: c.name.clone(),
                family: c.family.clone(),
                change: Some("added".into()),
                family_changed: None,
            });
        }
    }

    for (name, c) in &before_map {
        if !after_map.contains_key(name) {
            result.removed.push(ContractDelta {
                name: c.name.clone(),
                family: c.family.clone(),
                change: Some("removed".into()),
                family_changed: None,
            });
        }
    }

    for (name, b) in &before_map {
        if let Some(a) = after_map.get(name) {
            if a.family != b.family {
                result.modified.push(ContractDelta {
                    name: a.name.clone(),
                    family: a.family.clone(),
                    change: Some("modified".into()),
                    family_changed: Some((b.family.clone(), a.family.clone())),
                });
            }
        }
    }

    result
}

fn diff_stats(before: &SnapshotStats, after: &SnapshotStats) -> StatsDelta {
    StatsDelta {
        total_files: after.total_files as i64 - before.total_files as i64,
        total_packages: after.total_packages as i64 - before.total_packages as i64,
        total_types: after.total_types as i64 - before.total_types as i64,
        total_functions: after.total_functions as i64 - before.total_functions as i64,
        total_constants: after.total_constants as i64 - before.total_constants as i64,
        total_imports: after.total_imports as i64 - before.total_imports as i64,
        total_lines: after.total_lines as i64 - before.total_lines as i64,
        structs: after.structs as i64 - before.structs as i64,
        interfaces: after.interfaces as i64 - before.interfaces as i64,
        type_aliases: after.type_aliases as i64 - before.type_aliases as i64,
        exported_types: after.exported_types as i64 - before.exported_types as i64,
        exported_functions: after.exported_functions as i64 - before.exported_functions as i64,
        test_files: after.test_files as i64 - before.test_files as i64,
        arch_layers_detected: after.arch_layers_detected as i64 - before.arch_layers_detected as i64,
        contracts_detected: after.contracts_detected as i64 - before.contracts_detected as i64,
    }
}

// ── Inference engine ──────────────────────────────────────────────────

fn derive_inferences(sections: &DiffSections, stats: &StatsDelta) -> Vec<Inference> {
    let mut infs = Vec::new();

    // Contract surface changes
    if sections.contracts.total_changes > 0 {
        let added = sections.contracts.added.len();
        let removed = sections.contracts.removed.len();
        let modified = sections.contracts.modified.len();
        infs.push(Inference {
            category: "contract-surface".into(),
            severity: if removed > 0 { "warning" } else { "info" }.into(),
            message: format!(
                "Contract surface changed: +{added} -{removed} ~{modified}. \
                 Review downstream consumers for compatibility."
            ),
        });
    }

    // Interface breaking changes
    for iface in &sections.interfaces.modified {
        if !iface.methods_removed.is_empty() {
            infs.push(Inference {
                category: "breaking-interface".into(),
                severity: "warning".into(),
                message: format!(
                    "Interface {}.{} lost methods: {}. All implementors must be updated.",
                    iface.package,
                    iface.name,
                    iface.methods_removed.join(", ")
                ),
            });
        }
        if !iface.methods_added.is_empty() {
            infs.push(Inference {
                category: "interface-expansion".into(),
                severity: "info".into(),
                message: format!(
                    "Interface {}.{} gained methods: {}. Existing implementors need new methods.",
                    iface.package,
                    iface.name,
                    iface.methods_added.join(", ")
                ),
            });
        }
    }
    for iface in &sections.interfaces.removed {
        infs.push(Inference {
            category: "breaking-interface".into(),
            severity: "warning".into(),
            message: format!(
                "Interface {}.{} was removed. All dependents must be refactored.",
                iface.package, iface.name
            ),
        });
    }

    // Architecture layer changes
    if sections.arch_layers.total_changes > 0 {
        let added = sections.arch_layers.added.len();
        let removed = sections.arch_layers.removed.len();
        let modified = sections.arch_layers.modified.len();
        infs.push(Inference {
            category: "architecture-boundary".into(),
            severity: if removed > 0 || modified > 0 { "warning" } else { "info" }.into(),
            message: format!(
                "Architecture layers changed: +{added} -{removed} ~{modified}. \
                 Run arch-guard to verify boundary compliance."
            ),
        });
    }

    // Type field changes (possible struct compatibility issues)
    for t in &sections.types.modified {
        if !t.fields_removed.is_empty() {
            infs.push(Inference {
                category: "breaking-type".into(),
                severity: "warning".into(),
                message: format!(
                    "Type {}.{} lost fields: {}. Struct literal consumers may break.",
                    t.package, t.name, t.fields_removed.join(", ")
                ),
            });
        }
        if !t.fields_type_changed.is_empty() {
            let changes: Vec<String> = t.fields_type_changed.iter()
                .map(|c| format!("{}: {} → {}", c.name, c.before, c.after))
                .collect();
            infs.push(Inference {
                category: "type-migration".into(),
                severity: "warning".into(),
                message: format!(
                    "Type {}.{} changed field types: {}.",
                    t.package, t.name, changes.join(", ")
                ),
            });
        }
    }

    // Exported function signature changes
    for f in &sections.functions.modified {
        if let Some((ref before, ref after)) = f.signature_changed {
            let recv = f.receiver.as_deref().map(|r| format!("({r}) ")).unwrap_or_default();
            infs.push(Inference {
                category: "api-change".into(),
                severity: "warning".into(),
                message: format!(
                    "Exported function {recv}{} changed signature: {} → {}.",
                    f.name, before, after
                ),
            });
        }
    }

    // Large-scale changes
    if stats.total_types.abs() > 10 {
        infs.push(Inference {
            category: "scale".into(),
            severity: "info".into(),
            message: format!(
                "Significant type count change ({:+}). Consider reviewing module boundaries.",
                stats.total_types
            ),
        });
    }

    if stats.total_lines.abs() > 500 {
        infs.push(Inference {
            category: "scale".into(),
            severity: "info".into(),
            message: format!(
                "Large line count change ({:+}). Verify test coverage for new/modified code.",
                stats.total_lines
            ),
        });
    }

    infs
}

// ── Rendering ─────────────────────────────────────────────────────────

/// Render the diff as pretty-printed JSON.
pub fn render_json(diff: &SnapshotDiff) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(diff)
}

/// Render the diff in human-readable text.
pub fn render_human(diff: &SnapshotDiff, verbose: bool) -> String {
    let mut out = String::new();

    out.push_str("Snapshot Diff\n");
    out.push_str(&format!("  Before: {} (raccoon {})\n", diff.before.generated_at, diff.before.raccoon_version));
    out.push_str(&format!("  After:  {} (raccoon {})\n", diff.after.generated_at, diff.after.raccoon_version));
    out.push('\n');

    if !diff.has_changes {
        out.push_str("No structural changes detected.\n");
        return out;
    }

    // Stats delta summary
    out.push_str("Stats delta:\n");
    render_stat_line(&mut out, "Files", diff.stats_delta.total_files);
    render_stat_line(&mut out, "Packages", diff.stats_delta.total_packages);
    render_stat_line(&mut out, "Types", diff.stats_delta.total_types);
    render_stat_line(&mut out, "Functions", diff.stats_delta.total_functions);
    render_stat_line(&mut out, "Constants", diff.stats_delta.total_constants);
    render_stat_line(&mut out, "Imports", diff.stats_delta.total_imports);
    render_stat_line(&mut out, "Lines", diff.stats_delta.total_lines);
    render_stat_line(&mut out, "Test files", diff.stats_delta.test_files);
    render_stat_line(&mut out, "Arch layers", diff.stats_delta.arch_layers_detected);
    render_stat_line(&mut out, "Contracts", diff.stats_delta.contracts_detected);
    out.push('\n');

    // Section summaries
    render_section_summary(&mut out, "Packages", &diff.sections.packages);
    render_section_summary(&mut out, "Imports", &diff.sections.imports);
    render_section_summary(&mut out, "Types", &diff.sections.types);
    render_section_summary(&mut out, "Functions", &diff.sections.functions);
    render_section_summary(&mut out, "Constants", &diff.sections.constants);
    render_section_summary(&mut out, "Interfaces", &diff.sections.interfaces);
    render_section_summary(&mut out, "Arch layers", &diff.sections.arch_layers);
    render_section_summary(&mut out, "Contracts", &diff.sections.contracts);

    // Detailed changes
    if verbose || has_nontrivial_changes(diff) {
        out.push_str("\nChanges:\n");

        // Packages
        for p in &diff.sections.packages.added {
            out.push_str(&format!("  + package {} ({})\n", p.name, p.dir));
        }
        for p in &diff.sections.packages.removed {
            out.push_str(&format!("  - package {} ({})\n", p.name, p.dir));
        }
        for p in &diff.sections.packages.modified {
            out.push_str(&format!("  ~ package {} ({})\n", p.name, p.dir));
            for f in &p.files_added {
                out.push_str(&format!("      + {f}\n"));
            }
            for f in &p.files_removed {
                out.push_str(&format!("      - {f}\n"));
            }
        }

        // Types
        for t in &diff.sections.types.added {
            out.push_str(&format!("  + {} {}.{}\n", t.kind, t.package, t.name));
        }
        for t in &diff.sections.types.removed {
            out.push_str(&format!("  - {} {}.{}\n", t.kind, t.package, t.name));
        }
        for t in &diff.sections.types.modified {
            out.push_str(&format!("  ~ {} {}.{}\n", t.kind, t.package, t.name));
            if let Some((ref bk, ref ak)) = t.kind_changed {
                out.push_str(&format!("      kind: {bk} → {ak}\n"));
            }
            if let Some((ref bv, ref av)) = t.visibility_changed {
                out.push_str(&format!("      visibility: {bv} → {av}\n"));
            }
            for f in &t.fields_added {
                out.push_str(&format!("      + field {f}\n"));
            }
            for f in &t.fields_removed {
                out.push_str(&format!("      - field {f}\n"));
            }
            for c in &t.fields_type_changed {
                out.push_str(&format!("      ~ field {}: {} → {}\n", c.name, c.before, c.after));
            }
        }

        // Functions
        for f in &diff.sections.functions.added {
            let recv = f.receiver.as_deref().map(|r| format!("({r}) ")).unwrap_or_default();
            out.push_str(&format!("  + func {recv}{}\n", f.name));
        }
        for f in &diff.sections.functions.removed {
            let recv = f.receiver.as_deref().map(|r| format!("({r}) ")).unwrap_or_default();
            out.push_str(&format!("  - func {recv}{}\n", f.name));
        }
        for f in &diff.sections.functions.modified {
            let recv = f.receiver.as_deref().map(|r| format!("({r}) ")).unwrap_or_default();
            out.push_str(&format!("  ~ func {recv}{}\n", f.name));
            if let Some((ref bs, ref a_s)) = f.signature_changed {
                out.push_str(&format!("      sig: {bs} → {a_s}\n"));
            }
        }

        // Constants
        for c in &diff.sections.constants.added {
            out.push_str(&format!("  + const {}.{}\n", c.package, c.name));
        }
        for c in &diff.sections.constants.removed {
            out.push_str(&format!("  - const {}.{}\n", c.package, c.name));
        }
        for c in &diff.sections.constants.modified {
            out.push_str(&format!("  ~ const {}.{}\n", c.package, c.name));
        }

        // Interfaces
        for i in &diff.sections.interfaces.added {
            out.push_str(&format!("  + interface {}.{} ({} methods)\n",
                i.package, i.name, i.methods_added.len()));
        }
        for i in &diff.sections.interfaces.removed {
            out.push_str(&format!("  - interface {}.{}\n", i.package, i.name));
        }
        for i in &diff.sections.interfaces.modified {
            out.push_str(&format!("  ~ interface {}.{}\n", i.package, i.name));
            for m in &i.methods_added {
                out.push_str(&format!("      + method {m}\n"));
            }
            for m in &i.methods_removed {
                out.push_str(&format!("      - method {m}\n"));
            }
            for e in &i.embeds_added {
                out.push_str(&format!("      + embed {e}\n"));
            }
            for e in &i.embeds_removed {
                out.push_str(&format!("      - embed {e}\n"));
            }
        }

        // Arch layers
        for l in &diff.sections.arch_layers.added {
            out.push_str(&format!("  + layer {}\n", l.package_dir));
        }
        for l in &diff.sections.arch_layers.removed {
            out.push_str(&format!("  - layer {}\n", l.package_dir));
        }
        for l in &diff.sections.arch_layers.modified {
            if let Some((ref bl, ref al)) = l.layer_changed {
                out.push_str(&format!("  ~ layer {}: {bl} → {al}\n", l.package_dir));
            }
        }

        // Contracts
        for c in &diff.sections.contracts.added {
            out.push_str(&format!("  + contract {} ({})\n", c.name, c.family));
        }
        for c in &diff.sections.contracts.removed {
            out.push_str(&format!("  - contract {} ({})\n", c.name, c.family));
        }
        for c in &diff.sections.contracts.modified {
            if let Some((ref bf, ref af)) = c.family_changed {
                out.push_str(&format!("  ~ contract {}: {bf} → {af}\n", c.name));
            }
        }

        // Import changes (verbose only)
        if verbose {
            for i in &diff.sections.imports.added {
                out.push_str(&format!("  + import {} ({})\n", i.path, i.kind));
            }
            for i in &diff.sections.imports.removed {
                out.push_str(&format!("  - import {} ({})\n", i.path, i.kind));
            }
            for i in &diff.sections.imports.modified {
                out.push_str(&format!("  ~ import {}\n", i.path));
                for c in &i.consumers_added {
                    out.push_str(&format!("      + consumer {c}\n"));
                }
                for c in &i.consumers_removed {
                    out.push_str(&format!("      - consumer {c}\n"));
                }
            }
        }
    }

    // Inferences
    if !diff.inferences.is_empty() {
        out.push_str("\nInferences:\n");
        for inf in &diff.inferences {
            let tag = match inf.severity.as_str() {
                "warning" => "[!]",
                _ => "[i]",
            };
            out.push_str(&format!("  {tag} [{}] {}\n", inf.category, inf.message));
        }
    }

    out
}

fn render_stat_line(out: &mut String, label: &str, delta: i64) {
    if delta != 0 {
        out.push_str(&format!("  {label:14} {:+}\n", delta));
    }
}

fn render_section_summary<T: Serialize>(out: &mut String, name: &str, section: &SectionDiff<T>) {
    if section.total_changes > 0 {
        out.push_str(&format!(
            "{name}: +{} -{} ~{}\n",
            section.added.len(),
            section.removed.len(),
            section.modified.len()
        ));
    }
}

fn has_nontrivial_changes(diff: &SnapshotDiff) -> bool {
    diff.sections.types.total_changes > 0
        || diff.sections.functions.total_changes > 0
        || diff.sections.interfaces.total_changes > 0
        || diff.sections.contracts.total_changes > 0
        || diff.sections.arch_layers.total_changes > 0
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codeintel;
    use std::fs;
    use tempfile::TempDir;

    fn create_fixture_v1(tmp: &TempDir) -> &Path {
        let root = tmp.path();
        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::create_dir_all(root.join("internal/application/ports")).unwrap();
        fs::create_dir_all(root.join("internal/adapters/nats")).unwrap();

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
            root.join("internal/application/ports/configctl.go"),
            r#"package ports

import "context"

type ConfigctlGateway interface {
	CreateDraft(ctx context.Context, cmd string) (string, error)
	GetConfig(ctx context.Context, id string) (string, error)
}
"#,
        ).unwrap();

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
        ).unwrap();

        root
    }

    fn create_fixture_v2(tmp: &TempDir) -> &Path {
        let root = tmp.path();
        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::create_dir_all(root.join("internal/application/ports")).unwrap();
        fs::create_dir_all(root.join("internal/adapters/nats")).unwrap();
        fs::create_dir_all(root.join("internal/domain/scoring")).unwrap();

        // Modified: ConfigSet gains a field, ConfigVersion changes a field type
        fs::write(
            root.join("internal/domain/configctl/config.go"),
            r#"package configctl

import "time"

type ConfigSet struct {
	SetID    string
	Versions []ConfigVersion
	Label    string
}

type ConfigVersion struct {
	VersionID string
	CreatedAt int64
}

func NewConfigSet(id string, label string) ConfigSet {
	return ConfigSet{SetID: id, Label: label}
}
"#,
        ).unwrap();

        // Modified: interface gains a method
        fs::write(
            root.join("internal/application/ports/configctl.go"),
            r#"package ports

import "context"

type ConfigctlGateway interface {
	CreateDraft(ctx context.Context, cmd string) (string, error)
	GetConfig(ctx context.Context, id string) (string, error)
	DeleteConfig(ctx context.Context, id string) error
}
"#,
        ).unwrap();

        // Unchanged
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
        ).unwrap();

        // New package with a new contract type
        fs::write(
            root.join("internal/domain/scoring/score.go"),
            r#"package scoring

type ScoreComputedEvent struct {
	ScoreID string
	Value   float64
}

func NewScoreComputedEvent(id string, val float64) ScoreComputedEvent {
	return ScoreComputedEvent{ScoreID: id, Value: val}
}
"#,
        ).unwrap();

        root
    }

    fn snap(root: &Path) -> Snapshot {
        let index = codeintel::index::build_index(root);
        super::super::snapshot::build_snapshot_from_index(&index, root)
    }

    // ── No changes ────────────────────────────────────────────

    #[test]
    fn identical_snapshots_produce_no_diff() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture_v1(&tmp);
        let s1 = snap(root);
        let s2 = snap(root);

        let d = diff(&s1, &s2).unwrap();
        assert!(!d.has_changes);
        assert_eq!(d.sections.packages.total_changes, 0);
        assert_eq!(d.sections.types.total_changes, 0);
        assert_eq!(d.sections.functions.total_changes, 0);
        assert_eq!(d.sections.interfaces.total_changes, 0);
        assert_eq!(d.sections.contracts.total_changes, 0);
        assert!(d.inferences.is_empty());
    }

    #[test]
    fn no_change_human_output_says_no_changes() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture_v1(&tmp);
        let s = snap(root);
        let d = diff(&s, &s).unwrap();
        let text = render_human(&d, false);
        assert!(text.contains("No structural changes detected."));
    }

    // ── Small change (field added) ───────────────────────────

    #[test]
    fn detects_added_field() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        assert!(d.has_changes);

        // ConfigSet should have a field added (Label)
        let cs_mod = d.sections.types.modified.iter()
            .find(|t| t.name == "ConfigSet")
            .expect("ConfigSet should be modified");
        assert!(cs_mod.fields_added.contains(&"Label".to_string()));
    }

    #[test]
    fn detects_field_type_change() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();

        // ConfigVersion.CreatedAt changed from time.Time to int64
        let cv_mod = d.sections.types.modified.iter()
            .find(|t| t.name == "ConfigVersion")
            .expect("ConfigVersion should be modified");
        assert!(!cv_mod.fields_type_changed.is_empty());
        let change = &cv_mod.fields_type_changed[0];
        assert_eq!(change.name, "CreatedAt");
        assert_eq!(change.before, "time.Time");
        assert_eq!(change.after, "int64");
    }

    // ── Relevant change (new package, contract, interface method) ──

    #[test]
    fn detects_new_package() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let added_dirs: Vec<&str> = d.sections.packages.added.iter()
            .map(|p| p.dir.as_str())
            .collect();
        assert!(added_dirs.iter().any(|d| d.contains("scoring")));
    }

    #[test]
    fn detects_new_contract() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let added_contracts: Vec<&str> = d.sections.contracts.added.iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(added_contracts.contains(&"ScoreComputedEvent"));
    }

    #[test]
    fn detects_interface_method_added() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let gw = d.sections.interfaces.modified.iter()
            .find(|i| i.name == "ConfigctlGateway")
            .expect("ConfigctlGateway should be modified");
        assert!(gw.methods_added.contains(&"DeleteConfig".to_string()));
    }

    #[test]
    fn detects_function_signature_change() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let ncs = d.sections.functions.modified.iter()
            .find(|f| f.name == "NewConfigSet")
            .expect("NewConfigSet should be modified");
        assert!(ncs.signature_changed.is_some());
    }

    #[test]
    fn detects_new_function() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let added_fns: Vec<&str> = d.sections.functions.added.iter()
            .map(|f| f.name.as_str())
            .collect();
        assert!(added_fns.contains(&"NewScoreComputedEvent"));
    }

    #[test]
    fn detects_new_arch_layer() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let added_layers: Vec<&str> = d.sections.arch_layers.added.iter()
            .map(|l| l.package_dir.as_str())
            .collect();
        assert!(added_layers.iter().any(|d| d.contains("scoring")));
    }

    // ── Stats delta ───────────────────────────────────────────

    #[test]
    fn stats_delta_is_correct() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        assert!(d.stats_delta.total_packages > 0); // scoring added
        assert!(d.stats_delta.total_types > 0); // ScoreComputedEvent added
        assert!(d.stats_delta.contracts_detected > 0); // ScoreComputedEvent is a contract
    }

    // ── Inferences ────────────────────────────────────────────

    #[test]
    fn generates_contract_inference() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let contract_inf = d.inferences.iter()
            .find(|i| i.category == "contract-surface");
        assert!(contract_inf.is_some());
    }

    #[test]
    fn generates_interface_expansion_inference() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let iface_inf = d.inferences.iter()
            .find(|i| i.category == "interface-expansion");
        assert!(iface_inf.is_some());
        assert!(iface_inf.unwrap().message.contains("DeleteConfig"));
    }

    #[test]
    fn generates_type_migration_inference() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let type_inf = d.inferences.iter()
            .find(|i| i.category == "type-migration");
        assert!(type_inf.is_some());
        assert!(type_inf.unwrap().message.contains("CreatedAt"));
    }

    // ── Error handling ────────────────────────────────────────

    #[test]
    fn version_mismatch_returns_error() {
        let tmp = TempDir::new().unwrap();
        let root = create_fixture_v1(&tmp);
        let s1 = snap(root);
        let s2 = snap(root);

        // Hack: create a version mismatch by rebuilding with different version
        // Since version is &'static str, we test via JSON round-trip
        let mut j1 = serde_json::to_value(&s1).unwrap();
        j1["version"] = serde_json::Value::String("2".into());
        let s1_v2: Snapshot = serde_json::from_value(j1).unwrap();

        let result = diff(&s1_v2, &s2);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DiffError::VersionMismatch { .. }));
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = load_snapshot(Path::new("/nonexistent/snapshot.json"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DiffError::Io(_)));
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.json");
        fs::write(&path, "not json at all").unwrap();
        let result = load_snapshot(&path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DiffError::Json(_)));
    }

    // ── JSON round-trip ───────────────────────────────────────

    #[test]
    fn json_output_is_valid() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let json = render_json(&d).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["has_changes"].as_bool().unwrap());
        assert!(parsed["sections"]["types"]["modified"].is_array());
        assert!(parsed["inferences"].is_array());
        assert!(parsed["stats_delta"].is_object());
    }

    // ── Snapshot load + diff from files ───────────────────────

    #[test]
    fn load_and_diff_from_files() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let dir = TempDir::new().unwrap();
        let p1 = dir.path().join("before.json");
        let p2 = dir.path().join("after.json");

        fs::write(&p1, serde_json::to_string_pretty(&before).unwrap()).unwrap();
        fs::write(&p2, serde_json::to_string_pretty(&after).unwrap()).unwrap();

        let loaded_before = load_snapshot(&p1).unwrap();
        let loaded_after = load_snapshot(&p2).unwrap();
        let d = diff(&loaded_before, &loaded_after).unwrap();

        assert!(d.has_changes);
        assert!(!d.sections.types.modified.is_empty());
    }

    // ── Human rendering ───────────────────────────────────────

    #[test]
    fn human_rendering_contains_key_sections() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let before = snap(create_fixture_v1(&tmp1));
        let after = snap(create_fixture_v2(&tmp2));

        let d = diff(&before, &after).unwrap();
        let text = render_human(&d, true);

        assert!(text.contains("Snapshot Diff"));
        assert!(text.contains("Stats delta:"));
        assert!(text.contains("Changes:"));
        assert!(text.contains("Inferences:"));
        assert!(text.contains("ConfigSet"));
        assert!(text.contains("Label"));
    }
}
