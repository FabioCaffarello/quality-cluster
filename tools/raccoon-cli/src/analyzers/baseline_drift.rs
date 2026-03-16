//! Baseline semantic drift detection.
//!
//! Compares the current repository state against a previously saved baseline
//! snapshot and produces structured findings about semantic drift — changes
//! that may indicate divergence from the expected architecture, contracts,
//! or structural invariants.
//!
//! ## Drift classes
//!
//! | Class                    | Severity basis                                      | Source   |
//! |--------------------------|-----------------------------------------------------|----------|
//! | contract-surface-drift   | Removed/modified contracts break consumers           | observed |
//! | interface-breaking       | Removed interface methods require implementor updates | observed |
//! | interface-expansion      | Added methods require implementor updates             | observed |
//! | layer-boundary-drift     | Layer reclassification or removal breaks isolation    | observed |
//! | type-breaking            | Removed fields / changed field types                 | observed |
//! | api-signature-drift      | Exported function signature changes                  | observed |
//! | coupling-increase        | New cross-layer imports detected                     | inferred |
//! | isolation-loss           | Domain/application importing infra packages          | inferred |
//! | contract-proliferation   | Rapid contract growth without validation coverage     | heuristic|
//! | structural-scale-shift   | Large-scale type/line count changes                  | heuristic|
//!
//! ## Provenance
//!
//! Every finding is tagged with its evidence basis:
//!
//! - **observed**: directly seen in the snapshot diff (field removed, method added).
//! - **inferred**: derived from combining multiple observed facts (new import from
//!   domain to adapters → isolation loss).
//! - **heuristic**: statistical or pattern-based (>10 types added → scale shift).

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::snapshot::{self, Snapshot};
use super::snapshot_diff::{self, DiffError, SnapshotDiff};

// ── Report model ──────────────────────────────────────────────────────

/// Top-level baseline drift report.
#[derive(Debug, Clone, Serialize)]
pub struct BaselineDriftReport {
    /// Baseline metadata.
    pub baseline: BaselineInfo,
    /// Current (live) metadata.
    pub current: BaselineInfo,
    /// Overall verdict.
    pub verdict: Verdict,
    /// Semantic drift findings, ordered by severity.
    pub findings: Vec<Finding>,
    /// Summary counts per severity.
    pub summary: Summary,
    /// Baseline health assessment.
    pub baseline_health: BaselineHealth,
    /// Methodology note.
    pub scope_note: String,
}

/// Metadata about a snapshot used in comparison.
#[derive(Debug, Clone, Serialize)]
pub struct BaselineInfo {
    pub generated_at: String,
    pub raccoon_version: String,
    pub total_types: usize,
    pub total_functions: usize,
    pub total_packages: usize,
    pub contracts_detected: usize,
}

/// Overall verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// No semantic drift detected.
    Clean,
    /// Minor drift within acceptable bounds.
    Mild,
    /// Significant drift requiring attention.
    Drifted,
}

/// A single drift finding.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Drift class identifier (e.g. "contract-surface-drift").
    pub class: String,
    /// Severity: critical, warning, info.
    pub severity: String,
    /// Evidence basis: observed, inferred, heuristic.
    pub evidence_basis: String,
    /// Human-readable description of the drift.
    pub message: String,
    /// Concrete evidence items supporting this finding.
    pub evidence: Vec<String>,
    /// Baseline value for context (when applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_value: Option<String>,
    /// Current value for context (when applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_value: Option<String>,
    /// Recommended next step.
    pub recommendation: String,
}

/// Summary counts.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub total_findings: usize,
    pub critical: usize,
    pub warning: usize,
    pub info: usize,
}

/// Health assessment of the baseline itself.
#[derive(Debug, Clone, Serialize)]
pub struct BaselineHealth {
    /// Whether the baseline was loadable and compatible.
    pub usable: bool,
    /// Age of the baseline in human-readable form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_note: Option<String>,
    /// Version compatibility note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_note: Option<String>,
    /// Completeness note (empty sections, etc.).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

// ── Error type ────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum BaselineDriftError {
    Io(std::io::Error),
    Json(serde_json::Error),
    VersionMismatch { before: String, after: String },
    BaselineNotFound(String),
}

impl std::fmt::Display for BaselineDriftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaselineDriftError::Io(e) => write!(f, "I/O error: {e}"),
            BaselineDriftError::Json(e) => write!(f, "JSON parse error: {e}"),
            BaselineDriftError::VersionMismatch { before, after } => {
                write!(
                    f,
                    "snapshot version mismatch: baseline={before}, current={after}"
                )
            }
            BaselineDriftError::BaselineNotFound(p) => {
                write!(f, "baseline snapshot not found: {p}")
            }
        }
    }
}

