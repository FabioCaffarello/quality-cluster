use crate::codeintel;
use crate::codeintel::{GoFile, GoFunc, ImportKind, ProjectIndex, TypeKind, Visibility};
use crate::error::Result;
use crate::models::{CheckResult, Finding, Report};
use std::collections::HashSet;
use std::path::Path;

// ── Layer model ─────────────────────────────────────────────────────────────

/// Architectural layers in dependency order (inner → outer).
/// A layer may only import from layers with a lower or equal index,
/// plus the special "shared" layer which is allowed everywhere.
const LAYERS: &[&str] = &[
    "domain",      // 0 — innermost, no internal deps
    "application", // 1 — depends on domain
    "adapters",    // 2 — implements application ports
    "actors",      // 3 — orchestrates application + adapters
    "interfaces",  // 4 — HTTP handlers, depends on application
];

/// Infrastructure import prefixes forbidden in domain/.
const INFRA_PREFIXES: &[&str] = &[
    "github.com/nats-io",
    "github.com/segmentio/kafka",
    "github.com/anthdm/hollywood",
    "net/http",
    "database/sql",
];

/// Infrastructure package path fragments that indicate adapter-level types.
/// Used by semantic rules that inspect type expressions in struct fields and
/// function signatures to detect leaking infrastructure across layer boundaries.
const INFRA_TYPE_MARKERS: &[&str] = &[
    "nats",
    "kafka",
    "hollywood",
    "jetstream",
    "http.Client",
    "http.Server",
    "http.Handler",
    "sql.DB",
    "sql.Tx",
];

fn layer_index(segment: &str) -> Option<usize> {
    LAYERS.iter().position(|&l| l == segment)
}

/// Extract the first path segment after "internal/" from a Go import path.
fn extract_internal_layer(import_path: &str) -> Option<&str> {
    let rest = import_path.split("internal/").nth(1)?;
    rest.split('/').next()
}

/// Adjacency rules: which layers can depend on which.
fn is_allowed_dependency(from_layer: usize, to_layer: usize) -> bool {
    if from_layer == to_layer {
        return true;
    }
    match from_layer {
        0 => false,         // domain imports nothing internal
        1 => to_layer == 0, // application → domain only
        2 => to_layer <= 1, // adapters → domain, application
        3 => to_layer <= 2, // actors → domain, application, adapters
        4 => to_layer <= 1, // interfaces → domain, application (NOT adapters, actors)
        _ => false,
    }
}

/// Determine which layer a file belongs to by its path.
fn file_layer(file_path: &str) -> Option<&str> {
    let rest = file_path.strip_prefix("internal/")?;
    rest.split('/').next()
}

/// Check whether a type expression contains infrastructure markers.
fn type_expr_has_infra(expr: &str) -> Option<&'static str> {
    for marker in INFRA_TYPE_MARKERS {
        if expr.contains(marker) {
            return Some(marker);
        }
    }
    None
}

/// Check whether a type expression references an adapter-layer package.
fn type_expr_refs_adapter(expr: &str) -> bool {
    // Matches patterns like `natsadapter.Publisher`, `kafka.Producer` etc.
    // where the package qualifier is an adapter-layer package.
    let adapter_pkgs = [
        "nats.",
        "kafka.",
        "natsadapter.",
        "kafkaadapter.",
        "repository.",
        "repositories.",
    ];
    adapter_pkgs.iter().any(|p| expr.contains(p))
}

// ── Main entry point ────────────────────────────────────────────────────────

pub fn analyze(project_root: &Path) -> Result<Report> {
    let mut report = Report::new("arch-guard");

    let internal_dir = project_root.join("internal");
    if !internal_dir.is_dir() {
        report.add(CheckResult::from_findings(
            "internal-dir",
            vec![
                Finding::error("arch-guard", "internal/ directory not found")
                    .with_why("arch-guard scans internal/ for layer dependency violations")
                    .with_help("pass --project-root pointing to the quality-service root"),
            ],
        ));
        return Ok(report);
    }

    // Build the AST-based structural index once — all rules query it.
    let index = codeintel::build_index(project_root);

    // ── Import-based rules (existing, now using codeintel) ────────────
    report.add(check_layer_deps(&index));
    report.add(check_domain_purity(&index));
    report.add(check_application_isolation(&index));
    report.add(check_interfaces_isolation(&index));

    // ── Structural/boundary rules (existing, enhanced) ────────────────
    report.add(check_cmd_boundary(&index));
    report.add(check_tooling_boundary(project_root));
    report.add(check_no_cross_cmd_imports(&index));
    report.add(check_deploy_boundary(project_root));

    // ── Semantic rules (new — AST-based) ──────────────────────────────
    report.add(check_port_contract_leaks(&index));
    report.add(check_domain_type_contamination(&index));
    report.add(check_exported_func_signatures(&index));

    Ok(report)
}

// ── Rule 1: Layer dependency direction ──────────────────────────────────────

