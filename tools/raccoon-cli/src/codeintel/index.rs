//! Cross-file structural index.
//!
//! Aggregates parsed `GoFile`s into a queryable `ProjectIndex` with packages,
//! symbol lookups, and summary statistics.

use std::collections::BTreeMap;
use std::path::Path;

use super::parser;
use super::types::*;
use super::walker;

/// Build a complete structural index of all Go files under `root`.
pub fn build_index(root: &Path) -> ProjectIndex {
    let files_on_disk = walker::walk_go_files(root);

    let mut go_files: Vec<GoFile> = Vec::with_capacity(files_on_disk.len());

    for path in &files_on_disk {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let go_file = parser::parse_file(&rel_path, &content);
        go_files.push(go_file);
    }

    let packages = aggregate_packages(&go_files);
    let stats = compute_stats(&go_files, &packages);

    ProjectIndex {
        files: go_files,
        packages,
        stats,
    }
}

/// Aggregate parsed files into packages (one per directory + package name).
fn aggregate_packages(files: &[GoFile]) -> Vec<GoPackage> {
    // Group files by (directory, package_name)
    let mut groups: BTreeMap<(String, String), Vec<&GoFile>> = BTreeMap::new();

    for file in files {
        let dir = file_dir(&file.path);
        groups
            .entry((dir, file.package.clone()))
            .or_default()
            .push(file);
    }

    groups
        .into_iter()
        .map(|((dir, name), files)| {
            let file_paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();

            // Aggregate imports (deduplicated by path)
            let mut import_set: BTreeMap<String, GoImport> = BTreeMap::new();
            for file in &files {
                for imp in &file.imports {
                    import_set
                        .entry(imp.path.clone())
                        .or_insert_with(|| imp.clone());
                }
            }

            // Aggregate types
            let types: Vec<GoType> = files
                .iter()
                .flat_map(|f| f.types.iter().cloned())
                .collect();

            // Aggregate functions (not from test files)
            let functions: Vec<GoFunc> = files
                .iter()
                .filter(|f| !f.is_test)
                .flat_map(|f| f.functions.iter().cloned())
                .collect();

            // Aggregate constants
            let constants: Vec<GoConst> = files
                .iter()
                .flat_map(|f| f.constants.iter().cloned())
                .collect();

            GoPackage {
                name,
                dir,
                files: file_paths,
                imports: import_set.into_values().collect(),
                types,
                functions,
                constants,
            }
        })
        .collect()
}

/// Extract the directory portion of a file path.
fn file_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

/// Compute summary statistics for the index.
fn compute_stats(files: &[GoFile], packages: &[GoPackage]) -> IndexStats {
    let mut stats = IndexStats {
        total_files: files.len(),
        total_packages: packages.len(),
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
    };

    for file in files {
        stats.total_types += file.types.len();
        stats.total_functions += file.functions.len();
        stats.total_constants += file.constants.len();
        stats.total_imports += file.imports.len();
        stats.total_lines += file.line_count;

        if file.is_test {
            stats.test_files += 1;
        }

        for t in &file.types {
            match &t.kind {
                TypeKind::Struct { .. } => stats.structs += 1,
                TypeKind::Interface { .. } => stats.interfaces += 1,
                TypeKind::Alias { .. } => stats.type_aliases += 1,
            }
            if t.visibility == Visibility::Exported {
                stats.exported_types += 1;
            }
        }

        for f in &file.functions {
            if f.visibility == Visibility::Exported {
                stats.exported_functions += 1;
            }
        }
    }

    stats
}

// ── Query helpers ───────────────────────────────────────────────────────────

impl ProjectIndex {
    /// Find all types with a given name across all packages.
    pub fn find_type(&self, name: &str) -> Vec<&GoType> {
        self.files
            .iter()
            .flat_map(|f| f.types.iter())
            .filter(|t| t.name == name)
            .collect()
    }

    /// Find all functions/methods with a given name.
    pub fn find_func(&self, name: &str) -> Vec<&GoFunc> {
        self.files
            .iter()
            .flat_map(|f| f.functions.iter())
            .filter(|f| f.name == name)
            .collect()
    }