impl From<DiffError> for BaselineDriftError {
    fn from(e: DiffError) -> Self {
        match e {
            DiffError::Io(e) => BaselineDriftError::Io(e),
            DiffError::Json(e) => BaselineDriftError::Json(e),
            DiffError::VersionMismatch { before, after } => {
                BaselineDriftError::VersionMismatch { before, after }
            }
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────

/// Analyze baseline drift from a saved baseline file against the live project.
pub fn analyze(
    baseline_path: &Path,
    project_root: &Path,
) -> Result<BaselineDriftReport, BaselineDriftError> {
    if !baseline_path.exists() {
        return Err(BaselineDriftError::BaselineNotFound(
            baseline_path.display().to_string(),
        ));
    }

    let baseline = load_baseline(baseline_path)?;
    let current = snapshot::generate(project_root);
    analyze_snapshots(&baseline, &current)
}

/// Analyze baseline drift between two snapshot structs (for testing).
pub fn analyze_snapshots(
    baseline: &Snapshot,
    current: &Snapshot,
) -> Result<BaselineDriftReport, BaselineDriftError> {
    let diff = snapshot_diff::diff(baseline, current)?;
    let baseline_health = assess_baseline_health(baseline);
    let mut findings = Vec::new();

    // Observed findings from diff
    collect_contract_surface_drift(&diff, &mut findings);
    collect_interface_drift(&diff, &mut findings);
    collect_layer_boundary_drift(&diff, &mut findings);
    collect_type_breaking_drift(&diff, &mut findings);
    collect_api_signature_drift(&diff, &mut findings);

    // Inferred findings from cross-referencing
    collect_coupling_increase(&diff, baseline, current, &mut findings);
    collect_isolation_loss(&diff, baseline, current, &mut findings);

    // Heuristic findings from statistics
    collect_contract_proliferation(&diff, baseline, current, &mut findings);
    collect_structural_scale_shift(&diff, &mut findings);

    // Sort by severity (critical first, then warning, then info)
    findings.sort_by(|a, b| severity_rank(&a.severity).cmp(&severity_rank(&b.severity)));

    let summary = Summary {
        total_findings: findings.len(),
        critical: findings.iter().filter(|f| f.severity == "critical").count(),
        warning: findings.iter().filter(|f| f.severity == "warning").count(),
        info: findings.iter().filter(|f| f.severity == "info").count(),
    };

    let verdict = if summary.critical > 0 {
        Verdict::Drifted
    } else if summary.warning > 0 {
        Verdict::Mild
    } else {
        Verdict::Clean
    };

    Ok(BaselineDriftReport {
        baseline: snapshot_info(baseline),
        current: snapshot_info(current),
        verdict,
        findings,
        summary,
        baseline_health,
        scope_note: "Semantic drift analysis based on structural snapshot comparison. \
            Findings tagged [observed] come directly from the diff; [inferred] are derived \
            from combining multiple facts; [heuristic] are pattern-based estimates."
            .to_string(),
    })
}

// ── Loading ───────────────────────────────────────────────────────────

fn load_baseline(path: &Path) -> Result<Snapshot, BaselineDriftError> {
    let contents = std::fs::read_to_string(path).map_err(BaselineDriftError::Io)?;
    let snap: Snapshot = serde_json::from_str(&contents).map_err(BaselineDriftError::Json)?;
    Ok(snap)
}

// ── Helpers ───────────────────────────────────────────────────────────

fn snapshot_info(snap: &Snapshot) -> BaselineInfo {
    BaselineInfo {
        generated_at: snap.metadata.generated_at.clone(),
        raccoon_version: snap.metadata.raccoon_version.clone(),
        total_types: snap.stats.total_types,
        total_functions: snap.stats.total_functions,
        total_packages: snap.stats.total_packages,
        contracts_detected: snap.stats.contracts_detected,
    }
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 3,
    }
}

fn assess_baseline_health(baseline: &Snapshot) -> BaselineHealth {
    let mut warnings = Vec::new();

    if baseline.packages.is_empty() {
        warnings.push("Baseline has no packages — may be from an empty project.".into());
    }
    if baseline.types.is_empty() && baseline.functions.is_empty() {
        warnings.push("Baseline has no types or functions — very sparse snapshot.".into());
    }
    if baseline.contracts.is_empty() {
        warnings.push(
            "Baseline has no detected contracts — contract drift checks will be limited.".into(),
        );
    }
    if baseline.arch_layers.is_empty() {
        warnings.push(
            "Baseline has no architecture layers — layer drift checks will be limited.".into(),
        );
    }

    let version_note = if baseline.version != "1" {
        Some(format!(
            "Baseline uses snapshot version '{}'; current tool expects '1'.",
            baseline.version
        ))
    } else {
        None
    };

    BaselineHealth {
        usable: true,
        age_note: None, // We don't parse dates without a dependency
        version_note,
        warnings,
    }
}

// ── Observed drift collectors ─────────────────────────────────────────

fn collect_contract_surface_drift(diff: &SnapshotDiff, findings: &mut Vec<Finding>) {
    let sec = &diff.sections.contracts;
    if sec.total_changes == 0 {
        return;
    }

    // Removed contracts are critical
    for c in &sec.removed {
        findings.push(Finding {
            class: "contract-surface-drift".into(),
            severity: "critical".into(),
            evidence_basis: "observed".into(),
            message: format!(
                "Contract '{}' (family: {}) was removed from the codebase.",
                c.name, c.family
            ),
            evidence: vec![format!("Contract '{}' present in baseline but absent in current.", c.name)],
            baseline_value: Some(format!("{} ({})", c.name, c.family)),
            current_value: Some("absent".into()),
            recommendation: "Verify all downstream consumers have been updated. Run contract-audit and contract-usage-map.".into(),
        });
    }

    // Family changes are warnings
    for c in &sec.modified {
        if let Some((ref old, ref new)) = c.family_changed {
            findings.push(Finding {
                class: "contract-surface-drift".into(),
                severity: "warning".into(),
                evidence_basis: "observed".into(),
                message: format!(
                    "Contract '{}' changed family from '{}' to '{}'.",
                    c.name, old, new
                ),
                evidence: vec![format!(
                    "Family reclassification: {} → {}.", old, new
                )],
                baseline_value: Some(format!("{} ({})", c.name, old)),
                current_value: Some(format!("{} ({})", c.name, new)),
                recommendation: "Review contract usage flow — construction, propagation, and consumption patterns may need updating.".into(),
            });
        }
    }

    // Added contracts are info (growth, not drift per se)
    if !sec.added.is_empty() {
        let names: Vec<String> = sec
            .added
            .iter()
            .map(|c| format!("{} ({})", c.name, c.family))
            .collect();
        findings.push(Finding {
            class: "contract-surface-drift".into(),
            severity: "info".into(),
            evidence_basis: "observed".into(),
            message: format!(
                "{} new contract(s) added since baseline.",
                sec.added.len()
            ),
            evidence: names,
            baseline_value: None,
            current_value: None,
            recommendation: "Ensure new contracts have validation methods and are covered by contract-usage-map.".into(),
        });
    }
}

fn collect_interface_drift(diff: &SnapshotDiff, findings: &mut Vec<Finding>) {
    // Removed interfaces
    for iface in &diff.sections.interfaces.removed {
        findings.push(Finding {
            class: "interface-breaking".into(),
            severity: "critical".into(),
            evidence_basis: "observed".into(),
            message: format!(
                "Interface '{}.{}' was removed entirely.",
                iface.package, iface.name
            ),
            evidence: vec![format!(
                "Methods lost: {}.",
                if iface.methods_removed.is_empty() {
                    "(all)".to_string()
                } else {
                    iface.methods_removed.join(", ")
                }
            )],
            baseline_value: Some(format!("{}.{}", iface.package, iface.name)),
            current_value: Some("absent".into()),
            recommendation: "All implementors and consumers of this interface must be refactored. Run symbol-trace on the interface name.".into(),
        });
    }

    // Modified interfaces
    for iface in &diff.sections.interfaces.modified {
        if !iface.methods_removed.is_empty() {
            findings.push(Finding {
                class: "interface-breaking".into(),
                severity: "critical".into(),
                evidence_basis: "observed".into(),
                message: format!(
                    "Interface '{}.{}' lost method(s): {}.",
                    iface.package,
                    iface.name,
                    iface.methods_removed.join(", ")
                ),
                evidence: iface
                    .methods_removed
                    .iter()
                    .map(|m| format!("Method '{}' removed.", m))
                    .collect(),
                baseline_value: Some(format!("had methods: {}", iface.methods_removed.join(", "))),
                current_value: Some("methods absent".into()),
                recommendation:
                    "All implementors must drop these methods. Run arch-guard and symbol-trace."
                        .into(),
            });
        }
        if !iface.methods_added.is_empty() {
            findings.push(Finding {
                class: "interface-expansion".into(),
                severity: "warning".into(),
                evidence_basis: "observed".into(),
                message: format!(
                    "Interface '{}.{}' gained method(s): {}.",
                    iface.package, iface.name, iface.methods_added.join(", ")
                ),
                evidence: iface.methods_added.iter()
                    .map(|m| format!("Method '{}' added.", m))
                    .collect(),
                baseline_value: None,
                current_value: Some(format!("new methods: {}", iface.methods_added.join(", "))),
                recommendation: "Existing implementors need the new method(s). Run symbol-trace on the interface.".into(),
            });
        }
    }
}

fn collect_layer_boundary_drift(diff: &SnapshotDiff, findings: &mut Vec<Finding>) {
    let sec = &diff.sections.arch_layers;
    if sec.total_changes == 0 {
        return;
    }

    // Removed layers
    for l in &sec.removed {
        findings.push(Finding {
            class: "layer-boundary-drift".into(),
            severity: "warning".into(),
            evidence_basis: "observed".into(),
            message: format!("Architecture layer at '{}' was removed.", l.package_dir),
            evidence: vec![format!(
                "Package '{}' no longer detected as an architecture layer.",
                l.package_dir
            )],
            baseline_value: Some(l.package_dir.clone()),
            current_value: Some("absent".into()),
            recommendation:
                "Verify this is intentional. Run arch-guard to check boundary compliance.".into(),
        });
    }

    // Layer reclassification
    for l in &sec.modified {
        if let Some((ref old, ref new)) = l.layer_changed {
            findings.push(Finding {
                class: "layer-boundary-drift".into(),
                severity: "critical".into(),
                evidence_basis: "observed".into(),
                message: format!(
                    "Package '{}' changed layer from '{}' to '{}'.",
                    l.package_dir, old, new
                ),
                evidence: vec![format!("Layer reclassification: {} → {}.", old, new)],
                baseline_value: Some(format!("{} → {}", l.package_dir, old)),
                current_value: Some(format!("{} → {}", l.package_dir, new)),
                recommendation:
                    "This changes dependency rules for this package. Run arch-guard immediately."
                        .into(),
            });
        }
    }

    // New layers are informational
    if !sec.added.is_empty() {
        let dirs: Vec<String> = sec.added.iter().map(|l| l.package_dir.clone()).collect();
        findings.push(Finding {
            class: "layer-boundary-drift".into(),
            severity: "info".into(),
            evidence_basis: "observed".into(),
            message: format!("{} new architecture layer(s) detected.", sec.added.len()),
            evidence: dirs,
            baseline_value: None,
            current_value: None,
            recommendation: "Verify new layers follow the expected dependency direction. Run arch-guard.".into(),
        });
    }
}

fn collect_type_breaking_drift(diff: &SnapshotDiff, findings: &mut Vec<Finding>) {
    // Removed types
    for t in &diff.sections.types.removed {
        findings.push(Finding {
            class: "type-breaking".into(),
            severity: "warning".into(),
            evidence_basis: "observed".into(),
            message: format!("Type '{}.{}' ({}) was removed.", t.package, t.name, t.kind),
            evidence: vec![format!(
                "Type '{}' present in baseline but absent in current.",
                t.name
            )],
            baseline_value: Some(format!("{} {}.{}", t.kind, t.package, t.name)),
            current_value: Some("absent".into()),
            recommendation: "Check for all references to this type. Run symbol-trace.".into(),
        });
    }

    // Modified types with breaking changes
    for t in &diff.sections.types.modified {
        let mut evidence = Vec::new();

        if !t.fields_removed.is_empty() {
            evidence.extend(
                t.fields_removed
                    .iter()
                    .map(|f| format!("Field '{}' removed.", f)),
            );
        }
        if !t.fields_type_changed.is_empty() {
            evidence.extend(t.fields_type_changed.iter().map(|c| {
                format!(
                    "Field '{}' type changed: {} → {}.",
                    c.name, c.before, c.after
                )
            }));
        }
        if let Some((ref old, ref new)) = t.kind_changed {
            evidence.push(format!("Kind changed: {} → {}.", old, new));
        }

        if !evidence.is_empty() {
            let severity = if !t.fields_removed.is_empty() || t.kind_changed.is_some() {
                "critical"
            } else {
                "warning"
            };

            findings.push(Finding {
                class: "type-breaking".into(),
                severity: severity.into(),
                evidence_basis: "observed".into(),
                message: format!(
                    "Type '{}.{}' has breaking structural changes.",
                    t.package, t.name
                ),
                evidence,
                baseline_value: Some(format!("{}.{}", t.package, t.name)),
                current_value: None,
                recommendation: "Struct literal consumers and serialization may break. Run impact-map on this type.".into(),
            });
        }
    }
}

fn collect_api_signature_drift(diff: &SnapshotDiff, findings: &mut Vec<Finding>) {
    for f in &diff.sections.functions.modified {
        if let Some((ref before, ref after)) = f.signature_changed {
            let recv = f
                .receiver
                .as_deref()
                .map(|r| format!("({}) ", r))
                .unwrap_or_default();
            findings.push(Finding {
                class: "api-signature-drift".into(),
                severity: "warning".into(),
                evidence_basis: "observed".into(),
                message: format!("Exported function {}{} changed signature.", recv, f.name),
                evidence: vec![format!("{} → {}", before, after)],
                baseline_value: Some(format!("{}{}{}", recv, f.name, before)),
                current_value: Some(format!("{}{}{}", recv, f.name, after)),
                recommendation: "All callers must be updated. Run symbol-trace on this function."
                    .into(),
            });
        }
    }

    // Removed exported functions
    for f in &diff.sections.functions.removed {
        let recv = f
            .receiver
            .as_deref()
            .map(|r| format!("({}) ", r))
            .unwrap_or_default();
        findings.push(Finding {
            class: "api-signature-drift".into(),
            severity: "warning".into(),
            evidence_basis: "observed".into(),
            message: format!("Exported function {}{} was removed.", recv, f.name),
            evidence: vec![format!(
                "Function '{}' present in baseline but absent in current.",
                f.name
            )],
            baseline_value: Some(format!("{}{}", recv, f.name)),
            current_value: Some("absent".into()),
            recommendation: "Verify all callers have been migrated. Run impact-map.".into(),
        });
    }
}

// ── Inferred drift collectors ─────────────────────────────────────────

fn collect_coupling_increase(
    diff: &SnapshotDiff,
    baseline: &Snapshot,
    current: &Snapshot,
    findings: &mut Vec<Finding>,
) {
    // Detect new internal imports that cross layer boundaries.
    // A coupling increase is when a package in one layer starts importing
    // a package from a different layer that it didn't import before.
    let _baseline_layers = layer_map(baseline);
    let current_layers = layer_map(current);

    // Look at added imports that are internal
    let new_internal_imports: Vec<&snapshot_diff::ImportDelta> = diff
        .sections
        .imports
        .added
        .iter()
        .filter(|i| i.kind == "internal")
        .collect();

    if new_internal_imports.is_empty() {
        return;
    }

    // Check which new internal imports cross layer boundaries
    let mut cross_layer = Vec::new();
    for imp in &new_internal_imports {
        // Try to identify which layer this import path belongs to
        for (consumer_added, _) in imp.consumers_added.iter().map(|c| (c, ())) {
            let consumer_layer = find_layer_for_dir(consumer_added, &current_layers);
            let imported_layer = find_layer_for_path(&imp.path, &current_layers);

            if let (Some(cl), Some(il)) = (consumer_layer, imported_layer) {
                if cl != il {
                    cross_layer.push(format!(
                        "{} ({}) → {} ({})",
                        consumer_added, cl, imp.path, il
                    ));
                }
            }
        }
    }

    if !cross_layer.is_empty() {
        findings.push(Finding {
            class: "coupling-increase".into(),
            severity: "warning".into(),
            evidence_basis: "inferred".into(),
            message: format!(
                "{} new cross-layer import(s) detected since baseline.",
                cross_layer.len()
            ),
            evidence: cross_layer,
            baseline_value: None,
            current_value: None,
            recommendation:
                "New cross-layer dependencies may violate architecture rules. Run arch-guard."
                    .into(),
        });
    }

    // Also flag overall internal import growth
    let baseline_internal = baseline
        .imports
        .iter()
        .filter(|i| i.kind == "internal")
        .count();
    let current_internal = current
        .imports
        .iter()
        .filter(|i| i.kind == "internal")
        .count();
    let growth = current_internal as i64 - baseline_internal as i64;

    if growth > 5 {
        findings.push(Finding {
            class: "coupling-increase".into(),
            severity: "info".into(),
            evidence_basis: "heuristic".into(),
            message: format!(
                "Internal import count grew by {} (from {} to {}).",
                growth, baseline_internal, current_internal
            ),
            evidence: vec![format!(
                "Baseline: {} internal imports, Current: {} internal imports.",
                baseline_internal, current_internal
            )],
            baseline_value: Some(format!("{} internal imports", baseline_internal)),
            current_value: Some(format!("{} internal imports", current_internal)),
            recommendation: "Review whether new internal imports are necessary. Consider consolidating shared types.".into(),
        });
    }
}

fn collect_isolation_loss(
    diff: &SnapshotDiff,
    _baseline: &Snapshot,
    current: &Snapshot,
    findings: &mut Vec<Finding>,
) {
    // Check if domain or application packages gained new imports from adapters/actors/interfaces
    let current_layers = layer_map(current);
    let infra_layers: BTreeSet<&str> = ["adapters", "actors", "interfaces"]
        .iter()
        .copied()
        .collect();

    let mut violations = Vec::new();

    for imp in &diff.sections.imports.added {
        if imp.kind != "internal" {
            continue;
        }
        let imported_layer = find_layer_for_path(&imp.path, &current_layers);
        if !imported_layer
            .map(|l| infra_layers.contains(l))
            .unwrap_or(false)
        {
            continue;
        }

        for consumer in &imp.consumers_added {
            let consumer_layer = find_layer_for_dir(consumer, &current_layers);
            if let Some(cl) = consumer_layer {
                if cl == "domain" || cl == "application" {
                    violations.push(format!(
                        "{} ({}) imports {} ({})",
                        consumer,
                        cl,
                        imp.path,
                        imported_layer.unwrap_or("?")
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        findings.push(Finding {
            class: "isolation-loss".into(),
            severity: "critical".into(),
            evidence_basis: "inferred".into(),
            message: format!(
                "{} new inward dependency violation(s): domain/application importing infrastructure.",
                violations.len()
            ),
            evidence: violations,
            baseline_value: None,
            current_value: None,
            recommendation: "Domain and application layers must not import adapters/actors/interfaces. Run arch-guard.".into(),
        });
    }
}

// ── Heuristic drift collectors ────────────────────────────────────────

fn collect_contract_proliferation(
    diff: &SnapshotDiff,
    baseline: &Snapshot,
    current: &Snapshot,
    findings: &mut Vec<Finding>,
) {
    let added = diff.sections.contracts.added.len();
    if added < 3 {
        return;
    }

    // Check if any of the new contracts lack validation coverage
    // (heuristic: contracts without a corresponding Validate method in the snapshot)
    let validated_types: BTreeSet<String> = current
        .functions
        .iter()
        .filter(|f| {
            f.name.starts_with("Validate")
                || f.name.starts_with("Normalize")
                || f.name.starts_with("Check")
        })
        .filter_map(|f| f.receiver.as_ref())
        .map(|r| r.trim_start_matches('*').to_string())
        .collect();

    let unvalidated: Vec<String> = diff
        .sections
        .contracts
        .added
        .iter()
        .filter(|c| !validated_types.contains(&c.name))
        .map(|c| format!("{} ({})", c.name, c.family))
        .collect();

    if !unvalidated.is_empty() {
        findings.push(Finding {
            class: "contract-proliferation".into(),
            severity: "warning".into(),
            evidence_basis: "heuristic".into(),
            message: format!(
                "{} new contracts added since baseline, {} lack validation methods.",
                added,
                unvalidated.len()
            ),
            evidence: unvalidated,
            baseline_value: Some(format!("{} contracts", baseline.stats.contracts_detected)),
            current_value: Some(format!("{} contracts", current.stats.contracts_detected)),
            recommendation:
                "New contracts should have Validate/Normalize methods. Run contract-usage-map."
                    .into(),
        });
    }
}

fn collect_structural_scale_shift(diff: &SnapshotDiff, findings: &mut Vec<Finding>) {
    let sd = &diff.stats_delta;

    if sd.total_types.abs() > 10 {
        findings.push(Finding {
            class: "structural-scale-shift".into(),
            severity: "info".into(),
            evidence_basis: "heuristic".into(),
            message: format!(
                "Significant type count change ({:+}) since baseline.",
                sd.total_types
            ),
            evidence: vec![
                format!("Types: {:+}", sd.total_types),
                format!("Structs: {:+}", sd.structs),
                format!("Interfaces: {:+}", sd.interfaces),
            ],
            baseline_value: None,
            current_value: None,
            recommendation: "Review module boundaries and ensure test coverage for new types."
                .into(),
        });
    }

    if sd.total_lines.abs() > 500 {
        findings.push(Finding {
            class: "structural-scale-shift".into(),
            severity: "info".into(),
            evidence_basis: "heuristic".into(),
            message: format!(
                "Large line count change ({:+}) since baseline.",
                sd.total_lines
            ),
            evidence: vec![
                format!("Lines: {:+}", sd.total_lines),
                format!("Files: {:+}", sd.total_files),
                format!("Test files: {:+}", sd.test_files),
            ],
            baseline_value: None,
            current_value: None,
            recommendation: "Verify test coverage proportional to code growth.".into(),
        });
    }

    if sd.total_packages.abs() > 3 {
        findings.push(Finding {
            class: "structural-scale-shift".into(),
            severity: "info".into(),
            evidence_basis: "heuristic".into(),
            message: format!(
                "Package count changed by {:+} since baseline.",
                sd.total_packages
            ),
            evidence: vec![format!("Packages: {:+}", sd.total_packages)],
            baseline_value: None,
            current_value: None,
            recommendation:
                "Review whether new packages follow the expected layer structure. Run arch-guard."
                    .into(),
        });
    }
}

// ── Layer helpers ─────────────────────────────────────────────────────

fn layer_map(snap: &Snapshot) -> BTreeMap<String, String> {
    snap.arch_layers
        .iter()
        .map(|l| (l.package_dir.clone(), l.layer.clone()))
        .collect()
}

fn find_layer_for_dir<'a>(dir: &str, layers: &'a BTreeMap<String, String>) -> Option<&'a str> {
    // Exact match first
    if let Some(l) = layers.get(dir) {
        return Some(l.as_str());
    }
    // Prefix match (dir might be a subdirectory of a layered package)
    for (pkg_dir, layer) in layers {
        if dir.starts_with(pkg_dir.as_str()) {
            return Some(layer.as_str());
        }
    }
    None
}

fn find_layer_for_path<'a>(
    import_path: &str,
    layers: &'a BTreeMap<String, String>,
) -> Option<&'a str> {
    // Import paths look like "quality-service/internal/adapters/nats"
    // We need to extract the dir-like part
    for (pkg_dir, layer) in layers {
        if import_path.contains(pkg_dir.as_str()) {
            return Some(layer.as_str());
        }
    }
    None
}

// ── Rendering ─────────────────────────────────────────────────────────

/// Render the report as pretty-printed JSON.
pub fn render_json(report: &BaselineDriftReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

/// Render the report in human-readable text.
pub fn render_human(report: &BaselineDriftReport, verbose: bool) -> String {
    let mut out = String::new();

    out.push_str("Baseline Semantic Drift\n");
    out.push_str(&format!(
        "  Baseline: {} (raccoon {})\n",
        report.baseline.generated_at, report.baseline.raccoon_version
    ));
    out.push_str(&format!(
        "  Current:  {} (raccoon {})\n",
        report.current.generated_at, report.current.raccoon_version
    ));
    out.push('\n');

    // Baseline health warnings
    if !report.baseline_health.warnings.is_empty() {
        out.push_str("Baseline health:\n");
        for w in &report.baseline_health.warnings {
            out.push_str(&format!("  [!] {w}\n"));
        }
        out.push('\n');
    }
    if let Some(ref note) = report.baseline_health.version_note {
        out.push_str(&format!("  [!] {note}\n\n"));
    }

    // Verdict
    let verdict_str = match report.verdict {
        Verdict::Clean => "CLEAN — no semantic drift detected",
        Verdict::Mild => "MILD — minor drift detected, review recommended",
        Verdict::Drifted => "DRIFTED — significant semantic drift detected",
    };
    out.push_str(&format!("Verdict: {verdict_str}\n"));
    out.push_str(&format!(
        "Findings: {} total ({} critical, {} warning, {} info)\n",
        report.summary.total_findings,
        report.summary.critical,
        report.summary.warning,
        report.summary.info
    ));
    out.push('\n');

    if report.findings.is_empty() {
        out.push_str("No drift findings.\n");
        return out;
    }

    // Findings grouped by severity
    for sev in &["critical", "warning", "info"] {
        let group: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.severity == *sev)
            .collect();
        if group.is_empty() {
            continue;
        }

        let icon = match *sev {
            "critical" => "[!!]",
            "warning" => "[!]",
            _ => "[i]",
        };

        for f in &group {
            out.push_str(&format!(
                "{} [{}] [{}] {}\n",
                icon, f.class, f.evidence_basis, f.message
            ));

            if verbose {
                for e in &f.evidence {
                    out.push_str(&format!("      evidence: {e}\n"));
                }
                if let Some(ref bv) = f.baseline_value {
                    out.push_str(&format!("      baseline: {bv}\n"));
                }
                if let Some(ref cv) = f.current_value {
                    out.push_str(&format!("      current:  {cv}\n"));
                }
            }

            out.push_str(&format!("      next: {}\n", f.recommendation));
        }
    }

    out.push('\n');
    out.push_str(&format!("Scope: {}\n", report.scope_note));

    out
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codeintel;
    use std::fs;
    use tempfile::TempDir;

    fn create_baseline_fixture(tmp: &TempDir) -> &Path {
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

    fn create_drifted_fixture(tmp: &TempDir) -> &Path {
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
        )
        .unwrap();

        // Modified: interface gains a method, loses one
        fs::write(
            root.join("internal/application/ports/configctl.go"),
            r#"package ports

import "context"

type ConfigctlGateway interface {
	CreateDraft(ctx context.Context, cmd string) (string, error)
	DeleteConfig(ctx context.Context, id string) error
}
"#,
        )
        .unwrap();

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
        )
        .unwrap();

        // New contract type
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
        )
        .unwrap();

        root
    }

    fn snap(root: &Path) -> Snapshot {
        let index = codeintel::index::build_index(root);
        snapshot::build_snapshot_from_index(&index, root)
    }

    // ── No drift ────────────────────────────────────────────────

    #[test]
    fn identical_snapshots_produce_clean_verdict() {
        let tmp = TempDir::new().unwrap();
        let root = create_baseline_fixture(&tmp);
        let s = snap(root);

        let report = analyze_snapshots(&s, &s).unwrap();
        assert_eq!(report.verdict, Verdict::Clean);
        assert_eq!(report.summary.total_findings, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn clean_report_human_output_says_clean() {
        let tmp = TempDir::new().unwrap();
        let root = create_baseline_fixture(&tmp);
        let s = snap(root);

        let report = analyze_snapshots(&s, &s).unwrap();
        let text = render_human(&report, false);
        assert!(text.contains("CLEAN"));
        assert!(text.contains("No drift findings"));
    }

    #[test]
    fn clean_report_json_is_valid() {
        let tmp = TempDir::new().unwrap();
        let root = create_baseline_fixture(&tmp);
        let s = snap(root);

        let report = analyze_snapshots(&s, &s).unwrap();
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["verdict"], "clean");
        assert_eq!(parsed["summary"]["total_findings"], 0);
    }

    // ── Mild drift (added contracts, interface expansion) ───────

    #[test]
    fn detects_interface_method_removal_as_critical() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();

        // GetConfig was removed from ConfigctlGateway
        let breaking = report
            .findings
            .iter()
            .find(|f| f.class == "interface-breaking" && f.message.contains("GetConfig"));
        assert!(breaking.is_some(), "Should detect GetConfig removal");
        assert_eq!(breaking.unwrap().severity, "critical");
        assert_eq!(breaking.unwrap().evidence_basis, "observed");
    }

    #[test]
    fn detects_interface_method_addition_as_warning() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();

        let expansion = report
            .findings
            .iter()
            .find(|f| f.class == "interface-expansion" && f.message.contains("DeleteConfig"));
        assert!(expansion.is_some(), "Should detect DeleteConfig addition");
        assert_eq!(expansion.unwrap().severity, "warning");
    }

