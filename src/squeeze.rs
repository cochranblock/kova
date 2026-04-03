//! Squeeze: mine shell history, Claude Code sessions, and AI tool configs
//! for unaliased command patterns. Token economy autopilot.
//!
//! f393 = main entry, f394 = parse history, f395 = parse jsonl,
//! f396 = scan AI rules, f397 = parse aliases, f398 = format text, f399 = apply

use regex::Regex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// t177 = SqueezeConfig
pub struct t177 {
    pub project: PathBuf,
    pub remote: bool,
    pub top: usize,
    pub apply: bool,
    pub json: bool,
}

// Known node tokens: hostname → short token (from node_cmd.rs)
const NODE_MAP: &[(&str, &str)] = &[
    ("gd", "g"), ("lf", "l"), ("bt", "b"), ("st", "s"),
    ("kova-tunnel-god", "g"), ("kova-legion-forge", "l"),
    ("kova-thick-beast", "b"), ("kova-elite-support", "s"),
];
// IP last-octet → node token
const IP_NODE_MAP: &[(&str, &str)] = &[
    ("47", "g"), ("51", "g"),  // gd IPs
    ("50", "b"),               // bt IP
    ("45", "l"),               // lf IP
    ("53", "s"), ("52", "s"),  // st IPs
];

// Blocklist: never use these as alias names (shell builtins, common tools, confusing)
const ALIAS_BLOCKLIST: &[&str] = &[
    "c", "s", "l", "b", "g", "t", "r", "p", "x", "n", "e", "f", "d", "w", "m",
    "if", "do", "in", "fi", "cd", "ls", "rm", "cp", "mv", "sh", "su",
    "test", "true", "false", "echo", "read", "eval", "exec", "exit",
    "set", "env", "pwd", "let", "for", "while", "case", "then", "else",
    "alias", "export", "source", "return", "break", "continue",
    "ssh", "scp", "git", "cargo", "make", "curl", "grep", "find", "cat",
    "k", "ka", "ks", "kg", "kc", "kt", "kb", "kr", "km", "kp", "kx", "kn",
];

// Regex for lines that may contain secrets — strip from history
fn is_secret_line(line: &str) -> bool {
    let re = Regex::new(r"(?i)(TOKEN|SECRET|KEY|PASSWORD|PASS|API_KEY|CREDENTIALS|AUTH)[\s=:]").unwrap();
    re.is_match(line)
}

/// t178 = CommandEntry
struct t178 {
    cmd: String,
    #[allow(dead_code)]
    source: String,
}

/// t179 = AliasEntry
#[derive(Clone)]
struct t179 {
    name: String,
    expansion: String,
    #[allow(dead_code)]
    source: String,
}

/// t180 = Suggestion
pub struct t180 {
    pub rank: usize,
    pub pattern: String,
    pub freq: usize,
    pub tok_cost: usize,
    pub savings: usize,
    pub alias_name: String,
    pub alias_body: String,
}

/// t182 = RuleMention (command in rule file without alias)
pub struct t182r {
    pub file: String,
    pub command: String,
}

/// t182 = SqueezeReport
pub struct t182 {
    pub hist_sources: usize,
    pub jsonl_sources: usize,
    pub rule_sources: usize,
    pub existing_aliases: usize,
    pub suggestions: Vec<t180>,
    pub rule_mentions: Vec<t182r>,
}

// AI tool config paths to scan (relative to home or project)
const AI_TOOL_PATHS: &[(&str, &str)] = &[
    ("Claude Code", "CLAUDE.md"),
    ("Claude Code", ".claude/CLAUDE.md"),
    ("Cursor", ".cursorrules"),
    ("Codex", "CODEX.md"),
    ("Codex", "codex.md"),
    ("Gemini", "GEMINI.md"),
    ("Gemini", "gemini.md"),
    ("Aider", ".aider.conf.yml"),
    ("Aider", ".aider.chat.history.md"),
    ("Continue", ".continuerules"),
    ("Cline", ".clinerules"),
    ("Roo", ".roomodes"),
    ("Windsurf", ".windsurfrules"),
    ("Bolt", ".boltrules"),
    ("Copilot Agent", "AGENTS.md"),
    ("Copilot", ".github/copilot-instructions.md"),
];

