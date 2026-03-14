//! Contract usage map — map where contracts are defined, constructed,
//! propagated, consumed, and validated across the repository.
//!
//! Uses the codeintel AST index as the primary source and optionally enriches
//! with LSP references for deeper coverage (function body call sites).
//!
//! ## What it maps
//!
//! For each contract type discovered in the codebase:
//! - **Definition**: where the type is declared (file, line, package)
//! - **Construction**: where instances are created (builder calls, struct literals)
//! - **Propagation**: where the type is passed as a parameter, returned, or embedded
//! - **Consumption**: where it is received and destructured/accessed
//! - **Validation**: where `.Validate()` or similar checks are applied
//!
//! ## What it does NOT do
//!
//! - No runtime tracing or reflection analysis
//! - No cross-package type resolution without LSP
//! - Does not invent contracts — all types are derived from the real codebase
//!
//! All observations are tagged with provenance: "observed" (AST fact),
//! "inferred" (heuristic from naming/patterns), or "lsp" (gopls result).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::codeintel::{self, GoFunc, ProjectIndex, StructField, TypeKind, Visibility};
use crate::lsp::bridge::GoplsBridge;
use crate::lsp::types::LspStatus;

// ── Contract family classification ───────────────────────────────────────────

/// Known contract families in this codebase.
const CONTRACT_FAMILIES: &[ContractFamily] = &[
    ContractFamily {
        name: "Envelope",
        marker_types: &["Envelope"],
        marker_packages: &["shared/envelope"],
    },
    ContractFamily {
        name: "Event Metadata",
        marker_types: &["Metadata", "Event"],
        marker_packages: &["shared/events"],
    },
    ContractFamily {
        name: "Correlation ID",
        marker_types: &[],
        marker_packages: &["shared/requestctx"],
    },
    ContractFamily {
        name: "Problem",
        marker_types: &["Problem", "ProblemCode", "ValidationIssue"],
        marker_packages: &["shared/problem"],
    },
    ContractFamily {
        name: "Configctl Commands",
        marker_types: &[
            "CreateDraftCommand",
            "ValidateDraftCommand",
            "ValidateConfigCommand",
            "CompileConfigCommand",
            "ActivateConfigCommand",
            "DeactivateConfigCommand",
            "ArchiveConfigCommand",
            "RejectConfigCommand",
        ],
        marker_packages: &["application/configctl/contracts"],
    },
    ContractFamily {
        name: "Configctl Queries",
        marker_types: &[
            "GetConfigQuery",
            "ListConfigsQuery",
            "ListActiveIngestionBindingsQuery",
            "GetActiveConfigQuery",
        ],
        marker_packages: &["application/configctl/contracts"],
    },
    ContractFamily {
        name: "Configctl Replies",
        marker_types: &[
            "CreateDraftReply",
            "GetConfigReply",
            "ListConfigsReply",
            "ValidateDraftReply",
            "CompileConfigReply",
            "ActivateConfigReply",
        ],
        marker_packages: &["application/configctl/contracts"],
    },
    ContractFamily {
        name: "Configctl Records",
        marker_types: &[
            "ConfigVersionSummary",
            "ConfigVersionDetail",
            "BindingRecord",
            "FieldRecord",
            "RuleRecord",
            "ActivationScopeRecord",
            "ActivationRecord",
            "RuntimeProjectionRecord",
            "ActiveIngestionBindingRecord",
            "ConfigMetadataRecord",
            "CompilationArtifactSummaryRecord",
            "CompilationArtifactRecord",
        ],
        marker_packages: &["application/configctl/contracts"],
    },
    ContractFamily {
        name: "Domain Events",
        marker_types: &[
            "DraftCreatedEvent",
            "ConfigValidatedEvent",
            "ConfigCompiledEvent",
            "ConfigActivatedEvent",
            "ConfigDeactivatedEvent",
            "IngestionRuntimeChangedEvent",
            "ConfigArchivedEvent",
            "ConfigRejectedEvent",
        ],
        marker_packages: &["domain/configctl"],
    },
    ContractFamily {
        name: "Domain Runtime",
        marker_types: &[
            "CompilationArtifact",
            "ActivationScope",
            "Activation",
            "RuntimeProjection",
            "IngestionRuntimeProjection",
            "Binding",
            "Field",
            "Rule",
        ],
        marker_packages: &["domain/configctl"],
    },
    ContractFamily {
        name: "Data Plane",
        marker_types: &["Message", "OriginRecord", "MetadataRecord", "RoutedMessage"],
        marker_packages: &["application/dataplane"],
    },
    ContractFamily {
        name: "Validator Results",
        marker_types: &[
            "ValidationResultRecord",
            "ValidationBindingRecord",
            "ValidationConfigRecord",
            "ViolationRecord",
            "ListValidationResultsQuery",
            "ListValidationResultsReply",
        ],
        marker_packages: &["application/validatorresults"],
    },
    ContractFamily {
        name: "Runtime Contracts",
        marker_types: &["RuntimeRecord", "ScopeRecord", "ConfigRecord", "ArtifactRecord"],
        marker_packages: &["application/runtimecontracts"],
    },
    ContractFamily {
        name: "Validator Runtime",
        marker_types: &[
            "GetActiveRuntimeQuery",
            "GetActiveRuntimeReply",
            "ActiveRuntimeRecord",
        ],
        marker_packages: &["application/validatorruntime"],
    },
    ContractFamily {
        name: "NATS Registry",
        marker_types: &[
            "ControlSpec",
            "EventSpec",
            "StreamSpec",
            "ConsumerSpec",
            "ConfigctlRegistry",
        ],
        marker_packages: &["adapters/nats"],
    },
    ContractFamily {
        name: "Kafka Message",
        marker_types: &[],
        marker_packages: &["adapters/kafka"],
    },
    ContractFamily {
        name: "Actor Messages",
        marker_types: &[],
        marker_packages: &["actors/scopes"],
    },
];

struct ContractFamily {
    name: &'static str,
    marker_types: &'static [&'static str],
    marker_packages: &'static [&'static str],
}