fn check_layer_deps(index: &ProjectIndex) -> CheckResult {
    let mut findings = Vec::new();

    for file in internal_files(index) {
        let from_layer_name = match file_layer(&file.path) {
            Some(l) => l,
            None => continue,
        };
        let from_idx = match layer_index(from_layer_name) {
            Some(i) => i,
            None => continue, // e.g. "shared"
        };

        for imp in &file.imports {
            if imp.kind != ImportKind::Internal {
                continue;
            }
            if let Some(to_layer_name) = extract_internal_layer(&imp.path) {
                if to_layer_name == "shared" {
                    continue;
                }
                if let Some(to_idx) = layer_index(to_layer_name) {
                    if !is_allowed_dependency(from_idx, to_idx) {
                        findings.push(
                            Finding::error(
                                "layer-dependency",
                                format!("{from_layer_name}/ must not import {to_layer_name}/",),
                            )
                            .with_location(format!("{}:{}", file.path, imp.location.line))
                            .with_why(format!(
                                "clean architecture: {from_layer_name} (layer {from_idx}) \
                                 cannot depend on {to_layer_name} (layer {to_idx})"
                            ))
                            .with_help(format!(
                                "introduce a port interface in application/ports/ \
                                 instead of importing {to_layer_name}/ directly"
                            )),
                        );
                    }
                }
            }
        }
    }

    CheckResult::from_findings("layer-dependency-direction", findings)
}

// ── Rule 2: Domain purity ───────────────────────────────────────────────────

fn check_domain_purity(index: &ProjectIndex) -> CheckResult {
    let domain_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.starts_with("internal/domain/") && !f.is_test)
        .collect();

    if domain_files.is_empty() {
        return CheckResult::skip("domain-purity", "no domain files found");
    }

    let mut findings = Vec::new();

    for file in &domain_files {
        for imp in &file.imports {
            for prefix in INFRA_PREFIXES {
                if imp.path.starts_with(prefix) {
                    findings.push(
                        Finding::error(
                            "domain-purity",
                            format!("domain imports infrastructure package: {}", imp.path),
                        )
                        .with_location(format!("{}:{}", file.path, imp.location.line))
                        .with_why(
                            "domain layer must be free of infrastructure dependencies \
                             to remain portable and testable",
                        )
                        .with_help("move infrastructure concerns to adapters/"),
                    );
                }
            }
        }
    }

    CheckResult::from_findings("domain-purity", findings)
}

// ── Rule 3: Application isolation ───────────────────────────────────────────

fn check_application_isolation(index: &ProjectIndex) -> CheckResult {
    let app_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.starts_with("internal/application/") && !f.is_test)
        .collect();

    if app_files.is_empty() {
        return CheckResult::skip("application-isolation", "no application files found");
    }

    let mut findings = Vec::new();

    for file in &app_files {
        for imp in &file.imports {
            if imp.kind != ImportKind::Internal {
                continue;
            }
            if let Some(layer) = extract_internal_layer(&imp.path) {
                if layer == "adapters" || layer == "actors" || layer == "interfaces" {
                    findings.push(
                        Finding::error(
                            "application-isolation",
                            format!("application/ imports {layer}/"),
                        )
                        .with_location(format!("{}:{}", file.path, imp.location.line))
                        .with_why(
                            "application layer must depend only on domain and ports, \
                             never on concrete implementations",
                        )
                        .with_help(format!(
                            "define an interface in application/ports/ \
                             and implement it in {layer}/"
                        )),
                    );
                }
            }
        }
    }

    CheckResult::from_findings("application-isolation", findings)
}

// ── Rule 4: Interfaces isolation ────────────────────────────────────────────

fn check_interfaces_isolation(index: &ProjectIndex) -> CheckResult {
    let iface_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.starts_with("internal/interfaces/") && !f.is_test)
        .collect();

    if iface_files.is_empty() {
        return CheckResult::skip("interfaces-isolation", "no interfaces files found");
    }

    let mut findings = Vec::new();

    for file in &iface_files {
        for imp in &file.imports {
            if imp.kind != ImportKind::Internal {
                continue;
            }
            if let Some(layer) = extract_internal_layer(&imp.path) {
                if layer == "adapters" || layer == "actors" {
                    findings.push(
                        Finding::error(
                            "interfaces-isolation",
                            format!("interfaces/ imports {layer}/"),
                        )
                        .with_location(format!("{}:{}", file.path, imp.location.line))
                        .with_why(
                            "HTTP handlers should depend on application use cases, \
                             not on infrastructure adapters or actor orchestration",
                        )
                        .with_help(
                            "inject dependencies via constructor; \
                             interfaces/ should only import application/ and shared/",
                        ),
                    );
                }
            }
        }
    }

    CheckResult::from_findings("interfaces-isolation", findings)
}

// ── Rule 5: Cmd boundary (AST-based type counting) ──────────────────────────

