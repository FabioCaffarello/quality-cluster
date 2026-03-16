use crate::error::Result;
use crate::models::{CheckResult, Finding, Report};
use std::collections::BTreeMap;
use std::path::Path;

/// Quality dimension — an area the CLI can validate.
#[derive(Debug, Clone)]
struct Dimension {
    name: &'static str,
    description: &'static str,
    /// CLI command that validates this dimension.
    command: &'static str,
    /// What kind of infrastructure is required.
    requires_infra: bool,
}

const DIMENSIONS: &[Dimension] = &[
    Dimension {
        name: "project-structure",
        description: "go.work, directories, compose, config files",
        command: "raccoon-cli doctor",
        requires_infra: false,
    },
    Dimension {
        name: "topology",
        description: "config/compose/source wiring consistency",
        command: "raccoon-cli topology-doctor",
        requires_infra: false,
    },
    Dimension {
        name: "contracts",
        description: "messaging contracts, envelope, codec invariants",
        command: "raccoon-cli contract-audit",
        requires_infra: false,
    },
    Dimension {
        name: "runtime-bindings",
        description: "config -> kafka -> jetstream -> validator routing chain",
        command: "raccoon-cli runtime-bindings",
        requires_infra: false,
    },
    Dimension {
        name: "architecture",
        description: "clean architecture layer boundaries and purity rules",
        command: "raccoon-cli arch-guard",
        requires_infra: false,
    },
    Dimension {
        name: "drift",
        description: "cross-layer declaration/config/source/docs alignment",
        command: "raccoon-cli drift-detect",
        requires_infra: false,
    },
    Dimension {
        name: "runtime-smoke",
        description: "live E2E pipeline proof (6 stages)",
        command: "raccoon-cli runtime-smoke",
        requires_infra: true,
    },
    Dimension {
        name: "scenario:happy-path",
        description: "full E2E: config lifecycle + data plane + validation results",
        command: "raccoon-cli scenario-smoke happy-path",
        requires_infra: true,
    },
    Dimension {
        name: "scenario:config-lifecycle",
        description: "control plane lifecycle: draft -> validate -> compile -> activate",
        command: "raccoon-cli scenario-smoke config-lifecycle",
        requires_infra: true,
    },
    Dimension {
        name: "scenario:invalid-payload",
        description: "validator catches invalid data from emulator",
        command: "raccoon-cli scenario-smoke invalid-payload",
        requires_infra: true,
    },
    Dimension {
        name: "scenario:missing-binding",
        description: "non-existent scope returns empty results without error",
        command: "raccoon-cli scenario-smoke missing-binding",
        requires_infra: true,
    },
    Dimension {
        name: "scenario:readiness-probe",
        description: "cluster bootstrap and readiness verification",
        command: "raccoon-cli scenario-smoke readiness-probe",
        requires_infra: true,
    },
];

/// Sensitive area — a part of the codebase that demands specific validation coverage.
#[derive(Debug, Clone)]
struct SensitiveArea {
    name: &'static str,
    description: &'static str,
    /// Glob patterns for files in this area.
    patterns: &'static [&'static str],
    /// Which dimensions must cover this area.
    required_dimensions: &'static [&'static str],
}