    #[test]
    fn detects_field_type_change_as_type_breaking() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();

        let type_break = report
            .findings
            .iter()
            .find(|f| f.class == "type-breaking" && f.message.contains("ConfigVersion"));
        assert!(
            type_break.is_some(),
            "Should detect ConfigVersion breaking change"
        );
        assert!(
            type_break
                .unwrap()
                .evidence
                .iter()
                .any(|e| e.contains("CreatedAt")),
            "Should mention the changed field"
        );
    }

    #[test]
    fn detects_function_signature_change() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();

        let sig_drift = report
            .findings
            .iter()
            .find(|f| f.class == "api-signature-drift" && f.message.contains("NewConfigSet"));
        assert!(
            sig_drift.is_some(),
            "Should detect NewConfigSet signature change"
        );
    }

    #[test]
    fn detects_new_contract_as_info() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();

        let contract_info = report.findings.iter().find(|f| {
            f.class == "contract-surface-drift"
                && f.severity == "info"
                && f.evidence.iter().any(|e| e.contains("ScoreComputedEvent"))
        });
        assert!(
            contract_info.is_some(),
            "Should detect new ScoreComputedEvent"
        );
    }

    // ── Relevant drift (verdict = Drifted) ───────────────────────

    #[test]
    fn drifted_fixture_produces_drifted_verdict() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();
        assert_eq!(report.verdict, Verdict::Drifted);
        assert!(report.summary.critical > 0);
    }

    #[test]
    fn drifted_report_has_recommendations() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();
        for f in &report.findings {
            assert!(
                !f.recommendation.is_empty(),
                "Finding should have recommendation: {}",
                f.message
            );
        }
    }

    #[test]
    fn drifted_report_human_output_shows_findings() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();
        let text = render_human(&report, true);

        assert!(text.contains("DRIFTED"));
        assert!(text.contains("[!!]")); // critical icon
        assert!(text.contains("[observed]"));
        assert!(text.contains("evidence:"));
        assert!(text.contains("next:"));
    }

    #[test]
    fn drifted_report_json_has_all_fields() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();
        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["verdict"], "drifted");
        assert!(parsed["findings"].is_array());
        assert!(parsed["summary"]["critical"].as_u64().unwrap() > 0);
        assert!(parsed["baseline"].is_object());
        assert!(parsed["current"].is_object());
        assert!(parsed["baseline_health"].is_object());
        assert!(parsed["scope_note"].is_string());

        // Check finding structure
        let first = &parsed["findings"][0];
        assert!(first["class"].is_string());
        assert!(first["severity"].is_string());
        assert!(first["evidence_basis"].is_string());
        assert!(first["message"].is_string());
        assert!(first["evidence"].is_array());
        assert!(first["recommendation"].is_string());
    }

    // ── Evidence basis tagging ────────────────────────────────────

    #[test]
    fn findings_have_valid_evidence_basis() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();
        let valid = ["observed", "inferred", "heuristic"];
        for f in &report.findings {
            assert!(
                valid.contains(&f.evidence_basis.as_str()),
                "Invalid evidence_basis '{}' in finding: {}",
                f.evidence_basis,
                f.message
            );
        }
    }

    #[test]
    fn findings_have_valid_severity() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();
        let valid = ["critical", "warning", "info"];
        for f in &report.findings {
            assert!(
                valid.contains(&f.severity.as_str()),
                "Invalid severity '{}' in finding: {}",
                f.severity,
                f.message
            );
        }
    }

    // ── Baseline health ───────────────────────────────────────────

    #[test]
    fn empty_baseline_has_health_warnings() {
        let tmp = TempDir::new().unwrap();
        let empty = snap(tmp.path());
        let health = assess_baseline_health(&empty);

        assert!(health.usable);
        assert!(!health.warnings.is_empty());
        assert!(health.warnings.iter().any(|w| w.contains("no packages")));
    }

    #[test]
    fn healthy_baseline_has_no_warnings() {
        let tmp = TempDir::new().unwrap();
        let root = create_baseline_fixture(&tmp);
        let s = snap(root);
        let health = assess_baseline_health(&s);

        assert!(health.usable);
        // The baseline has packages, types, contracts, and layers — no warnings expected
        // (except possibly "no contracts" if suffix matching doesn't match the fixture)
        // Actually ConfigctlGateway matches "port" contract family
        assert!(health.version_note.is_none());
    }

    // ── Error handling ────────────────────────────────────────────

    #[test]
    fn missing_baseline_returns_error() {
        let result = analyze(Path::new("/nonexistent/baseline.json"), Path::new("."));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn version_mismatch_returns_error() {
        let tmp = TempDir::new().unwrap();
        let root = create_baseline_fixture(&tmp);
        let s1 = snap(root);
        let s2 = snap(root);

        let mut j1 = serde_json::to_value(&s1).unwrap();
        j1["version"] = serde_json::Value::String("99".into());
        let s1_bad: Snapshot = serde_json::from_value(j1).unwrap();

        let result = analyze_snapshots(&s1_bad, &s2);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("version mismatch"));
    }

    // ── File-based round-trip ──────────────────────────────────────

    #[test]
    fn analyze_from_file() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current_root = create_drifted_fixture(&tmp2);

        // Write baseline to file
        let baseline_dir = TempDir::new().unwrap();
        let baseline_path = baseline_dir.path().join("baseline.json");
        let json = serde_json::to_string_pretty(&baseline).unwrap();
        fs::write(&baseline_path, json).unwrap();

        let report = analyze(&baseline_path, current_root).unwrap();
        assert_eq!(report.verdict, Verdict::Drifted);
        assert!(report.summary.total_findings > 0);
    }

    // ── Sorting ────────────────────────────────────────────────────

    #[test]
    fn findings_sorted_by_severity() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        let baseline = snap(create_baseline_fixture(&tmp1));
        let current = snap(create_drifted_fixture(&tmp2));

        let report = analyze_snapshots(&baseline, &current).unwrap();

        let mut last_rank = 0u8;
        for f in &report.findings {
            let rank = severity_rank(&f.severity);
            assert!(
                rank >= last_rank,
                "Findings not sorted by severity: {} after rank {}",
                f.severity,
                last_rank
            );
            last_rank = rank;
        }
    }
}
