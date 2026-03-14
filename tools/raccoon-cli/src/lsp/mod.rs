//! LSP Bridge — optional semantic enrichment via `gopls`.
//!
//! This module provides a bridge to the Go language server (`gopls`) to enrich
//! the deterministic AST facts from `codeintel` with semantic information:
//! type-resolved definitions, cross-package references, hover/type info.
//!
//! ## Architecture
//!
//! ```text
//! lsp/
//! ├── types.rs     — Semantic response types (LspFact, EnrichedSymbol, etc.)
//! ├── protocol.rs  — LSP JSON-RPC primitives (request/response encoding)
//! ├── client.rs    — gopls process lifecycle and low-level communication
//! └── bridge.rs    — High-level API: merges AST facts with LSP enrichment
//! ```
//!
//! ## Design principles
//!
//! 1. **Optional enrichment** — the CLI works without `gopls`. LSP adds semantic
//!    depth but never gates basic functionality.
//!
//! 2. **Graceful degradation** — if `gopls` is absent, workspace is invalid, or
//!    a query times out, the bridge returns `LspFact::Unavailable` with a reason.
//!    Callers always get a response; they never get a panic or hard error.
//!
//! 3. **Fact provenance** — every piece of information is tagged with its source:
//!    `Ast` (from codeintel), `Lsp` (from gopls), or `Unavailable` (enrichment
//!    failed). Consumers can decide how much to trust each layer.
//!
//! 4. **Encapsulated lifecycle** — `GoplsClient` owns the child process. It
//!    starts `gopls` on demand, initializes the LSP handshake, and shuts down
//!    cleanly on drop.
//!
//! 5. **No runtime coupling** — this module lives in `tools/raccoon-cli` and
//!    communicates with `gopls` over stdio JSON-RPC. It does not import or
//!    link against Go code.

pub mod bridge;
pub mod client;
pub mod protocol;
pub mod types;

pub use bridge::GoplsBridge;
pub use types::EnrichedSymbol;
