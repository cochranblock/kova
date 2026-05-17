// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! tool_router — Tier-1 classifier mapping user prompts to kova MCP tool names.
//!
//! Architecture: trigram-hash featurizer → linear classifier (sub-100K params).
//! Wraps the generic `swarm::train` infrastructure with kova-router-specific
//! class ordering and a JSONL loader for mined training data (T217).
//!
//! Class indices are fixed by KOVA_ROUTER_TOOLS so a trained checkpoint stays
//! compatible across runs. Adding a tool means appending to the end of the
//! array — never reordering.
//!
//! f424=train_tool_router, f425=classify_tool, f426=load_mined_examples,
//! f427=default_router_path.

#![allow(non_camel_case_types)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::swarm::train::{f389, predict, Example, SubatomicConfig};

/// Stable class ordering for the tier-1 tool_router classifier. Index in this
/// array = class label. Append-only — never reorder; doing so invalidates
/// every previously trained checkpoint.
pub const KOVA_ROUTER_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "exec",
    "grep",
    "glob",
    "memory_write",
    "undo_edit",
    "todo_write",
    "agent",
    "ask_user_question",
    "web_fetch",
    "web_search",
    "enter_plan_mode",
    "exit_plan_mode",
];

/// Look up a kova tool's class index. Returns None for tools not in the
/// router's registered set (e.g., kova-only extras like `code_outline`).
pub fn tool_to_class(name: &str) -> Option<usize> {
    KOVA_ROUTER_TOOLS.iter().position(|t| *t == name)
}

/// Inverse of tool_to_class.
pub fn class_to_tool(idx: usize) -> Option<&'static str> {
    KOVA_ROUTER_TOOLS.get(idx).copied()
}

/// f427=default_router_path. Standard on-disk location for the trained model.
/// Resolves under HOME so each user's checkpoint stays isolated.
pub fn f427() -> PathBuf {
    crate::config::kova_dir().join("models").join("tool_router")
}

/// f426=load_mined_examples. Parse a JSONL file produced by training_mine::f414
/// (rows of T217). Returns Examples ready for the trainer:
///   - kova-mapped examples become (prompt, class_idx) pairs
///   - unmapped examples (tool_name_kova=None) are skipped
///   - examples whose mapped name isn't in KOVA_ROUTER_TOOLS are skipped
///
/// Returns (Vec<Example>, skipped_count). Skips are returned so the caller can
/// surface "data integrity" stats on training kickoff.
pub fn f426(jsonl_path: &Path) -> Result<(Vec<Example>, usize), String> {
    let file = fs::File::open(jsonl_path)
        .map_err(|e| format!("open {}: {}", jsonl_path.display(), e))?;
    let reader = BufReader::new(file);
    let mut examples = Vec::new();
    let mut skipped = 0usize;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let record: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let prompt = match record.get("prompt").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                skipped += 1;
                continue;
            }
        };
        let kova_name = match record.get("tool_name_kova").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                skipped += 1;
                continue;
            }
        };
        let label = match tool_to_class(kova_name) {
            Some(i) => i,
            None => {
                skipped += 1;
                continue;
            }
        };
        examples.push(Example {
            text: prompt,
            label,
        });
    }

    Ok((examples, skipped))
}

/// Training knobs for tool_router. Defaults chosen to converge quickly on
/// modest data sizes (low thousands of examples) without overfitting the
/// trigram-hash feature space.
pub struct RouterTrainConfig {
    pub feature_dim: usize,
    pub epochs: usize,
    pub lr: f64,
}

impl Default for RouterTrainConfig {
    fn default() -> Self {
        Self {
            feature_dim: 8192,
            epochs: 30,
            lr: 0.01,
        }
    }
}

/// f424=train_tool_router. Train the tier-1 router and save under `output_dir/
/// tool_router/` as the standard f389 checkpoint (weights.bin, bias.bin,
/// config.json with class_names from KOVA_ROUTER_TOOLS).
///
/// Wrapper over `swarm::train::f389` — sets the class ordering, validates the
/// examples are non-empty, and forwards.
pub fn f424(
    examples: &[Example],
    output_dir: &Path,
    cfg: &RouterTrainConfig,
) -> Result<PathBuf, String> {
    if examples.is_empty() {
        return Err("tool_router training: no examples".into());
    }
    let class_names: Vec<String> = KOVA_ROUTER_TOOLS.iter().map(|s| (*s).to_string()).collect();
    let train_cfg = SubatomicConfig {
        name: "tool_router".into(),
        num_classes: KOVA_ROUTER_TOOLS.len(),
        class_names,
        feature_dim: cfg.feature_dim,
        epochs: cfg.epochs,
        lr: cfg.lr,
    };
    f389(&train_cfg, examples, output_dir)
}