// Cursor dirs scanned with glob
const CURSOR_RULE_GLOBS: &[&str] = &[
    ".cursor/rules/*.mdc",
    ".cursor/shared-rules/*.mdc",
];

/// f393 — main entry: orchestrate all sources, build report
pub fn f393(cfg: &t177) -> anyhow::Result<t182> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

    // 1. Parse existing aliases
    let aliases = f397(&home);
    let alias_expansions: HashMap<String, String> = aliases
        .iter()
        .map(|a| (normalize_cmd(&a.expansion), a.name.clone()))
        .collect();
    let alias_names: std::collections::HashSet<String> =
        aliases.iter().map(|a| a.name.clone()).collect();

    // 2. Collect commands
    let mut commands: Vec<t178> = Vec::new();
    let mut hist_sources = 0usize;
    let mut jsonl_sources = 0usize;

    // Local shell history
    for name in &[".zsh_history", ".bash_history"] {
        let path = home.join(name);
        if path.exists() {
            let cmds = f394(&path);
            if !cmds.is_empty() {
                hist_sources += 1;
                commands.extend(cmds);
            }
        }
    }

    // Remote node histories (optional)
    if cfg.remote {
        for node in &["gd", "lf", "bt", "st"] {
            let output = std::process::Command::new("ssh")
                .args(["-o", "ConnectTimeout=5", node,
                       "cat ~/.bash_history ~/.zsh_history 2>/dev/null"])
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let re_zsh = Regex::new(r"^: \d+:\d+;(.+)$").unwrap();
                    for line in text.lines() {
                        let cmd = if let Some(caps) = re_zsh.captures(line) {
                            caps.get(1).unwrap().as_str().to_string()
                        } else {
                            line.trim().to_string()
                        };
                        if !cmd.is_empty() && !is_secret_line(&cmd) {
                            commands.push(t178 {
                                cmd,
                                source: format!("ssh:{node}"),
                            });
                        }
                    }
                    hist_sources += 1;
                }
            }
        }
    }

    // Claude Code session logs (.jsonl)
    let jsonl_pattern = home
        .join(".claude/projects/*/*.jsonl")
        .to_string_lossy()
        .to_string();
    if let Ok(paths) = glob::glob(&jsonl_pattern) {
        for entry in paths.flatten() {
            let cmds = f395(&entry);
            if !cmds.is_empty() {
                jsonl_sources += 1;
                commands.extend(cmds);
            }
        }
    }

    // 3. Normalize and group
    let mut freq_map: HashMap<String, usize> = HashMap::new();
    let mut raw_map: HashMap<String, String> = HashMap::new(); // normalized → first raw example
    for c in &commands {
        let norm = normalize_cmd(&c.cmd);
        // Skip noise: short commands, internal callbacks, bare exports
        if norm.len() < 5
            || norm == "callback"
            || norm.starts_with("export PATH=")
            || norm.starts_with("export ANDROID")
            || norm == "ls" || norm == "cd" || norm == "exit" || norm == "echo"
            || norm.starts_with("PATH=")
        {
            continue;
        }
        *freq_map.entry(norm.clone()).or_default() += 1;
        raw_map.entry(norm).or_insert_with(|| c.cmd.clone());
    }

    // 4. Score and filter
    let mut scored: Vec<(String, String, usize, usize, usize)> = freq_map
        .iter()
        .filter(|(norm, _)| !alias_expansions.contains_key(norm.as_str()))
        .filter(|(_, freq)| **freq >= 3) // minimum 3 occurrences
        .map(|(norm, freq)| {
            let freq = *freq;
            let tok = norm.len() / 4 + 1;
            let save = freq * tok;
            let raw = raw_map.get(norm).cloned().unwrap_or_default();
            (norm.clone(), raw, freq, tok, save)
        })
        .collect();
    scored.sort_by(|a, b| b.4.cmp(&a.4));
    scored.truncate(cfg.top);

    // 5. Generate alias suggestions
    let mut suggestions = Vec::new();
    let mut used_names = alias_names.clone();

    for (i, (norm, raw, freq, tok, save)) in scored.iter().enumerate() {
        let (alias_name, alias_body) = generate_alias(&norm, &raw, &mut used_names);
        used_names.insert(alias_name.clone());
        suggestions.push(t180 {
            rank: i + 1,
            pattern: if raw.len() > 50 {
                format!("{}...", &raw[..47])
            } else {
                raw.clone()
            },
            freq: *freq,
            tok_cost: *tok,
            savings: *save,
            alias_name,
            alias_body,
        });
    }

    // 6. Scan AI rule files
    let rule_mentions = f396(&home, &cfg.project, &alias_expansions);
    let rule_sources = rule_mentions.len();

    Ok(t182 {
        hist_sources,
        jsonl_sources,
        rule_sources,
        existing_aliases: aliases.len(),
        suggestions,
        rule_mentions,
    })
}

