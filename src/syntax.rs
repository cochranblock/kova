// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! syntax — Syntax-aware code analysis. Extracts structure from Rust source files.
//! Research: tree-sitter (AST patterns), syn (Rust-native parsing).
//! Uses regex-based heuristics for fast extraction without heavy deps.
//! f201=extract_symbols, f202=extract_functions, f203=extract_structs, f204=extract_impls.
//! t132=Symbol, t133=SymbolKind.

use serde::{Deserialize, Serialize};

// ── Types ────────────────────────────────────────────────────────

/// t133=SymbolKind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Mod,
    Const,
    Static,
    TypeAlias,
}

impl SymbolKind {
    pub fn short(&self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Mod => "mod",
            Self::Const => "const",
            Self::Static => "static",
            Self::TypeAlias => "type",
        }
    }
}

/// t132=Symbol. A named code element with location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line_start: usize,
    pub line_end: usize,
    pub is_public: bool,
    pub signature: String,
}

// ── Extraction ───────────────────────────────────────────────────

/// f201=extract_symbols. Extract all symbols from Rust source.
pub fn extract_symbols(source: &str) -> Vec<Symbol> {
    let lines: Vec<&str> = source.lines().collect();
    let mut symbols = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if let Some(sym) = try_parse_symbol(trimmed, i, &lines) {
            symbols.push(sym);
        }
        i += 1;
    }

    symbols
}

/// f202=extract_functions. Extract only function symbols.
pub fn extract_functions(source: &str) -> Vec<Symbol> {
    extract_symbols(source)
        .into_iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect()
}

/// f203=extract_structs. Extract struct and enum symbols.
pub fn extract_structs(source: &str) -> Vec<Symbol> {
    extract_symbols(source)
        .into_iter()
        .filter(|s| matches!(s.kind, SymbolKind::Struct | SymbolKind::Enum))
        .collect()
}

/// f204=extract_impls. Extract impl blocks.
pub fn extract_impls(source: &str) -> Vec<Symbol> {
    extract_symbols(source)
        .into_iter()
        .filter(|s| s.kind == SymbolKind::Impl)
        .collect()
}

/// f205=format_outline. Format symbols as a code outline.
pub fn format_outline(symbols: &[Symbol]) -> String {
    let mut out = String::new();
    for s in symbols {
        let vis = if s.is_public { "pub " } else { "" };
        out.push_str(&format!(
            "  L{:<4} {}{} {}\n",
            s.line_start + 1,
            vis,
            s.kind.short(),
            s.name
        ));
    }
    out
}

/// f206=outline_file. Parse a file and return its outline.
pub fn outline_file(path: &std::path::Path) -> anyhow::Result<Vec<Symbol>> {
    let content = std::fs::read_to_string(path)?;
    Ok(extract_symbols(&content))
}

// ── Internal Parsing ─────────────────────────────────────────────

fn try_parse_symbol(line: &str, line_idx: usize, all_lines: &[&str]) -> Option<Symbol> {
    let is_pub = line.starts_with("pub ");
    let rest = if is_pub {
        line.strip_prefix("pub ").unwrap_or(line)
    } else {
        line
    };
    // Skip pub(crate) etc
    let rest = if rest.starts_with("(crate) ") || rest.starts_with("(super) ") {
        rest.split_once(") ").map(|(_, r)| r).unwrap_or(rest)
    } else {
        rest
    };

    // fn name(...)
    if rest.starts_with("fn ")
        || rest.starts_with("async fn ")
        || rest.starts_with("const fn ")
        || rest.starts_with("unsafe fn ")
    {
        let name = extract_fn_name(rest)?;
        let end = find_block_end(line_idx, all_lines);
        let sig = line.trim_end().to_string();
        return Some(Symbol {
            name,
            kind: SymbolKind::Function,
            line_start: line_idx,
            line_end: end,
            is_public: is_pub,
            signature: sig,
        });
    }

    // struct Name
    if rest.starts_with("struct ") {
        let name = extract_item_name(rest, "struct ")?;
        let end = find_block_end(line_idx, all_lines);
        return Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Struct,
            line_start: line_idx,
            line_end: end,
            is_public: is_pub,
            signature: format!("struct {}", name),
        });
    }

    // enum Name
    if rest.starts_with("enum ") {
        let name = extract_item_name(rest, "enum ")?;
        let end = find_block_end(line_idx, all_lines);
        return Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Enum,
            line_start: line_idx,
            line_end: end,
            is_public: is_pub,
            signature: format!("enum {}", name),
        });
    }

    // trait Name
    if rest.starts_with("trait ") {
        let name = extract_item_name(rest, "trait ")?;
        let end = find_block_end(line_idx, all_lines);
        return Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Trait,
            line_start: line_idx,
            line_end: end,
            is_public: is_pub,
            signature: format!("trait {}", name),
        });
    }

    // impl Name / impl Trait for Name
    if rest.starts_with("impl ") || rest.starts_with("impl<") {
        let name = extract_impl_name(rest);
        let end = find_block_end(line_idx, all_lines);
        return Some(Symbol {
            name,
            kind: SymbolKind::Impl,
            line_start: line_idx,
            line_end: end,
            is_public: false,
            signature: line.trim_end().to_string(),
        });
    }

    // mod name
    if rest.starts_with("mod ") {
        let name = extract_item_name(rest, "mod ")?;
        let has_body = line.contains('{');
        let end = if has_body {
            find_block_end(line_idx, all_lines)
        } else {
            line_idx
        };
        return Some(Symbol {
            name: name.clone(),
            kind: SymbolKind::Mod,
            line_start: line_idx,
            line_end: end,
            is_public: is_pub,
            signature: format!("mod {}", name),
        });
    }

    // const NAME / static NAME
    if rest.starts_with("const ") {
        let name = extract_item_name(rest, "const ")?;
        return Some(Symbol {
            name,
            kind: SymbolKind::Const,
            line_start: line_idx,
            line_end: line_idx,
            is_public: is_pub,
            signature: line.trim_end().to_string(),
        });
    }

    if rest.starts_with("static ") {
        let name = extract_item_name(rest, "static ")?;
        return Some(Symbol {
            name,
            kind: SymbolKind::Static,
            line_start: line_idx,
            line_end: line_idx,
            is_public: is_pub,
            signature: line.trim_end().to_string(),
        });
    }

    // type Alias = ...
    if rest.starts_with("type ") {
        let name = extract_item_name(rest, "type ")?;
        return Some(Symbol {
            name,
            kind: SymbolKind::TypeAlias,
            line_start: line_idx,
            line_end: line_idx,
            is_public: is_pub,
            signature: line.trim_end().to_string(),
        });
    }

    None
}