    /// Find all methods on a given type.
    pub fn methods_of(&self, type_name: &str) -> Vec<&GoFunc> {
        self.files
            .iter()
            .flat_map(|f| f.functions.iter())
            .filter(|f| {
                f.receiver
                    .as_ref()
                    .map(|r| r.type_name == type_name)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Find a package by directory path.
    pub fn find_package(&self, dir: &str) -> Option<&GoPackage> {
        self.packages.iter().find(|p| p.dir == dir)
    }

    /// List all interfaces in the index.
    pub fn all_interfaces(&self) -> Vec<&GoType> {
        self.files
            .iter()
            .flat_map(|f| f.types.iter())
            .filter(|t| matches!(t.kind, TypeKind::Interface { .. }))
            .collect()
    }

    /// List all structs in the index.
    pub fn all_structs(&self) -> Vec<&GoType> {
        self.files
            .iter()
            .flat_map(|f| f.types.iter())
            .filter(|t| matches!(t.kind, TypeKind::Struct { .. }))
            .collect()
    }

    /// Get all files in a given directory.
    pub fn files_in_dir(&self, dir: &str) -> Vec<&GoFile> {
        self.files.iter().filter(|f| file_dir(&f.path) == dir).collect()
    }

    /// List all import paths used across the project, with counts.
    pub fn import_frequency(&self) -> Vec<(String, usize)> {
        let mut freq: BTreeMap<String, usize> = BTreeMap::new();
        for file in &self.files {
            for imp in &file.imports {
                *freq.entry(imp.path.clone()).or_default() += 1;
            }
        }
        let mut sorted: Vec<_> = freq.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted
    }

    /// Find all constants of a given type hint.
    pub fn constants_of_type(&self, type_name: &str) -> Vec<&GoConst> {
        self.files
            .iter()
            .flat_map(|f| f.constants.iter())
            .filter(|c| c.type_hint.as_deref() == Some(type_name))
            .collect()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_project(tmp: &TempDir) -> &Path {
        let root = tmp.path();

        fs::create_dir_all(root.join("internal/domain/configctl")).unwrap();
        fs::create_dir_all(root.join("internal/application/ports")).unwrap();

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

        root
    }

    #[test]
    fn builds_index_from_directory() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);

        assert_eq!(index.stats.total_files, 4);
        assert_eq!(index.stats.test_files, 1);
        assert!(index.stats.total_types > 0);
        assert!(index.stats.total_functions > 0);
    }

    #[test]
    fn aggregates_packages() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);

        assert_eq!(index.packages.len(), 2); // configctl + ports
    }

    #[test]
    fn package_has_correct_files() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let pkg = index
            .packages
            .iter()
            .find(|p| p.name == "configctl")
            .unwrap();

        assert_eq!(pkg.files.len(), 3); // config.go, lifecycle.go, config_test.go
    }

    #[test]
    fn find_type_by_name() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let types = index.find_type("ConfigSet");
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0].kind, TypeKind::Struct { .. }));
    }

    #[test]
    fn find_interface() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let ifaces = index.all_interfaces();
        assert_eq!(ifaces.len(), 1);
        assert_eq!(ifaces[0].name, "ConfigctlGateway");
    }

    #[test]
    fn find_methods_of_type() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let methods = index.methods_of("ConfigSet");
        assert_eq!(methods.len(), 2); // AddVersion, VersionCount
    }

    #[test]
    fn find_func_by_name() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let funcs = index.find_func("NewConfigSet");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].receiver.is_none());
    }

    #[test]
    fn finds_constants_of_type() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let consts = index.constants_of_type("VersionLifecycle");
        assert_eq!(consts.len(), 4);
    }

    #[test]
    fn stats_are_correct() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);

        assert!(index.stats.structs >= 2); // ConfigSet, ConfigVersion
        assert!(index.stats.interfaces >= 1); // ConfigctlGateway
        assert!(index.stats.type_aliases >= 1); // VersionLifecycle
        assert!(index.stats.total_constants >= 4);
    }

    #[test]
    fn import_frequency_works() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let freq = index.import_frequency();

        // "time" is imported once, "context" once, "testing" once
        assert!(!freq.is_empty());
    }

    #[test]
    fn empty_directory_produces_empty_index() {
        let tmp = TempDir::new().unwrap();
        let index = build_index(tmp.path());

        assert_eq!(index.files.len(), 0);
        assert_eq!(index.packages.len(), 0);
        assert_eq!(index.stats.total_files, 0);
    }

    #[test]
    fn find_package_by_dir() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let pkg = index.find_package("internal/application/ports");
        assert!(pkg.is_some());
        assert_eq!(pkg.unwrap().name, "ports");
    }

    #[test]
    fn files_in_dir_returns_correct_files() {
        let tmp = TempDir::new().unwrap();
        let root = create_project(&tmp);

        let index = build_index(root);
        let files = index.files_in_dir("internal/domain/configctl");
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn file_dir_extracts_directory() {
        assert_eq!(file_dir("internal/domain/config.go"), "internal/domain");
        assert_eq!(file_dir("main.go"), ".");
        assert_eq!(file_dir("a/b/c/d.go"), "a/b/c");
    }
}
