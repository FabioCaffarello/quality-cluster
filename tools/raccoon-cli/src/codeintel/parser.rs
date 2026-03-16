//! Go source file parser.
//!
//! Extracts structural facts from a single Go file using line-based parsing.
//! Go's regular syntax makes this approach reliable for the declarations we
//! care about: package, imports, types (struct/interface/alias), functions,
//! methods, constants, and variables.
//!
//! ## Deliberate limits
//!
//! - No type resolution or constant evaluation.
//! - No cross-file analysis (that's the index layer's job).
//! - Nested/anonymous struct fields are captured as type expressions, not recursed.
//! - Function bodies are skipped (we only parse signatures).

use super::types::*;

/// Module path prefix for classifying imports as internal.
/// Empty string means "auto-detect from go.mod or go.work".
const DEFAULT_MODULE_PREFIX: &str = "quality-service/internal/";

/// Parse a single Go source file into a `GoFile`.
pub fn parse_file(path: &str, source: &str) -> GoFile {
    let lines: Vec<&str> = source.lines().collect();
    let is_test = path.ends_with("_test.go");

    let package = extract_package(&lines);
    let imports = extract_imports(&lines, path);
    let types = extract_types(&lines, path);
    let functions = extract_functions(&lines, path);
    let constants = extract_constants(&lines, path);
    let variables = extract_variables(&lines, path);

    GoFile {
        path: path.to_string(),
        package,
        imports,
        types,
        functions,
        constants,
        variables,
        is_test,
        line_count: lines.len(),
    }
}

// ── Package ─────────────────────────────────────────────────────────────────

fn extract_package(lines: &[&str]) -> String {
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("package ") {
            let rest = trimmed.strip_prefix("package ").unwrap().trim();
            // Remove trailing comments
            let name = rest.split("//").next().unwrap_or(rest).trim();
            return name.to_string();
        }
    }
    String::new()
}

// ── Imports ─────────────────────────────────────────────────────────────────

fn extract_imports(lines: &[&str], file: &str) -> Vec<GoImport> {
    let mut imports = Vec::new();
    let mut in_block = false;
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Single-line import: `import "path"` or `import alias "path"`
        if trimmed.starts_with("import \"") || trimmed.starts_with("import\t\"") {
            if let Some(path) = extract_quoted(trimmed) {
                imports.push(GoImport {
                    path: path.clone(),
                    alias: None,
                    kind: classify_import(&path),
                    location: loc(file, i + 1),
                });
            }
            i += 1;
            continue;
        }

        // Start of import block
        if trimmed == "import (" || trimmed.starts_with("import (") {
            in_block = true;
            i += 1;
            continue;
        }

        if in_block {
            if trimmed == ")" {
                in_block = false;
                i += 1;
                continue;
            }

            // Skip blank lines and comments inside import block
            if trimmed.is_empty() || trimmed.starts_with("//") {
                i += 1;
                continue;
            }

            // Parse import line: optional alias + quoted path
            let (alias, path) = parse_import_line(trimmed);
            if let Some(path) = path {
                imports.push(GoImport {
                    path: path.clone(),
                    alias,
                    kind: classify_import(&path),
                    location: loc(file, i + 1),
                });
            }
        }

        i += 1;
    }

    imports
}

/// Parse an import line, returning (optional alias, optional path).
fn parse_import_line(line: &str) -> (Option<String>, Option<String>) {
    let trimmed = line.trim();

    // Remove trailing inline comment
    let content = trimmed.split("//").next().unwrap_or(trimmed).trim();

    // Find the quoted path
    if let Some(start) = content.find('"') {
        if let Some(end) = content[start + 1..].find('"') {
            let path = content[start + 1..start + 1 + end].to_string();

            // Check for alias before the quote
            let before = content[..start].trim();
            let alias = if before.is_empty() || before == "." || before == "_" {
                if before == "." || before == "_" {
                    Some(before.to_string())
                } else {
                    None
                }
            } else {
                Some(before.to_string())
            };

            return (alias, Some(path));
        }
    }

    (None, None)
}

fn classify_import(path: &str) -> ImportKind {
    // Internal: contains our module path
    if path.contains(DEFAULT_MODULE_PREFIX) || path.contains("/internal/") {
        return ImportKind::Internal;
    }

    // Stdlib: first segment has no dots (e.g. "fmt", "context", "net/http")
    let first_segment = path.split('/').next().unwrap_or(path);
    if !first_segment.contains('.') {
        return ImportKind::Stdlib;
    }

    ImportKind::External
}

// ── Types ───────────────────────────────────────────────────────────────────

fn extract_types(lines: &[&str], file: &str) -> Vec<GoType> {
    let mut types = Vec::new();
    let mut i = 0;
    let mut in_type_block = false;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Grouped type block: `type (`
        if trimmed == "type (" {
            in_type_block = true;
            i += 1;
            continue;
        }

        if in_type_block {
            if trimmed == ")" {
                in_type_block = false;
                i += 1;
                continue;
            }

            // Inside type block, each type starts with its name
            if let Some(t) = try_parse_type_decl(lines, &mut i, file, true) {
                types.push(t);
                continue;
            }
            i += 1;
            continue;
        }

        // Top-level type: `type Name ...`
        if trimmed.starts_with("type ") {
            if let Some(t) = try_parse_type_decl(lines, &mut i, file, false) {
                types.push(t);
                continue;
            }
        }

        i += 1;
    }

    types
}

