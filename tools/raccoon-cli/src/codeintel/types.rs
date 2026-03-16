//! Canonical data structures for Go structural indexing.
//!
//! These types represent deterministic, observable facts extracted from Go source
//! files. They deliberately avoid semantic inference (type resolution, constant
//! evaluation, cross-package binding) — those belong in a future enrichment layer.

use serde::Serialize;

// ── Location ────────────────────────────────────────────────────────────────

/// A source location: file path + line number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub file: String,
    pub line: usize,
}

// ── Visibility ──────────────────────────────────────────────────────────────

/// Go visibility: exported (uppercase first letter) or unexported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Exported,
    Unexported,
}

impl Visibility {
    pub fn from_name(name: &str) -> Self {
        if name.starts_with(|c: char| c.is_uppercase()) {
            Visibility::Exported
        } else {
            Visibility::Unexported
        }
    }
}

// ── Imports ─────────────────────────────────────────────────────────────────

/// Classification of a Go import path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportKind {
    /// Standard library (no dots in first segment)
    Stdlib,
    /// Project-internal import (contains the module path)
    Internal,
    /// Third-party module
    External,
}

/// A single Go import declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoImport {
    pub path: String,
    pub alias: Option<String>,
    pub kind: ImportKind,
    pub location: Location,
}

// ── Struct fields ───────────────────────────────────────────────────────────

/// A field within a Go struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructField {
    pub name: String,
    pub type_expr: String,
    pub tag: Option<String>,
    /// True when the field is an embedded type (no explicit name).
    pub embedded: bool,
    pub visibility: Visibility,
    pub location: Location,
}

// ── Interface methods ───────────────────────────────────────────────────────

/// A method signature within a Go interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InterfaceMethod {
    pub name: String,
    pub signature: String,
    pub location: Location,
}

/// An embedded interface within another interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InterfaceEmbed {
    pub type_name: String,
    pub location: Location,
}

// ── Type definitions ────────────────────────────────────────────────────────

/// The kind of a Go type definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeKind {
    /// `type Foo struct { ... }`
    Struct { fields: Vec<StructField> },
    /// `type Foo interface { ... }`
    Interface {
        methods: Vec<InterfaceMethod>,
        embeds: Vec<InterfaceEmbed>,
    },
    /// `type Foo = Bar` or `type Foo Bar`
    Alias { underlying: String },
}

/// A Go type definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoType {
    pub name: String,
    pub kind: TypeKind,
    pub visibility: Visibility,
    pub location: Location,
}

// ── Functions & methods ─────────────────────────────────────────────────────

/// A Go function or method parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Param {
    pub name: String,
    pub type_expr: String,
}

/// A Go function or method receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Receiver {
    pub name: String,
    pub type_name: String,
    pub pointer: bool,
}

/// A Go function or method declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoFunc {
    pub name: String,
    pub receiver: Option<Receiver>,
    pub params: Vec<Param>,
    pub returns: Vec<Param>,
    pub visibility: Visibility,
    pub location: Location,
}

// ── Constants & variables ───────────────────────────────────────────────────

/// A Go constant declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoConst {
    pub name: String,
    pub type_hint: Option<String>,
    pub value: Option<String>,
    pub visibility: Visibility,
    pub location: Location,
}

/// A Go package-level variable declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoVar {
    pub name: String,
    pub type_hint: Option<String>,
    pub value: Option<String>,
    pub visibility: Visibility,
    pub location: Location,
}

// ── File ────────────────────────────────────────────────────────────────────

/// A parsed Go source file with all extracted structural facts.
#[derive(Debug, Clone, Serialize)]
pub struct GoFile {
    pub path: String,
    pub package: String,
    pub imports: Vec<GoImport>,
    pub types: Vec<GoType>,
    pub functions: Vec<GoFunc>,
    pub constants: Vec<GoConst>,
    pub variables: Vec<GoVar>,
    pub is_test: bool,
    pub line_count: usize,
}

// ── Package ─────────────────────────────────────────────────────────────────

/// Aggregated view of a Go package (directory with same package declaration).
#[derive(Debug, Clone, Serialize)]
pub struct GoPackage {
    pub name: String,
    pub dir: String,
    pub files: Vec<String>,
    pub imports: Vec<GoImport>,
    pub types: Vec<GoType>,
    pub functions: Vec<GoFunc>,
    pub constants: Vec<GoConst>,
}

// ── Index ───────────────────────────────────────────────────────────────────

/// The complete structural index of a Go project.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectIndex {
    pub files: Vec<GoFile>,
    pub packages: Vec<GoPackage>,
    pub stats: IndexStats,
}

/// Summary statistics for the index.
#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
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
}
