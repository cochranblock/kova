// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! academy — MoE-powered autonomous development agent.
//! Takes a high-level task, breaks it into steps, uses IRONHIVE cluster to
//! generate code, wires it into the real codebase, tests, fixes, commits.
//!
//! This is the "do all the shit" module. Human direction → AI execution.

use crate::cluster::{Cluster, TaskKind};
use crate::ollama;
use crate::trace::LastTrace;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// f115=explain_trace. Explain a pipeline trace using IRONHIVE cluster.
/// Kept for serve.rs compat — now cluster-backed instead of Kalosm.
pub async fn explain_trace(trace: &LastTrace, _model_path: &Path) -> Result<String, String> {
    let cluster = Cluster::default_hive();

    let project = crate::config::default_project();
    let cursor = crate::cursor_prompts::load_cursor_prompts(&project);
    let ddi_note = "Fix loop loses effectiveness after 2-3 attempts (DDI). We cap retries to avoid worse output.";
    let system = if cursor.is_empty() {
        format!(
            "You are Recursive Academy. Explain this Kova execution trace. \
             What did the user want? What failed? Why? How would a user fix it? Be concise.\n\n{}",
            ddi_note
        )
    } else {
        format!(
            "You are Recursive Academy. Explain this Kova execution trace. \
             What did the user want? What failed? Why? How would a user fix it? Be concise.\n\n{}\n\n--- Cursor rules ---\n{}",
            ddi_note, cursor
        )
    };

    let user_msg = format!(
        "Intent: {}\nUser: {}\nStage: {}\nOutcome: {}\nRetries: {}\nStderr:\n```\n{}\n```\nChain: {}",
        trace.intent,
        trace.user_msg,
        trace.stage,
        trace.outcome,
        trace.retry_count,
        trace.stderr,
        trace.chain.join(" -> ")
    );

    cluster
        .dispatch(TaskKind::General, &system, &user_msg, Some(4096))
        .map(|(_, response)| response)
}

/// A planned step in the academy pipeline.
#[derive(Debug, Clone)]
pub struct Step {
    pub id: usize,
    pub action: StepAction,
    pub description: String,
    pub status: StepStatus,
    pub output: String,
}