// ── Public report types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ContractUsageMapReport {
    pub contracts: Vec<ContractEntry>,
    pub families: Vec<FamilySummary>,
    pub sensitive_areas: Vec<SensitiveArea>,
    pub statistics: UsageStatistics,
    pub scope_note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp_enrichment: Option<LspEnrichmentStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractEntry {
    pub name: String,
    pub family: String,
    pub definition: Option<UsagePoint>,
    pub construction_sites: Vec<UsagePoint>,
    pub propagation_sites: Vec<UsagePoint>,
    pub consumption_sites: Vec<UsagePoint>,
    pub validation_sites: Vec<UsagePoint>,
    pub usage_breadth: UsageBreadth,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsagePoint {
    pub file: String,
    pub line: usize,
    pub package: String,
    pub kind: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    Observed,
    Inferred,
    Lsp,
}

impl std::fmt::Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provenance::Observed => write!(f, "observed"),
            Provenance::Inferred => write!(f, "inferred"),
            Provenance::Lsp => write!(f, "lsp"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageBreadth {
    /// Used across multiple layers (domain → application → adapters → actors)
    WellDistributed,
    /// Used in 2-3 packages
    Moderate,
    /// Defined but used in only 1 package or not at all
    Limited,
    /// Definition found but no clear usage sites observed
    Orphan,
}

impl std::fmt::Display for UsageBreadth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageBreadth::WellDistributed => write!(f, "well-distributed"),
            UsageBreadth::Moderate => write!(f, "moderate"),
            UsageBreadth::Limited => write!(f, "limited"),
            UsageBreadth::Orphan => write!(f, "orphan"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FamilySummary {
    pub name: String,
    pub contract_count: usize,
    pub total_usage_sites: usize,
    pub packages_involved: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SensitiveArea {
    pub description: String,
    pub contracts_involved: Vec<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageStatistics {
    pub total_contracts: usize,
    pub total_usage_sites: usize,
    pub well_distributed: usize,
    pub moderate: usize,
    pub limited: usize,
    pub orphan: usize,
    pub families_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspEnrichmentStatus {
    pub status: LspStatus,
    pub additional_references: usize,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Analyze contract usage across the project (AST only).
pub fn analyze(project_root: &Path) -> ContractUsageMapReport {
    let index = codeintel::build_index(project_root);
    analyze_with_index(&index, project_root)
}

/// Analyze contract usage with LSP enrichment.
pub fn analyze_with_lsp(
    project_root: &Path,
    bridge: &mut GoplsBridge,
) -> ContractUsageMapReport {
    let index = codeintel::build_index(project_root);
    let mut report = analyze_with_index(&index, project_root);

    // Enrich each contract with LSP references
    let mut additional_refs = 0usize;
    for contract in &mut report.contracts {
        let enriched = bridge.enrich_symbol_with_index(&index, project_root, &contract.name);

        for lsp_ref in &enriched.lsp_references {
            let pkg = file_to_package(&lsp_ref.location.file);
            let already_tracked = contract.propagation_sites.iter()
                .chain(contract.consumption_sites.iter())
                .chain(contract.construction_sites.iter())
                .any(|s| s.file == lsp_ref.location.file && s.line == lsp_ref.location.line);

            if !already_tracked {
                let point = UsagePoint {
                    file: lsp_ref.location.file.clone(),
                    line: lsp_ref.location.line,
                    package: pkg,
                    kind: "reference".to_string(),
                    provenance: Provenance::Lsp,
                };
                contract.propagation_sites.push(point);
                additional_refs += 1;
            }
        }

        // Recompute breadth after LSP enrichment
        contract.usage_breadth = compute_breadth(contract);
    }

    let lsp_status = if bridge.is_available() {
        if additional_refs > 0 {
            LspStatus::Enriched
        } else {
            LspStatus::NoResults
        }
    } else {
        LspStatus::Unavailable {
            reason: bridge.unavailable_reason().unwrap_or("unknown").to_string(),
        }
    };

    report.lsp_enrichment = Some(LspEnrichmentStatus {
        status: lsp_status.clone(),
        additional_references: additional_refs,
    });

    report.scope_note = match &lsp_status {
        LspStatus::Enriched => {
            "Map combines structural AST indexing with gopls semantic analysis. \
             Each usage site is tagged with its source: [observed], [inferred], or [lsp]."
                .to_string()
        }
        LspStatus::NoResults => {
            "Map is computed from structural AST indexing. gopls was available but \
             returned no additional results."
                .to_string()
        }
        LspStatus::Unavailable { reason } => {
            format!(
                "Map is computed from structural AST indexing only. LSP enrichment \
                 unavailable: {reason}. Function body references are not visible."
            )
        }
    };

    // Recompute statistics
    report.statistics = compute_statistics(&report.contracts, &report.families);

    report
}

// ── Core analysis ────────────────────────────────────────────────────────────

fn analyze_with_index(index: &ProjectIndex, project_root: &Path) -> ContractUsageMapReport {
    // 1. Discover all contract types from the index
    let contract_types = discover_contracts(index);

    // 2. For each contract, trace usage across the codebase
    let mut contracts: Vec<ContractEntry> = Vec::new();
    for (name, family, def_file, def_line, def_pkg) in &contract_types {
        let entry = trace_contract_usage(index, project_root, name, family, def_file, *def_line, def_pkg);
        contracts.push(entry);
    }

    // Sort by family then name for stable output
    contracts.sort_by(|a, b| a.family.cmp(&b.family).then(a.name.cmp(&b.name)));

    // 3. Build family summaries
    let families = build_family_summaries(&contracts);

    // 4. Identify sensitive areas
    let sensitive_areas = identify_sensitive_areas(&contracts, index);

    // 5. Compute statistics
    let statistics = compute_statistics(&contracts, &families);

    ContractUsageMapReport {
        contracts,
        families,
        sensitive_areas,
        statistics,
        scope_note: "Map is computed from structural AST indexing (declarations, \
            struct fields, function signatures, const/var references). Function body \
            call sites require --lsp enrichment."
            .to_string(),
        lsp_enrichment: None,
    }
}

/// Discover all contract types by matching against known families.
fn discover_contracts(
    index: &ProjectIndex,
) -> Vec<(String, String, String, usize, String)> {
    let mut results = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }

        let file_dir = file_directory(&file.path);

        for typ in &file.types {
            if typ.visibility != Visibility::Exported {
                continue;
            }

            // Check if this type belongs to a known contract family
            if let Some(family_name) = classify_type(&typ.name, &file_dir) {
                if seen.insert(typ.name.clone()) {
                    results.push((
                        typ.name.clone(),
                        family_name.to_string(),
                        file.path.clone(),
                        typ.location.line,
                        file.package.clone(),
                    ));
                }
            }
        }
    }

    results
}

/// Classify a type into a contract family.
fn classify_type(type_name: &str, file_dir: &str) -> Option<&'static str> {
    for family in CONTRACT_FAMILIES {
        // Check explicit marker types
        if family.marker_types.contains(&type_name) {
            return Some(family.name);
        }

        // Check package-based classification for types in known contract packages
        for pkg in family.marker_packages {
            if file_dir.contains(pkg) {
                // Only include exported types that look like contracts
                if is_contract_like_name(type_name) {
                    return Some(family.name);
                }
            }
        }
    }
    None
}

/// Heuristic: type names that look like contracts (commands, queries, records, events, etc.)
fn is_contract_like_name(name: &str) -> bool {
    let suffixes = [
        "Command", "Query", "Reply", "Record", "Event", "Message",
        "Spec", "Registry", "Binding", "Route", "Scope",
        "Projection", "Artifact", "Config", "Runtime",
        "Envelope", "Problem", "Metadata", "Violation",
        "Status", "Diagnostic", "Issue",
    ];

    // Types that are exact matches to important names
    let exact_matches = [
        "Envelope", "Problem", "Metadata", "Event",
        "Activation", "Binding", "Field", "Rule",
        "Message", "Kind",
    ];

    if exact_matches.contains(&name) {
        return true;
    }

    suffixes.iter().any(|s| name.ends_with(s))
        || name.contains("Spec")
        || name.contains("Registry")
}

/// Trace all usage sites of a contract type.
fn trace_contract_usage(
    index: &ProjectIndex,
    _project_root: &Path,
    type_name: &str,
    family: &str,
    def_file: &str,
    def_line: usize,
    def_pkg: &str,
) -> ContractEntry {
    let mut construction_sites = Vec::new();
    let mut propagation_sites = Vec::new();
    let mut consumption_sites = Vec::new();
    let mut validation_sites = Vec::new();

    for file in &index.files {
        if file.is_test {
            continue;
        }

        let file_pkg = &file.package;
        let file_dir = file_directory(&file.path);

        // 1. Check functions for construction/propagation/consumption patterns
        for func in &file.functions {
            let returns_type = func_returns_type(func, type_name);
            let takes_type = func_takes_type(func, type_name);
            let is_method_on_type = func.receiver.as_ref()
                .map(|r| r.type_name.contains(type_name))
                .unwrap_or(false);

            // Construction: factory functions (New*, Create*) or methods that return the type
            if returns_type && is_constructor_name(&func.name, type_name) {
                construction_sites.push(UsagePoint {
                    file: file.path.clone(),
                    line: func.location.line,
                    package: file_pkg.clone(),
                    kind: format!("constructor: {}", func.name),
                    provenance: Provenance::Observed,
                });
            } else if returns_type && is_method_on_type {
                // Builder methods (With*)
                if func.name.starts_with("With") {
                    construction_sites.push(UsagePoint {
                        file: file.path.clone(),
                        line: func.location.line,
                        package: file_pkg.clone(),
                        kind: format!("builder: {}.{}", type_name, func.name),
                        provenance: Provenance::Observed,
                    });
                } else {
                    propagation_sites.push(UsagePoint {
                        file: file.path.clone(),
                        line: func.location.line,
                        package: file_pkg.clone(),
                        kind: format!("method: {}.{}", type_name, func.name),
                        provenance: Provenance::Observed,
                    });
                }
            } else if returns_type && !takes_type {
                // Function that returns the type but isn't a method on it — likely construction
                construction_sites.push(UsagePoint {
                    file: file.path.clone(),
                    line: func.location.line,
                    package: file_pkg.clone(),
                    kind: format!("factory: {}", func.name),
                    provenance: Provenance::Inferred,
                });
            } else if takes_type && returns_type {
                // Transforms/propagates
                propagation_sites.push(UsagePoint {
                    file: file.path.clone(),
                    line: func.location.line,
                    package: file_pkg.clone(),
                    kind: format!("transform: {}", func.name),
                    provenance: Provenance::Observed,
                });
            } else if takes_type {
                // Consumes the type
                let is_handler = func.name.contains("Handle")
                    || func.name.contains("Process")
                    || func.name.contains("Execute")
                    || func.name.contains("handle")
                    || func.name.starts_with("decode")
                    || func.name.starts_with("Decode");

                if is_handler {
                    consumption_sites.push(UsagePoint {
                        file: file.path.clone(),
                        line: func.location.line,
                        package: file_pkg.clone(),
                        kind: format!("handler: {}", func.name),
                        provenance: Provenance::Observed,
                    });
                } else {
                    consumption_sites.push(UsagePoint {
                        file: file.path.clone(),
                        line: func.location.line,
                        package: file_pkg.clone(),
                        kind: format!("consumer: {}", func.name),
                        provenance: Provenance::Observed,
                    });
                }
            }

            // Validation: methods named Validate, Normalize, or functions checking the type
            if is_method_on_type
                && (func.name == "Validate" || func.name == "Normalize")
            {
                validation_sites.push(UsagePoint {
                    file: file.path.clone(),
                    line: func.location.line,
                    package: file_pkg.clone(),
                    kind: format!("validation: {}.{}", type_name, func.name),
                    provenance: Provenance::Observed,
                });
            }
        }

        // 2. Check struct fields for embedding/referencing this type
        for typ in &file.types {
            if typ.name == type_name {
                continue; // Skip the definition itself
            }

            if let TypeKind::Struct { fields } = &typ.kind {
                for field in fields {
                    if field_references_type(field, type_name) {
                        let kind = if field.embedded {
                            format!("embedded in {}", typ.name)
                        } else {
                            format!("field {}.{}", typ.name, field.name)
                        };

                        // Fields in the same package as definition = propagation
                        // Fields in different packages = consumption/propagation
                        let site = UsagePoint {
                            file: file.path.clone(),
                            line: field.location.line,
                            package: file_pkg.clone(),
                            kind,
                            provenance: Provenance::Observed,
                        };

                        if file_dir == file_directory(def_file) {
                            propagation_sites.push(site);
                        } else {
                            consumption_sites.push(site);
                        }
                    }
                }
            }

            // Check interfaces that reference this type
            if let TypeKind::Interface { methods, embeds } = &typ.kind {
                for method in methods {
                    if method.signature.contains(type_name) {
                        propagation_sites.push(UsagePoint {
                            file: file.path.clone(),
                            line: method.location.line,
                            package: file_pkg.clone(),
                            kind: format!("interface method: {}.{}", typ.name, method.name),
                            provenance: Provenance::Observed,
                        });
                    }
                }
                for embed in embeds {
                    if embed.type_name == type_name {
                        propagation_sites.push(UsagePoint {
                            file: file.path.clone(),
                            line: embed.location.line,
                            package: file_pkg.clone(),
                            kind: format!("interface embed in {}", typ.name),
                            provenance: Provenance::Observed,
                        });
                    }
                }
            }
        }

        // 3. Check constants that reference the type (e.g., event name constants)
        for constant in &file.constants {
            if constant.type_hint.as_deref() == Some(type_name)
                || constant.name.contains(type_name)
            {
                propagation_sites.push(UsagePoint {
                    file: file.path.clone(),
                    line: constant.location.line,
                    package: file_pkg.clone(),
                    kind: format!("constant: {}", constant.name),
                    provenance: Provenance::Observed,
                });
            }
        }
    }

    let definition = Some(UsagePoint {
        file: def_file.to_string(),
        line: def_line,
        package: def_pkg.to_string(),
        kind: "definition".to_string(),
        provenance: Provenance::Observed,
    });

    let mut entry = ContractEntry {
        name: type_name.to_string(),
        family: family.to_string(),
        definition,
        construction_sites,
        propagation_sites,
        consumption_sites,
        validation_sites,
        usage_breadth: UsageBreadth::Orphan, // computed below
    };

    entry.usage_breadth = compute_breadth(&entry);

    entry
}

fn compute_breadth(entry: &ContractEntry) -> UsageBreadth {
    let mut packages: BTreeSet<String> = BTreeSet::new();
    if let Some(ref def) = entry.definition {
        packages.insert(package_layer(&def.file));
    }
    for site in entry.construction_sites.iter()
        .chain(entry.propagation_sites.iter())
        .chain(entry.consumption_sites.iter())
        .chain(entry.validation_sites.iter())
    {
        packages.insert(package_layer(&site.file));
    }

    let total_sites = entry.construction_sites.len()
        + entry.propagation_sites.len()
        + entry.consumption_sites.len()
        + entry.validation_sites.len();

    if total_sites == 0 {
        UsageBreadth::Orphan
    } else if packages.len() >= 3 {
        UsageBreadth::WellDistributed
    } else if packages.len() >= 2 || total_sites >= 3 {
        UsageBreadth::Moderate
    } else {
        UsageBreadth::Limited
    }
}

/// Extract the architectural layer from a file path.
fn package_layer(file: &str) -> String {
    let layers = ["domain", "application", "adapters", "actors", "interfaces", "shared", "cmd"];
    for layer in layers {
        let prefix = format!("internal/{layer}");
        if file.contains(&prefix) {
            return layer.to_string();
        }
    }
    "other".to_string()
}

fn build_family_summaries(contracts: &[ContractEntry]) -> Vec<FamilySummary> {
    let mut by_family: BTreeMap<String, Vec<&ContractEntry>> = BTreeMap::new();
    for c in contracts {
        by_family.entry(c.family.clone()).or_default().push(c);
    }

    by_family
        .into_iter()
        .map(|(name, entries)| {
            let mut all_packages: BTreeSet<String> = BTreeSet::new();
            let mut total_usage = 0usize;

            for e in &entries {
                total_usage += e.construction_sites.len()
                    + e.propagation_sites.len()
                    + e.consumption_sites.len()
                    + e.validation_sites.len();

                if let Some(ref def) = e.definition {
                    all_packages.insert(def.package.clone());
                }
                for site in e.construction_sites.iter()
                    .chain(e.propagation_sites.iter())
                    .chain(e.consumption_sites.iter())
                    .chain(e.validation_sites.iter())
                {
                    all_packages.insert(site.package.clone());
                }
            }

            FamilySummary {
                name,
                contract_count: entries.len(),
                total_usage_sites: total_usage,
                packages_involved: all_packages.into_iter().collect(),
            }
        })
        .collect()
}

fn identify_sensitive_areas(
    contracts: &[ContractEntry],
    _index: &ProjectIndex,
) -> Vec<SensitiveArea> {
    let mut areas = Vec::new();

    // 1. Contracts with no validation sites
    let unvalidated: Vec<String> = contracts
        .iter()
        .filter(|c| {
            c.validation_sites.is_empty()
                && !c.construction_sites.is_empty()
                && (c.name.ends_with("Command")
                    || c.name.ends_with("Query")
                    || c.name.contains("Message"))
        })
        .map(|c| c.name.clone())
        .collect();

    if !unvalidated.is_empty() {
        areas.push(SensitiveArea {
            description: "Commands/queries/messages without observed validation".to_string(),
            contracts_involved: unvalidated,
            provenance: Provenance::Inferred,
        });
    }

    // 2. Contracts with limited usage breadth
    let orphans: Vec<String> = contracts
        .iter()
        .filter(|c| c.usage_breadth == UsageBreadth::Orphan)
        .map(|c| c.name.clone())
        .collect();

    if !orphans.is_empty() {
        areas.push(SensitiveArea {
            description: "Contract types defined but with no observed usage sites".to_string(),
            contracts_involved: orphans,
            provenance: Provenance::Observed,
        });
    }

    // 3. Correlation ID propagation gaps — contracts that carry correlation data
    let correlation_types: Vec<String> = contracts
        .iter()
        .filter(|c| {
            c.name.contains("Correlation")
                || c.family == "Correlation ID"
                || c.family == "Event Metadata"
                || c.name == "Envelope"
        })
        .map(|c| c.name.clone())
        .collect();

    if !correlation_types.is_empty() {
        areas.push(SensitiveArea {
            description: "Correlation/tracing contracts — changes here affect observability chain"
                .to_string(),
            contracts_involved: correlation_types,
            provenance: Provenance::Inferred,
        });
    }

    // 4. Cross-boundary contracts (used in both domain and adapters)
    let cross_boundary: Vec<String> = contracts
        .iter()
        .filter(|c| {
            let layers: BTreeSet<String> = c.construction_sites.iter()
                .chain(c.propagation_sites.iter())
                .chain(c.consumption_sites.iter())
                .map(|s| package_layer(&s.file))
                .collect();
            layers.contains("domain") && (layers.contains("adapters") || layers.contains("actors"))
        })
        .map(|c| c.name.clone())
        .collect();

    if !cross_boundary.is_empty() {
        areas.push(SensitiveArea {
            description: "Contracts crossing domain/infrastructure boundary — changes propagate widely"
                .to_string(),
            contracts_involved: cross_boundary,
            provenance: Provenance::Observed,
        });
    }

    areas
}

fn compute_statistics(contracts: &[ContractEntry], families: &[FamilySummary]) -> UsageStatistics {
    let total_usage: usize = contracts
        .iter()
        .map(|c| {
            c.construction_sites.len()
                + c.propagation_sites.len()
                + c.consumption_sites.len()
                + c.validation_sites.len()
        })
        .sum();

    UsageStatistics {
        total_contracts: contracts.len(),
        total_usage_sites: total_usage,
        well_distributed: contracts
            .iter()
            .filter(|c| c.usage_breadth == UsageBreadth::WellDistributed)
            .count(),
        moderate: contracts
            .iter()
            .filter(|c| c.usage_breadth == UsageBreadth::Moderate)
            .count(),
        limited: contracts
            .iter()
            .filter(|c| c.usage_breadth == UsageBreadth::Limited)
            .count(),
        orphan: contracts
            .iter()
            .filter(|c| c.usage_breadth == UsageBreadth::Orphan)
            .count(),
        families_count: families.len(),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn file_directory(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

fn file_to_package(file: &str) -> String {
    let dir = file_directory(file);
    match dir.rfind('/') {
        Some(pos) => dir[pos + 1..].to_string(),
        None => dir,
    }
}

fn func_returns_type(func: &GoFunc, type_name: &str) -> bool {
    func.returns.iter().any(|r| r.type_expr.contains(type_name))
}

fn func_takes_type(func: &GoFunc, type_name: &str) -> bool {
    func.params.iter().any(|p| p.type_expr.contains(type_name))
}

fn is_constructor_name(func_name: &str, type_name: &str) -> bool {
    func_name.starts_with("New")
        || func_name.starts_with("Create")
        || func_name.starts_with("Make")
        || func_name == type_name
        || func_name.starts_with(&format!("new{}", type_name))
        || func_name.starts_with(&format!("New{}", type_name))
}

fn field_references_type(field: &StructField, type_name: &str) -> bool {
    // Check if the field's type expression contains the type name
    // Handle pointers (*Type), slices ([]Type), maps (map[K]Type), generics (Foo[Type])
    field.type_expr.contains(type_name)
}

// ── Rendering ────────────────────────────────────────────────────────────────

pub fn render_json(report: &ContractUsageMapReport) -> crate::error::Result<String> {
    Ok(serde_json::to_string_pretty(report)?)
}

pub fn render_human(report: &ContractUsageMapReport, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "=== Contract Usage Map ===").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Scope: {}", report.scope_note).unwrap();
    writeln!(out).unwrap();

    // Statistics summary
    let s = &report.statistics;
    writeln!(
        out,
        "Contracts: {} across {} families | {} total usage sites",
        s.total_contracts, s.families_count, s.total_usage_sites
    )
    .unwrap();
    writeln!(
        out,
        "Breadth: {} well-distributed, {} moderate, {} limited, {} orphan",
        s.well_distributed, s.moderate, s.limited, s.orphan
    )
    .unwrap();
    writeln!(out).unwrap();

    // LSP status
    if let Some(ref lsp) = report.lsp_enrichment {
        match &lsp.status {
            LspStatus::Enriched => {
                writeln!(
                    out,
                    "LSP: enriched (+{} references from gopls)",
                    lsp.additional_references
                )
                .unwrap();
            }
            LspStatus::NoResults => {
                writeln!(out, "LSP: connected but no additional results").unwrap();
            }
            LspStatus::Unavailable { reason } => {
                writeln!(out, "LSP: unavailable ({reason})").unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    // Family summaries
    writeln!(out, "--- Families ---").unwrap();
    for family in &report.families {
        writeln!(
            out,
            "  {}: {} contracts, {} usage sites, {} packages",
            family.name,
            family.contract_count,
            family.total_usage_sites,
            family.packages_involved.len()
        )
        .unwrap();
        if verbose {
            for pkg in &family.packages_involved {
                writeln!(out, "    - {pkg}").unwrap();
            }
        }
    }
    writeln!(out).unwrap();

    // Sensitive areas
    if !report.sensitive_areas.is_empty() {
        writeln!(out, "--- Sensitive Areas ---").unwrap();
        for area in &report.sensitive_areas {
            writeln!(
                out,
                "  [{}] {}",
                area.provenance, area.description
            )
            .unwrap();
            if verbose || area.contracts_involved.len() <= 5 {
                for c in &area.contracts_involved {
                    writeln!(out, "    - {c}").unwrap();
                }
            } else {
                for c in area.contracts_involved.iter().take(3) {
                    writeln!(out, "    - {c}").unwrap();
                }
                writeln!(
                    out,
                    "    ... and {} more (use -v to show all)",
                    area.contracts_involved.len() - 3
                )
                .unwrap();
            }
        }
        writeln!(out).unwrap();
    }

    // Contract details
    if verbose {
        writeln!(out, "--- Contract Details ---").unwrap();
        for contract in &report.contracts {
            writeln!(out).unwrap();
            writeln!(
                out,
                "  {} [{}] ({})",
                contract.name, contract.family, contract.usage_breadth
            )
            .unwrap();

            if let Some(ref def) = contract.definition {
                writeln!(out, "    Definition: {}:{}", def.file, def.line).unwrap();
            }

            if !contract.construction_sites.is_empty() {
                writeln!(
                    out,
                    "    Construction ({}):",
                    contract.construction_sites.len()
                )
                .unwrap();
                for site in &contract.construction_sites {
                    writeln!(
                        out,
                        "      [{}] {}:{} — {}",
                        site.provenance, site.file, site.line, site.kind
                    )
                    .unwrap();
                }
            }

            if !contract.propagation_sites.is_empty() {
                writeln!(
                    out,
                    "    Propagation ({}):",
                    contract.propagation_sites.len()
                )
                .unwrap();
                for site in &contract.propagation_sites {
                    writeln!(
                        out,
                        "      [{}] {}:{} — {}",
                        site.provenance, site.file, site.line, site.kind
                    )
                    .unwrap();
                }
            }

            if !contract.consumption_sites.is_empty() {
                writeln!(
                    out,
                    "    Consumption ({}):",
                    contract.consumption_sites.len()
                )
                .unwrap();
                for site in &contract.consumption_sites {
                    writeln!(
                        out,
                        "      [{}] {}:{} — {}",
                        site.provenance, site.file, site.line, site.kind
                    )
                    .unwrap();
                }
            }

            if !contract.validation_sites.is_empty() {
                writeln!(
                    out,
                    "    Validation ({}):",
                    contract.validation_sites.len()
                )
                .unwrap();
                for site in &contract.validation_sites {
                    writeln!(
                        out,
                        "      [{}] {}:{} — {}",
                        site.provenance, site.file, site.line, site.kind
                    )
                    .unwrap();
                }
            }
        }
    } else {
        // Non-verbose: show contracts grouped by breadth
        writeln!(out, "--- Contracts by Breadth ---").unwrap();

        for breadth in &[
            UsageBreadth::WellDistributed,
            UsageBreadth::Moderate,
            UsageBreadth::Limited,
            UsageBreadth::Orphan,
        ] {
            let group: Vec<&ContractEntry> = report
                .contracts
                .iter()
                .filter(|c| c.usage_breadth == *breadth)
                .collect();

            if group.is_empty() {
                continue;
            }

            writeln!(out, "\n  {} ({}):", breadth, group.len()).unwrap();
            for c in &group {
                let total = c.construction_sites.len()
                    + c.propagation_sites.len()
                    + c.consumption_sites.len()
                    + c.validation_sites.len();
                writeln!(
                    out,
                    "    {} [{}] — {} usage sites",
                    c.name, c.family, total
                )
                .unwrap();
            }
        }
    }

    writeln!(out).unwrap();

    out
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codeintel::types::*;

    fn make_index(files: Vec<GoFile>) -> ProjectIndex {
        ProjectIndex {
            files,
            packages: Vec::new(),
            stats: IndexStats {
                total_files: 0,
                total_packages: 0,
                total_types: 0,
                total_functions: 0,
                total_constants: 0,
                total_imports: 0,
                total_lines: 0,
                structs: 0,
                interfaces: 0,
                type_aliases: 0,
                exported_types: 0,
                exported_functions: 0,
                test_files: 0,
            },
        }
    }

    fn envelope_file() -> GoFile {
        GoFile {
            path: "internal/shared/envelope/envelope.go".to_string(),
            package: "envelope".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "Envelope".to_string(),
                kind: TypeKind::Struct {
                    fields: vec![
                        StructField {
                            name: "Kind".to_string(),
                            type_expr: "Kind".to_string(),
                            tag: None,
                            embedded: false,
                            visibility: Visibility::Exported,
                            location: Location { file: "internal/shared/envelope/envelope.go".to_string(), line: 10 },
                        },
                        StructField {
                            name: "Payload".to_string(),
                            type_expr: "T".to_string(),
                            tag: None,
                            embedded: false,
                            visibility: Visibility::Exported,
                            location: Location { file: "internal/shared/envelope/envelope.go".to_string(), line: 11 },
                        },
                    ],
                },
                visibility: Visibility::Exported,
                location: Location { file: "internal/shared/envelope/envelope.go".to_string(), line: 5 },
            }],
            functions: vec![
                GoFunc {
                    name: "New".to_string(),
                    receiver: None,
                    params: vec![
                        Param { name: "kind".to_string(), type_expr: "Kind".to_string() },
                        Param { name: "typ".to_string(), type_expr: "string".to_string() },
                        Param { name: "payload".to_string(), type_expr: "T".to_string() },
                    ],
                    returns: vec![Param { name: String::new(), type_expr: "Envelope[T]".to_string() }],
                    visibility: Visibility::Exported,
                    location: Location { file: "internal/shared/envelope/envelope.go".to_string(), line: 20 },
                },
                GoFunc {
                    name: "WithCorrelationID".to_string(),
                    receiver: Some(Receiver {
                        name: "e".to_string(),
                        type_name: "Envelope[T]".to_string(),
                        pointer: false,
                    }),
                    params: vec![Param { name: "id".to_string(), type_expr: "string".to_string() }],
                    returns: vec![Param { name: String::new(), type_expr: "Envelope[T]".to_string() }],
                    visibility: Visibility::Exported,
                    location: Location { file: "internal/shared/envelope/envelope.go".to_string(), line: 30 },
                },
                GoFunc {
                    name: "Validate".to_string(),
                    receiver: Some(Receiver {
                        name: "e".to_string(),
                        type_name: "Envelope[T]".to_string(),
                        pointer: false,
                    }),
                    params: Vec::new(),
                    returns: vec![Param { name: String::new(), type_expr: "*Problem".to_string() }],
                    visibility: Visibility::Exported,
                    location: Location { file: "internal/shared/envelope/envelope.go".to_string(), line: 40 },
                },
            ],
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 50,
        }
    }

    fn codec_file() -> GoFile {
        GoFile {
            path: "internal/adapters/nats/codec.go".to_string(),
            package: "nats".to_string(),
            imports: Vec::new(),
            types: Vec::new(),
            functions: vec![
                GoFunc {
                    name: "encodeControlRequest".to_string(),
                    receiver: None,
                    params: vec![
                        Param { name: "spec".to_string(), type_expr: "ControlSpec".to_string() },
                        Param { name: "payload".to_string(), type_expr: "T".to_string() },
                    ],
                    returns: vec![Param { name: String::new(), type_expr: "Envelope[T]".to_string() }],
                    visibility: Visibility::Unexported,
                    location: Location { file: "internal/adapters/nats/codec.go".to_string(), line: 13 },
                },
                GoFunc {
                    name: "decodeControlRequest".to_string(),
                    receiver: None,
                    params: vec![
                        Param { name: "spec".to_string(), type_expr: "ControlSpec".to_string() },
                        Param { name: "data".to_string(), type_expr: "[]byte".to_string() },
                    ],
                    returns: vec![Param { name: String::new(), type_expr: "Envelope[T]".to_string() }],
                    visibility: Visibility::Unexported,
                    location: Location { file: "internal/adapters/nats/codec.go".to_string(), line: 32 },
                },
            ],
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 100,
        }
    }

    fn command_file() -> GoFile {
        GoFile {
            path: "internal/application/configctl/contracts/commands.go".to_string(),
            package: "contracts".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "CreateDraftCommand".to_string(),
                kind: TypeKind::Struct {
                    fields: vec![
                        StructField {
                            name: "Name".to_string(),
                            type_expr: "string".to_string(),
                            tag: None,
                            embedded: false,
                            visibility: Visibility::Exported,
                            location: Location { file: "internal/application/configctl/contracts/commands.go".to_string(), line: 5 },
                        },
                    ],
                },
                visibility: Visibility::Exported,
                location: Location { file: "internal/application/configctl/contracts/commands.go".to_string(), line: 3 },
            }],
            functions: vec![
                GoFunc {
                    name: "Validate".to_string(),
                    receiver: Some(Receiver {
                        name: "c".to_string(),
                        type_name: "CreateDraftCommand".to_string(),
                        pointer: false,
                    }),
                    params: Vec::new(),
                    returns: vec![Param { name: String::new(), type_expr: "*Problem".to_string() }],
                    visibility: Visibility::Exported,
                    location: Location { file: "internal/application/configctl/contracts/commands.go".to_string(), line: 10 },
                },
                GoFunc {
                    name: "Normalize".to_string(),
                    receiver: Some(Receiver {
                        name: "c".to_string(),
                        type_name: "CreateDraftCommand".to_string(),
                        pointer: true,
                    }),
                    params: Vec::new(),
                    returns: Vec::new(),
                    visibility: Visibility::Exported,
                    location: Location { file: "internal/application/configctl/contracts/commands.go".to_string(), line: 15 },
                },
            ],
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 20,
        }
    }

    fn handler_file() -> GoFile {
        GoFile {
            path: "internal/actors/scopes/configctl/control_router.go".to_string(),
            package: "configctl".to_string(),
            imports: Vec::new(),
            types: Vec::new(),
            functions: vec![GoFunc {
                name: "handleCreateDraft".to_string(),
                receiver: None,
                params: vec![Param {
                    name: "cmd".to_string(),
                    type_expr: "CreateDraftCommand".to_string(),
                }],
                returns: vec![Param { name: String::new(), type_expr: "CreateDraftReply".to_string() }],
                visibility: Visibility::Unexported,
                location: Location { file: "internal/actors/scopes/configctl/control_router.go".to_string(), line: 50 },
            }],
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 100,
        }
    }

    // ── Well-distributed contract ────────────────────────────────────

    #[test]
    fn envelope_is_well_distributed() {
        let index = make_index(vec![envelope_file(), codec_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let envelope = report.contracts.iter().find(|c| c.name == "Envelope");
        assert!(envelope.is_some(), "Envelope contract should be discovered");
        let envelope = envelope.unwrap();

        assert_eq!(envelope.family, "Envelope");
        assert!(!envelope.construction_sites.is_empty(), "Should have construction sites");
        assert!(!envelope.validation_sites.is_empty(), "Should have validation sites");

        // Envelope is used across shared and adapters layers
        assert!(
            matches!(envelope.usage_breadth, UsageBreadth::WellDistributed | UsageBreadth::Moderate),
            "Envelope should be well-distributed or moderate, got: {}",
            envelope.usage_breadth
        );
    }

    // ── Command with validation ──────────────────────────────────────

    #[test]
    fn command_has_validation() {
        let index = make_index(vec![command_file(), handler_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let cmd = report.contracts.iter().find(|c| c.name == "CreateDraftCommand");
        assert!(cmd.is_some(), "CreateDraftCommand should be discovered");
        let cmd = cmd.unwrap();

        assert_eq!(cmd.family, "Configctl Commands");
        assert!(
            cmd.validation_sites.len() >= 2,
            "Should have Validate + Normalize, got {}",
            cmd.validation_sites.len()
        );
    }

    // ── Command consumed in handler ──────────────────────────────────

    #[test]
    fn command_consumed_in_handler() {
        let index = make_index(vec![command_file(), handler_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let cmd = report.contracts.iter().find(|c| c.name == "CreateDraftCommand").unwrap();

        assert!(
            cmd.consumption_sites.iter().any(|s| s.kind.contains("handler")),
            "Should have handler consumption site, sites: {:?}",
            cmd.consumption_sites
        );
    }

    // ── Limited usage contract ───────────────────────────────────────

    #[test]
    fn isolated_contract_has_limited_breadth() {
        // A contract defined and used only in one package
        let isolated = GoFile {
            path: "internal/application/validatorruntime/contracts/runtime.go".to_string(),
            package: "contracts".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "GetActiveRuntimeQuery".to_string(),
                kind: TypeKind::Struct {
                    fields: vec![StructField {
                        name: "ScopeKind".to_string(),
                        type_expr: "string".to_string(),
                        tag: None,
                        embedded: false,
                        visibility: Visibility::Exported,
                        location: Location {
                            file: "internal/application/validatorruntime/contracts/runtime.go".to_string(),
                            line: 5,
                        },
                    }],
                },
                visibility: Visibility::Exported,
                location: Location {
                    file: "internal/application/validatorruntime/contracts/runtime.go".to_string(),
                    line: 3,
                },
            }],
            functions: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 10,
        };

        let index = make_index(vec![isolated]);
        let report = analyze_with_index(&index, Path::new("."));

        let query = report.contracts.iter().find(|c| c.name == "GetActiveRuntimeQuery");
        assert!(query.is_some());
        let query = query.unwrap();

        assert!(
            matches!(query.usage_breadth, UsageBreadth::Orphan | UsageBreadth::Limited),
            "Isolated contract should be orphan or limited, got: {}",
            query.usage_breadth
        );
    }

    // ── Orphan detection ─────────────────────────────────────────────

    #[test]
    fn orphan_contract_detected() {
        let orphan = GoFile {
            path: "internal/domain/configctl/events.go".to_string(),
            package: "configctl".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "ConfigArchivedEvent".to_string(),
                kind: TypeKind::Struct { fields: Vec::new() },
                visibility: Visibility::Exported,
                location: Location {
                    file: "internal/domain/configctl/events.go".to_string(),
                    line: 90,
                },
            }],
            functions: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 100,
        };

        let index = make_index(vec![orphan]);
        let report = analyze_with_index(&index, Path::new("."));

        let event = report.contracts.iter().find(|c| c.name == "ConfigArchivedEvent").unwrap();
        assert_eq!(event.usage_breadth, UsageBreadth::Orphan);
    }

    // ── Sensitive areas: unvalidated commands ────────────────────────

    #[test]
    fn unvalidated_command_flagged() {
        // A command with no Validate method
        let no_validate = GoFile {
            path: "internal/application/configctl/contracts/commands.go".to_string(),
            package: "contracts".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "ArchiveConfigCommand".to_string(),
                kind: TypeKind::Struct { fields: Vec::new() },
                visibility: Visibility::Exported,
                location: Location {
                    file: "internal/application/configctl/contracts/commands.go".to_string(),
                    line: 50,
                },
            }],
            functions: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 60,
        };

        // Add a handler that uses it (so it has construction sites)
        let handler = GoFile {
            path: "internal/actors/scopes/configctl/handler.go".to_string(),
            package: "configctl".to_string(),
            imports: Vec::new(),
            types: Vec::new(),
            functions: vec![GoFunc {
                name: "CreateArchive".to_string(),
                receiver: None,
                params: Vec::new(),
                returns: vec![Param { name: String::new(), type_expr: "ArchiveConfigCommand".to_string() }],
                visibility: Visibility::Unexported,
                location: Location {
                    file: "internal/actors/scopes/configctl/handler.go".to_string(),
                    line: 10,
                },
            }],
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 20,
        };

        let index = make_index(vec![no_validate, handler]);
        let report = analyze_with_index(&index, Path::new("."));

        let unvalidated = report.sensitive_areas.iter()
            .find(|a| a.description.contains("without observed validation"));
        assert!(
            unvalidated.is_some(),
            "Should flag unvalidated command. Areas: {:?}",
            report.sensitive_areas
        );
        assert!(
            unvalidated.unwrap().contracts_involved.contains(&"ArchiveConfigCommand".to_string())
        );
    }

    // ── Family summaries ─────────────────────────────────────────────

    #[test]
    fn family_summaries_aggregate() {
        let index = make_index(vec![envelope_file(), command_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        assert!(!report.families.is_empty(), "Should have family summaries");

        let envelope_family = report.families.iter().find(|f| f.name == "Envelope");
        assert!(envelope_family.is_some(), "Should have Envelope family");
        assert!(envelope_family.unwrap().contract_count >= 1);
    }

    // ── Statistics ───────────────────────────────────────────────────

    #[test]
    fn statistics_are_consistent() {
        let index = make_index(vec![envelope_file(), command_file(), handler_file(), codec_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let s = &report.statistics;
        assert!(s.total_contracts > 0, "Should have contracts");
        assert_eq!(
            s.well_distributed + s.moderate + s.limited + s.orphan,
            s.total_contracts,
            "Breadth counts should sum to total"
        );
    }

    // ── JSON serialization ───────────────────────────────────────────

    #[test]
    fn json_roundtrip() {
        let index = make_index(vec![envelope_file(), command_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let json = render_json(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed["contracts"].is_array());
        assert!(parsed["families"].is_array());
        assert!(parsed["statistics"]["total_contracts"].is_number());
        assert!(parsed["scope_note"].is_string());
    }

    // ── Human rendering ──────────────────────────────────────────────

    #[test]
    fn human_output_contains_header() {
        let index = make_index(vec![envelope_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let out = render_human(&report, false);
        assert!(out.contains("Contract Usage Map"));
        assert!(out.contains("Families"));
        assert!(out.contains("Contracts by Breadth"));
    }

    #[test]
    fn verbose_output_shows_details() {
        let index = make_index(vec![envelope_file(), codec_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let out = render_human(&report, true);
        assert!(out.contains("Contract Details"));
        assert!(out.contains("Definition:"));
    }

    // ── Provenance tagging ───────────────────────────────────────────

    #[test]
    fn provenance_display() {
        assert_eq!(Provenance::Observed.to_string(), "observed");
        assert_eq!(Provenance::Inferred.to_string(), "inferred");
        assert_eq!(Provenance::Lsp.to_string(), "lsp");
    }

    #[test]
    fn provenance_serialized_lowercase() {
        let json = serde_json::to_string(&Provenance::Observed).unwrap();
        assert_eq!(json, "\"observed\"");
    }

    // ── Test files excluded ──────────────────────────────────────────

    #[test]
    fn test_files_excluded_from_analysis() {
        let test_file = GoFile {
            path: "internal/shared/envelope/envelope_test.go".to_string(),
            package: "envelope".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "TestEnvelope".to_string(),
                kind: TypeKind::Struct { fields: Vec::new() },
                visibility: Visibility::Exported,
                location: Location {
                    file: "internal/shared/envelope/envelope_test.go".to_string(),
                    line: 10,
                },
            }],
            functions: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: true,
            line_count: 20,
        };

        let index = make_index(vec![test_file]);
        let report = analyze_with_index(&index, Path::new("."));

        assert!(
            report.contracts.iter().all(|c| c.name != "TestEnvelope"),
            "Test types should not appear as contracts"
        );
    }

    // ── Builder methods classified as construction ───────────────────

    #[test]
    fn builder_methods_classified_as_construction() {
        let index = make_index(vec![envelope_file()]);
        let report = analyze_with_index(&index, Path::new("."));

        let envelope = report.contracts.iter().find(|c| c.name == "Envelope").unwrap();
        assert!(
            envelope.construction_sites.iter().any(|s| s.kind.contains("builder")),
            "WithCorrelationID should be classified as builder. Sites: {:?}",
            envelope.construction_sites
        );
    }

    // ── Usage breadth computation ────────────────────────────────────

    #[test]
    fn breadth_display() {
        assert_eq!(UsageBreadth::WellDistributed.to_string(), "well-distributed");
        assert_eq!(UsageBreadth::Moderate.to_string(), "moderate");
        assert_eq!(UsageBreadth::Limited.to_string(), "limited");
        assert_eq!(UsageBreadth::Orphan.to_string(), "orphan");
    }

    #[test]
    fn empty_report_valid() {
        let index = make_index(Vec::new());
        let report = analyze_with_index(&index, Path::new("."));

        assert!(report.contracts.is_empty());
        assert!(report.families.is_empty());
        assert_eq!(report.statistics.total_contracts, 0);

        // Should still render without panic
        let _human = render_human(&report, false);
        let _json = render_json(&report).unwrap();
    }

    // ── Ambiguity test: same name in different packages ─────────────

    #[test]
    fn same_name_different_packages_tracked() {
        let binding1 = GoFile {
            path: "internal/application/configctl/contracts/config.go".to_string(),
            package: "contracts".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "BindingRecord".to_string(),
                kind: TypeKind::Struct { fields: Vec::new() },
                visibility: Visibility::Exported,
                location: Location {
                    file: "internal/application/configctl/contracts/config.go".to_string(),
                    line: 10,
                },
            }],
            functions: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 20,
        };

        let binding2 = GoFile {
            path: "internal/application/dataplane/contracts.go".to_string(),
            package: "dataplane".to_string(),
            imports: Vec::new(),
            types: vec![GoType {
                name: "BindingRecord".to_string(),
                kind: TypeKind::Struct { fields: Vec::new() },
                visibility: Visibility::Exported,
                location: Location {
                    file: "internal/application/dataplane/contracts.go".to_string(),
                    line: 20,
                },
            }],
            functions: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            is_test: false,
            line_count: 20,
        };

        let index = make_index(vec![binding1, binding2]);
        let report = analyze_with_index(&index, Path::new("."));

        // BindingRecord should appear once (first seen wins)
        let bindings: Vec<&ContractEntry> = report.contracts.iter()
            .filter(|c| c.name == "BindingRecord")
            .collect();
        assert_eq!(bindings.len(), 1, "Duplicate names should be deduplicated");
    }
}