/// f394 — parse shell history file (zsh extended + bash raw)
fn f394(path: &Path) -> Vec<t178> {
    let mut out = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return out;
    };
    let reader = BufReader::new(file);
    let re_zsh = Regex::new(r"^: \d+:\d+;(.+)$").unwrap();
    let source = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    for line in reader.lines().flatten() {
        let cmd = if let Some(caps) = re_zsh.captures(&line) {
            caps.get(1).unwrap().as_str().to_string()
        } else {
            line.trim().to_string()
        };
        if !cmd.is_empty() && !cmd.starts_with('#') && !is_secret_line(&cmd) {
            out.push(t178 {
                cmd,
                source: source.clone(),
            });
        }
    }
    out
}

/// f395 — stream-parse .jsonl for bash tool commands
fn f395(path: &Path) -> Vec<t178> {
    let mut out = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return out;
    };
    let reader = BufReader::new(file);
    // Match "command":"<value>" in JSON lines
    let re_cmd = Regex::new(r#""command"\s*:\s*"((?:[^"\\]|\\.)*)""#).unwrap();

    for line in reader.lines().flatten() {
        // Fast pre-filter: skip lines without "command"
        if !line.contains("\"command\"") {
            continue;
        }
        for caps in re_cmd.captures_iter(&line) {
            let raw = caps.get(1).unwrap().as_str();
            // Unescape JSON string basics
            let cmd = raw
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
            // Take first line only (multi-line commands)
            let first = cmd.lines().next().unwrap_or("").trim().to_string();
            if !first.is_empty() {
                out.push(t178 {
                    cmd: first,
                    source: "jsonl".to_string(),
                });
            }
        }
    }
    out
}