/// Try to parse a type declaration starting at `lines[*i]`.
/// Advances `*i` past the declaration on success.
fn try_parse_type_decl(
    lines: &[&str],
    i: &mut usize,
    file: &str,
    in_block: bool,
) -> Option<GoType> {
    let trimmed = lines[*i].trim();
    let line_num = *i + 1;

    // Determine the declaration text
    let decl = if in_block {
        trimmed.to_string()
    } else {
        // Remove "type " prefix
        trimmed.strip_prefix("type ")?.to_string()
    };

    // Skip blank lines, comments
    if decl.is_empty() || decl.starts_with("//") {
        return None;
    }

    // Split into name and rest
    let parts: Vec<&str> = decl.splitn(2, char::is_whitespace).collect();
    if parts.len() < 2 {
        return None;
    }

    let name = parts[0].to_string();
    let rest = parts[1].trim();

    // Struct
    if rest == "struct {" || rest.starts_with("struct {") {
        let fields = parse_struct_body(lines, i, file);
        return Some(GoType {
            visibility: Visibility::from_name(&name),
            name,
            kind: TypeKind::Struct { fields },
            location: loc(file, line_num),
        });
    }
    // Empty struct on same line
    if rest == "struct{}" || rest == "struct{ }" {
        *i += 1;
        return Some(GoType {
            visibility: Visibility::from_name(&name),
            name,
            kind: TypeKind::Struct { fields: vec![] },
            location: loc(file, line_num),
        });
    }

    // Interface
    if rest == "interface {" || rest.starts_with("interface {") {
        let (methods, embeds) = parse_interface_body(lines, i, file);
        return Some(GoType {
            visibility: Visibility::from_name(&name),
            name,
            kind: TypeKind::Interface { methods, embeds },
            location: loc(file, line_num),
        });
    }
    // Empty interface
    if rest == "interface{}" || rest == "interface{ }" {
        *i += 1;
        return Some(GoType {
            visibility: Visibility::from_name(&name),
            name,
            kind: TypeKind::Interface {
                methods: vec![],
                embeds: vec![],
            },
            location: loc(file, line_num),
        });
    }

    // Type alias: `type Foo = Bar` or type definition: `type Foo Bar`
    let underlying = if let Some(alias_type) = rest.strip_prefix("= ") {
        alias_type.trim().to_string()
    } else {
        rest.to_string()
    };

    *i += 1;
    Some(GoType {
        visibility: Visibility::from_name(&name),
        name,
        kind: TypeKind::Alias { underlying },
        location: loc(file, line_num),
    })
}

/// Parse struct body between `{` and `}`, extracting fields.
/// Advances `*i` past the closing `}`.
fn parse_struct_body(lines: &[&str], i: &mut usize, file: &str) -> Vec<StructField> {
    let mut fields = Vec::new();
    let mut depth = 0;

    // Count opening braces on the current line
    let first_line = lines[*i];
    depth += first_line.matches('{').count();
    depth -= first_line.matches('}').count();

    *i += 1;

    while *i < lines.len() && depth > 0 {
        let trimmed = lines[*i].trim();

        // Track brace depth for nested structs
        depth += trimmed.matches('{').count();
        depth -= trimmed.matches('}').count();

        if depth <= 0 {
            *i += 1;
            break;
        }

        // Only parse fields at depth 1 (top level of this struct)
        if depth == 1 {
            if let Some(field) = try_parse_struct_field(trimmed, file, *i + 1) {
                fields.push(field);
            }
        }

        *i += 1;
    }

    fields
}

/// Try to parse a struct field from a trimmed line.
fn try_parse_struct_field(line: &str, file: &str, line_num: usize) -> Option<StructField> {
    let trimmed = line.trim();

    // Skip blank lines, comments, closing brace
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed == "}" {
        return None;
    }

    // Extract struct tag if present
    let (content, tag) = split_struct_tag(trimmed);

    // Remove inline comments from content
    let content = content.split("//").next().unwrap_or(content).trim();

    if content.is_empty() {
        return None;
    }

    // Split into tokens
    let tokens: Vec<&str> = content.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    // Embedded field: single token that's a type name (possibly with *)
    if tokens.len() == 1 {
        let type_expr = tokens[0].to_string();
        let clean_name = type_expr.trim_start_matches('*');
        // Get the last segment for package-qualified types
        let base_name = clean_name.rsplit('.').next().unwrap_or(clean_name);
        let name = base_name.to_string();
        let visibility = Visibility::from_name(base_name);
        return Some(StructField {
            name,
            type_expr,
            tag: tag.map(|t| t.to_string()),
            embedded: true,
            visibility,
            location: loc(file, line_num),
        });
    }

    // Regular field: name type [tag]
    let name = tokens[0].to_string();
    let type_expr = tokens[1..].join(" ");

    Some(StructField {
        visibility: Visibility::from_name(&name),
        name,
        type_expr,
        tag: tag.map(|t| t.to_string()),
        embedded: false,
        location: loc(file, line_num),
    })
}

