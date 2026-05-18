//! Interactive REPL. Kova's Claude Code replacement.
//! f137=repl_run, f138=repl_stream_print, f139=repl_build_system_prompt.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

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

    // Resolve model. Mock mode (KOVA_INFERENCE=mock) accepts a synthetic
    // path that doesn't need to exist on disk — used by end-to-end agent loop
    // tests in exopack/agent_loop_tests.rs.
    let mock_mode = std::env::var("KOVA_INFERENCE").as_deref() == Ok("mock");
    let model_path = if mock_mode {
        crate::config::inference_model_path()
            .unwrap_or_else(|| std::path::PathBuf::from("/dev/null/mock"))
    } else {
        let p = crate::config::inference_model_path()
            .ok_or_else(|| anyhow::anyhow!("No model found. Run: kova model install"))?;
        if !p.exists() {
            anyhow::bail!(
                "Model not found at {}. Run: kova model install",
                p.display()
            );
        }
        p
    };

    let system_prompt = f139(&project_dir);
    let max_iterations = crate::config::orchestration_max_fix_retries() + 20;

    // Subatomic pyramid (T1) — preprocessing classifier bank. Verified once at
    // startup; ~9KB embedded, BLAKE3-checked. Telemetry-only for now (BACKLOG #3).
    let starter_nb = crate::nanobyte::starter().ok();

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

        // Subatomic T1: classify input type for routing hint.
        if let Some(nb) = starter_nb.as_ref() {
            if let Ok((class_idx, conf)) = nb.infer("code_vs_english", input) {
                let label = if class_idx == 1 { "code" } else { "english" };
                eprintln!("\x1b[90m[input: {label}, {conf:.2}]\x1b[0m");
            }
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

        // Subatomic T1: slop detection on output.
        if let Some(nb) = starter_nb.as_ref() {
            if let Ok((class_idx, conf)) = nb.infer("slop_detector", &response) {
                if class_idx == 1 && conf > 0.65 {
                    eprintln!("\x1b[33m[slop: {conf:.2} — review output]\x1b[0m");
                }
            }
        }

        if let Ok(store) = crate::storage::t12::f39() {
            let _ = crate::context::f73(&store, "user", input);
            let _ = crate::context::f73(&store, "assistant", &response);

            // Subatomic T1 telemetry — raw text + classification bank stored for retraining.
            if let Some(nb) = starter_nb.as_ref() {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let inputs = crate::nanobyte::classify_with_starters(nb, input);
                let outputs = crate::nanobyte::classify_with_starters(nb, &response);
                let _ = store.f40(format!("tele/{ts}/i").as_bytes(), &inputs);
                let _ = store.f40(format!("tele/{ts}/o").as_bytes(), &outputs);
                let _ = store.f40(format!("tele/{ts}/raw_i").as_bytes(), &input);
                let _ = store.f40(format!("tele/{ts}/raw_o").as_bytes(), &response);
            }
        }

        eprintln!();
    }

    Ok(())
}