/// f396 — scan AI rule files for command patterns without aliases
fn f396(
    home: &Path,
    project: &Path,
    alias_map: &HashMap<String, String>,
) -> Vec<t182r> {
    let mut mentions = Vec::new();

    // Regex for backtick code blocks and $-prefixed lines
    let re_backtick = Regex::new(r"`([^`]{5,80})`").unwrap();
    let re_dollar = Regex::new(r"^\s*\$\s+(.+)$").unwrap();
    // Command-like pattern: starts with common command words
    let re_cmdlike =
        Regex::new(r"^(cargo|git|ssh|rsync|scp|curl|systemctl|kova|npm|docker|kubectl)\s")
            .unwrap();

    let mut scan = |label: &str, path: &Path| {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        for line in content.lines() {
            // Check backtick-wrapped commands
            for caps in re_backtick.captures_iter(line) {
                let inner = caps.get(1).unwrap().as_str().trim();
                if re_cmdlike.is_match(inner) {
                    let norm = normalize_cmd(inner);
                    if !alias_map.contains_key(&norm) {
                        mentions.push(t182r {
                            file: label.to_string(),
                            command: inner.to_string(),
                        });
                    }
                }
            }
            // Check $-prefixed lines
            if let Some(caps) = re_dollar.captures(line) {
                let cmd = caps.get(1).unwrap().as_str().trim();
                if re_cmdlike.is_match(cmd) {
                    let norm = normalize_cmd(cmd);
                    if !alias_map.contains_key(&norm) {
                        mentions.push(t182r {
                            file: label.to_string(),
                            command: cmd.to_string(),
                        });
                    }
                }
            }
        }
    };

    // Scan fixed paths (relative to home and project)
    for (tool, rel) in AI_TOOL_PATHS {
        let hp = home.join(rel);
        if hp.exists() {
            scan(&format!("{tool}:{rel}"), &hp);
        }
        let pp = project.join(rel);
        if pp.exists() && pp != hp {
            scan(&format!("{tool}:{rel}"), &pp);
        }
    }

    // Scan cursor rule dirs via glob
    for pattern in CURSOR_RULE_GLOBS {
        let full = home.join(pattern).to_string_lossy().to_string();
        if let Ok(paths) = glob::glob(&full) {
            for entry in paths.flatten() {
                let label = entry
                    .strip_prefix(home)
                    .unwrap_or(&entry)
                    .to_string_lossy()
                    .to_string();
                scan(&format!("Cursor:{label}"), &entry);
            }
        }
    }

    // Scan copilot command history
    let copilot_hist = home.join(".copilot/command-history-state.json");
    if copilot_hist.exists() {
        if let Ok(content) = std::fs::read_to_string(&copilot_hist) {
            let re = Regex::new(r#""command"\s*:\s*"([^"]+)""#).unwrap();
            for caps in re.captures_iter(&content) {
                let cmd = caps.get(1).unwrap().as_str();
                if re_cmdlike.is_match(cmd) {
                    let norm = normalize_cmd(cmd);
                    if !alias_map.contains_key(&norm) {
                        mentions.push(t182r {
                            file: "Copilot:command-history".to_string(),
                            command: cmd.to_string(),
                        });
                    }
                }
            }
        }
    }

    // Dedup
    let mut seen = std::collections::HashSet::new();
    mentions.retain(|m| seen.insert(format!("{}:{}", m.file, normalize_cmd(&m.command))));
    mentions
}

/// f397 — parse existing aliases from .kova-aliases and .zshrc
fn f397(home: &Path) -> Vec<t179> {
    let mut aliases = Vec::new();

    // Regex for alias lines: alias name='expansion' or alias name="expansion"
    let re_alias = Regex::new(r#"^alias\s+([a-zA-Z0-9_-]+)=['"](.*?)['"]"#).unwrap();
    // Regex for function definitions: name() { ... }
    let re_func = Regex::new(r"^([a-zA-Z0-9_]+)\(\)\s*\{(.+)\}").unwrap();
    // Multi-line function start
    let re_func_start = Regex::new(r"^([a-zA-Z0-9_]+)\(\)\s*\{").unwrap();

    let files = [
        home.join(".kova-aliases"),
        // Also check the kova repo copy
        home.join("kova/.kova-aliases"),
        home.join(".zshrc"),
    ];

    for path in &files {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let source = path.file_name().unwrap().to_string_lossy().to_string();

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();

            // alias x='...'
            if let Some(caps) = re_alias.captures(line) {
                aliases.push(t179 {
                    name: caps.get(1).unwrap().as_str().to_string(),
                    expansion: caps.get(2).unwrap().as_str().to_string(),
                    source: source.clone(),
                });
            }
            // one-line function: name() { body; }
            else if let Some(caps) = re_func.captures(line) {
                aliases.push(t179 {
                    name: caps.get(1).unwrap().as_str().to_string(),
                    expansion: caps.get(2).unwrap().as_str().trim().to_string(),
                    source: source.clone(),
                });
            }
            // multi-line function: name() {
            else if let Some(caps) = re_func_start.captures(line) {
                let name = caps.get(1).unwrap().as_str().to_string();
                // Collect until closing }
                let mut body = String::new();
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with('}') {
                    body.push_str(lines[i].trim());
                    body.push(' ');
                    i += 1;
                }
                aliases.push(t179 {
                    name,
                    expansion: body.trim().to_string(),
                    source: source.clone(),
                });
            }
            i += 1;
        }
    }
    aliases
}

/// f398 — format report as compressed text
pub fn f398(report: &t182) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "== squeeze ==\nsrc: {} hist {} jsonl | {} aliases loaded\n\n",
        report.hist_sources, report.jsonl_sources, report.existing_aliases
    ));

    if !report.suggestions.is_empty() {
        out.push_str("== top unaliased ==\n");
        out.push_str(&format!(
            "{:<3} {:<45} {:>4} {:>4} {:>5}  {}\n",
            "#", "pattern", "freq", "tok", "save", "alias"
        ));
        for s in &report.suggestions {
            let save_str = if s.savings >= 1000 {
                format!("{:.1}k", s.savings as f64 / 1000.0)
            } else {
                s.savings.to_string()
            };
            out.push_str(&format!(
                "{:<3} {:<45} {:>4} {:>4} {:>5}  {}\n",
                s.rank, s.pattern, s.freq, s.tok_cost, save_str, s.alias_name
            ));
        }
        out.push('\n');
    }

    if !report.rule_mentions.is_empty() {
        out.push_str("== rules w/o aliases ==\n");
        for m in &report.rule_mentions {
            let cmd_short = if m.command.len() > 60 {
                format!("{}...", &m.command[..57])
            } else {
                m.command.clone()
            };
            out.push_str(&format!("{}: {}\n", m.file, cmd_short));
        }
        out.push('\n');
    }

    if !report.suggestions.is_empty() {
        out.push_str("== suggested additions ==\n");
        for s in &report.suggestions {
            out.push_str(&format!("{}\n", s.alias_body));
        }
    }

    out
}

