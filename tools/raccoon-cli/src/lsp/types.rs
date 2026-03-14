//! Semantic response types for LSP enrichment.
//!
//! These types model the results of LSP queries in a way that is independent of
//! the LSP wire protocol. They are designed to compose with the AST facts from
//! `codeintel::types` and to serialize cleanly to JSON.

use serde::Serialize;

use crate::codeintel::Location;

// ── Fact provenance ─────────────────────────────────────────────────────────

/// Where a piece of information came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    /// Deterministic structural fact from AST parsing.
    Ast,
    /// Semantic fact from gopls LSP.
    Lsp,
    /// Enrichment was attempted but unavailable.
    Unavailable { reason: String },
}

impl FactSource {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        FactSource::Unavailable {
            reason: reason.into(),
        }
    }

    pub fn is_available(&self) -> bool {
        !matches!(self, FactSource::Unavailable { .. })
    }
}

// ── LSP definition ──────────────────────────────────────────────────────────

/// A type-resolved definition location from gopls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspDefinition {
    pub location: Location,
    /// The fully qualified type or symbol path, if resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    pub source: FactSource,
}

// ── LSP reference ───────────────────────────────────────────────────────────

/// A reference location found by gopls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspReference {
    pub location: Location,
    /// Short context snippet (the line containing the reference).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub source: FactSource,
}

// ── Hover / type info ───────────────────────────────────────────────────────

/// Hover information: resolved type signature and documentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoverInfo {
    /// The resolved type signature or declaration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Documentation extracted from comments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub source: FactSource,
}

// ── Enriched symbol ─────────────────────────────────────────────────────────

/// A symbol enriched with both AST and LSP facts.
///
/// This is the primary output of the bridge: it carries the symbol name,
/// AST-observed definitions, LSP-resolved definitions, references from both
/// sources, and hover info. Each fact is tagged with its provenance.
#[derive(Debug, Clone, Serialize)]
pub struct EnrichedSymbol {
    pub symbol: String,
    /// Definitions found by AST (codeintel).
    pub ast_definitions: Vec<LspDefinition>,
    /// Definitions found by gopls (type-resolved).
    pub lsp_definitions: Vec<LspDefinition>,
    /// References found by gopls.
    pub lsp_references: Vec<LspReference>,
    /// Hover/type information from gopls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<HoverInfo>,
    /// Overall enrichment status — did LSP contribute?
    pub lsp_status: LspStatus,
}

/// Summary of LSP enrichment availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LspStatus {
    /// gopls was available and returned results.
    Enriched,
    /// gopls was available but returned no additional info.
    NoResults,
    /// gopls was not available; reason explains why.
    Unavailable { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_source_unavailable_constructor() {
        let src = FactSource::unavailable("gopls not found");
        assert!(!src.is_available());
        assert_eq!(
            src,
            FactSource::Unavailable {
                reason: "gopls not found".into()
            }
        );
    }

    #[test]
    fn fact_source_available() {
        assert!(FactSource::Ast.is_available());
        assert!(FactSource::Lsp.is_available());
    }

    #[test]
    fn enriched_symbol_json_round_trip() {
        let sym = EnrichedSymbol {
            symbol: "ConfigSet".into(),
            ast_definitions: vec![LspDefinition {
                location: Location {
                    file: "config.go".into(),
                    line: 10,
                },
                qualified_name: None,
                source: FactSource::Ast,
            }],
            lsp_definitions: vec![],
            lsp_references: vec![],
            hover: None,
            lsp_status: LspStatus::Unavailable {
                reason: "gopls not installed".into(),
            },
        };
        let json = serde_json::to_string(&sym).unwrap();
        assert!(json.contains("\"symbol\":\"ConfigSet\""));
        assert!(json.contains("\"lsp_status\""));
        // hover should be absent (skip_serializing_if)
        assert!(!json.contains("\"hover\""));
    }

    #[test]
    fn lsp_status_variants_serialize() {
        let enriched = serde_json::to_value(LspStatus::Enriched).unwrap();
        assert_eq!(enriched, serde_json::json!("enriched"));

        let no_results = serde_json::to_value(LspStatus::NoResults).unwrap();
        assert_eq!(no_results, serde_json::json!("no_results"));

        let unavail =
            serde_json::to_value(LspStatus::Unavailable { reason: "x".into() }).unwrap();
        assert!(unavail.is_object());
    }
}