/// Split a line into (content before tag, optional tag).
/// Tags are backtick-delimited: `json:"foo"`
fn split_struct_tag(line: &str) -> (&str, Option<&str>) {
    if let Some(start) = line.find('`') {
        if let Some(end) = line[start + 1..].find('`') {
            let tag = &line[start..start + end + 2];
            let content = line[..start].trim();
            return (content, Some(tag));
        }
    }
    (line, None)
}

/// Parse interface body between `{` and `}`.
fn parse_interface_body(
    lines: &[&str],
    i: &mut usize,
    file: &str,
) -> (Vec<InterfaceMethod>, Vec<InterfaceEmbed>) {
    let mut methods = Vec::new();
    let mut embeds = Vec::new();
    let mut depth = 0;

    let first_line = lines[*i];
    depth += first_line.matches('{').count();
    depth -= first_line.matches('}').count();

    *i += 1;

    while *i < lines.len() && depth > 0 {
        let trimmed = lines[*i].trim();

        depth += trimmed.matches('{').count();
        depth -= trimmed.matches('}').count();

        if depth <= 0 {
            *i += 1;
            break;
        }

        if depth == 1 && !trimmed.is_empty() && !trimmed.starts_with("//") {
            // Method signature: has `(` in it
            if trimmed.contains('(') {
                let name = trimmed.split('(').next().unwrap_or("").trim();
                if !name.is_empty() {
                    methods.push(InterfaceMethod {
                        name: name.to_string(),
                        signature: trimmed.to_string(),
                        location: loc(file, *i + 1),
                    });
                }
            } else {
                // Embedded interface
                let type_name = trimmed.split("//").next().unwrap_or(trimmed).trim();
                if !type_name.is_empty() {
                    embeds.push(InterfaceEmbed {
                        type_name: type_name.to_string(),
                        location: loc(file, *i + 1),
                    });
                }
            }
        }

        *i += 1;
    }

    (methods, embeds)
}

// ── Functions ───────────────────────────────────────────────────────────────

fn extract_functions(lines: &[&str], file: &str) -> Vec<GoFunc> {
    let mut functions = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("func ") || trimmed.starts_with("func(") {
            if let Some(f) = try_parse_func(trimmed, file, i + 1) {
                functions.push(f);
            }
        }

        i += 1;
    }

    functions
}

/// Try to parse a function/method declaration.
fn try_parse_func(line: &str, file: &str, line_num: usize) -> Option<GoFunc> {
    let rest = line.strip_prefix("func")?;
    let rest = rest.trim_start();

    // Method: `func (r *Type) Name(params) returns`
    if rest.starts_with('(') {
        // Find the closing paren of the receiver
        let recv_end = find_matching_paren(rest, 0)?;
        let recv_str = &rest[1..recv_end];
        let receiver = parse_receiver(recv_str);

        let after_recv = rest[recv_end + 1..].trim();
        // Get the function name
        let name_end = after_recv.find('(')?;
        let name = after_recv[..name_end].trim().to_string();

        if name.is_empty() {
            return None;
        }

        let sig_rest = &after_recv[name_end..];
        let (params, returns) = parse_func_signature(sig_rest);

        return Some(GoFunc {
            visibility: Visibility::from_name(&name),
            name,
            receiver,
            params,
            returns,
            location: loc(file, line_num),
        });
    }

    // Regular function: `func Name(params) returns`
    let name_end = rest.find('(')?;
    let name = rest[..name_end].trim().to_string();

    if name.is_empty() {
        return None;
    }

    let sig_rest = &rest[name_end..];
    let (params, returns) = parse_func_signature(sig_rest);

    Some(GoFunc {
        visibility: Visibility::from_name(&name),
        name,
        receiver: None,
        params,
        returns,
        location: loc(file, line_num),
    })
}

/// Parse a receiver string like `r *Type` or `r Type`.
fn parse_receiver(s: &str) -> Option<Receiver> {
    let trimmed = s.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    match parts.len() {
        1 => {
            // Just the type, no name
            let pointer = parts[0].starts_with('*');
            let type_name = parts[0].trim_start_matches('*').to_string();
            Some(Receiver {
                name: String::new(),
                type_name,
                pointer,
            })
        }
        2 => {
            let name = parts[0].to_string();
            let pointer = parts[1].starts_with('*');
            let type_name = parts[1].trim_start_matches('*').to_string();
            Some(Receiver {
                name,
                type_name,
                pointer,
            })
        }
        _ => None,
    }
}