fn check_cmd_boundary(index: &ProjectIndex) -> CheckResult {
    let cmd_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.starts_with("cmd/") && !f.is_test)
        .collect();

    if cmd_files.is_empty() {
        return CheckResult::skip("cmd-boundary", "no cmd/ files found");
    }

    let mut findings = Vec::new();

    for file in &cmd_files {
        // Check that cmd/ does not import domain/ directly
        for imp in &file.imports {
            if imp.kind != ImportKind::Internal {
                continue;
            }
            if let Some(layer) = extract_internal_layer(&imp.path) {
                if layer == "domain" {
                    findings.push(
                        Finding::warning("cmd-boundary", "cmd/ imports domain/ directly")
                            .with_location(format!("{}:{}", file.path, imp.location.line))
                            .with_why(
                                "cmd packages should orchestrate via application use cases, \
                             not reach into domain directly",
                            )
                            .with_help("access domain types through application layer contracts"),
                    );
                }
            }
        }

        // AST-based: count struct/interface type definitions per cmd file
        let type_count = file.types.len();
        if type_count > 5 {
            findings.push(
                Finding::warning(
                    "cmd-boundary",
                    format!(
                        "{} defines {} types — cmd/ should only wire, not define models",
                        file.path, type_count,
                    ),
                )
                .with_why("business types in cmd/ bypass the layered architecture")
                .with_help("move types to internal/application/ or internal/domain/"),
            );
        }
    }

    CheckResult::from_findings("cmd-boundary", findings)
}

// ── Rule 6: Tooling boundary ────────────────────────────────────────────────
// Rust tooling checks remain filesystem-based since they scan Rust source.

fn check_tooling_boundary(project_root: &Path) -> CheckResult {
    let tools_dir = project_root.join("tools");
    if !tools_dir.is_dir() {
        return CheckResult::skip("tooling-boundary", "tools/ directory not found");
    }

    let mut findings = Vec::new();

    walk_files_with_name(&tools_dir, "go.mod", &mut |path| {
        let rel = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy();
        findings.push(
            Finding::error(
                "tooling-boundary",
                format!("Go module found in tools/: {rel}"),
            )
            .with_why("tools/ is the Rust tooling boundary — Go modules here break separation")
            .with_help("move Go code to cmd/ or internal/"),
        );
    });

    walk_rust_files(&tools_dir, &mut |file_path| {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let rel = file_path
                .strip_prefix(project_root)
                .unwrap_or(file_path)
                .to_string_lossy();

            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with('"')
                    || trimmed.starts_with("r#")
                    || trimmed.starts_with("r\"")
                {
                    continue;
                }
                let is_use_stmt =
                    trimmed.starts_with("use ") && trimmed.contains("quality_service");
                let is_mod_stmt = trimmed.starts_with("mod ") && trimmed.contains("internal");
                if is_use_stmt || is_mod_stmt {
                    findings.push(
                        Finding::error(
                            "tooling-boundary",
                            "Rust source references Go internals as Rust modules",
                        )
                        .with_location(format!("{rel}:{}", i + 1))
                        .with_why("Rust tooling must analyze Go source as text, not import it")
                        .with_help("use file scanning instead of module references"),
                    );
                }
            }
        }
    });

    CheckResult::from_findings("tooling-boundary", findings)
}

// ── Rule 7: No cross-cmd imports ────────────────────────────────────────────

fn check_no_cross_cmd_imports(index: &ProjectIndex) -> CheckResult {
    let cmd_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.starts_with("cmd/") && !f.is_test)
        .collect();

    if cmd_files.is_empty() {
        return CheckResult::skip("no-cross-cmd", "no cmd/ files found");
    }

    // Discover cmd subpackage names
    let cmd_names: HashSet<String> = cmd_files
        .iter()
        .filter_map(|f| {
            f.path
                .strip_prefix("cmd/")
                .and_then(|rest| rest.split('/').next())
                .map(|s| s.to_string())
        })
        .collect();

    let mut findings = Vec::new();

    for file in &cmd_files {
        let own_cmd = match file
            .path
            .strip_prefix("cmd/")
            .and_then(|rest| rest.split('/').next())
        {
            Some(c) => c,
            None => continue,
        };

        for imp in &file.imports {
            for other_cmd in &cmd_names {
                if other_cmd == own_cmd {
                    continue;
                }
                if imp.path.contains(&format!("cmd/{other_cmd}")) {
                    findings.push(
                        Finding::error(
                            "no-cross-cmd",
                            format!("cmd/{own_cmd} imports cmd/{other_cmd}"),
                        )
                        .with_location(format!("{}:{}", file.path, imp.location.line))
                        .with_why(
                            "each cmd/ binary must be independently deployable; \
                             cross-cmd imports create hidden coupling",
                        )
                        .with_help(
                            "extract shared logic to internal/application/ or internal/shared/",
                        ),
                    );
                }
            }
        }
    }

    CheckResult::from_findings("no-cross-cmd", findings)
}

// ── Rule 8: Deploy boundary ─────────────────────────────────────────────────
// Remains filesystem-based (scanning for string literals in Go source).

fn check_deploy_boundary(project_root: &Path) -> CheckResult {
    let internal_dir = project_root.join("internal");
    if !internal_dir.is_dir() {
        return CheckResult::skip("deploy-boundary", "internal/ directory not found");
    }

    let mut findings = Vec::new();

    walk_go_files(&internal_dir, &mut |file_path| {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let rel = file_path
                .strip_prefix(project_root)
                .unwrap_or(file_path)
                .to_string_lossy();

            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                    continue;
                }
                if trimmed.contains("deploy/configs/")
                    || trimmed.contains("deploy/compose/")
                    || trimmed.contains("deploy/docker/")
                {
                    findings.push(
                        Finding::warning("deploy-boundary", "Go source hardcodes a deploy/ path")
                            .with_location(format!("{rel}:{}", i + 1))
                            .with_why(
                                "deploy paths should be injected via configuration, \
                             not hardcoded in Go source",
                            )
                            .with_help("use settings/config to pass paths at runtime"),
                    );
                }
            }
        }
    });

    CheckResult::from_findings("deploy-boundary", findings)
}

