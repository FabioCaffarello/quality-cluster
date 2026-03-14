//! High-level bridge: merges AST facts from codeintel with LSP enrichment.
//!
//! `GoplsBridge` is the single entry point for callers who want enriched symbol
//! information. It attempts to start `gopls`; if that fails, it degrades
//! gracefully and returns AST-only results tagged with `LspStatus::Unavailable`.

use std::path::Path;

use crate::codeintel::{self, Location, ProjectIndex, TypeKind};

use super::client::GoplsClient;
use super::protocol;
use super::types::{EnrichedSymbol, FactSource, HoverInfo, LspDefinition, LspReference, LspStatus};

/// The gopls bridge: optional semantic enrichment over AST facts.
pub struct GoplsBridge {
    client: Option<GoplsClient>,
    unavailable_reason: Option<String>,
}

impl GoplsBridge {
    /// Create a bridge, attempting to start gopls.
    ///
    /// If gopls is not available, the bridge still works — it just returns
    /// AST-only results with `LspStatus::Unavailable`.
    pub fn new(workspace_root: &Path) -> Self {
        match GoplsClient::start(workspace_root) {
            Ok(client) => Self {
                client: Some(client),
                unavailable_reason: None,
            },
            Err(e) => Self {
                client: None,
                unavailable_reason: Some(e.to_string()),
            },
        }
    }