/// Parse function signature `(params) (returns)` into param/return lists.
/// Captures type expressions but doesn't fully resolve them.
fn parse_func_signature(sig: &str) -> (Vec<Param>, Vec<Param>) {
    let trimmed = sig.trim();

    // Find the parameter list
    let param_end = match find_matching_paren(trimmed, 0) {
        Some(e) => e,
        None => return (vec![], vec![]),
    };

    let param_str = &trimmed[1..param_end];
    let params = parse_param_list(param_str);

    // Returns come after params
    let rest = trimmed[param_end + 1..].trim();
    // Strip trailing `{` or block
    let rest = rest.trim_end_matches('{').trim();

    let returns = if rest.is_empty() {
        vec![]
    } else if rest.starts_with('(') {
        // Multiple returns: (type1, type2)
        if let Some(ret_end) = find_matching_paren(rest, 0) {
            parse_param_list(&rest[1..ret_end])
        } else {
            vec![]
        }
    } else {
        // Single return: just a type
        vec![Param {
            name: String::new(),
            type_expr: rest.to_string(),
        }]
    };

    (params, returns)
}

/// Parse a comma-separated parameter list.
fn parse_param_list(s: &str) -> Vec<Param> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let parts = split_params(trimmed);
    let mut params = Vec::new();

    for part in &parts {
        let tokens: Vec<&str> = part.trim().splitn(2, char::is_whitespace).collect();
        match tokens.len() {
            0 => {}
            1 => {
                // Just a type (unnamed param or return)
                params.push(Param {
                    name: String::new(),
                    type_expr: tokens[0].to_string(),
                });
            }
            _ => {
                let first = tokens[0].trim();
                let second = tokens[1].trim();
                // If first token looks like a type (starts with * or [ or uppercase or known type),
                // it might be an unnamed param with a complex type
                if first.starts_with("func(") || first.starts_with("func (") {
                    params.push(Param {
                        name: String::new(),
                        type_expr: part.trim().to_string(),
                    });
                } else {
                    params.push(Param {
                        name: first.to_string(),
                        type_expr: second.to_string(),
                    });
                }
            }
        }
    }

    params
}

/// Split parameters by commas, respecting nested parentheses and brackets.
fn split_params(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for ch in s.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        parts.push(current);
    }

    parts
}

/// Find the index of the matching closing paren for an opening paren at `start`.
fn find_matching_paren(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if start >= bytes.len() || bytes[start] != b'(' {
        return None;
    }

    let mut depth = 0;
    for (j, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + j);
                }
            }
            _ => {}
        }
    }

    None
}

// ── Constants ───────────────────────────────────────────────────────────────

fn extract_constants(lines: &[&str], file: &str) -> Vec<GoConst> {
    let mut constants = Vec::new();
    let mut i = 0;
    let mut in_block = false;
    let mut block_type: Option<String> = None;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // `const (`
        if trimmed == "const (" || trimmed.starts_with("const (") {
            in_block = true;
            block_type = None;
            i += 1;
            continue;
        }

        if in_block {
            if trimmed == ")" {
                in_block = false;
                block_type = None;
                i += 1;
                continue;
            }

            if !trimmed.is_empty() && !trimmed.starts_with("//") {
                if let Some(c) = try_parse_const(trimmed, file, i + 1, &block_type) {
                    // Track the type for iota-style blocks
                    if c.type_hint.is_some() {
                        block_type = c.type_hint.clone();
                    }
                    constants.push(c);
                }
            }

            i += 1;
            continue;
        }

        // Single-line const: `const Name Type = Value`
        if trimmed.starts_with("const ") {
            let rest = trimmed.strip_prefix("const ").unwrap().trim();
            if let Some(c) = try_parse_const(rest, file, i + 1, &None) {
                constants.push(c);
            }
        }

        i += 1;
    }

    constants
}

fn try_parse_const(
    line: &str,
    file: &str,
    line_num: usize,
    inherited_type: &Option<String>,
) -> Option<GoConst> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }

    // Remove inline comment
    let content = trimmed.split("//").next().unwrap_or(trimmed).trim();

    // Split on `=`
    if let Some(eq_pos) = content.find('=') {
        let lhs = content[..eq_pos].trim();
        let rhs = content[eq_pos + 1..].trim();

        let tokens: Vec<&str> = lhs.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }

        let name = tokens[0].to_string();
        let type_hint = if tokens.len() > 1 {
            Some(tokens[1..].join(" "))
        } else {
            inherited_type.clone()
        };

        Some(GoConst {
            visibility: Visibility::from_name(&name),
            name,
            type_hint,
            value: Some(rhs.to_string()),
            location: loc(file, line_num),
        })
    } else {
        // Iota continuation: just a name (no `=`)
        let name = content.split_whitespace().next()?.to_string();
        Some(GoConst {
            visibility: Visibility::from_name(&name),
            name,
            type_hint: inherited_type.clone(),
            value: None,
            location: loc(file, line_num),
        })
    }
}

// ── Variables ───────────────────────────────────────────────────────────────

fn extract_variables(lines: &[&str], file: &str) -> Vec<GoVar> {
    let mut variables = Vec::new();
    let mut i = 0;
    let mut in_block = false;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // `var (`
        if trimmed == "var (" || trimmed.starts_with("var (") {
            in_block = true;
            i += 1;
            continue;
        }

        if in_block {
            if trimmed == ")" {
                in_block = false;
                i += 1;
                continue;
            }

            if !trimmed.is_empty() && !trimmed.starts_with("//") {
                if let Some(v) = try_parse_var(trimmed, file, i + 1) {
                    variables.push(v);
                }
            }

            i += 1;
            continue;
        }

        // Single-line var: `var Name Type = Value`
        if trimmed.starts_with("var ") {
            let rest = trimmed.strip_prefix("var ").unwrap().trim();
            if let Some(v) = try_parse_var(rest, file, i + 1) {
                variables.push(v);
            }
        }

        i += 1;
    }

    variables
}