#[derive(Debug, Clone)]
pub enum StepAction {
    /// Read files to understand context.
    ReadFiles(Vec<String>),
    /// Generate a new file via MoE.
    GenerateFile { path: String, prompt: String },
    /// Edit an existing file — insert or replace.
    EditFile { path: String, prompt: String },
    /// Run a shell command (cargo check, cargo test, etc).
    RunCommand(String),
    /// Commit and push changes.
    GitCommit(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed(String),
}

/// Academy configuration.
pub struct AcademyConfig {
    pub project_dir: PathBuf,
    pub num_experts: usize,
    pub max_fix_retries: u32,
    pub num_ctx: u32,
    pub auto_commit: bool,
    pub dry_run: bool,
}

impl Default for AcademyConfig {
    fn default() -> Self {
        Self {
            project_dir: std::env::current_dir().unwrap_or_default(),
            num_experts: 2,
            max_fix_retries: 3,
            num_ctx: 8192,
            auto_commit: true,
            dry_run: false,
        }
    }
}

/// Academy result.
#[derive(Debug)]
pub struct AcademyResult {
    pub steps: Vec<Step>,
    pub success: bool,
    pub files_changed: Vec<String>,
}

/// Run the academy: plan → execute → verify → commit.
pub fn run_academy(task: &str, config: AcademyConfig) -> AcademyResult {
    let cluster = Cluster::default_hive();

    println!("[academy] IRONHIVE Academy");
    println!("[academy] task: {}", task);
    println!("[academy] project: {}", config.project_dir.display());
    println!();

    // ── Phase 1: Gather context ──
    println!("[academy] phase 1: gathering context...");
    let context = gather_context(&config.project_dir);
    println!(
        "[academy] found {} source files, {} lines",
        context.file_count, context.total_lines
    );

    // ── Phase 2: Plan ──
    println!("[academy] phase 2: planning...");
    let plan = plan_task(&cluster, task, &context, &config);
    println!("[academy] plan: {} steps", plan.len());
    for (i, step) in plan.iter().enumerate() {
        println!("  {}. {}", i + 1, step.description);
    }
    println!();

    if config.dry_run {
        println!("[academy] dry run — stopping before execution");
        return AcademyResult {
            steps: plan,
            success: true,
            files_changed: vec![],
        };
    }

    // ── Phase 3: Execute ──
    println!("[academy] phase 3: executing...");
    let mut steps = plan;
    let mut files_changed: Vec<String> = Vec::new();

    for i in 0..steps.len() {
        steps[i].status = StepStatus::Running;
        println!(
            "\n[academy] step {}/{}: {}",
            i + 1,
            steps.len(),
            steps[i].description
        );

        let result = execute_step(&steps[i], &cluster, &context, &config, &files_changed);

        match result {
            Ok((output, changed)) => {
                steps[i].status = StepStatus::Done;
                steps[i].output = output.clone();
                for f in changed {
                    if !files_changed.contains(&f) {
                        files_changed.push(f);
                    }
                }
                println!("[academy] step {} done", i + 1);
            }
            Err(e) => {
                steps[i].status = StepStatus::Failed(e.clone());
                steps[i].output = e.clone();
                eprintln!("[academy] step {} failed: {}", i + 1, e);

                // Try to fix and retry once
                println!("[academy] attempting fix...");
                match fix_step(&steps[i], &e, &cluster, &context, &config) {
                    Ok((output, changed)) => {
                        steps[i].status = StepStatus::Done;
                        steps[i].output = output;
                        for f in changed {
                            if !files_changed.contains(&f) {
                                files_changed.push(f);
                            }
                        }
                        println!("[academy] fix applied, step {} done", i + 1);
                    }
                    Err(e2) => {
                        steps[i].status = StepStatus::Failed(e2.clone());
                        eprintln!("[academy] fix also failed: {}", e2);
                    }
                }
            }
        }
    }

    // ── Phase 4: Verify ──
    println!("\n[academy] phase 4: verify...");
    let verify_ok = verify_project(&config.project_dir);

    // ── Phase 5: Commit ──
    if config.auto_commit && verify_ok && !files_changed.is_empty() {
        println!("[academy] phase 5: commit...");
        let commit_msg = generate_commit_msg(&cluster, task, &files_changed, config.num_ctx);
        do_git_commit(&config.project_dir, &files_changed, &commit_msg);
    }

    let all_done = steps.iter().all(|s| s.status == StepStatus::Done);

    // ── Summary ──
    println!("\n── Academy Summary ──");
    for step in &steps {
        let icon = match &step.status {
            StepStatus::Done => "ok",
            StepStatus::Failed(_) => "XX",
            StepStatus::Running => "..",
            StepStatus::Pending => "--",
        };
        println!("  [{}] {}", icon, step.description);
    }
    println!(
        "\n[academy] {} files changed: {}",
        files_changed.len(),
        files_changed.join(", ")
    );
    println!(
        "[academy] {}",
        if all_done && verify_ok {
            "SUCCESS"
        } else {
            "PARTIAL — some steps failed"
        }
    );

    AcademyResult {
        steps,
        success: all_done && verify_ok,
        files_changed,
    }
}

// ── Context ──

struct ProjectContext {
    file_count: usize,
    total_lines: usize,
    /// Key files and their first few lines (for planning context).
    file_summaries: HashMap<String, String>,
    /// Module declarations from lib.rs.
    modules: Vec<String>,
    /// CLI commands from main.rs Cmd enum.
    commands: Vec<String>,
    /// Cargo.toml content.
    cargo_toml: String,
}

fn gather_context(project_dir: &Path) -> ProjectContext {
    let mut ctx = ProjectContext {
        file_count: 0,
        total_lines: 0,
        file_summaries: HashMap::new(),
        modules: Vec::new(),
        commands: Vec::new(),
        cargo_toml: String::new(),
    };

    // Read Cargo.toml
    if let Ok(content) = std::fs::read_to_string(project_dir.join("Cargo.toml")) {
        ctx.cargo_toml = content;
    }

    // Scan src/ for .rs files
    let src_dir = project_dir.join("src");
    if let Ok(entries) = glob_rs_files(&src_dir) {
        for path in entries {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel = path
                    .strip_prefix(project_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let lines = content.lines().count();
                ctx.total_lines += lines;
                ctx.file_count += 1;

                // Keep first 10 lines as summary
                let summary: String = content.lines().take(10).collect::<Vec<_>>().join("\n");
                ctx.file_summaries.insert(rel, summary);
            }
        }
    }

    // Parse lib.rs for module declarations
    if let Ok(lib_rs) = std::fs::read_to_string(src_dir.join("lib.rs")) {
        for line in lib_rs.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub mod ")
                && let Some(name) = trimmed
                    .strip_prefix("pub mod ")
                    .and_then(|s| s.strip_suffix(';'))
            {
                ctx.modules.push(name.to_string());
            }
        }
    }

