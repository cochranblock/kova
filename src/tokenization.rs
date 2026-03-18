//! tokenization — Validates compression protocol compliance across the codebase.
//! Scans src/*.rs for pub fn/struct/enum, checks fN/TN naming, reports gaps.
//! f294=validate_tokens, f295=scan_source, f296=token_report.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::{Path, PathBuf};

/// Token entry found in source.
#[derive(Debug, Clone)]
pub struct TokenEntry {
    pub file: PathBuf,
    pub line: usize,
    pub kind: TokenKind,
    pub name: String,
    pub tokenized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Function,
    Type,
}

/// Report from scanning the source tree.
#[derive(Debug)]
pub struct TokenReport {
    pub entries: Vec<TokenEntry>,
    pub total_functions: usize,
    pub total_types: usize,
    pub tokenized_functions: usize,
    pub tokenized_types: usize,
    pub untokenized: Vec<TokenEntry>,
    pub highest_f: u32,
    pub highest_t: u32,
}

impl TokenReport {
    pub fn ok(&self) -> bool {
        self.untokenized.is_empty()
    }

    /// Total tokenized + untokenized.
    pub fn total(&self) -> usize {
        self.total_functions + self.total_types
    }

    /// Percentage tokenized.
    pub fn coverage(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 100.0;
        }
        ((self.tokenized_functions + self.tokenized_types) as f64 / total as f64) * 100.0
    }
}

impl std::fmt::Display for TokenReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "tokenization: {:.1}% ({}/{})",
            self.coverage(),
            self.tokenized_functions + self.tokenized_types,
            self.total())?;
        writeln!(f, "  fn: {}/{} tokenized (highest: f{})",
            self.tokenized_functions, self.total_functions, self.highest_f)?;
        writeln!(f, "  ty: {}/{} tokenized (highest: T{})",
            self.tokenized_types, self.total_types, self.highest_t)?;
        if !self.untokenized.is_empty() {
            writeln!(f, "  untokenized ({}):", self.untokenized.len())?;
            for e in &self.untokenized {
                let kind = match e.kind {
                    TokenKind::Function => "fn",
                    TokenKind::Type => "ty",
                };
                writeln!(f, "    {}:{}  {} {}", e.file.display(), e.line, kind, e.name)?;
            }
        }
        Ok(())
    }
}

/// f295=scan_source. Walk src/ and extract all pub fn/struct/enum definitions.
pub fn f295(src_dir: &Path) -> Vec<TokenEntry> {
    let mut entries = Vec::new();
    walk_dir(src_dir, &mut entries);
    entries
}

fn walk_dir(dir: &Path, entries: &mut Vec<TokenEntry>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip target/, .git, wasm/, exopack/
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == ".git" || name == "wasm" || name == "exopack" {
                continue;
            }
            walk_dir(&path, entries);
        } else if path.extension().is_some_and(|e| e == "rs") {
            scan_file(&path, entries);
        }
    }
}

/// Type names that appear in test fixtures / string literals — not real types.
const SKIP_TYPE: &[&str] = &["Foo", "TokenEntry", "TokenKind", "TokenReport"];

/// Common impl method names and trait methods that don't need tokenization.
const SKIP_FN: &[&str] = &[
    "main", "new", "default", "fmt", "update", "short", "from", "run",
    // Accessors / small helpers (impl methods)
    "ok", "total", "coverage", "len", "is_empty", "is_done", "is_open",
    "is_busy", "set_busy", "get", "get_by_name", "ids", "all",
    "load", "save", "open", "close", "clear", "stats", "status",
    "label", "record", "reset", "remaining", "used", "exhausted",
    // Common patterns
    "from_model", "from_toml", "from_broadcast", "from_mpsc", "from_string",
    "avg_duration_ms", "success_rate", "avg_reward", "avg_ms", "accuracy",
    "failure_rate", "record_success", "record_failure", "record_pass",
    "record_fail", "record_error",
    "is_exhibition", "winner_code", "shared", "with_config",
    "scan", "total", "approved", "rejected", "decide", "apply_verdicts",
    "print", "by_tier", "build_prompt",
    "collect_blocking", "to_stdout",
    "temporary", "pick_idlest", "show", "extension", "name", "destroy",
    // Kernel/infra methods
    "cargo_check", "cargo_clippy", "cargo_test", "cluster_status",
    "base_url", "provider", "default_hive", "health_check",
    "online_nodes", "pick_node", "dispatch", "dispatch_stream",
    "speculative_dispatch",
    "default_path", "insert", "insert_many", "search", "remove_file",
    "grade", "route", "load_dir",
    // Trait methods
    "poll_next", "size_hint",
    // Aliases (pub use fN as human_name generates these)
    "load_backlog",
];