fn extract_fn_name(s: &str) -> Option<String> {
    // "fn name(" or "async fn name(" etc
    let after_fn = s
        .find("fn ")
        .map(|i| &s[i + 3..])?;
    let name_end = after_fn
        .find(|c: char| c == '(' || c == '<' || c == ' ')
        .unwrap_or(after_fn.len());
    let name = after_fn[..name_end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn extract_item_name(s: &str, prefix: &str) -> Option<String> {
    let after = s.strip_prefix(prefix)?;
    let name_end = after
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());
    let name = &after[..name_end];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn extract_impl_name(s: &str) -> String {
    // "impl Foo" or "impl<T> Foo<T>" or "impl Trait for Foo"
    let trimmed = s.trim_end_matches('{').trim();
    // Remove "impl" prefix and any generic params
    let after_impl = if let Some(rest) = trimmed.strip_prefix("impl<") {
        // Skip past the generic params
        if let Some(close) = rest.find('>') {
            rest[close + 1..].trim()
        } else {
            rest
        }
    } else {
        trimmed.strip_prefix("impl ").unwrap_or(trimmed)
    };
    after_impl.trim().to_string()
}

fn find_block_end(start: usize, lines: &[&str]) -> usize {
    let mut depth: i32 = 0;
    for i in start..lines.len() {
        for ch in lines[i].chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 && i > start {
                        return i;
                    }
                }
                _ => {}
            }
        }
    }
    // No closing brace found (single-line item or parse error)
    start
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_basic_function() {
        let src = "pub fn hello() {\n    println!(\"hi\");\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].kind, SymbolKind::Function);
        assert!(syms[0].is_public);
    }

    #[test]
    fn extract_struct_and_impl() {
        let src = "struct Foo {\n    x: i32,\n}\n\nimpl Foo {\n    fn new() -> Self {\n        Foo { x: 0 }\n    }\n}";
        let syms = extract_symbols(src);
        assert!(syms.len() >= 2);
        assert_eq!(syms[0].name, "Foo");
        assert_eq!(syms[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn extract_enum() {
        let src = "pub enum Color {\n    Red,\n    Green,\n    Blue,\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Color");
        assert_eq!(syms[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn extract_trait() {
        let src = "pub trait Display {\n    fn fmt(&self) -> String;\n}";
        let syms = extract_symbols(src);
        assert!(syms.len() >= 1);
        assert_eq!(syms[0].name, "Display");
        assert_eq!(syms[0].kind, SymbolKind::Trait);
    }

    #[test]
    fn extract_async_fn() {
        let src = "pub async fn fetch() -> Result<(), Error> {\n    Ok(())\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "fetch");
        assert_eq!(syms[0].kind, SymbolKind::Function);
    }

    #[test]
    fn format_outline_shows_lines() {
        let src = "fn foo() {}\nstruct Bar {}";
        let syms = extract_symbols(src);
        let outline = format_outline(&syms);
        assert!(outline.contains("fn foo"));
        assert!(outline.contains("struct Bar"));
    }

    #[test]
    fn empty_source() {
        let syms = extract_symbols("");
        assert!(syms.is_empty());
    }

    #[test]
    fn symbol_kind_short() {
        assert_eq!(SymbolKind::Function.short(), "fn");
        assert_eq!(SymbolKind::Struct.short(), "struct");
        assert_eq!(SymbolKind::Impl.short(), "impl");
    }
}