    /// Create a bridge that is explicitly unavailable (for testing or when
    /// the user opts out of LSP enrichment).
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            client: None,
            unavailable_reason: Some(reason.into()),
        }
    }

    /// Whether gopls is connected and ready.
    pub fn is_available(&self) -> bool {
        self.client.is_some()
    }

    /// Why gopls is unavailable (None if it is available).
    pub fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable_reason.as_deref()
    }

    /// Enrich a symbol with both AST and LSP facts.
    ///
    /// 1. Finds AST definitions via the codeintel index.
    /// 2. If gopls is available, queries definition, references, and hover
    ///    at the first AST definition location.
    /// 3. Tags every fact with its provenance (`Ast` / `Lsp` / `Unavailable`).
    pub fn enrich_symbol(&mut self, project_root: &Path, symbol: &str) -> EnrichedSymbol {
        let index = codeintel::build_index(project_root);
        self.enrich_symbol_with_index(&index, project_root, symbol)
    }

    /// Enrich using an existing index (avoids re-parsing for batch queries).
    pub fn enrich_symbol_with_index(
        &mut self,
        index: &ProjectIndex,
        project_root: &Path,
        symbol: &str,
    ) -> EnrichedSymbol {
        let ast_definitions = collect_ast_definitions(index, symbol);

        if self.client.is_none() {
            return EnrichedSymbol {
                symbol: symbol.to_string(),
                ast_definitions,
                lsp_definitions: vec![],
                lsp_references: vec![],
                hover: None,
                lsp_status: LspStatus::Unavailable {
                    reason: self
                        .unavailable_reason
                        .clone()
                        .unwrap_or_else(|| "gopls not started".into()),
                },
            };
        }

        // Pick the first AST definition as anchor for LSP queries.
        let anchor = ast_definitions.first().map(|d| {
            let abs_path = project_root.join(&d.location.file);
            (
                abs_path.to_string_lossy().to_string(),
                // LSP lines are 0-indexed; our Location is 1-indexed.
                d.location.line.saturating_sub(1) as u32,
            )
        });

        let (lsp_definitions, lsp_references, hover) = match anchor {
            Some((file, line)) => self.query_lsp(&file, line, symbol),
            None => (vec![], vec![], None),
        };

        let lsp_status = if !lsp_definitions.is_empty() || !lsp_references.is_empty() || hover.is_some() {
            LspStatus::Enriched
        } else if self.client.is_some() {
            LspStatus::NoResults
        } else {
            LspStatus::Unavailable {
                reason: "gopls not available".into(),
            }
        };

        EnrichedSymbol {
            symbol: symbol.to_string(),
            ast_definitions,
            lsp_definitions,
            lsp_references,
            hover,
            lsp_status,
        }
    }

    /// Query gopls for definition, references, and hover at a position.
    ///
    /// Each query is independent: if one fails, the others still run.
    fn query_lsp(
        &mut self,
        file: &str,
        line: u32,
        _symbol: &str,
    ) -> (Vec<LspDefinition>, Vec<LspReference>, Option<HoverInfo>) {
        let client = match self.client.as_mut() {
            Some(c) => c,
            None => return (vec![], vec![], None),
        };

        // Character 0 is a safe default — gopls will find the symbol on that line.
        // For better precision, we could scan the line for the symbol name,
        // but character 0 works well for type/func declarations.
        let character = 0;

        // Definition query.
        let lsp_defs = match client.definition(file, line, character) {
            Ok(locs) => locs
                .into_iter()
                .map(|loc| LspDefinition {
                    location: lsp_location_to_ours(&loc),
                    qualified_name: None,
                    source: FactSource::Lsp,
                })
                .collect(),
            Err(e) => {
                eprintln!("[gopls-bridge] definition query failed: {e}");
                vec![]
            }
        };

        // References query.
        let lsp_refs = match client.references(file, line, character, false) {
            Ok(locs) => locs
                .into_iter()
                .map(|loc| LspReference {
                    location: lsp_location_to_ours(&loc),
                    context: None,
                    source: FactSource::Lsp,
                })
                .collect(),
            Err(e) => {
                eprintln!("[gopls-bridge] references query failed: {e}");
                vec![]
            }
        };

        // Hover query.
        let hover = match client.hover(file, line, character) {
            Ok(Some(h)) => {
                let text = h.contents.text().to_string();
                let (sig, doc) = split_hover_text(&text);
                Some(HoverInfo {
                    signature: sig,
                    documentation: doc,
                    source: FactSource::Lsp,
                })
            }
            Ok(None) => None,
            Err(e) => {
                eprintln!("[gopls-bridge] hover query failed: {e}");
                None
            }
        };

        (lsp_defs, lsp_refs, hover)
    }

    /// Gracefully shut down the gopls process.
    pub fn shutdown(mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.shutdown();
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Collect AST-based definitions for a symbol from the codeintel index.
fn collect_ast_definitions(index: &ProjectIndex, symbol: &str) -> Vec<LspDefinition> {
    let mut defs = Vec::new();

    // Types.
    for t in index.find_type(symbol) {
        defs.push(LspDefinition {
            location: t.location.clone(),
            qualified_name: Some(format!(
                "{} ({})",
                t.name,
                match &t.kind {
                    TypeKind::Struct { .. } => "struct",
                    TypeKind::Interface { .. } => "interface",
                    TypeKind::Alias { .. } => "alias",
                }
            )),
            source: FactSource::Ast,
        });
    }

    // Functions / methods.
    for f in index.find_func(symbol) {
        let qualified = if let Some(ref recv) = f.receiver {
            format!("({}).{}", recv.type_name, f.name)
        } else {
            f.name.clone()
        };
        defs.push(LspDefinition {
            location: f.location.clone(),
            qualified_name: Some(format!("{qualified} (func)")),
            source: FactSource::Ast,
        });
    }

    // Constants.
    for file in &index.files {
        for c in &file.constants {
            if c.name == symbol {
                defs.push(LspDefinition {
                    location: c.location.clone(),
                    qualified_name: Some(format!("{} (const)", c.name)),
                    source: FactSource::Ast,
                });
            }
        }
    }

    // Variables.
    for file in &index.files {
        for v in &file.variables {
            if v.name == symbol {
                defs.push(LspDefinition {
                    location: v.location.clone(),
                    qualified_name: Some(format!("{} (var)", v.name)),
                    source: FactSource::Ast,
                });
            }
        }
    }

    defs
}

/// Convert an LSP protocol location to our `Location` type.
fn lsp_location_to_ours(loc: &protocol::LspLocation) -> Location {
    Location {
        file: protocol::uri_to_path(&loc.uri),
        // Convert 0-indexed LSP line to 1-indexed.
        line: (loc.range.start.line + 1) as usize,
    }
}

/// Split gopls hover text into signature and documentation.
///
/// gopls typically returns hover as:
/// ```
/// func Foo(x int) string
///
/// Documentation here...
/// ```
fn split_hover_text(text: &str) -> (Option<String>, Option<String>) {
    // Remove markdown code fences if present.
    let text = text
        .strip_prefix("```go\n")
        .and_then(|t| t.strip_suffix("\n```"))
        .unwrap_or(text);

    let parts: Vec<&str> = text.splitn(2, "\n\n").collect();
    match parts.len() {
        0 => (None, None),
        1 => {
            let s = parts[0].trim();
            if s.is_empty() {
                (None, None)
            } else {
                (Some(s.to_string()), None)
            }
        }
        _ => {
            let sig = parts[0].trim();
            let doc = parts[1].trim();
            (
                if sig.is_empty() { None } else { Some(sig.to_string()) },
                if doc.is_empty() { None } else { Some(doc.to_string()) },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codeintel::types::*;

    fn make_test_index() -> ProjectIndex {
        ProjectIndex {
            files: vec![GoFile {
                path: "internal/domain/configctl/config_set.go".into(),
                package: "configctl".into(),
                imports: vec![],
                types: vec![GoType {
                    name: "ConfigSet".into(),
                    kind: TypeKind::Struct { fields: vec![] },
                    visibility: Visibility::Exported,
                    location: Location {
                        file: "internal/domain/configctl/config_set.go".into(),
                        line: 10,
                    },
                }],
                functions: vec![GoFunc {
                    name: "NewConfigSet".into(),
                    receiver: None,
                    params: vec![],
                    returns: vec![],
                    visibility: Visibility::Exported,
                    location: Location {
                        file: "internal/domain/configctl/config_set.go".into(),
                        line: 25,
                    },
                }],
                constants: vec![GoConst {
                    name: "MaxVersions".into(),
                    type_hint: Some("int".into()),
                    value: Some("100".into()),
                    visibility: Visibility::Exported,
                    location: Location {
                        file: "internal/domain/configctl/config_set.go".into(),
                        line: 5,
                    },
                }],
                variables: vec![],
                is_test: false,
                line_count: 50,
            }],
            packages: vec![],
            stats: IndexStats {
                total_files: 1,
                total_packages: 1,
                total_types: 1,
                total_functions: 1,
                total_constants: 1,
                total_imports: 0,
                total_lines: 50,
                structs: 1,
                interfaces: 0,
                type_aliases: 0,
                exported_types: 1,
                exported_functions: 1,
                test_files: 0,
            },
        }
    }

    #[test]
    fn collect_ast_definitions_finds_types() {
        let index = make_test_index();
        let defs = collect_ast_definitions(&index, "ConfigSet");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].location.line, 10);
        assert!(defs[0].qualified_name.as_ref().unwrap().contains("struct"));
        assert_eq!(defs[0].source, FactSource::Ast);
    }

    #[test]
    fn collect_ast_definitions_finds_functions() {
        let index = make_test_index();
        let defs = collect_ast_definitions(&index, "NewConfigSet");
        assert_eq!(defs.len(), 1);
        assert!(defs[0].qualified_name.as_ref().unwrap().contains("func"));
    }

    #[test]
    fn collect_ast_definitions_finds_constants() {
        let index = make_test_index();
        let defs = collect_ast_definitions(&index, "MaxVersions");
        assert_eq!(defs.len(), 1);
        assert!(defs[0].qualified_name.as_ref().unwrap().contains("const"));
    }

    #[test]
    fn collect_ast_definitions_returns_empty_for_unknown() {
        let index = make_test_index();
        let defs = collect_ast_definitions(&index, "DoesNotExist");
        assert!(defs.is_empty());
    }

    #[test]
    fn unavailable_bridge_returns_ast_only() {
        let index = make_test_index();
        let mut bridge = GoplsBridge::unavailable("test: no gopls");
        assert!(!bridge.is_available());
        assert_eq!(bridge.unavailable_reason(), Some("test: no gopls"));

        let enriched = bridge.enrich_symbol_with_index(
            &index,
            Path::new("."),
            "ConfigSet",
        );
        assert_eq!(enriched.symbol, "ConfigSet");
        assert_eq!(enriched.ast_definitions.len(), 1);
        assert!(enriched.lsp_definitions.is_empty());
        assert!(enriched.lsp_references.is_empty());
        assert!(enriched.hover.is_none());
        assert!(matches!(enriched.lsp_status, LspStatus::Unavailable { .. }));
    }

    #[test]
    fn split_hover_text_signature_only() {
        let (sig, doc) = split_hover_text("func Foo(x int) string");
        assert_eq!(sig.unwrap(), "func Foo(x int) string");
        assert!(doc.is_none());
    }

    #[test]
    fn split_hover_text_with_doc() {
        let text = "func Foo(x int) string\n\nFoo does something useful.";
        let (sig, doc) = split_hover_text(text);
        assert_eq!(sig.unwrap(), "func Foo(x int) string");
        assert_eq!(doc.unwrap(), "Foo does something useful.");
    }

    #[test]
    fn split_hover_text_markdown_fences() {
        let text = "```go\nfunc Bar() error\n```";
        let (sig, doc) = split_hover_text(text);
        assert_eq!(sig.unwrap(), "func Bar() error");
        assert!(doc.is_none());
    }

    #[test]
    fn split_hover_text_empty() {
        let (sig, doc) = split_hover_text("");
        assert!(sig.is_none());
        assert!(doc.is_none());
    }

    #[test]
    fn lsp_location_conversion() {
        let lsp_loc = protocol::LspLocation {
            uri: "file:///src/main.go".into(),
            range: protocol::Range {
                start: protocol::Position { line: 9, character: 0 },
                end: protocol::Position { line: 9, character: 10 },
            },
        };
        let loc = lsp_location_to_ours(&lsp_loc);
        assert_eq!(loc.file, "/src/main.go");
        assert_eq!(loc.line, 10); // 0-indexed → 1-indexed
    }

    #[test]
    fn enriched_symbol_serializes_cleanly() {
        let index = make_test_index();
        let mut bridge = GoplsBridge::unavailable("test");
        let enriched = bridge.enrich_symbol_with_index(&index, Path::new("."), "ConfigSet");
        let json = serde_json::to_string_pretty(&enriched).unwrap();
        assert!(json.contains("\"symbol\": \"ConfigSet\""));
        assert!(json.contains("\"ast_definitions\""));
        assert!(json.contains("\"lsp_status\""));
    }
}