/// f399 — append suggestions to ~/.kova-aliases (with preview)
pub fn f399(report: &t182) -> anyhow::Result<()> {
    use std::io::Write;
    if report.suggestions.is_empty() {
        eprintln!("squeeze: nothing to apply");
        return Ok(());
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let path = home.join("kova/.kova-aliases");

    // Preview what will be written
    eprintln!("== applying {} aliases to {} ==", report.suggestions.len(), path.display());
    for s in &report.suggestions {
        eprintln!("  + {}", s.alias_body);
    }

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)?;

    writeln!(file, "\n# squeeze-generated aliases ({})",
        chrono_stub())?;
    for s in &report.suggestions {
        writeln!(file, "{}", s.alias_body)?;
    }
    eprintln!("squeeze: done. Run `source ~/.kova-aliases` or `ka` to reload.");
    Ok(())
}

// --- helpers ---

/// Normalize a command for grouping: strip whitespace, parametrize quoted strings
fn normalize_cmd(cmd: &str) -> String {
    let trimmed = cmd.trim();
    // Replace single-quoted strings with <arg>
    let re_sq = Regex::new(r"'[^']*'").unwrap();
    let s = re_sq.replace_all(trimmed, "'<arg>'").to_string();
    // Replace double-quoted strings with <arg>
    let re_dq = Regex::new(r#""[^"]*""#).unwrap();
    let s = re_dq.replace_all(&s, "\"<arg>\"").to_string();
    // Collapse whitespace
    let re_ws = Regex::new(r"\s+").unwrap();
    re_ws.replace_all(&s, " ").to_string()
}

/// Generate an alias name and body from a command pattern
fn generate_alias(
    _norm: &str,
    raw: &str,
    used: &std::collections::HashSet<String>,
) -> (String, String) {
    let re_ssh_simple = Regex::new(r"^ssh\s+([a-z]{2,4})\s").unwrap();
    let re_ssh_long = Regex::new(r"^ssh\s+(?:-o\s+\S+\s+)*(?:-i\s+\S+\s+)?(?:\S+@)?(\S+)\s").unwrap();
    let re_git = Regex::new(r"^git\s+(\w+)").unwrap();
    let re_cargo = Regex::new(r"^cargo\s+(\w+)").unwrap();
    let re_source = Regex::new(r"^source\s+(\S+)").unwrap();
    let re_export_path = Regex::new(r#"^export\s+PATH="#).unwrap();

    // SSH to short hostname: use node token map (gd→rg, lf→rl, bt→rb, st→rs)
    if let Some(caps) = re_ssh_simple.captures(raw) {
        let host = caps.get(1).unwrap().as_str();
        let token = NODE_MAP.iter()
            .find(|(h, _)| *h == host)
            .map(|(_, t)| *t)
            .unwrap_or(&host[..1.min(host.len())]);
        let name = format!("r{}", token); // r = remote + node token
        let name = dedup_name(&name, used);
        let body = format!("{}() {{ ssh {} \"$@\"; }}", name, host);
        return (name, body);
    }

    // SSH with long options (IP-based) — map IP to node token
    if let Some(caps) = re_ssh_long.captures(raw) {
        let target = caps.get(1).unwrap().as_str();
        let re_ip = Regex::new(r"\.(\d+)$").unwrap();
        let token = if let Some(ip_caps) = re_ip.captures(target) {
            let octet = ip_caps.get(1).unwrap().as_str();
            IP_NODE_MAP.iter()
                .find(|(o, _)| *o == octet)
                .map(|(_, t)| format!("r{}", t))
                .unwrap_or_else(|| format!("r{}", octet))
        } else {
            // Try hostname match
            NODE_MAP.iter()
                .find(|(h, _)| target.contains(*h))
                .map(|(_, t)| format!("r{}", t))
                .unwrap_or_else(|| format!("r{}", &target[..2.min(target.len())]))
        };
        let name = dedup_name(&token, used);
        let body = format!("{}() {{ ssh -o ConnectTimeout=5 {} \"$@\"; }}", name, target);
        return (name, body);
    }

    // Git pattern
    if let Some(caps) = re_git.captures(raw) {
        let sub = caps.get(1).unwrap().as_str();
        let name = format!("g{}", &sub[..sub.len().min(2)]);
        let name = dedup_name(&name, used);
        let cmd = if sub == "push" {
            format!("alias {}='git push 2>&1 | tail -1'", name)
        } else if sub == "commit" {
            format!("alias {}='git commit -m'", name)
        } else {
            format!("alias {}='git {}'", name, sub)
        };
        return (name, cmd);
    }

    // Cargo pattern
    if let Some(caps) = re_cargo.captures(raw) {
        let sub = caps.get(1).unwrap().as_str();
        let name = format!("cx{}", &sub[..sub.len().min(2)]);
        let name = dedup_name(&name, used);
        let body = format!("alias {}='cargo {}'", name, sub);
        return (name, body);
    }

    // source ~/.foo
    if let Some(caps) = re_source.captures(raw) {
        let file = caps.get(1).unwrap().as_str();
        let short = Path::new(file).file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("src");
        let name = format!("s{}", &short[..short.len().min(3)]);
        let name = dedup_name(&name, used);
        let body = format!("alias {}='{}'", name, raw.replace('\'', "'\\''"));
        return (name, body);
    }

    // export PATH=... chains — skip, too context-specific
    if re_export_path.is_match(raw) {
        let name = "xp".to_string();
        let name = dedup_name(&name, used);
        let body = format!("alias {}='{}'", name, raw.replace('\'', "'\\''"));
        return (name, body);
    }

    // Generic: first letter of each alphanumeric word, max 4 chars
    let re_word = Regex::new(r"[a-zA-Z]+").unwrap();
    let name: String = re_word.find_iter(raw)
        .take(4)
        .filter_map(|m| m.as_str().chars().next())
        .collect::<String>()
        .to_lowercase();
    let name = if name.is_empty() || name.len() < 2 {
        "sq0".to_string()
    } else {
        name
    };
    let name = dedup_name(&name, used);

    let body = format!("alias {}='{}'", name, raw.replace('\'', "'\\''"));
    (name, body)
}

fn dedup_name(base: &str, used: &std::collections::HashSet<String>) -> String {
    let blocked = ALIAS_BLOCKLIST.contains(&base);
    if !blocked && !used.contains(base) {
        return base.to_string();
    }
    // If blocklisted or taken, start numbering from 2
    for i in 2..=99 {
        let candidate = format!("{}{}", base, i);
        if !ALIAS_BLOCKLIST.contains(&candidate.as_str()) && !used.contains(&candidate) {
            return candidate;
        }
    }
    format!("{}_sq", base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Write;

    // ── normalize_cmd ──────────────────────────────────────

    #[test]
    fn normalize_cmd_collapses_whitespace() {
        assert_eq!(normalize_cmd("  cargo   build  "), "cargo build");
    }

    #[test]
    fn normalize_cmd_redacts_single_quotes() {
        let out = normalize_cmd("git commit -m 'fix the bug'");
        assert!(out.contains("<arg>"), "got: {}", out);
    }

    #[test]
    fn normalize_cmd_redacts_double_quotes() {
        let out = normalize_cmd(r#"echo "hello world""#);
        assert!(out.contains("<arg>"), "got: {}", out);
    }

    #[test]
    fn normalize_cmd_passthrough_plain() {
        assert_eq!(normalize_cmd("cargo check"), "cargo check");
    }

    // ── is_secret_line ────────────────────────────────────

    #[test]
    fn is_secret_line_detects_token() {
        assert!(is_secret_line("export TOKEN=abc123"));
        assert!(is_secret_line("API_KEY=xyz"));
        assert!(is_secret_line("PASSWORD: hunter2"));
    }

    #[test]
    fn is_secret_line_clean_line() {
        assert!(!is_secret_line("cargo build -p kova"));
        assert!(!is_secret_line("git status"));
    }

    #[test]
    fn is_secret_line_case_insensitive() {
        assert!(is_secret_line("secret=value"));
        assert!(is_secret_line("credentials: whatever"));
    }

    // ── dedup_name ────────────────────────────────────────

    #[test]
    fn dedup_name_returns_base_when_available() {
        let used: HashSet<String> = HashSet::new();
        assert_eq!(dedup_name("zz", &used), "zz");
    }

    #[test]
    fn dedup_name_increments_when_taken() {
        let mut used: HashSet<String> = HashSet::new();
        used.insert("zz".to_string());
        assert_eq!(dedup_name("zz", &used), "zz2");
    }

    #[test]
    fn dedup_name_skips_blocklist() {
        // Single-char names like "g" are on the blocklist
        let used: HashSet<String> = HashSet::new();
        let out = dedup_name("g", &used);
        assert_ne!(out, "g", "blocked name should be renamed");
    }

    #[test]
    fn dedup_name_continues_past_taken_numbers() {
        let mut used: HashSet<String> = HashSet::new();
        used.insert("zz2".to_string());
        used.insert("zz3".to_string());
        let out = dedup_name("zz", &used);
        // zz is free (not in used, not blocked)
        assert_eq!(out, "zz");
    }

    // ── generate_alias ────────────────────────────────────

    #[test]
    fn generate_alias_ssh_simple() {
        let mut used = HashSet::new();
        let (name, body) = generate_alias("ssh gd somecmd", "ssh gd somecmd", &mut used);
        // Name should use node token "g" for "gd"
        assert!(name.starts_with('r'), "got name: {}", name);
        assert!(body.contains("ssh gd"), "got body: {}", body);
    }

    #[test]
    fn generate_alias_git_pattern() {
        let mut used = HashSet::new();
        let (name, body) = generate_alias("git status", "git status", &mut used);
        assert!(name.starts_with('g'), "got name: {}", name);
        assert!(body.contains("git"), "got body: {}", body);
    }

    #[test]
    fn generate_alias_cargo_pattern() {
        let mut used = HashSet::new();
        let (name, body) = generate_alias("cargo build", "cargo build", &mut used);
        assert!(name.starts_with("cx"), "got name: {}", name);
        assert!(body.contains("cargo build"), "got body: {}", body);
    }

    #[test]
    fn generate_alias_generic_fallback() {
        let mut used = HashSet::new();
        let (name, body) = generate_alias("kubectl apply -f foo.yaml", "kubectl apply -f foo.yaml", &mut used);
        // Generic: first letters of words
        assert!(!name.is_empty());
        assert!(body.contains("kubectl"), "got body: {}", body);
    }

    // ── f394 (parse_history) ─────────────────────────────

    #[test]
    fn f394_parses_zsh_extended_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".zsh_history");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, ": 1700000000:0;cargo build -p kova").unwrap();
            writeln!(f, ": 1700000001:0;git status").unwrap();
        }
        let cmds = f394(&path);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].cmd, "cargo build -p kova");
        assert_eq!(cmds[1].cmd, "git status");
    }

    #[test]
    fn f394_parses_bash_raw_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".bash_history");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "cargo check").unwrap();
            writeln!(f, "ls -la").unwrap();
        }
        let cmds = f394(&path);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].cmd, "cargo check");
    }

    #[test]
    fn f394_strips_secret_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".bash_history");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "export TOKEN=abc").unwrap();
            writeln!(f, "cargo build").unwrap();
        }
        let cmds = f394(&path);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].cmd, "cargo build");
    }

    #[test]
    fn f394_skips_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".zsh_history");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "# This is a comment").unwrap();
            writeln!(f, "cargo test").unwrap();
        }
        let cmds = f394(&path);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].cmd, "cargo test");
    }

    // ── f395 (parse_jsonl) ────────────────────────────────

    #[test]
    fn f395_extracts_bash_command_from_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"tool_use","name":"Bash","input":{{"command":"cargo build -p kova"}}}}"#).unwrap();
        }
        let cmds = f395(&path);
        // The regex matches "command":"<value>" anywhere in the line
        assert!(!cmds.is_empty(), "should extract at least one command");
        assert!(cmds.iter().any(|c| c.cmd.contains("cargo build")));
    }

    #[test]
    fn f395_skips_lines_without_command() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"message","content":"hello"}}"#).unwrap();
        }
        let cmds = f395(&path);
        assert!(cmds.is_empty());
    }

    // ── f398 (format_report) ─────────────────────────────

    #[test]
    fn f398_empty_report() {
        let report = t182 {
            hist_sources: 1,
            jsonl_sources: 2,
            rule_sources: 0,
            existing_aliases: 10,
            suggestions: vec![],
            rule_mentions: vec![],
        };
        let out = f398(&report);
        assert!(out.contains("squeeze"));
        assert!(out.contains("1 hist"));
        assert!(out.contains("2 jsonl"));
        assert!(out.contains("10"));
    }

    #[test]
    fn f398_with_suggestion() {
        let report = t182 {
            hist_sources: 1,
            jsonl_sources: 0,
            rule_sources: 0,
            existing_aliases: 5,
            suggestions: vec![t180 {
                rank: 1,
                pattern: "cargo build -p kova".to_string(),
                freq: 15,
                tok_cost: 5,
                savings: 75,
                alias_name: "cxbu".to_string(),
                alias_body: "alias cxbu='cargo build'".to_string(),
            }],
            rule_mentions: vec![],
        };
        let out = f398(&report);
        assert!(out.contains("top unaliased"));
        assert!(out.contains("cxbu"));
        assert!(out.contains("suggested additions"));
    }

    #[test]
    fn f398_savings_over_1k_uses_k_suffix() {
        let report = t182 {
            hist_sources: 1,
            jsonl_sources: 0,
            rule_sources: 0,
            existing_aliases: 0,
            suggestions: vec![t180 {
                rank: 1,
                pattern: "cargo build".to_string(),
                freq: 1000,
                tok_cost: 2,
                savings: 2000,
                alias_name: "cb".to_string(),
                alias_body: "alias cb='cargo build'".to_string(),
            }],
            rule_mentions: vec![],
        };
        let out = f398(&report);
        assert!(out.contains("2.0k"), "got: {}", out);
    }
}

fn chrono_stub() -> String {
    // Simple date without chrono dep
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("epoch:{}", epoch)
}
