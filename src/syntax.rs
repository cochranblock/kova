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
    } else if line.starts_with("pub(") {
        // pub(crate), pub(super), pub(in path::to::mod) — skip past ") "
        if let Some(close) = line.find(") ") {
            return try_parse_symbol_inner(&line[close + 2..], line_idx, all_lines, true);
        } else {
            return None;
        }
    } else {
        line
    };
    // Skip pub(crate) etc when reached via "pub " prefix (shouldn't happen, but defensive)
    let rest = if rest.starts_with('(') {
        if let Some(close) = rest.find(") ") {
            &rest[close + 2..]
        } else {
            rest
        }
    } else {
        rest
    };

    try_parse_symbol_inner(rest, line_idx, all_lines, is_pub)
}

fn try_parse_symbol_inner(rest: &str, line_idx: usize, all_lines: &[&str], is_pub: bool) -> Option<Symbol> {
    let line_str = all_lines[line_idx];

    // fn name(...)
    if rest.starts_with("fn ")
        || rest.starts_with("async fn ")
        || rest.starts_with("const fn ")
        || rest.starts_with("unsafe fn ")
    {
        let name = extract_fn_name(rest)?;
        let end = find_block_end(line_idx, all_lines);
        let sig = line_str.trim_end().to_string();
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
            signature: line_str.trim_end().to_string(),
        });
    }

    // mod name
    if rest.starts_with("mod ") {
        let name = extract_item_name(rest, "mod ")?;
        let has_body = line_str.contains('{');
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
            signature: line_str.trim_end().to_string(),
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
            signature: line_str.trim_end().to_string(),
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
            signature: line_str.trim_end().to_string(),
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
        .find(|c: char| ['(', '<', ' '].contains(&c))
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
    let mut in_string = false;
    let mut in_block_comment = false;

    for (i, line) in lines.iter().enumerate().skip(start) {
        let chars: Vec<char> = line.chars().collect();
        let mut in_line_comment = false;
        let mut j = 0;

        while j < chars.len() {
            let ch = chars[j];
            let next = chars.get(j + 1).copied();

            if in_block_comment {
                if ch == '*' && next == Some('/') {
                    in_block_comment = false;
                    j += 2;
                    continue;
                }
                j += 1;
                continue;
            }

            if in_line_comment {
                j += 1;
                continue;
            }

            if in_string {
                if ch == '\\' {
                    j += 2; // skip escaped char
                    continue;
                }
                if ch == '"' {
                    in_string = false;
                }
                j += 1;
                continue;
            }

            // Not in any special context.
            if ch == '/' && next == Some('/') {
                in_line_comment = true;
                j += 2;
                continue;
            }
            if ch == '/' && next == Some('*') {
                in_block_comment = true;
                j += 2;
                continue;
            }
            // Raw string: r"...", r#"..."#, r##"..."## etc
            if ch == 'r' && (next == Some('"') || next == Some('#')) {
                let mut k = j + 1;
                let mut hashes = 0u32;
                while k < chars.len() && chars[k] == '#' {
                    hashes += 1;
                    k += 1;
                }
                if k < chars.len() && chars[k] == '"' {
                    k += 1; // skip opening "
                    loop {
                        if k >= chars.len() {
                            j = k;
                            break;
                        }
                        if chars[k] == '"' {
                            let mut ch_count = 0u32;
                            let mut m = k + 1;
                            while m < chars.len() && chars[m] == '#' && ch_count < hashes {
                                ch_count += 1;
                                m += 1;
                            }
                            if ch_count == hashes {
                                j = m;
                                break;
                            }
                        }
                        k += 1;
                    }
                    continue;
                }
            }
            if ch == '"' {
                in_string = true;
                j += 1;
                continue;
            }

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
            j += 1;
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

    #[test]
    fn find_block_end_ignores_braces_in_strings() {
        let src = "fn foo() {\n    let s = \"{ not a block }\";\n    let x = 1;\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "foo");
        assert_eq!(syms[0].line_end, 3);
    }

    #[test]
    fn find_block_end_ignores_braces_in_comments() {
        let src = "fn bar() {\n    // { this is a comment }\n    let x = 2;\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "bar");
        assert_eq!(syms[0].line_end, 3);
    }

    #[test]
    fn find_block_end_ignores_block_comment() {
        let src = "fn baz() {\n    /* { nested } */\n    let y = 3;\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "baz");
        assert_eq!(syms[0].line_end, 3);
    }

    #[test]
    fn extract_const_and_static() {
        let src = "pub const MAX: usize = 100;\nstatic COUNT: usize = 0;";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].name, "MAX");
        assert_eq!(syms[0].kind, SymbolKind::Const);
        assert_eq!(syms[1].name, "COUNT");
        assert_eq!(syms[1].kind, SymbolKind::Static);
    }

    #[test]
    fn extract_type_alias() {
        let src = "pub type Result<T> = std::result::Result<T, Error>;";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "Result");
        assert_eq!(syms[0].kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn extract_mod_with_body() {
        let src = "mod inner {\n    fn private() {}\n}";
        let syms = extract_symbols(src);
        assert!(syms.iter().any(|s| s.name == "inner" && s.kind == SymbolKind::Mod));
    }

    #[test]
    fn find_block_end_ignores_braces_in_raw_string() {
        let src = "fn raw() {\n    let s = r#\"{ not a block }\"#;\n    let x = 1;\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "raw");
        assert_eq!(syms[0].line_end, 3);
    }

    #[test]
    fn extract_pub_crate_fn() {
        let src = "pub(crate) fn internal() {\n    42\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "internal");
        assert!(syms[0].is_public);
    }

    #[test]
    fn extract_pub_in_path_fn() {
        let src = "pub(in crate::module) fn scoped() {\n    1\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "scoped");
    }

    #[test]
    fn find_block_end_escaped_quote_in_string() {
        let src = "fn esc() {\n    let s = \"she said \\\"{ hi }\\\"\";\n    let x = 1;\n}";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "esc");
        assert_eq!(syms[0].line_end, 3);
    }

    #[test]
    fn extract_impl_trait_for() {
        let src = "impl std::fmt::Display for Foo {\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {\n        Ok(())\n    }\n}";
        let syms = extract_symbols(src);
        let impl_sym = syms.iter().find(|s| s.kind == SymbolKind::Impl).unwrap();
        assert!(impl_sym.name.contains("Display"));
        assert!(impl_sym.name.contains("Foo"));
    }

    #[test]
    fn extract_fn_nested_generics() {
        let src = "fn foo<T: Clone, U: Into<T>>(a: T, b: U) -> T where T: Default, U: Send { a }";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "foo");
        assert_eq!(syms[0].kind, SymbolKind::Function);
    }

    #[test]
    fn extract_fn_lifetime_bounds() {
        let src = "fn bar<'a, 'b: 'a>(x: &'a str, y: &'b str) -> &'a str where 'b: 'a { x }";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "bar");
    }

    #[test]
    fn extract_fn_where_clause() {
        let src = "fn baz<T>(v: T) -> T where T: std::fmt::Debug + Send { v }";
        let syms = extract_symbols(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "baz");
    }
}