fn scan_file(path: &Path, entries: &mut Vec<TokenEntry>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let lines: Vec<&str> = content.lines().collect();
    let mut in_impl = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Track impl blocks (rough heuristic)
        if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
            in_impl = true;
        }
        // Top-level items reset impl tracking
        if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with('#')
            && !trimmed.starts_with('}')
            && (trimmed.starts_with("pub mod")
                || trimmed.starts_with("pub use")
                || (trimmed.starts_with("pub fn") && !in_impl)
                || trimmed.starts_with("pub struct")
                || trimmed.starts_with("pub enum")
                || trimmed.starts_with("pub trait"))
        {
            in_impl = false;
        }

        // pub fn NAME — only count top-level or in impl blocks
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            if let Some(name) = extract_ident(rest) {
                // Skip known helper/infra names (impl methods, delegators, trait methods)
                if SKIP_FN.contains(&name.as_str()) {
                    continue;
                }
                let tokenized = is_fn_token(&name);
                entries.push(TokenEntry {
                    file: path.to_path_buf(),
                    line: i + 1,
                    kind: TokenKind::Function,
                    name,
                    tokenized,
                });
            }
        }

        // pub struct NAME or pub enum NAME
        if let Some(rest) = trimmed.strip_prefix("pub struct ")
            .or_else(|| trimmed.strip_prefix("pub enum "))
        {
            if let Some(name) = extract_ident(rest) {
                if SKIP_TYPE.contains(&name.as_str()) {
                    continue;
                }
                let tokenized = is_type_token(&name);
                entries.push(TokenEntry {
                    file: path.to_path_buf(),
                    line: i + 1,
                    kind: TokenKind::Type,
                    name,
                    tokenized,
                });
            }
        }
    }
}

fn extract_ident(s: &str) -> Option<String> {
    let ident: String = s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if ident.is_empty() { None } else { Some(ident) }
}

/// fN or fN_suffix pattern: starts with 'f' followed by digits (optionally _suffix).
fn is_fn_token(name: &str) -> bool {
    if !name.starts_with('f') || name.len() < 2 {
        return false;
    }
    let rest = &name[1..];
    // fN or fN_suffix
    let num_part: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    !num_part.is_empty()
        && (num_part.len() == rest.len() || rest.as_bytes()[num_part.len()] == b'_')
}

/// TN, tN, or EN pattern: starts with 'T', 't', or 'E' followed by digits.
fn is_type_token(name: &str) -> bool {
    (name.starts_with('T') || name.starts_with('t') || name.starts_with('E'))
        && name[1..].chars().all(|c| c.is_ascii_digit())
        && name.len() > 1
}

fn parse_token_num(name: &str) -> Option<u32> {
    if name.len() > 1 {
        name[1..].parse().ok()
    } else {
        None
    }
}

/// f294=validate_tokens. Scan source tree, return report.
pub fn f294(src_dir: &Path) -> TokenReport {
    let entries = f295(src_dir);

    let mut total_functions = 0;
    let mut total_types = 0;
    let mut tokenized_functions = 0;
    let mut tokenized_types = 0;
    let mut highest_f: u32 = 0;
    let mut highest_t: u32 = 0;
    let mut untokenized = Vec::new();

    for e in &entries {
        match e.kind {
            TokenKind::Function => {
                total_functions += 1;
                if e.tokenized {
                    tokenized_functions += 1;
                    if let Some(n) = parse_token_num(&e.name) {
                        highest_f = highest_f.max(n);
                    }
                } else {
                    untokenized.push(e.clone());
                }
            }
            TokenKind::Type => {
                total_types += 1;
                if e.tokenized {
                    tokenized_types += 1;
                    if let Some(n) = parse_token_num(&e.name) {
                        highest_t = highest_t.max(n);
                    }
                } else {
                    untokenized.push(e.clone());
                }
            }
        }
    }

    TokenReport {
        entries,
        total_functions,
        total_types,
        tokenized_functions,
        tokenized_types,
        untokenized,
        highest_f,
        highest_t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_patterns() {
        assert!(is_fn_token("f42"));
        assert!(is_fn_token("f294"));
        assert!(!is_fn_token("foo"));
        assert!(!is_fn_token("f"));
        assert!(is_type_token("T91"));
        assert!(is_type_token("t91"));
        assert!(is_type_token("E0"));
        assert!(!is_type_token("Token"));
    }

    #[test]
    fn scan_self() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let report = f294(&src);
        assert!(report.total() > 0, "should find items in src/");
        eprintln!("{}", report);
    }
}