    // Parse main.rs for Cmd enum variants
    if let Ok(main_rs) = std::fs::read_to_string(src_dir.join("main.rs")) {
        let mut in_cmd = false;
        for line in main_rs.lines() {
            let trimmed = line.trim();
            if trimmed.contains("enum Cmd") {
                in_cmd = true;
                continue;
            }
            if in_cmd {
                if trimmed == "}" {
                    break;
                }
                // Look for variant names like "Factory(FactoryArgs)," or "Bootstrap,"
                if trimmed.starts_with("///") || trimmed.starts_with("#[") {
                    continue;
                }
                if let Some(name) = trimmed
                    .split('(')
                    .next()
                    .or_else(|| trimmed.strip_suffix(','))
                {
                    let name = name.trim().trim_end_matches(',');
                    if !name.is_empty()
                        && name
                            .chars()
                            .next()
                            .map(|c| c.is_uppercase())
                            .unwrap_or(false)
                    {
                        ctx.commands.push(name.to_string());
                    }
                }
            }
        }
    }

    ctx
}

fn glob_rs_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(glob_rs_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    Ok(files)
}

// ── Planning ──

fn plan_task(
    cluster: &Cluster,
    task: &str,
    context: &ProjectContext,
    config: &AcademyConfig,
) -> Vec<Step> {
    let modules_list = context.modules.join(", ");
    let commands_list = context.commands.join(", ");
    let files_list: Vec<_> = context.file_summaries.keys().collect();

    let plan_prompt = format!(
        "You are a Rust development planner for the kova project.\n\
        \n\
        Project context:\n\
        - Modules: {}\n\
        - CLI commands: {}\n\
        - Source files: {}\n\
        - Cargo.toml deps: reqwest, serde, sled, tokio, clap, etc.\n\
        \n\
        Task: {}\n\
        \n\
        Create a step-by-step plan. Each step must be ONE of these types:\n\
        - READ: <file_path> — read a file for context\n\
        - GENERATE: <file_path> | <description of what to generate>\n\
        - EDIT: <file_path> | <description of what to change>\n\
        - RUN: <command> — shell command (cargo check, cargo test, etc)\n\
        - COMMIT: <message> — git commit\n\
        \n\
        Rules:\n\
        - Start by reading key files you'll need to modify\n\
        - Generate new files before editing existing ones to wire them in\n\
        - Always run cargo check after code changes\n\
        - Run cargo clippy and cargo test at the end\n\
        - End with a commit if changes pass\n\
        \n\
        Output each step on one line:\n\
        STEP: <TYPE>: <args>\n\
        \n\
        No explanation. Just the steps.",
        modules_list,
        commands_list,
        files_list
            .iter()
            .map(|f| f.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        task
    );

    let system = "You are a precise development planner. Output only STEP lines. No commentary.";

    let response = match cluster.dispatch(
        TaskKind::General,
        system,
        &plan_prompt,
        Some(config.num_ctx),
    ) {
        Ok((_, r)) => r,
        Err(e) => {
            eprintln!("[academy] planning failed: {}", e);
            // Fallback: minimal plan
            return vec![
                Step {
                    id: 0,
                    action: StepAction::ReadFiles(vec!["src/lib.rs".into(), "src/main.rs".into()]),
                    description: "read lib.rs and main.rs".into(),
                    status: StepStatus::Pending,
                    output: String::new(),
                },
                Step {
                    id: 1,
                    action: StepAction::RunCommand("cargo check -p kova".into()),
                    description: "cargo check".into(),
                    status: StepStatus::Pending,
                    output: String::new(),
                },
            ];
        }
    };

    parse_plan(&response)
}

fn parse_plan(response: &str) -> Vec<Step> {
    let mut steps = Vec::new();

    for line in response.lines() {
        let trimmed = line.trim();
        // Match "STEP: TYPE: args" or just "TYPE: args"
        let content = if let Some(rest) = trimmed.strip_prefix("STEP:") {
            rest.trim()
        } else if trimmed.starts_with("READ:")
            || trimmed.starts_with("GENERATE:")
            || trimmed.starts_with("EDIT:")
            || trimmed.starts_with("RUN:")
            || trimmed.starts_with("COMMIT:")
        {
            trimmed
        } else {
            // Try numbered format: "1. READ: ..."
            if let Some((_num, rest)) = trimmed.split_once('.') {
                let rest = rest.trim();
                if rest.starts_with("READ:")
                    || rest.starts_with("GENERATE:")
                    || rest.starts_with("EDIT:")
                    || rest.starts_with("RUN:")
                    || rest.starts_with("COMMIT:")
                {
                    rest
                } else {
                    continue;
                }
            } else {
                continue;
            }
        };

        let id = steps.len();

        if let Some(args) = content.strip_prefix("READ:") {
            let files: Vec<String> = args
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            steps.push(Step {
                id,
                description: format!("read {}", args.trim()),
                action: StepAction::ReadFiles(files),
                status: StepStatus::Pending,
                output: String::new(),
            });
        } else if let Some(args) = content.strip_prefix("GENERATE:") {
            let parts: Vec<&str> = args.splitn(2, '|').collect();
            if parts.len() == 2 {
                steps.push(Step {
                    id,
                    description: format!("generate {}", parts[0].trim()),
                    action: StepAction::GenerateFile {
                        path: parts[0].trim().to_string(),
                        prompt: parts[1].trim().to_string(),
                    },
                    status: StepStatus::Pending,
                    output: String::new(),
                });
            }
        } else if let Some(args) = content.strip_prefix("EDIT:") {
            let parts: Vec<&str> = args.splitn(2, '|').collect();
            if parts.len() == 2 {
                steps.push(Step {
                    id,
                    description: format!("edit {}", parts[0].trim()),
                    action: StepAction::EditFile {
                        path: parts[0].trim().to_string(),
                        prompt: parts[1].trim().to_string(),
                    },
                    status: StepStatus::Pending,
                    output: String::new(),
                });
            }
        } else if let Some(args) = content.strip_prefix("RUN:") {
            steps.push(Step {
                id,
                description: format!("run: {}", args.trim()),
                action: StepAction::RunCommand(args.trim().to_string()),
                status: StepStatus::Pending,
                output: String::new(),
            });
        } else if let Some(args) = content.strip_prefix("COMMIT:") {
            steps.push(Step {
                id,
                description: "git commit".to_string(),
                action: StepAction::GitCommit(args.trim().to_string()),
                status: StepStatus::Pending,
                output: String::new(),
            });
        }
    }

    steps
}

// ── Execution ──

fn execute_step(
    step: &Step,
    cluster: &Cluster,
    context: &ProjectContext,
    config: &AcademyConfig,
    _files_changed: &[String],
) -> Result<(String, Vec<String>), String> {
    match &step.action {
        StepAction::ReadFiles(files) => {
            let mut output = String::new();
            for file in files {
                let path = config.project_dir.join(file);
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let lines = content.lines().count();
                        output.push_str(&format!("{}:{} lines\n", file, lines));
                    }
                    Err(e) => {
                        output.push_str(&format!("{}: err: {}\n", file, e));
                    }
                }
            }
            Ok((output, vec![]))
        }

        StepAction::GenerateFile { path, prompt } => {
            generate_file(cluster, config, path, prompt, context)
        }

        StepAction::EditFile { path, prompt } => edit_file(cluster, config, path, prompt, context),

        StepAction::RunCommand(cmd) => {
            let output = Command::new("sh")
                .args(["-c", cmd])
                .current_dir(&config.project_dir)
                .output()
                .map_err(|e| e.to_string())?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);

            if output.status.success() {
                Ok((combined.to_string(), vec![]))
            } else {
                Err(combined.to_string())
            }
        }

        StepAction::GitCommit(msg) => {
            do_git_commit(&config.project_dir, &[], msg);
            Ok(("committed".into(), vec![]))
        }
    }
}

