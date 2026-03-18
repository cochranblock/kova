// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Interactive REPL. Kova's Claude Code replacement.
//! f137=repl_run, f138=repl_stream_print, f139=repl_build_system_prompt.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// f139=repl_build_system_prompt. Assemble system prompt from all sources.
pub fn f139(project_dir: &Path) -> String {
    let mut parts = Vec::new();

    // Base persona.
    let persona_path = crate::config::prompts_dir().join("persona.md");
    if let Ok(persona) = std::fs::read_to_string(&persona_path)
        && !persona.trim().is_empty()
    {
        parts.push(persona.trim().to_string());
    }

    // System prompt.
    let system_path = crate::config::prompts_dir().join("system.md");
    if let Ok(system) = std::fs::read_to_string(&system_path)
        && !system.trim().is_empty()
    {
        parts.push(system.trim().to_string());
    }

    // Cursor rules.
    if crate::config::cursor_prompts_enabled() {
        let cursor = crate::cursor_prompts::f111(project_dir);
        if !cursor.is_empty() {
            parts.push(cursor);
        }
    }

    // KOVA.md from project root.
    let kova_md = project_dir.join("KOVA.md");
    if let Ok(content) = std::fs::read_to_string(&kova_md)
        && !content.trim().is_empty()
    {
        parts.push(format!(
            "## Project Instructions (KOVA.md)\n{}",
            content.trim()
        ));
    }

    // Memory.
    let memory_path = crate::config::kova_dir().join("memory.md");
    if let Ok(content) = std::fs::read_to_string(&memory_path)
        && !content.trim().is_empty()
    {
        parts.push(format!("## Persistent Memory\n{}", content.trim()));
    }

    // Project context (Cargo.toml, recent changes).
    let ctx = crate::context_loader::f82_with_recent(project_dir, "", Some(30));
    let context_block = crate::context_loader::f112(&ctx);
    if !context_block.is_empty() {
        parts.push(context_block);
    }

    // Tool definitions.
    parts.push(crate::tools::f149());

    // Core instructions.
    parts.push("You are Kova, an augment engine. Execute, don't debate. Be concise. Use tools to read, edit, and test code. Follow project conventions.".to_string());

    parts.join("\n\n---\n\n")
}

/// f137=repl_run. Main REPL entry.
pub fn f137(project: Option<PathBuf>) -> anyhow::Result<()> {
    let project_dir = project
        .or_else(|| std::env::var("KOVA_PROJECT").ok().map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Resolve model.
    let model_path = crate::config::inference_model_path()
        .ok_or_else(|| anyhow::anyhow!("No model found. Run: kova model install"))?;

    if !model_path.exists() {
        anyhow::bail!(
            "Model not found at {}. Run: kova model install",
            model_path.display()
        );
    }

    let system_prompt = f139(&project_dir);
    let max_iterations = crate::config::orchestration_max_fix_retries() + 20;

    // Print banner.
    eprintln!("\x1b[36m╭─────────────────────────────╮\x1b[0m");
    eprintln!("\x1b[36m│\x1b[0m  \x1b[1mKova\x1b[0m — augment engine     \x1b[36m│\x1b[0m");
    eprintln!(
        "\x1b[36m│\x1b[0m  project: \x1b[33m{}\x1b[0m",
        project_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    eprintln!(
        "\x1b[36m│\x1b[0m  model: \x1b[33m{}\x1b[0m",
        model_path.file_name().unwrap_or_default().to_string_lossy()
    );
    eprintln!("\x1b[36m│\x1b[0m  /exit /clear /project <p>  \x1b[36m│\x1b[0m");
    eprintln!("\x1b[36m╰─────────────────────────────╯\x1b[0m");
    eprintln!();

    let stdin = io::stdin();

    loop {
        // Prompt.
        eprint!("\x1b[36mkova>\x1b[0m ");
        let _ = io::stderr().flush();

        // Read input.
        let mut input = String::new();
        let bytes = stdin.lock().read_line(&mut input)?;
        if bytes == 0 {
            // EOF.
            break;
        }
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Commands.
        if input == "/exit" || input == "/quit" || input == "/q" {
            break;
        }
        if input == "/clear" {
            eprintln!("\x1b[90m[conversation cleared]\x1b[0m");
            continue;
        }
        if input.starts_with("/project ") {
            let new_proj = input.strip_prefix("/project ").unwrap().trim();
            let p = PathBuf::from(new_proj);
            if p.exists() {
                eprintln!("\x1b[90m[project: {}]\x1b[0m", p.display());
            } else {
                eprintln!("\x1b[31m[project not found: {}]\x1b[0m", new_proj);
            }
            continue;
        }
        if input == "/tools" {
            for tool in crate::tools::TOOLS {
                eprintln!("  \x1b[1m{}\x1b[0m — {}", tool.name, tool.description);
            }
            continue;
        }

        // Run agent loop.
        eprintln!();
        let response = crate::agent_loop::f148(
            &model_path,
            &system_prompt,
            input,
            &project_dir,
            max_iterations,
        );

        // Store in sled for history persistence.
        let store_path = crate::config::sled_path();
        if let Ok(store) = crate::storage::t12::f39(&store_path) {
            let _ = crate::context::f73(&store, "user", input);
            let _ = crate::context::f73(&store, "assistant", &response);
        }

        eprintln!();
    }

    Ok(())
}