const SENSITIVE_AREAS: &[SensitiveArea] = &[
    SensitiveArea {
        name: "config-files",
        description: "deploy/configs/*.jsonc — service configuration",
        patterns: &["deploy/configs/"],
        required_dimensions: &["project-structure", "topology", "drift"],
    },
    SensitiveArea {
        name: "compose",
        description: "docker-compose.yaml — service orchestration",
        patterns: &["deploy/compose/"],
        required_dimensions: &["project-structure", "topology", "drift"],
    },
    SensitiveArea {
        name: "nats-adapters",
        description: "NATS/JetStream adapter layer — transport wiring",
        patterns: &["internal/adapters/nats/"],
        required_dimensions: &["contracts", "runtime-bindings", "architecture"],
    },
    SensitiveArea {
        name: "kafka-adapters",
        description: "Kafka adapter layer — data plane transport",
        patterns: &["internal/adapters/kafka/"],
        required_dimensions: &["topology", "runtime-bindings", "architecture"],
    },
    SensitiveArea {
        name: "domain",
        description: "domain layer — business rules, must be pure",
        patterns: &["internal/domain/"],
        required_dimensions: &["architecture", "contracts"],
    },
    SensitiveArea {
        name: "application",
        description: "application layer — use cases and ports",
        patterns: &["internal/application/"],
        required_dimensions: &["architecture", "contracts"],
    },
    SensitiveArea {
        name: "http-handlers",
        description: "HTTP interface layer — API endpoints",
        patterns: &["internal/interfaces/http/"],
        required_dimensions: &["architecture"],
    },
    SensitiveArea {
        name: "actors",
        description: "actor supervision trees — runtime wiring",
        patterns: &["internal/actors/"],
        required_dimensions: &["architecture", "runtime-bindings"],
    },
    SensitiveArea {
        name: "validator-logic",
        description: "validator scope — validation rules and results",
        patterns: &[
            "internal/actors/scopes/validator/",
            "internal/application/validatorresults/",
        ],
        required_dimensions: &[
            "contracts",
            "runtime-bindings",
            "scenario:happy-path",
            "scenario:invalid-payload",
        ],
    },
    SensitiveArea {
        name: "consumer-pipeline",
        description: "consumer scope — kafka-to-jetstream bridging",
        patterns: &[
            "internal/actors/scopes/consumer/",
            "internal/application/dataplane/",
        ],
        required_dimensions: &["topology", "runtime-bindings", "scenario:happy-path"],
    },
    SensitiveArea {
        name: "config-lifecycle",
        description: "configctl scope — config draft/validate/compile/activate",
        patterns: &[
            "internal/actors/scopes/configctl/",
            "internal/application/configctl/",
        ],
        required_dimensions: &["contracts", "scenario:config-lifecycle"],
    },
];

