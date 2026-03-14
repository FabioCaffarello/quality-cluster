//! Directory walker for Go source files.
//!
//! Recursively walks directories, collecting `.go` files while skipping
//! vendor, testdata, hidden directories, and other non-interesting paths.

use std::path::{Path, PathBuf};

/// Directories to skip during traversal.
const SKIP_DIRS: &[&str] = &["vendor", "testdata", "node_modules", ".git"];

/// Recursively collect all `.go` file paths under `root`.
///
/// Skips vendor, testdata, hidden directories, and non-Go files.
pub fn walk_go_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_recursive(root, &mut files);
    files.sort();
    files
}

fn walk_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("go") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn collects_go_files_recursively() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create directory structure
        fs::create_dir_all(root.join("pkg/sub")).unwrap();
        fs::write(root.join("main.go"), "package main").unwrap();
        fs::write(root.join("pkg/handler.go"), "package pkg").unwrap();
        fs::write(root.join("pkg/sub/util.go"), "package sub").unwrap();

        let files = walk_go_files(root);
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn skips_vendor_directory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("vendor/lib")).unwrap();
        fs::write(root.join("main.go"), "package main").unwrap();
        fs::write(root.join("vendor/lib/dep.go"), "package lib").unwrap();

        let files = walk_go_files(root);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn skips_hidden_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("main.go"), "package main").unwrap();
        fs::write(root.join(".hidden/secret.go"), "package hidden").unwrap();

        let files = walk_go_files(root);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn ignores_non_go_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("main.go"), "package main").unwrap();
        fs::write(root.join("readme.md"), "# readme").unwrap();
        fs::write(root.join("config.json"), "{}").unwrap();

        let files = walk_go_files(root);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn returns_empty_for_nonexistent_dir() {
        let files = walk_go_files(Path::new("/nonexistent/dir"));
        assert!(files.is_empty());
    }

    #[test]
    fn results_are_sorted() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("z.go"), "package main").unwrap();
        fs::write(root.join("a.go"), "package main").unwrap();
        fs::write(root.join("m.go"), "package main").unwrap();

        let files = walk_go_files(root);
        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.go", "m.go", "z.go"]);
    }
}