fn generate_file(
    cluster: &Cluster,
    config: &AcademyConfig,
    rel_path: &str,
    prompt: &str,
    context: &ProjectContext,
) -> Result<(String, Vec<String>), String> {
    let full_path = config.project_dir.join(rel_path);

    // Build context-aware system prompt
    let modules_str = context.modules.join(", ");
    let system = format!(
        "You are a Rust expert working on the kova project.\n\
        Existing modules: {}\n\
        Write clean, idiomatic Rust. Follow kova conventions:\n\
        - Comments: // style, first line is Unlicense header\n\
        - Compression: function names may use fN/tN tokens\n\
        - Error handling: anyhow for binaries, thiserror for libraries\n\
        - No slop words (utilize/leverage/optimize/comprehensive/robust/seamlessly)\n\
        Put all code in a single ```rust block.",
        modules_str
    );

    // MoE: fan-out to multiple experts if configured
    let code = if config.num_experts > 1 {
        moe_generate(cluster, &system, prompt, config)?
    } else {
        single_generate(cluster, &system, prompt, config)?
    };

    // Write file
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&full_path, &code).map_err(|e| e.to_string())?;

    println!("[academy] wrote {} ({} bytes)", rel_path, code.len());
    Ok((
        format!("generated {} bytes", code.len()),
        vec![rel_path.to_string()],
    ))
}