// ── Rule 9 (NEW): Port contract leaks ───────────────────────────────────────
// Interfaces defined in application/ports/ must not reference adapter-level
// types in their method signatures. If a port method returns `*nats.Conn` or
// accepts `kafka.Reader`, the port contract is contaminated with infrastructure.

fn check_port_contract_leaks(index: &ProjectIndex) -> CheckResult {
    let port_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.contains("application/ports") && !f.is_test)
        .collect();

    if port_files.is_empty() {
        return CheckResult::skip("port-contract-leaks", "no application/ports/ files found");
    }

    let mut findings = Vec::new();

    for file in &port_files {
        for typ in &file.types {
            if let TypeKind::Interface { methods, .. } = &typ.kind {
                for method in methods {
                    if let Some(marker) = type_expr_has_infra(&method.signature) {
                        findings.push(
                            Finding::error(
                                "port-contract-leaks",
                                format!(
                                    "port interface {}.{} signature references infrastructure type '{}'",
                                    typ.name, method.name, marker,
                                ),
                            )
                            .with_location(format!("{}:{}", file.path, method.location.line))
                            .with_why(
                                "port interfaces define application-level contracts; \
                                 infrastructure types in signatures couple the application \
                                 layer to specific adapters, breaking substitutability",
                            )
                            .with_help(
                                "use domain or application-level types in port signatures; \
                                 map to infrastructure types inside the adapter implementation",
                            ),
                        );
                    }
                    if type_expr_refs_adapter(&method.signature) {
                        findings.push(
                            Finding::error(
                                "port-contract-leaks",
                                format!(
                                    "port interface {}.{} signature references adapter package",
                                    typ.name, method.name,
                                ),
                            )
                            .with_location(format!("{}:{}", file.path, method.location.line))
                            .with_why(
                                "adapter-qualified types in port signatures create \
                                 compile-time coupling between application and adapter layers",
                            )
                            .with_help(
                                "define return/parameter types in the application or domain layer",
                            ),
                        );
                    }
                }
            }
        }
    }

    CheckResult::from_findings("port-contract-leaks", findings)
}

// ── Rule 10 (NEW): Domain type contamination ────────────────────────────────
// Struct fields in domain/ must not use infrastructure type expressions.
// e.g., a domain struct with field `conn *nats.Conn` is a contamination.

fn check_domain_type_contamination(index: &ProjectIndex) -> CheckResult {
    let domain_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| f.path.starts_with("internal/domain/") && !f.is_test)
        .collect();

    if domain_files.is_empty() {
        return CheckResult::skip("domain-type-contamination", "no domain files found");
    }

    let mut findings = Vec::new();

    for file in &domain_files {
        for typ in &file.types {
            if let TypeKind::Struct { fields } = &typ.kind {
                for field in fields {
                    if let Some(marker) = type_expr_has_infra(&field.type_expr) {
                        findings.push(
                            Finding::error(
                                "domain-type-contamination",
                                format!(
                                    "domain struct {}.{} has infrastructure type '{}' in field type '{}'",
                                    typ.name, field.name, marker, field.type_expr,
                                ),
                            )
                            .with_location(format!("{}:{}", file.path, field.location.line))
                            .with_why(
                                "domain types must be infrastructure-agnostic; \
                                 embedding infrastructure types makes the domain \
                                 untestable without concrete adapters",
                            )
                            .with_help(
                                "use a domain-level abstraction or move this struct to adapters/",
                            ),
                        );
                    }
                }
            }
        }
    }

    CheckResult::from_findings("domain-type-contamination", findings)
}

// ── Rule 11 (NEW): Exported function signature leaks ────────────────────────
// Exported functions in application/ and domain/ must not use adapter-qualified
// types in their params or returns. This catches cases like:
//   func NewService(conn *nats.Conn) *Service
// which should accept an interface instead.