fn try_parse_var(line: &str, file: &str, line_num: usize) -> Option<GoVar> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }

    let content = trimmed.split("//").next().unwrap_or(trimmed).trim();

    if let Some(eq_pos) = content.find('=') {
        let lhs = content[..eq_pos].trim();
        let rhs = content[eq_pos + 1..].trim();

        let tokens: Vec<&str> = lhs.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }

        let name = tokens[0].to_string();
        let type_hint = if tokens.len() > 1 {
            Some(tokens[1..].join(" "))
        } else {
            None
        };

        Some(GoVar {
            visibility: Visibility::from_name(&name),
            name,
            type_hint,
            value: Some(rhs.to_string()),
            location: loc(file, line_num),
        })
    } else {
        let tokens: Vec<&str> = content.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }

        let name = tokens[0].to_string();
        let type_hint = if tokens.len() > 1 {
            Some(tokens[1..].join(" "))
        } else {
            None
        };

        Some(GoVar {
            visibility: Visibility::from_name(&name),
            name,
            type_hint,
            value: None,
            location: loc(file, line_num),
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn loc(file: &str, line: usize) -> Location {
    Location {
        file: file.to_string(),
        line,
    }
}

/// Extract the first quoted string from a line.
fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let end = s[start + 1..].find('"')?;
    Some(s[start + 1..start + 1 + end].to_string())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Package ─────────────────────────────────────────────────────

    #[test]
    fn extracts_package_declaration() {
        let file = parse_file("main.go", "package main\n\nfunc main() {}\n");
        assert_eq!(file.package, "main");
    }

    #[test]
    fn extracts_package_with_comment() {
        let file = parse_file("pkg.go", "package configctl // domain package\n");
        assert_eq!(file.package, "configctl");
    }

    #[test]
    fn empty_source_returns_empty_package() {
        let file = parse_file("empty.go", "");
        assert_eq!(file.package, "");
    }

    // ── Imports ─────────────────────────────────────────────────────

    #[test]
    fn extracts_single_import() {
        let src = "package main\n\nimport \"fmt\"\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].path, "fmt");
        assert_eq!(file.imports[0].kind, ImportKind::Stdlib);
        assert!(file.imports[0].alias.is_none());
    }

    #[test]
    fn extracts_grouped_imports() {
        let src = r#"package main

import (
	"context"
	"fmt"

	"github.com/nats-io/nats.go"

	"quality-service/internal/domain/configctl"
)
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.imports.len(), 4);
        assert_eq!(file.imports[0].kind, ImportKind::Stdlib);
        assert_eq!(file.imports[1].kind, ImportKind::Stdlib);
        assert_eq!(file.imports[2].kind, ImportKind::External);
        assert_eq!(file.imports[3].kind, ImportKind::Internal);
    }

    #[test]
    fn extracts_aliased_import() {
        let src = r#"package main

import (
	configdomain "quality-service/internal/domain/configctl"
)
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.imports.len(), 1);
        assert_eq!(file.imports[0].alias.as_deref(), Some("configdomain"));
        assert_eq!(
            file.imports[0].path,
            "quality-service/internal/domain/configctl"
        );
    }

    #[test]
    fn extracts_dot_import() {
        let src = "package main\n\nimport (\n\t. \"testing\"\n)\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.imports[0].alias.as_deref(), Some("."));
    }

    #[test]
    fn extracts_blank_import() {
        let src = "package main\n\nimport (\n\t_ \"net/http/pprof\"\n)\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.imports[0].alias.as_deref(), Some("_"));
    }

    // ── Types: Structs ──────────────────────────────────────────────

    #[test]
    fn extracts_struct_with_fields() {
        let src = r#"package main

type User struct {
	Name    string
	Age     int
	Email   string
}
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.types.len(), 1);
        let t = &file.types[0];
        assert_eq!(t.name, "User");
        assert_eq!(t.visibility, Visibility::Exported);
        match &t.kind {
            TypeKind::Struct { fields } => {
                assert_eq!(fields.len(), 3);
                assert_eq!(fields[0].name, "Name");
                assert_eq!(fields[0].type_expr, "string");
                assert!(!fields[0].embedded);
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn extracts_struct_with_tags() {
        let src = r#"package main

type Config struct {
	Name   string `json:"name"`
	Value  int    `json:"value,omitempty"`
}
"#;
        let file = parse_file("main.go", src);
        match &file.types[0].kind {
            TypeKind::Struct { fields } => {
                assert_eq!(fields[0].tag.as_deref(), Some("`json:\"name\"`"));
                assert_eq!(fields[1].tag.as_deref(), Some("`json:\"value,omitempty\"`"));
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn extracts_embedded_field() {
        let src = r#"package main

type MyHandler struct {
	events.Metadata
	Name string
}
"#;
        let file = parse_file("main.go", src);
        match &file.types[0].kind {
            TypeKind::Struct { fields } => {
                assert_eq!(fields.len(), 2);
                assert!(fields[0].embedded);
                assert_eq!(fields[0].name, "Metadata");
                assert_eq!(fields[0].type_expr, "events.Metadata");
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn extracts_empty_struct() {
        let src = "package main\n\ntype Empty struct{}\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.types.len(), 1);
        match &file.types[0].kind {
            TypeKind::Struct { fields } => assert!(fields.is_empty()),
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn extracts_unexported_struct() {
        let src = "package main\n\ntype internalState struct {\n\tvalue int\n}\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.types[0].visibility, Visibility::Unexported);
    }

    // ── Types: Interfaces ───────────────────────────────────────────

    #[test]
    fn extracts_interface_with_methods() {
        let src = r#"package main

type Reader interface {
	Read(p []byte) (n int, err error)
	Close() error
}
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.types.len(), 1);
        match &file.types[0].kind {
            TypeKind::Interface { methods, embeds } => {
                assert_eq!(methods.len(), 2);
                assert_eq!(methods[0].name, "Read");
                assert!(embeds.is_empty());
            }
            _ => panic!("expected interface"),
        }
    }

    #[test]
    fn extracts_interface_with_embed() {
        let src = r#"package main

type DomainEvent interface {
	events.Event
	Validate() error
}
"#;
        let file = parse_file("main.go", src);
        match &file.types[0].kind {
            TypeKind::Interface { methods, embeds } => {
                assert_eq!(methods.len(), 1);
                assert_eq!(embeds.len(), 1);
                assert_eq!(embeds[0].type_name, "events.Event");
            }
            _ => panic!("expected interface"),
        }
    }

    // ── Types: Aliases ──────────────────────────────────────────────

    #[test]
    fn extracts_type_alias() {
        let src = "package main\n\ntype VersionLifecycle string\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.types.len(), 1);
        match &file.types[0].kind {
            TypeKind::Alias { underlying } => assert_eq!(underlying, "string"),
            _ => panic!("expected alias"),
        }
    }

    #[test]
    fn extracts_type_alias_with_equals() {
        let src = "package main\n\ntype Byte = uint8\n";
        let file = parse_file("main.go", src);
        match &file.types[0].kind {
            TypeKind::Alias { underlying } => assert_eq!(underlying, "uint8"),
            _ => panic!("expected alias"),
        }
    }

    // ── Functions ───────────────────────────────────────────────────

    #[test]
    fn extracts_simple_function() {
        let src = "package main\n\nfunc main() {\n}\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].name, "main");
        assert!(file.functions[0].receiver.is_none());
    }

    #[test]
    fn extracts_function_with_params_and_returns() {
        let src = "package main\n\nfunc Add(a int, b int) int {\n\treturn a + b\n}\n";
        let file = parse_file("main.go", src);
        let f = &file.functions[0];
        assert_eq!(f.name, "Add");
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name, "a");
        assert_eq!(f.params[0].type_expr, "int");
        assert_eq!(f.returns.len(), 1);
        assert_eq!(f.returns[0].type_expr, "int");
    }

    #[test]
    fn extracts_method_with_pointer_receiver() {
        let src = "package main\n\nfunc (s *ConfigSet) Activate(id string) error {\n}\n";
        let file = parse_file("main.go", src);
        let f = &file.functions[0];
        assert_eq!(f.name, "Activate");
        let recv = f.receiver.as_ref().unwrap();
        assert_eq!(recv.name, "s");
        assert_eq!(recv.type_name, "ConfigSet");
        assert!(recv.pointer);
    }

    #[test]
    fn extracts_method_with_value_receiver() {
        let src = "package main\n\nfunc (c ConfigSet) IsActive() bool {\n}\n";
        let file = parse_file("main.go", src);
        let recv = file.functions[0].receiver.as_ref().unwrap();
        assert!(!recv.pointer);
    }

    #[test]
    fn extracts_function_with_multiple_returns() {
        let src = "package main\n\nfunc Get(key string) ([]byte, bool) {\n}\n";
        let file = parse_file("main.go", src);
        let f = &file.functions[0];
        assert_eq!(f.returns.len(), 2);
    }

    #[test]
    fn extracts_exported_function() {
        let src = "package main\n\nfunc HandleRequest() {}\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.functions[0].visibility, Visibility::Exported);
    }

    #[test]
    fn extracts_unexported_function() {
        let src = "package main\n\nfunc handleInternal() {}\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.functions[0].visibility, Visibility::Unexported);
    }

    // ── Constants ───────────────────────────────────────────────────

    #[test]
    fn extracts_single_const() {
        let src = "package main\n\nconst MaxRetries = 3\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.constants.len(), 1);
        assert_eq!(file.constants[0].name, "MaxRetries");
        assert_eq!(file.constants[0].value.as_deref(), Some("3"));
    }

    #[test]
    fn extracts_typed_const() {
        let src = "package main\n\nconst EventActivated events.Name = \"config.activated\"\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.constants[0].name, "EventActivated");
        assert_eq!(file.constants[0].type_hint.as_deref(), Some("events.Name"));
        assert_eq!(
            file.constants[0].value.as_deref(),
            Some("\"config.activated\"")
        );
    }

    #[test]
    fn extracts_const_block() {
        let src = r#"package main

const (
	LifecycleDraft     VersionLifecycle = "draft"
	LifecycleValidated VersionLifecycle = "validated"
	LifecycleCompiled  VersionLifecycle = "compiled"
)
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.constants.len(), 3);
        assert_eq!(file.constants[0].name, "LifecycleDraft");
        assert_eq!(
            file.constants[0].type_hint.as_deref(),
            Some("VersionLifecycle")
        );
        assert_eq!(file.constants[2].name, "LifecycleCompiled");
    }

    #[test]
    fn extracts_iota_const_block() {
        let src = r#"package main

const (
	Info Severity = iota
	Warning
	Error
)
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.constants.len(), 3);
        assert_eq!(file.constants[0].name, "Info");
        assert_eq!(file.constants[0].type_hint.as_deref(), Some("Severity"));
        // Subsequent entries inherit the type
        assert_eq!(file.constants[1].name, "Warning");
        assert_eq!(file.constants[1].type_hint.as_deref(), Some("Severity"));
    }

    // ── Variables ───────────────────────────────────────────────────

    #[test]
    fn extracts_var_declaration() {
        let src = "package main\n\nvar ErrNotFound = errors.New(\"not found\")\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.variables.len(), 1);
        assert_eq!(file.variables[0].name, "ErrNotFound");
    }

    #[test]
    fn extracts_typed_var() {
        let src = "package main\n\nvar defaultTimeout time.Duration = 30 * time.Second\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.variables[0].name, "defaultTimeout");
        assert_eq!(
            file.variables[0].type_hint.as_deref(),
            Some("time.Duration")
        );
    }

    #[test]
    fn extracts_var_block() {
        let src = r#"package main

var (
	ErrNilDB       = errors.New("nil db")
	ErrNilCallback = errors.New("nil callback")
)
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.variables.len(), 2);
    }

    // ── File metadata ───────────────────────────────────────────────

    #[test]
    fn detects_test_file() {
        let file = parse_file("handler_test.go", "package main\n");
        assert!(file.is_test);
    }

    #[test]
    fn detects_non_test_file() {
        let file = parse_file("handler.go", "package main\n");
        assert!(!file.is_test);
    }

    #[test]
    fn tracks_line_count() {
        let src = "package main\n\nfunc main() {\n}\n";
        let file = parse_file("main.go", src);
        assert_eq!(file.line_count, 4);
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn handles_struct_with_pointer_fields() {
        let src = r#"package main

type Config struct {
	ExpiresAt *time.Time
	Parent    *Config
}
"#;
        let file = parse_file("main.go", src);
        match &file.types[0].kind {
            TypeKind::Struct { fields } => {
                assert_eq!(fields[0].type_expr, "*time.Time");
                assert_eq!(fields[1].type_expr, "*Config");
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn handles_slice_and_map_fields() {
        let src = r#"package main

type Container struct {
	Items   []string
	Labels  map[string]string
}
"#;
        let file = parse_file("main.go", src);
        match &file.types[0].kind {
            TypeKind::Struct { fields } => {
                assert_eq!(fields[0].type_expr, "[]string");
                assert_eq!(fields[1].type_expr, "map[string]string");
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn handles_multiple_types_in_file() {
        let src = r#"package main

type A struct {
	X int
}

type B interface {
	Do()
}

type C string
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.types.len(), 3);
        assert!(matches!(file.types[0].kind, TypeKind::Struct { .. }));
        assert!(matches!(file.types[1].kind, TypeKind::Interface { .. }));
        assert!(matches!(file.types[2].kind, TypeKind::Alias { .. }));
    }

    #[test]
    fn locations_are_correct() {
        let src = r#"package main

import "fmt"

type Foo struct {
	Bar string
}

func Hello() {
}
"#;
        let file = parse_file("test.go", src);
        assert_eq!(file.imports[0].location.line, 3);
        assert_eq!(file.types[0].location.line, 5);
        assert_eq!(file.functions[0].location.line, 9);
    }

    // ── Comprehensive real-world pattern ────────────────────────────

    #[test]
    fn parses_realistic_domain_file() {
        let src = r#"package configctl

import (
	"strings"
	"time"

	"quality-service/internal/shared/events"
	"quality-service/internal/shared/problem"
)

type VersionLifecycle string

const (
	LifecycleDraft     VersionLifecycle = "draft"
	LifecycleValidated VersionLifecycle = "validated"
)

type ConfigVersion struct {
	VersionID   string            `json:"version_id"`
	Lifecycle   VersionLifecycle  `json:"lifecycle"`
	CreatedAt   time.Time         `json:"created_at"`
}

type ConfigSet struct {
	SetID         string
	Versions      []ConfigVersion
	pendingEvents []events.Event
}

func NewConfigSet(setID string, createdAt time.Time) (ConfigSet, *problem.Problem) {
	return ConfigSet{}, nil
}

func (s *ConfigSet) PullEvents() []events.Event {
	return s.pendingEvents
}

func (s ConfigSet) hasOpenCandidate() bool {
	return false
}
"#;
        let file = parse_file("internal/domain/configctl/config_set.go", src);

        // Package
        assert_eq!(file.package, "configctl");
        assert!(!file.is_test);

        // Imports
        assert_eq!(file.imports.len(), 4);
        assert_eq!(file.imports[0].kind, ImportKind::Stdlib);
        assert_eq!(file.imports[2].kind, ImportKind::Internal);

        // Types
        assert_eq!(file.types.len(), 3);
        // VersionLifecycle alias
        assert_eq!(file.types[0].name, "VersionLifecycle");
        assert!(matches!(file.types[0].kind, TypeKind::Alias { .. }));
        // ConfigVersion struct
        assert_eq!(file.types[1].name, "ConfigVersion");
        match &file.types[1].kind {
            TypeKind::Struct { fields } => assert_eq!(fields.len(), 3),
            _ => panic!("expected struct"),
        }
        // ConfigSet struct
        match &file.types[2].kind {
            TypeKind::Struct { fields } => {
                assert_eq!(fields.len(), 3);
                assert_eq!(fields[2].visibility, Visibility::Unexported);
            }
            _ => panic!("expected struct"),
        }

        // Constants
        assert_eq!(file.constants.len(), 2);
        assert_eq!(file.constants[0].name, "LifecycleDraft");

        // Functions
        assert_eq!(file.functions.len(), 3);
        assert_eq!(file.functions[0].name, "NewConfigSet");
        assert!(file.functions[0].receiver.is_none());
        assert_eq!(file.functions[1].name, "PullEvents");
        assert!(file.functions[1].receiver.as_ref().unwrap().pointer);
        assert_eq!(file.functions[2].name, "hasOpenCandidate");
        assert!(!file.functions[2].receiver.as_ref().unwrap().pointer);
        assert_eq!(file.functions[2].visibility, Visibility::Unexported);
    }

    #[test]
    fn parses_port_interface() {
        let src = r#"package ports

import (
	"context"

	"quality-service/internal/application/configctl/contracts"
	"quality-service/internal/shared/problem"
)

type ConfigctlGateway interface {
	CreateDraft(ctx context.Context, cmd contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem)
	GetConfig(ctx context.Context, query contracts.GetConfigQuery) (contracts.GetConfigReply, *problem.Problem)
	ListConfigs(ctx context.Context, query contracts.ListConfigsQuery) (contracts.ListConfigsReply, *problem.Problem)
}
"#;
        let file = parse_file("internal/application/ports/configctl.go", src);
        assert_eq!(file.package, "ports");
        assert_eq!(file.types.len(), 1);
        match &file.types[0].kind {
            TypeKind::Interface { methods, embeds } => {
                assert_eq!(methods.len(), 3);
                assert_eq!(methods[0].name, "CreateDraft");
                assert!(embeds.is_empty());
            }
            _ => panic!("expected interface"),
        }
    }

    // ── Import edge cases ───────────────────────────────────────────

    #[test]
    fn classify_stdlib_imports() {
        assert_eq!(classify_import("fmt"), ImportKind::Stdlib);
        assert_eq!(classify_import("net/http"), ImportKind::Stdlib);
        assert_eq!(classify_import("context"), ImportKind::Stdlib);
        assert_eq!(classify_import("encoding/json"), ImportKind::Stdlib);
    }

    #[test]
    fn classify_external_imports() {
        assert_eq!(
            classify_import("github.com/nats-io/nats.go"),
            ImportKind::External
        );
        assert_eq!(
            classify_import("github.com/anthdm/hollywood"),
            ImportKind::External
        );
    }

    #[test]
    fn classify_internal_imports() {
        assert_eq!(
            classify_import("quality-service/internal/domain/configctl"),
            ImportKind::Internal
        );
    }

    // ── find_matching_paren ─────────────────────────────────────────

    #[test]
    fn find_matching_paren_simple() {
        assert_eq!(find_matching_paren("(abc)", 0), Some(4));
    }

    #[test]
    fn find_matching_paren_nested() {
        assert_eq!(find_matching_paren("(a(b)c)", 0), Some(6));
    }

    #[test]
    fn find_matching_paren_no_match() {
        assert_eq!(find_matching_paren("(abc", 0), None);
    }

    // ── Grouped type blocks ─────────────────────────────────────────

    #[test]
    fn extracts_types_from_grouped_block() {
        let src = r#"package main

type (
	Foo struct {
		X int
	}

	Bar string

	Baz interface {
		Do()
	}
)
"#;
        let file = parse_file("main.go", src);
        assert_eq!(file.types.len(), 3);
        assert_eq!(file.types[0].name, "Foo");
        assert!(matches!(file.types[0].kind, TypeKind::Struct { .. }));
        assert_eq!(file.types[1].name, "Bar");
        assert!(matches!(file.types[1].kind, TypeKind::Alias { .. }));
        assert_eq!(file.types[2].name, "Baz");
        assert!(matches!(file.types[2].kind, TypeKind::Interface { .. }));
    }
}