fn edit_file(
    cluster: &Cluster,
    config: &AcademyConfig,
    rel_path: &str,
    prompt: &str,
    _context: &ProjectContext,
) -> Result<(String, Vec<String>), String> {
    let full_path = config.project_dir.join(rel_path);
    let existing = std::fs::read_to_string(&full_path).map_err(|e| e.to_string())?;

    let system = format!(
        "You are editing an existing Rust file in the kova project.\n\
        The file is: {}\n\
        Follow kova conventions. No slop words.\n\
        \n\
        IMPORTANT: Output the COMPLETE modified file content in a ```rust block.\n\
        Do not output partial files or diffs — output the full file.",
        rel_path
    );

    let edit_prompt = format!(
        "{}\n\nCurrent file content:\n```rust\n{}\n```\n\n\
        Output the complete modified file in a ```rust block.",
        prompt, existing
    );

    let new_content = if config.num_experts > 1 {
        moe_generate(cluster, &system, &edit_prompt, config)?
    } else {
        single_generate(cluster, &system, &edit_prompt, config)?
    };

    // Validate the edit compiles before writing
    let tmp = tempfile::TempDir::new().map_err(|e| e.to_string())?;
    write_validation_project(tmp.path(), &new_content, rel_path);
    let (ok, stderr) = cargo_check_local(tmp.path());

    if !ok {
        // Try to fix
        let fix_prompt = format!(
            "Fix this Rust code. Error:\n```\n{}\n```\n\nCode:\n```rust\n{}\n```\n\n\
            Return the complete fixed file in a ```rust block.",
            truncate(&stderr, 1000),
            new_content
        );

        let fixed = single_generate(cluster, &system, &fix_prompt, config)?;
        std::fs::write(&full_path, &fixed).map_err(|e| e.to_string())?;
        println!("[academy] wrote {} (fixed)", rel_path);
        return Ok((
            format!("edited + fixed {} bytes", fixed.len()),
            vec![rel_path.to_string()],
        ));
    }

    std::fs::write(&full_path, &new_content).map_err(|e| e.to_string())?;
    println!("[academy] wrote {}", rel_path);
    Ok(("edited".into(), vec![rel_path.to_string()]))
}