fn check_exported_func_signatures(index: &ProjectIndex) -> CheckResult {
    let target_files: Vec<&GoFile> = index
        .files
        .iter()
        .filter(|f| {
            !f.is_test
                && (f.path.starts_with("internal/domain/")
                    || f.path.starts_with("internal/application/"))
        })
        .collect();

    if target_files.is_empty() {
        return CheckResult::skip(
            "exported-signature-leaks",
            "no domain/application files found",
        );
    }

    let mut findings = Vec::new();

    for file in &target_files {
        let layer = file_layer(&file.path).unwrap_or("unknown");

        for func in &file.functions {
            if func.visibility != Visibility::Exported {
                continue;
            }

            // Check parameters
            for param in &func.params {
                if let Some(marker) = type_expr_has_infra(&param.type_expr) {
                    findings.push(
                        Finding::warning(
                            "exported-signature-leaks",
                            format!(
                                "{layer}/{}: exported func {} has param '{}' with infra type '{}'",
                                file_name(&file.path),
                                func_display_name(func),
                                param.name,
                                marker,
                            ),
                        )
                        .with_location(format!("{}:{}", file.path, func.location.line))
                        .with_why(
                            "exported functions in inner layers with infrastructure params \
                             force callers to depend on concrete adapters",
                        )
                        .with_help("accept an interface parameter defined in application/ports/"),
                    );
                }
            }

            // Check return types
            for ret in &func.returns {
                if let Some(marker) = type_expr_has_infra(&ret.type_expr) {
                    findings.push(
                        Finding::warning(
                            "exported-signature-leaks",
                            format!(
                                "{layer}/{}: exported func {} returns infra type '{}'",
                                file_name(&file.path),
                                func_display_name(func),
                                marker,
                            ),
                        )
                        .with_location(format!("{}:{}", file.path, func.location.line))
                        .with_why(
                            "returning infrastructure types from inner layers \
                             exposes implementation details to outer layers",
                        )
                        .with_help("return a domain/application-level type or interface"),
                    );
                }
            }
        }
    }

    CheckResult::from_findings("exported-signature-leaks", findings)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Get all non-test files under internal/.
fn internal_files(index: &ProjectIndex) -> Vec<&GoFile> {
    index
        .files
        .iter()
        .filter(|f| f.path.starts_with("internal/") && !f.is_test)
        .collect()
}

/// Extract the file name from a path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Display name for a function, including receiver if present.
fn func_display_name(func: &GoFunc) -> String {
    match &func.receiver {
        Some(r) => {
            let ptr = if r.pointer { "*" } else { "" };
            format!("({ptr}{}).{}", r.type_name, func.name)
        }
        None => func.name.clone(),
    }
}

/// Walk directory tree collecting .go files (non-test, non-vendor).
fn walk_go_files(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "vendor" || name.starts_with('.') {
                continue;
            }
            walk_go_files(&path, cb);
        } else if path.extension().and_then(|e| e.to_str()) == Some("go") {
            cb(&path);
        }
    }
}

/// Walk directory tree collecting files with a specific name.
fn walk_files_with_name(dir: &Path, name: &str, cb: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_files_with_name(&path, name, cb);
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            cb(&path);
        }
    }
}

