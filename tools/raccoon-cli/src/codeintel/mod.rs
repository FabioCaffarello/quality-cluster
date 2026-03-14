//! Code Intelligence — structural indexing for Go source.
//!
//! This module provides a deterministic, AST-like representation of Go source
//! files without requiring the Go compiler or any external tooling. It parses
//! packages, imports, types (struct/interface/alias), functions, methods,
//! constants, and variables into canonical data structures that can be queried
//! by downstream commands (impact-map, arch-guard, symbol-trace, tdd, etc.).
//!
//! ## Architecture
//!
//! ```text
//! codeintel/
//! ├── types.rs    — Canonical data structures (GoFile, GoPackage, GoType, ...)
//! ├── walker.rs   — Directory walker (collects .go files, skips vendor/hidden)
//! ├── parser.rs   — Single-file parser (source text → GoFile)
//! └── index.rs    — Cross-file indexer (GoFile[] → ProjectIndex with queries)
//! ```
//!
//! ## Design principles
//!
//! 1. **Observable facts only** — every indexed item maps to a concrete source
//!    location. No type inference, no constant evaluation, no semantic binding.
//!
//! 2. **Deterministic** — same source files always produce the same index.
//!    No network calls, no caching side effects.
//!
//! 3. **Zero external dependencies** — uses only the Rust stdlib for parsing.
//!    Go's regular syntax makes line-based parsing reliable for declarations.
//!
//! 4. **Extensible** — the `ProjectIndex` query API is designed to grow.
//!    Future commands add query methods without changing the core structures.
//!
//! ## Phase 1 scope
//!
//! - Package declarations and directory grouping
//! - Import classification (stdlib / internal / external) with aliases
//! - Struct definitions with fields, tags, embedded types
//! - Interface definitions with methods and embeds
//! - Type aliases / definitions
//! - Function and method signatures with receivers
//! - Constant and variable declarations
//! - File metadata (test file detection, line counts)
//! - Package-level aggregation and deduplication
//! - Summary statistics
//!
//! ## What is deliberately out of scope (future phases)
//!
//! - Type resolution across packages
//! - Interface satisfaction checking
//! - Call graph construction
//! - Constant/expression evaluation
//! - Function body analysis
//! - LSP integration / incremental updates

pub mod index;
pub mod parser;
pub mod types;
pub mod walker;

// Re-export the main entry point and key types for ergonomic use.
pub use index::build_index;
pub use types::{
    GoConst, GoFile, GoFunc, GoImport, GoPackage, GoType, GoVar, ImportKind, IndexStats,
    InterfaceEmbed, InterfaceMethod, Location, Param, ProjectIndex, Receiver, StructField,
    TypeKind, Visibility,
};