/// f425=classify_tool. Run a trained tool_router checkpoint on a prompt.
/// Returns (kova_tool_name, confidence). Wraps `swarm::train::predict`.
pub fn f425(model_dir: &Path, prompt: &str) -> Result<(String, f32), String> {
    let (_label, class_name, confidence) = predict(model_dir, prompt)?;
    Ok((class_name, confidence))
}

/// Curated (prompt, kova_tool) pairs that express clean intent→tool signal.
/// Real bt transcripts skew heavily toward Bash because Claude investigates
/// aggressively; the synthetic set teaches the classifier the lexical signals
/// for "show", "create", "change", "find", "list", "run", "undo", "remember",
/// "remind", "save", etc. Mix into training via `kova train-router
/// --mix-synthetic`.
///
/// Coverage includes every tool that has at least one router_spec entry plus
/// the rarer kova-only tools (undo_edit, memory_write, plan mode) that have
/// zero real-data examples.
pub const SYNTH_ROUTER_PAIRS: &[(&str, &str)] = &[
    // read_file
    ("show me src/lib.rs", "read_file"),
    ("read the README", "read_file"),
    ("open Cargo.toml", "read_file"),
    ("what's in main.rs", "read_file"),
    ("show me the config", "read_file"),
    ("let me see the source", "read_file"),
    ("display the contents of x.txt", "read_file"),
    ("print the file", "read_file"),
    ("cat the log", "read_file"),
    ("show the contents", "read_file"),
    // write_file
    ("create hello.txt with hi", "write_file"),
    ("write a new file called notes.md", "write_file"),
    ("make a config file", "write_file"),
    ("save this as out.txt", "write_file"),
    ("create the module file", "write_file"),
    ("write the script to disk", "write_file"),
    ("generate a new test file", "write_file"),
    ("scaffold a new module", "write_file"),
    // edit_file — targeted in-place mutation of existing file content
    // "change X to Y" is the primary verb signal; keep it strong and uncontested
    ("change foo to bar in lib.rs", "edit_file"),
    ("change the class name to Foo", "edit_file"),
    ("change the variable name to snake_case", "edit_file"),
    ("change the timeout value in config.rs", "edit_file"),
    ("replace the import line", "edit_file"),
    ("replace old_api with new_api in main.rs", "edit_file"),
    ("replace the placeholder with the real value", "edit_file"),
    ("rename old_name to new_name", "edit_file"),
    ("rename the function in the module", "edit_file"),
    ("fix the typo in the comment", "edit_file"),
    ("fix the off-by-one in the loop", "edit_file"),
    ("modify the function signature", "edit_file"),
    ("update the version string", "edit_file"),
    ("update the return type in the function", "edit_file"),
    ("edit the config value", "edit_file"),
    ("swap the parameter order", "edit_file"),
    ("patch the error message text", "edit_file"),
    // grep — content-pattern search inside files (no "in file.rs" tail; that
    // belongs to edit_file's "change X to Y in file" pattern)
    ("find all TODO comments", "grep"),
    ("find all TODO comments throughout the code", "grep"),
    ("find all FIXME markers", "grep"),
    ("find all unwrap calls", "grep"),
    ("find where this trait is implemented", "grep"),
    ("find the function that handles auth", "grep"),
    ("find all uses of fn main", "grep"),
    ("find all imports of serde", "grep"),
    ("find where X is called", "grep"),
    ("find the declaration of struct Foo", "grep"),
    ("search for the function name", "grep"),
    ("where is X defined", "grep"),
    ("look for FIXME markers", "grep"),
    ("grep for the pattern", "grep"),
    ("locate the import", "grep"),
    ("find references to foo", "grep"),
    ("search the code for X", "grep"),
    ("search the source for this symbol", "grep"),
    ("look for all panic! calls", "grep"),
    ("look for lines matching the error pattern", "grep"),
    // glob — file-name / path enumeration
    ("list all .rs files in src", "glob"),
    ("list all rust files", "glob"),
    ("list all markdown files", "glob"),
    ("what .md files exist", "glob"),
    ("enumerate files in tests", "glob"),
    ("list files matching pattern", "glob"),
    ("show all yaml files", "glob"),
    ("which json files are here", "glob"),
    ("list every .toml in the repo", "glob"),
    ("what files are in the src directory", "glob"),
    // exec — shell command execution
    ("run cargo test", "exec"),
    ("run the unit tests", "exec"),
    ("run cargo build --release", "exec"),
    ("run clippy", "exec"),
    ("run rustfmt on the project", "exec"),
    ("run make all", "exec"),
    ("run the deploy script", "exec"),
    ("run pytest", "exec"),
    ("run the binary with --help", "exec"),
    ("execute the build", "exec"),
    ("execute git status", "exec"),
    ("execute make clean", "exec"),
    ("compile it", "exec"),
    ("invoke make", "exec"),
    ("launch the binary", "exec"),
    ("run npm install", "exec"),
    ("kick off the deploy", "exec"),
    ("run the shell script", "exec"),
    ("run the integration tests", "exec"),
    // undo_edit (no real-data examples) — avoid "change" trigrams; those belong to edit_file
    ("undo my last edit", "undo_edit"),
    ("undo my last edit to lib.rs", "undo_edit"),
    ("undo the modification", "undo_edit"),
    ("undo the last write", "undo_edit"),
    ("revert lib.rs", "undo_edit"),
    ("revert that file", "undo_edit"),
    ("revert to the previous state", "undo_edit"),
    ("rollback the edit to lib.rs", "undo_edit"),
    ("rollback the last write", "undo_edit"),
    ("restore the previous version", "undo_edit"),
    ("restore the file to what it was", "undo_edit"),
    ("take back the last edit", "undo_edit"),
    // memory_write (no real-data examples)
    ("remember I prefer tabs", "memory_write"),
    ("save this note for later", "memory_write"),
    ("make a note about the bug", "memory_write"),
    ("commit this to memory", "memory_write"),
    ("remind me later about X", "memory_write"),
    ("note that the API uses oauth", "memory_write"),
    ("store this fact", "memory_write"),
    // todo_write
    ("add a task for the refactor", "todo_write"),
    ("mark the auth task complete", "todo_write"),
    ("create a todo for tests", "todo_write"),
    ("track this as a task", "todo_write"),
    ("update my task list", "todo_write"),
    // agent — subagent dispatch (no "run" prefix — use spawn/dispatch/delegate/use)
    ("spawn a subagent to investigate", "agent"),
    ("spawn a parallel agent for this", "agent"),
    ("dispatch a research agent", "agent"),
    ("dispatch a code-review subagent", "agent"),
    ("delegate this to a code reviewer agent", "agent"),
    ("delegate the investigation to a subagent", "agent"),
    ("use a subagent to review the PR", "agent"),
    ("use a parallel agent to analyze this", "agent"),
    ("launch a parallel agent", "agent"),
    ("have a subagent check the logs", "agent"),
    // ask_user_question
    ("ask the user which path to take", "ask_user_question"),
    ("present these options to the user", "ask_user_question"),
    ("clarify with a multiple-choice question", "ask_user_question"),
    ("which approach does the user want", "ask_user_question"),
    // web_fetch
    ("fetch the URL", "web_fetch"),
    ("download the page", "web_fetch"),
    ("get the response from https://example.com", "web_fetch"),
    ("retrieve the JSON from the endpoint", "web_fetch"),
    // web_search — online lookup (no "find" prefix — use search/look/check/google)
    ("search the web for rust async patterns", "web_search"),
    ("search online for tokio examples", "web_search"),
    ("search stackoverflow for this error", "web_search"),
    ("look up the latest serde docs online", "web_search"),
    ("look it up on stackoverflow", "web_search"),
    ("look up how to configure tokio", "web_search"),
    ("google for the crate documentation", "web_search"),
    ("google how to use async in rust", "web_search"),
    ("check the rust reference online", "web_search"),
    ("check crates.io for this crate", "web_search"),
    // enter_plan_mode (no real-data examples)
    ("enter plan mode", "enter_plan_mode"),
    ("start planning before any changes", "enter_plan_mode"),
    ("switch to read-only planning", "enter_plan_mode"),
    ("begin a plan-only investigation", "enter_plan_mode"),
    // exit_plan_mode (no real-data examples)
    ("exit plan mode with this plan", "exit_plan_mode"),
    ("present the plan and resume mutations", "exit_plan_mode"),
    ("leave planning mode", "exit_plan_mode"),
    ("finalize the plan and unblock writes", "exit_plan_mode"),
];

/// Build Examples from SYNTH_ROUTER_PAIRS. Used by both the CLI
/// (`--mix-synthetic`) and the router_training_tests exopack suite.
pub fn synth_examples() -> Vec<crate::swarm::train::Example> {
    SYNTH_ROUTER_PAIRS
        .iter()
        .filter_map(|(prompt, tool)| {
            tool_to_class(tool).map(|label| crate::swarm::train::Example {
                text: (*prompt).into(),
                label,
            })
        })
        .collect()
}