/// Walk directory tree collecting .rs files.
fn walk_rust_files(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "target" || name.starts_with('.') {
                continue;
            }
            walk_rust_files(&path, cb);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            cb(&path);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CheckStatus, Severity};
    use std::fs;

    /// Create a minimal valid project structure.
    fn scaffold(dir: &Path) {
        fs::write(dir.join("go.work"), "go 1.25\n").unwrap();
        fs::create_dir_all(dir.join("internal/domain/configctl")).unwrap();
        fs::create_dir_all(dir.join("internal/application/configctl")).unwrap();
        fs::create_dir_all(dir.join("internal/application/ports")).unwrap();
        fs::create_dir_all(dir.join("internal/adapters/nats")).unwrap();
        fs::create_dir_all(dir.join("internal/actors/scopes")).unwrap();
        fs::create_dir_all(dir.join("internal/interfaces/http")).unwrap();
        fs::create_dir_all(dir.join("internal/shared/settings")).unwrap();
        fs::create_dir_all(dir.join("cmd/server")).unwrap();
        fs::create_dir_all(dir.join("tools/raccoon-cli/src")).unwrap();
        fs::create_dir_all(dir.join("deploy/configs")).unwrap();
    }

    // ── Layer dependency direction (Rule 1) ─────────────────────────────

    #[test]
    fn clean_project_passes() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport \"fmt\"\n\ntype Config struct{}\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            "package configctl\n\nimport (\n\t\"quality-service/internal/domain/configctl\"\n)\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("internal/adapters/nats/gateway.go"),
            "package nats\n\nimport (\n\t\"quality-service/internal/application/configctl\"\n)\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("internal/actors/scopes/router.go"),
            "package scopes\n\nimport (\n\t\"quality-service/internal/adapters/nats\"\n\t\"quality-service/internal/application/configctl\"\n)\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("internal/interfaces/http/handler.go"),
            "package http\n\nimport (\n\t\"quality-service/internal/application/configctl\"\n)\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("cmd/server/main.go"),
            "package main\n\nimport (\n\t\"quality-service/internal/shared/bootstrap\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        assert!(
            report.passed(),
            "clean project should pass, but got:\n{}",
            report
        );
    }

    #[test]
    fn detects_domain_importing_application() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport (\n\t\"quality-service/internal/application/ports\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        assert!(!report.passed());

        let check = report
            .checks
            .iter()
            .find(|c| c.name == "layer-dependency-direction")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.findings[0].message.contains("domain/"));
        assert!(check.findings[0].message.contains("application/"));
    }

    #[test]
    fn detects_application_importing_adapters() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            "package configctl\n\nimport (\n\t\"quality-service/internal/adapters/nats\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        assert!(!report.passed());

        let check = report
            .checks
            .iter()
            .find(|c| c.name == "application-isolation")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
    }

    #[test]
    fn detects_interfaces_importing_adapters() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/interfaces/http/handler.go"),
            "package http\n\nimport (\n\t\"quality-service/internal/adapters/nats\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "interfaces-isolation")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
    }

    #[test]
    fn detects_interfaces_importing_actors() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/interfaces/http/handler.go"),
            "package http\n\nimport (\n\t\"quality-service/internal/actors/scopes\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "interfaces-isolation")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
    }

    // ── is_allowed_dependency ───────────────────────────────────────────

    #[test]
    fn domain_cannot_import_other_layers() {
        assert!(is_allowed_dependency(0, 0));
        for to in 1..5 {
            assert!(
                !is_allowed_dependency(0, to),
                "domain should not import layer {to}"
            );
        }
    }

    #[test]
    fn application_can_import_domain_and_self() {
        assert!(is_allowed_dependency(1, 0));
        assert!(is_allowed_dependency(1, 1));
        assert!(!is_allowed_dependency(1, 2));
        assert!(!is_allowed_dependency(1, 3));
        assert!(!is_allowed_dependency(1, 4));
    }

    #[test]
    fn adapters_can_import_domain_application_and_self() {
        assert!(is_allowed_dependency(2, 0));
        assert!(is_allowed_dependency(2, 1));
        assert!(is_allowed_dependency(2, 2));
        assert!(!is_allowed_dependency(2, 3));
    }

    #[test]
    fn actors_can_import_up_to_adapters_and_self() {
        assert!(is_allowed_dependency(3, 0));
        assert!(is_allowed_dependency(3, 1));
        assert!(is_allowed_dependency(3, 2));
        assert!(is_allowed_dependency(3, 3));
        assert!(!is_allowed_dependency(3, 4));
    }

    #[test]
    fn interfaces_can_import_domain_application_and_self() {
        assert!(is_allowed_dependency(4, 0));
        assert!(is_allowed_dependency(4, 1));
        assert!(!is_allowed_dependency(4, 2));
        assert!(!is_allowed_dependency(4, 3));
        assert!(is_allowed_dependency(4, 4));
    }

    // ── Domain purity (Rule 2) ──────────────────────────────────────────

    #[test]
    fn detects_domain_importing_nats() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport (\n\t\"github.com/nats-io/nats.go\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "domain-purity")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.findings[0].message.contains("nats"));
    }

    #[test]
    fn detects_domain_importing_kafka() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport (\n\t\"github.com/segmentio/kafka-go\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "domain-purity")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
    }

    #[test]
    fn domain_with_stdlib_only_passes_purity() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport (\n\t\"fmt\"\n\t\"strings\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "domain-purity")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    // ── Shared imports allowed everywhere (Rule 1 edge case) ────────────

    #[test]
    fn shared_imports_are_allowed_in_all_layers() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        for layer in LAYERS {
            let layer_dir = if *layer == "actors" {
                dir.path().join("internal/actors/scopes")
            } else if *layer == "interfaces" {
                dir.path().join("internal/interfaces/http")
            } else {
                dir.path().join(format!("internal/{layer}/configctl"))
            };
            fs::create_dir_all(&layer_dir).unwrap();
            fs::write(
                layer_dir.join("shared_user.go"),
                "package pkg\n\nimport (\n\t\"quality-service/internal/shared/settings\"\n)\n",
            )
            .unwrap();
        }

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "layer-dependency-direction")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    // ── Cmd boundary (Rule 5) ───────────────────────────────────────────

    #[test]
    fn cmd_importing_domain_directly_warns() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("cmd/server/main.go"),
            "package main\n\nimport (\n\t\"quality-service/internal/domain/configctl\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "cmd-boundary")
            .unwrap();
        assert!(
            check
                .findings
                .iter()
                .any(|f| f.severity == Severity::Warning),
            "cmd importing domain should be a warning"
        );
    }

    #[test]
    fn cmd_with_too_many_types_warns() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("cmd/server/main.go"),
            r#"package main

type A struct{}
type B struct{}
type C struct{}
type D struct{}
type E struct{}
type F struct{}
type G interface{}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "cmd-boundary")
            .unwrap();
        assert!(
            check
                .findings
                .iter()
                .any(|f| f.message.contains("defines") && f.message.contains("types")),
            "should warn about too many types in cmd/"
        );
    }

    // ── Cross-cmd imports (Rule 7) ──────────────────────────────────────

    #[test]
    fn detects_cross_cmd_imports() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());
        fs::create_dir_all(dir.path().join("cmd/validator")).unwrap();

        fs::write(
            dir.path().join("cmd/server/main.go"),
            "package main\n\nimport (\n\t\"quality-service/cmd/validator\"\n)\n",
        )
        .unwrap();

        fs::write(dir.path().join("cmd/validator/main.go"), "package main\n").unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "no-cross-cmd")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.findings[0].message.contains("server"));
        assert!(check.findings[0].message.contains("validator"));
    }

    #[test]
    fn no_cross_cmd_when_clean() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("cmd/server/main.go"),
            "package main\n\nimport \"fmt\"\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "no-cross-cmd")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    // ── Tooling boundary (Rule 6) ───────────────────────────────────────

    #[test]
    fn detects_go_mod_in_tools() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("tools/raccoon-cli/go.mod"),
            "module raccoon\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "tooling-boundary")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
    }

    #[test]
    fn clean_tools_dir_passes() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("tools/raccoon-cli/src/main.rs"),
            "fn main() {}\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "tooling-boundary")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    // ── Deploy boundary (Rule 8) ────────────────────────────────────────

    #[test]
    fn detects_hardcoded_deploy_paths() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/shared/settings/loader.go"),
            "package settings\n\nvar path = \"deploy/configs/server.jsonc\"\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "deploy-boundary")
            .unwrap();
        assert!(
            check
                .findings
                .iter()
                .any(|f| f.severity == Severity::Warning),
            "hardcoded deploy path should be a warning"
        );
    }

    #[test]
    fn deploy_path_in_comments_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/shared/settings/loader.go"),
            "package settings\n\n// See deploy/configs/server.jsonc for details\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "deploy-boundary")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    // ── Port contract leaks (Rule 9 — NEW semantic) ─────────────────────

    #[test]
    fn clean_port_interface_passes() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/ports/configctl.go"),
            r#"package ports