/// MoE generation: fan-out to multiple nodes, compile-test, pick best.
fn moe_generate(
    cluster: &Cluster,
    system: &str,
    prompt: &str,
    config: &AcademyConfig,
) -> Result<String, String> {
    // Pick nodes
    let mut nodes: Vec<(&str, String)> = Vec::new();
    for node in &cluster.nodes {
        if nodes.len() >= config.num_experts {
            break;
        }
        if ollama::health(&node.base_url()) {
            nodes.push((&node.id, node.base_url()));
        }
    }

    if nodes.is_empty() {
        return Err("no online nodes".into());
    }

    // Fan-out
    let (tx, rx) = std::sync::mpsc::channel();
    let _handles: Vec<_> = nodes
        .iter()
        .map(|(node_id, base_url)| {
            let tx = tx.clone();
            let node_id = node_id.to_string();
            let base_url = base_url.clone();
            let system = system.to_string();
            let prompt = prompt.to_string();
            let num_ctx = config.num_ctx;

            let model = cluster
                .nodes
                .iter()
                .find(|n| n.id == node_id)
                .map(|n| n.model.clone())
                .unwrap_or_default();

            std::thread::spawn(move || {
                let result = ollama::generate(&base_url, &model, &system, &prompt, Some(num_ctx));
                let _ = tx.send((node_id, result));
            })
        })
        .collect();
    drop(tx);

    // Collect results, pick first that compiles
    let mut candidates: Vec<(String, String)> = Vec::new();
    for (node_id, result) in rx {
        if let Ok(response) = result {
            let code = extract_rust_block(&response).unwrap_or(response);
            println!("[academy] {} generated {} chars", node_id, code.len());
            candidates.push((node_id, code));
        }
    }

    if candidates.is_empty() {
        return Err("all experts failed".into());
    }

    // If only one candidate, return it
    if candidates.len() == 1 {
        return Ok(candidates.remove(0).1);
    }

    // Quick compile test each
    for (node_id, code) in &candidates {
        let tmp = tempfile::TempDir::new().map_err(|e| e.to_string())?;
        write_temp_crate(tmp.path(), code);
        let (ok, _) = cargo_check_local(tmp.path());
        if ok {
            println!("[academy] winner: {} (compiles)", node_id);
            return Ok(code.clone());
        }
    }

    // None compiled — return the first and let the caller's fix loop handle it
    Ok(candidates.remove(0).1)
}

/// Single-node generation.
fn single_generate(
    cluster: &Cluster,
    system: &str,
    prompt: &str,
    config: &AcademyConfig,
) -> Result<String, String> {
    let (_, response) =
        cluster.dispatch(TaskKind::CodeGen, system, prompt, Some(config.num_ctx))?;
    Ok(extract_rust_block(&response).unwrap_or(response))
}

/// Fix a failed step.
fn fix_step(
    step: &Step,
    error: &str,
    cluster: &Cluster,
    context: &ProjectContext,
    config: &AcademyConfig,
) -> Result<(String, Vec<String>), String> {
    match &step.action {
        StepAction::RunCommand(cmd) => {
            // If cargo check/clippy/test failed, we can't fix a command itself
            // But we can try to fix the code that caused the failure
            if cmd.contains("cargo check")
                || cmd.contains("cargo clippy")
                || cmd.contains("cargo test")
            {
                let fix_prompt = format!(
                    "The command `{}` failed with:\n```\n{}\n```\n\n\
                    What file needs to be fixed? Identify the file path and the fix needed.\n\
                    Reply with: FILE: <path>\nFIX: <description>",
                    cmd,
                    truncate(error, 1000)
                );

                let (_, response) = cluster
                    .dispatch(
                        TaskKind::General,
                        "Identify which file needs fixing from this error. Reply with FILE: and FIX: lines.",
                        &fix_prompt,
                        Some(config.num_ctx),
                    )?;

                // Parse file path from response
                let file_path = response
                    .lines()
                    .find(|l| l.starts_with("FILE:"))
                    .and_then(|l| l.strip_prefix("FILE:"))
                    .map(|s| s.trim().to_string());

                if let Some(file_path) = file_path {
                    let fix_desc = response
                        .lines()
                        .find(|l| l.starts_with("FIX:"))
                        .and_then(|l| l.strip_prefix("FIX:"))
                        .unwrap_or("fix the error")
                        .trim();

                    let edit_prompt = format!(
                        "Fix this error:\n```\n{}\n```\n\n{}",
                        truncate(error, 500),
                        fix_desc
                    );

                    return edit_file(cluster, config, &file_path, &edit_prompt, context);
                }
            }

            Err(format!("cannot auto-fix command: {}", cmd))
        }

        StepAction::GenerateFile { path, prompt } => {
            // Re-generate with error context
            let retry_prompt = format!(
                "{}\n\nPrevious attempt failed with:\n```\n{}\n```\n\nFix the issues.",
                prompt,
                truncate(error, 500)
            );
            let step_retry = Step {
                id: step.id,
                action: StepAction::GenerateFile {
                    path: path.clone(),
                    prompt: retry_prompt,
                },
                description: step.description.clone(),
                status: StepStatus::Pending,
                output: String::new(),
            };
            execute_step(&step_retry, cluster, context, config, &[])
        }

        _ => Err("no fix strategy for this step type".into()),
    }
}