/// Analyze coverage: which dimensions exist, which sensitive areas have full coverage.
pub fn analyze(project_root: &Path) -> Result<Report> {
    let mut report = Report::new("coverage-map");

    // Check 1: Dimension inventory
    let mut inventory_findings = Vec::new();
    let static_count = DIMENSIONS.iter().filter(|d| !d.requires_infra).count();
    let runtime_count = DIMENSIONS.iter().filter(|d| d.requires_infra).count();
    inventory_findings.push(Finding::info(
        "dimension-inventory",
        format!(
            "{} quality dimensions: {} static (no infra), {} runtime (requires cluster)",
            DIMENSIONS.len(),
            static_count,
            runtime_count
        ),
    ));
    for dim in DIMENSIONS {
        let infra_tag = if dim.requires_infra {
            " [requires infra]"
        } else {
            ""
        };
        inventory_findings.push(Finding::info(
            "dimension-inventory",
            format!(
                "  {}: {} — `{}`{}",
                dim.name, dim.description, dim.command, infra_tag
            ),
        ));
    }
    report.add(CheckResult::from_findings(
        "dimension-inventory",
        inventory_findings,
    ));

    // Check 2: Sensitive area coverage
    let mut coverage_ok = true;
    for area in SENSITIVE_AREAS {
        let mut area_findings = Vec::new();
        let area_exists = area.patterns.iter().any(|p| project_root.join(p).exists());

        if !area_exists {
            area_findings.push(Finding::info(
                &format!("coverage:{}", area.name),
                format!(
                    "{} — area not found in project (patterns: {})",
                    area.description,
                    area.patterns.join(", ")
                ),
            ));
            report.add(CheckResult::from_findings(
                &format!("coverage:{}", area.name),
                area_findings,
            ));
            continue;
        }

        let covered: Vec<&str> = area.required_dimensions.iter().copied().collect();
        let dim_names: Vec<&str> = DIMENSIONS.iter().map(|d| d.name).collect();
        let missing: Vec<&str> = covered
            .iter()
            .filter(|d| !dim_names.contains(d))
            .copied()
            .collect();

        if missing.is_empty() {
            area_findings.push(Finding::info(
                &format!("coverage:{}", area.name),
                format!(
                    "{} — covered by {} dimensions: {}",
                    area.description,
                    covered.len(),
                    covered.join(", ")
                ),
            ));
        } else {
            coverage_ok = false;
            area_findings.push(
                Finding::error(
                    &format!("coverage:{}", area.name),
                    format!(
                        "{} — missing coverage dimensions: {}",
                        area.description,
                        missing.join(", ")
                    ),
                )
                .with_why("sensitive areas without full quality coverage allow unsafe changes")
                .with_help("implement the missing dimension or add a scenario covering this area"),
            );
        }

        // Show which commands validate this area
        for dim_name in &covered {
            if let Some(dim) = DIMENSIONS.iter().find(|d| d.name == *dim_name) {
                area_findings.push(Finding::info(
                    &format!("coverage:{}", area.name),
                    format!("  validate with: `{}`", dim.command),
                ));
            }
        }

        report.add(CheckResult::from_findings(
            &format!("coverage:{}", area.name),
            area_findings,
        ));
    }

    // Check 3: Go test coverage scan
    let go_test_areas = scan_go_tests(project_root);
    let mut go_findings = Vec::new();
    if go_test_areas.is_empty() {
        go_findings.push(
            Finding::warning("go-test-coverage", "no Go test files found")
                .with_why("Go unit tests are the first line of defense for business logic")
                .with_help("add _test.go files alongside your Go source"),
        );
    } else {
        go_findings.push(Finding::info(
            "go-test-coverage",
            format!("{} Go packages with tests detected", go_test_areas.len()),
        ));
        for (pkg, count) in &go_test_areas {
            go_findings.push(Finding::info(
                "go-test-coverage",
                format!("  {} — {} test file(s)", pkg, count),
            ));
        }
    }
    report.add(CheckResult::from_findings("go-test-coverage", go_findings));

    // Check 4: Coverage summary
    let total_areas = SENSITIVE_AREAS.len();
    let existing_areas = SENSITIVE_AREAS
        .iter()
        .filter(|a| a.patterns.iter().any(|p| project_root.join(p).exists()))
        .count();
    let mut summary_findings = Vec::new();
    summary_findings.push(Finding::info(
        "coverage-summary",
        format!(
            "{}/{} sensitive areas present in project, {} quality dimensions available",
            existing_areas,
            total_areas,
            DIMENSIONS.len()
        ),
    ));
    if coverage_ok {
        summary_findings.push(Finding::info(
            "coverage-summary",
            "all present sensitive areas have full dimension coverage",
        ));
    }
    report.add(CheckResult::from_findings(
        "coverage-summary",
        summary_findings,
    ));

    Ok(report)
}

/// Scan project for Go test files and return package -> test count mapping.
fn scan_go_tests(project_root: &Path) -> BTreeMap<String, usize> {
    let mut results = BTreeMap::new();
    let internal = project_root.join("internal");
    if !internal.is_dir() {
        return results;
    }
    scan_go_tests_recursive(&internal, project_root, &mut results);
    results
}

fn scan_go_tests_recursive(dir: &Path, project_root: &Path, results: &mut BTreeMap<String, usize>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut test_count = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_go_tests_recursive(&path, project_root, results);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with("_test.go") {
                test_count += 1;
            }
        }
    }
    if test_count > 0 {
        let rel = dir.strip_prefix(project_root).unwrap_or(dir);
        results.insert(rel.display().to_string(), test_count);
    }
}

/// For a given changed file path, return which sensitive areas and dimensions are relevant.
pub fn relevant_checks_for_path(path: &str) -> Vec<(&'static str, Vec<&'static str>)> {
    let mut result = Vec::new();
    for area in SENSITIVE_AREAS {
        if area.patterns.iter().any(|p| path.contains(p)) {
            let commands: Vec<&str> = area
                .required_dimensions
                .iter()
                .filter_map(|dim_name| {
                    DIMENSIONS
                        .iter()
                        .find(|d| d.name == *dim_name)
                        .map(|d| d.command)
                })
                .collect();
            result.push((area.name, commands));
        }
    }
    result
}