import "context"

type ConfigctlGateway interface {
	CreateDraft(ctx context.Context, name string) (string, error)
	GetConfig(ctx context.Context, id string) (string, error)
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "port-contract-leaks")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn detects_infra_type_in_port_signature() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/ports/messaging.go"),
            r#"package ports

type MessagePublisher interface {
	Publish(subject string, conn *nats.Conn) error
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "port-contract-leaks")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.findings[0].message.contains("nats"));
    }

    #[test]
    fn detects_adapter_pkg_in_port_signature() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/ports/messaging.go"),
            r#"package ports

type MessagePublisher interface {
	Connect() (kafka.Producer, error)
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "port-contract-leaks")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
    }

    // ── Domain type contamination (Rule 10 — NEW semantic) ──────────────

    #[test]
    fn clean_domain_struct_passes() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            r#"package configctl

import "time"

type ConfigSet struct {
	SetID     string
	Name      string
	CreatedAt time.Time
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "domain-type-contamination")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn detects_infra_type_in_domain_struct() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            r#"package configctl

type ConfigSet struct {
	SetID string
	Conn  *nats.Conn
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "domain-type-contamination")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.findings[0].message.contains("nats"));
        assert!(check.findings[0].message.contains("ConfigSet"));
    }

    #[test]
    fn detects_kafka_type_in_domain_struct() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            r#"package configctl

type EventRouter struct {
	Reader *kafka.Reader
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "domain-type-contamination")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.findings[0].message.contains("kafka"));
    }

    // ── Exported function signature leaks (Rule 11 — NEW semantic) ──────

    #[test]
    fn clean_exported_func_passes() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            r#"package configctl

import "context"

func CreateDraft(ctx context.Context, name string) (string, error) {
	return "", nil
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "exported-signature-leaks")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn detects_infra_param_in_exported_func() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            r#"package configctl

func NewService(conn *nats.Conn) *Service {
	return nil
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "exported-signature-leaks")
            .unwrap();
        assert!(
            check.findings.iter().any(|f| f.message.contains("nats")),
            "should detect nats.Conn in exported func param"
        );
    }

    #[test]
    fn unexported_func_with_infra_param_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            r#"package configctl