// ── Verification ──

fn verify_project(project_dir: &Path) -> bool {
    println!("[academy] cargo check...");
    let check = Command::new("cargo")
        .args(["check", "-p", "kova", "--features", "serve"])
        .current_dir(project_dir)
        .output();

    match check {
        Ok(o) if o.status.success() => {
            println!("[academy] cargo check: ok");
            true
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("[academy] cargo check failed:\n{}", truncate(&stderr, 500));
            false
        }
        Err(e) => {
            eprintln!("[academy] cargo check error: {}", e);
            false
        }
    }
}

// ── Git ──

fn generate_commit_msg(cluster: &Cluster, task: &str, files: &[String], num_ctx: u32) -> String {
    let prompt = format!(
        "Write a git commit message for this change.\n\
        Task: {}\n\
        Files changed: {}\n\
        \n\
        Format: one subject line (imperative mood, <72 chars), blank line, body.\n\
        No explanation. Just the commit message.",
        task,
        files.join(", ")
    );

    match cluster.dispatch(
        TaskKind::General,
        "Write concise git commit messages. Subject line in imperative mood.",
        &prompt,
        Some(num_ctx.min(1024)),
    ) {
        Ok((_, msg)) => {
            let msg = msg.trim().to_string();
            if msg.is_empty() {
                format!("academy: {}", truncate(task, 60))
            } else {
                msg
            }
        }
        Err(_) => format!("academy: {}", truncate(task, 60)),
    }
}

fn do_git_commit(project_dir: &Path, files: &[String], msg: &str) {
    // Stage files
    if files.is_empty() {
        // Stage all changes
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(project_dir)
            .output();
    } else {
        for file in files {
            let _ = Command::new("git")
                .args(["add", file])
                .current_dir(project_dir)
                .output();
        }
    }

    // Commit
    let result = Command::new("git")
        .args(["commit", "-m", msg])
        .current_dir(project_dir)
        .output();

    match result {
        Ok(o) if o.status.success() => {
            println!("[academy] committed");
            // Push
            let _ = Command::new("git")
                .args(["push"])
                .current_dir(project_dir)
                .output();
            println!("[academy] pushed");
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("[academy] commit failed: {}", stderr.trim());
        }
        Err(e) => eprintln!("[academy] git error: {}", e),
    }
}

// ── Helpers ──

fn extract_rust_block(s: &str) -> Option<String> {
    let (start_tag, tag_len) = if let Some(pos) = s.find("```rust") {
        (pos, 7)
    } else if let Some(pos) = s.find("```\n") {
        (pos, 4)
    } else {
        return None;
    };
    let after = &s[start_tag + tag_len..];
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

fn write_temp_crate(dir: &Path, code: &str) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .ok();
    std::fs::create_dir_all(dir.join("src")).ok();
    std::fs::write(
        dir.join("src/lib.rs"),
        format!("#![allow(dead_code)]\n{}", code),
    )
    .ok();
}

fn write_validation_project(dir: &Path, code: &str, rel_path: &str) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .ok();
    std::fs::create_dir_all(dir.join("src")).ok();

    let file_name = if rel_path.contains("main") {
        "main.rs"
    } else {
        "lib.rs"
    };
    let content = if file_name == "lib.rs" {
        format!("#![allow(dead_code)]\n{}", code)
    } else {
        code.to_string()
    };
    std::fs::write(dir.join("src").join(file_name), content).ok();
}

fn cargo_check_local(dir: &Path) -> (bool, String) {
    match Command::new("cargo")
        .args(["check"])
        .current_dir(dir)
        .output()
    {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