/// Return TDD guidance for a set of changed files.
pub fn tdd_guidance(changed_files: &[String]) -> TddGuidance {
    let mut before_commands: Vec<&'static str> = Vec::new();
    let mut after_commands: Vec<&'static str> = Vec::new();
    let mut affected_areas: Vec<&'static str> = Vec::new();
    let mut needs_infra = false;

    for file in changed_files {
        for area in SENSITIVE_AREAS {
            if area.patterns.iter().any(|p| file.contains(p)) {
                if !affected_areas.contains(&area.name) {
                    affected_areas.push(area.name);
                }
                for dim_name in area.required_dimensions {
                    if let Some(dim) = DIMENSIONS.iter().find(|d| d.name == *dim_name) {
                        if !before_commands.contains(&dim.command) {
                            before_commands.push(dim.command);
                        }
                        if !after_commands.contains(&dim.command) {
                            after_commands.push(dim.command);
                        }
                        if dim.requires_infra {
                            needs_infra = true;
                        }
                    }
                }
            }
        }
    }

    // Always include the canonical gate as the after command
    if !after_commands.iter().any(|c| c.contains("quality-gate")) {
        after_commands.push("make verify");
    }

    TddGuidance {
        affected_areas,
        before_commands,
        after_commands,
        needs_infra,
    }
}

#[derive(Debug, Clone)]
pub struct TddGuidance {
    pub affected_areas: Vec<&'static str>,
    pub before_commands: Vec<&'static str>,
    pub after_commands: Vec<&'static str>,
    pub needs_infra: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_are_non_empty() {
        assert!(!DIMENSIONS.is_empty());
    }

    #[test]
    fn sensitive_areas_are_non_empty() {
        assert!(!SENSITIVE_AREAS.is_empty());
    }

    #[test]
    fn all_required_dimensions_exist() {
        let dim_names: Vec<&str> = DIMENSIONS.iter().map(|d| d.name).collect();
        for area in SENSITIVE_AREAS {
            for required in area.required_dimensions {
                assert!(
                    dim_names.contains(required),
                    "area '{}' requires dimension '{}' which doesn't exist",
                    area.name,
                    required
                );
            }
        }
    }

    #[test]
    fn dimension_names_are_unique() {
        let names: Vec<&str> = DIMENSIONS.iter().map(|d| d.name).collect();
        for (i, name) in names.iter().enumerate() {
            assert!(
                !names[i + 1..].contains(name),
                "duplicate dimension name: {name}"
            );
        }
    }

    #[test]
    fn sensitive_area_names_are_unique() {
        let names: Vec<&str> = SENSITIVE_AREAS.iter().map(|a| a.name).collect();
        for (i, name) in names.iter().enumerate() {
            assert!(
                !names[i + 1..].contains(name),
                "duplicate area name: {name}"
            );
        }
    }

    #[test]
    fn relevant_checks_for_nats_adapter() {
        let checks = relevant_checks_for_path("internal/adapters/nats/configctl_gateway.go");
        assert!(!checks.is_empty());
        let (area_name, _) = &checks[0];
        assert_eq!(*area_name, "nats-adapters");
    }

    #[test]
    fn relevant_checks_for_domain() {
        let checks = relevant_checks_for_path("internal/domain/configctl/config.go");
        assert!(!checks.is_empty());
        let (area_name, _) = &checks[0];
        assert_eq!(*area_name, "domain");
    }

    #[test]
    fn relevant_checks_for_unknown_path() {
        let checks = relevant_checks_for_path("README.md");
        assert!(checks.is_empty());
    }

    #[test]
    fn tdd_guidance_for_nats_changes() {
        let guidance = tdd_guidance(&["internal/adapters/nats/codec.go".to_string()]);
        assert!(!guidance.affected_areas.is_empty());
        assert!(guidance.affected_areas.contains(&"nats-adapters"));
        assert!(!guidance.before_commands.is_empty());
        assert!(!guidance.after_commands.is_empty());
    }

    #[test]
    fn tdd_guidance_for_validator_changes() {
        let guidance =
            tdd_guidance(&["internal/actors/scopes/validator/supervisor.go".to_string()]);
        assert!(guidance.affected_areas.contains(&"validator-logic"));
        assert!(
            guidance.needs_infra,
            "validator changes should recommend runtime scenarios"
        );
    }