func newService(conn *nats.Conn) *service {
	return nil
}
"#,
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        let check = report
            .checks
            .iter()
            .find(|c| c.name == "exported-signature-leaks")
            .unwrap();
        assert_eq!(
            check.status,
            CheckStatus::Pass,
            "unexported functions should not be checked"
        );
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn missing_internal_dir_fails_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("go.work"), "go 1.25\n").unwrap();

        let report = analyze(dir.path()).unwrap();
        assert!(!report.passed());
        assert_eq!(report.checks[0].name, "internal-dir");
    }

    #[test]
    fn all_findings_have_why_and_help() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport (\n\t\"quality-service/internal/application/ports\"\n\t\"github.com/nats-io/nats.go\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        for check in &report.checks {
            for finding in &check.findings {
                if finding.severity >= Severity::Warning {
                    assert!(
                        finding.why.is_some(),
                        "finding '{}' in check '{}' should have 'why'",
                        finding.message,
                        check.name,
                    );
                    assert!(
                        finding.help.is_some(),
                        "finding '{}' in check '{}' should have 'help'",
                        finding.message,
                        check.name,
                    );
                }
            }
        }
    }

    #[test]
    fn report_title_is_arch_guard() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());
        let report = analyze(dir.path()).unwrap();
        assert_eq!(report.title, "arch-guard");
    }

    #[test]
    fn all_checks_have_unique_names() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());
        let report = analyze(dir.path()).unwrap();
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "check names must be unique: {names:?}"
        );
    }

    #[test]
    fn reports_multiple_violations() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        // domain → application (layer violation) + nats import (purity)
        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\nimport (\n\t\"quality-service/internal/application/ports\"\n\t\"github.com/nats-io/nats.go\"\n)\n",
        )
        .unwrap();

        // application → adapters (isolation violation)
        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            "package configctl\n\nimport (\n\t\"quality-service/internal/adapters/nats\"\n)\n",
        )
        .unwrap();

        let report = analyze(dir.path()).unwrap();
        assert!(!report.passed());

        let failing_checks: Vec<&str> = report
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .map(|c| c.name.as_str())
            .collect();

        assert!(
            failing_checks.contains(&"layer-dependency-direction"),
            "should catch layer violation"
        );
        assert!(
            failing_checks.contains(&"domain-purity"),
            "should catch domain purity violation"
        );
        assert!(
            failing_checks.contains(&"application-isolation"),
            "should catch application isolation violation"
        );
    }

    // ── Layer extraction ────────────────────────────────────────────────

    #[test]
    fn extracts_layer_from_import() {
        assert_eq!(
            extract_internal_layer("quality-service/internal/adapters/nats"),
            Some("adapters")
        );
        assert_eq!(
            extract_internal_layer("quality-service/internal/domain/configctl"),
            Some("domain")
        );
        assert_eq!(
            extract_internal_layer("quality-service/internal/shared/settings"),
            Some("shared")
        );
    }

    #[test]
    fn returns_none_for_external_import() {
        assert_eq!(extract_internal_layer("github.com/nats-io/nats.go"), None);
        assert_eq!(extract_internal_layer("fmt"), None);
    }

    // ── type_expr_has_infra ─────────────────────────────────────────────

    #[test]
    fn detects_infra_markers_in_type_expr() {
        assert!(type_expr_has_infra("*nats.Conn").is_some());
        assert!(type_expr_has_infra("kafka.Reader").is_some());
        assert!(type_expr_has_infra("*http.Client").is_some());
        assert!(type_expr_has_infra("jetstream.Stream").is_some());
        assert!(type_expr_has_infra("sql.DB").is_some());
    }

    #[test]
    fn clean_type_exprs_pass() {
        assert!(type_expr_has_infra("string").is_none());
        assert!(type_expr_has_infra("context.Context").is_none());
        assert!(type_expr_has_infra("*ConfigSet").is_none());
        assert!(type_expr_has_infra("time.Time").is_none());
        assert!(type_expr_has_infra("[]byte").is_none());
    }

    // ── type_expr_refs_adapter ──────────────────────────────────────────

    #[test]
    fn detects_adapter_pkg_refs() {
        assert!(type_expr_refs_adapter("nats.Publisher"));
        assert!(type_expr_refs_adapter("kafka.Producer"));
        assert!(type_expr_refs_adapter("natsadapter.Gateway"));
    }

    #[test]
    fn non_adapter_pkg_refs_pass() {
        assert!(!type_expr_refs_adapter("context.Context"));
        assert!(!type_expr_refs_adapter("configctl.Config"));
        assert!(!type_expr_refs_adapter("string"));
    }

    // ── file_layer ──────────────────────────────────────────────────────

    #[test]
    fn file_layer_extraction() {
        assert_eq!(
            file_layer("internal/domain/configctl/model.go"),
            Some("domain")
        );
        assert_eq!(
            file_layer("internal/adapters/nats/gateway.go"),
            Some("adapters")
        );
        assert_eq!(
            file_layer("internal/shared/settings/schema.go"),
            Some("shared")
        );
        assert_eq!(file_layer("cmd/server/main.go"), None);
    }

    // ── func_display_name ───────────────────────────────────────────────

    #[test]
    fn func_display_without_receiver() {
        let f = GoFunc {
            name: "CreateDraft".into(),
            receiver: None,
            params: vec![],
            returns: vec![],
            visibility: Visibility::Exported,
            location: codeintel::Location {
                file: "test.go".into(),
                line: 1,
            },
        };
        assert_eq!(func_display_name(&f), "CreateDraft");
    }

    #[test]
    fn func_display_with_pointer_receiver() {
        let f = GoFunc {
            name: "AddVersion".into(),
            receiver: Some(codeintel::Receiver {
                name: "s".into(),
                type_name: "ConfigSet".into(),
                pointer: true,
            }),
            params: vec![],
            returns: vec![],
            visibility: Visibility::Exported,
            location: codeintel::Location {
                file: "test.go".into(),
                line: 1,
            },
        };
        assert_eq!(func_display_name(&f), "(*ConfigSet).AddVersion");
    }

    #[test]
    fn func_display_with_value_receiver() {
        let f = GoFunc {
            name: "Count".into(),
            receiver: Some(codeintel::Receiver {
                name: "s".into(),
                type_name: "ConfigSet".into(),
                pointer: false,
            }),
            params: vec![],
            returns: vec![],
            visibility: Visibility::Exported,
            location: codeintel::Location {
                file: "test.go".into(),
                line: 1,
            },
        };
        assert_eq!(func_display_name(&f), "(ConfigSet).Count");
    }

    // ── Check count ─────────────────────────────────────────────────────

    #[test]
    fn report_has_eleven_checks() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path());

        // Need at least a file per layer for checks not to skip
        fs::write(
            dir.path().join("internal/domain/configctl/model.go"),
            "package configctl\n\ntype Config struct{ Name string }\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("internal/application/configctl/usecase.go"),
            "package configctl\n\nfunc Get() string { return \"\" }\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("internal/application/ports/gw.go"),
            "package ports\n\ntype Gateway interface{ Get() string }\n",
        )
        .unwrap();
        fs::write(dir.path().join("cmd/server/main.go"), "package main\n").unwrap();

        let report = analyze(dir.path()).unwrap();
        assert_eq!(
            report.checks.len(),
            11,
            "arch-guard should produce 11 checks, got: {:?}",
            report.checks.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
    }
}