    #[test]
    fn tdd_guidance_includes_make_verify() {
        let guidance = tdd_guidance(&["internal/domain/configctl/config.go".to_string()]);
        assert!(
            guidance
                .after_commands
                .iter()
                .any(|c| c.contains("verify") || c.contains("quality-gate")),
            "should always include canonical verification"
        );
    }

    #[test]
    fn tdd_guidance_for_no_files() {
        let guidance = tdd_guidance(&[]);
        assert!(guidance.affected_areas.is_empty());
        // Should still recommend make verify as fallback
        assert!(guidance.after_commands.iter().any(|c| c.contains("verify")));
    }

    #[test]
    fn tdd_guidance_deduplicates_commands() {
        let guidance = tdd_guidance(&[
            "internal/adapters/nats/codec.go".to_string(),
            "internal/adapters/nats/configctl_gateway.go".to_string(),
        ]);
        // Should not have duplicates
        let mut seen = std::collections::HashSet::new();
        for cmd in &guidance.before_commands {
            assert!(seen.insert(cmd), "duplicate before command: {cmd}");
        }
    }

    #[test]
    fn analyze_on_nonexistent_project() {
        let report = analyze(Path::new("/nonexistent")).unwrap();
        // Should still produce a valid report (areas won't exist)
        assert!(report.passed());
        assert!(!report.checks.is_empty());
    }

    #[test]
    fn scan_go_tests_on_nonexistent() {
        let results = scan_go_tests(Path::new("/nonexistent"));
        assert!(results.is_empty());
    }

    #[test]
    fn all_dimensions_have_commands() {
        for dim in DIMENSIONS {
            assert!(
                !dim.command.is_empty(),
                "dimension '{}' has no command",
                dim.name
            );
            assert!(
                dim.command.contains("raccoon-cli"),
                "dimension '{}' command should reference raccoon-cli",
                dim.name
            );
        }
    }

    #[test]
    fn all_sensitive_areas_have_patterns() {
        for area in SENSITIVE_AREAS {
            assert!(
                !area.patterns.is_empty(),
                "area '{}' has no patterns",
                area.name
            );
        }
    }

    #[test]
    fn all_sensitive_areas_have_required_dimensions() {
        for area in SENSITIVE_AREAS {
            assert!(
                !area.required_dimensions.is_empty(),
                "area '{}' has no required dimensions",
                area.name
            );
        }
    }

    #[test]
    fn relevant_checks_for_config_files() {
        let checks = relevant_checks_for_path("deploy/configs/consumer.jsonc");
        assert!(!checks.is_empty());
        let (area_name, _) = &checks[0];
        assert_eq!(*area_name, "config-files");
    }

    #[test]
    fn relevant_checks_for_kafka_adapter() {
        let checks = relevant_checks_for_path("internal/adapters/kafka/consumer.go");
        assert!(!checks.is_empty());
        let (area_name, _) = &checks[0];
        assert_eq!(*area_name, "kafka-adapters");
    }

    #[test]
    fn relevant_checks_for_compose() {
        let checks = relevant_checks_for_path("deploy/compose/docker-compose.yaml");
        assert!(!checks.is_empty());
        let (area_name, _) = &checks[0];
        assert_eq!(*area_name, "compose");
    }

    #[test]
    fn tdd_guidance_for_config_lifecycle_changes() {
        let guidance =
            tdd_guidance(&["internal/application/configctl/create_draft.go".to_string()]);
        assert!(guidance.affected_areas.contains(&"config-lifecycle"));
        // Should recommend scenario:config-lifecycle
        assert!(
            guidance
                .before_commands
                .iter()
                .any(|c| c.contains("config-lifecycle")),
            "config lifecycle changes should recommend config-lifecycle scenario"
        );
    }

    #[test]
    fn tdd_guidance_for_consumer_pipeline() {
        let guidance =
            tdd_guidance(&["internal/actors/scopes/consumer/topic_router.go".to_string()]);
        assert!(guidance.affected_areas.contains(&"consumer-pipeline"));
        assert!(guidance.needs_infra);
    }
}
