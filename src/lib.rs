//! Kova — augment engine. Core lib for GUI + serve.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

pub mod backlog;
pub mod cargo;
pub mod codegen;
pub mod compute;
pub mod config;
pub mod context;
pub mod cursor_prompts;
pub mod error;
pub mod intent;
pub mod kernel;
pub mod nanobyte;
pub mod plan;
pub mod storage;
pub mod surface;
pub mod swarm;
pub mod trace;

pub use backlog::f25;
pub use compute::{t6, t7};
pub use config::{
    f78, f94, f95, f97, f98, f99, f100, f101, f102, f103, f104, f105, f106, f107, f108, f109,
    f110, f207, f208, f209, f210, f211, f212, f213, f214, f215, f216, f217, f218, f219, f220,
    all_build_presets, backlog_path, bind_addr, bootstrap, cursor_prompts_enabled, default_project,
    discover_projects, home, infer_preset_name, inference_model_path, kova_dir,
    load_build_preset, load_prompt, models_dir, orchestration_max_fix_retries,
    orchestration_router_resident, orchestration_run_clippy,
    orchestration_specialist_idle_unload_secs, prompts_dir, sled_path, workspace_root,
    BuildPreset, ModelRole,
    code_gen_structured, default_model, fast_localhost, hive_local_base, hive_shared_base,
    model_cache_size, model_idle_unload_secs, ollama_url, router_structured,
};
pub use context::{f73, f74, t91};
/// Backward compat alias.
pub type Message = t91;
pub use backlog::{f293, t8, t9};
pub use intent::{f62, f325, t0, t1, t2};
pub use plan::{t3, t4, t5};
#[cfg(feature = "inference")]
pub use router::{f79, T94};

pub use error::{T176, T176Result};
pub use kernel::T208;

#[cfg(feature = "inference")]
pub mod academy;
#[cfg(feature = "inference")]
pub mod context_loader;
#[cfg(feature = "inference")]
pub mod inference;
#[cfg(feature = "tui")]
pub mod tui;
#[cfg(feature = "serve")]
pub mod output;
#[cfg(feature = "inference")]
pub mod pipeline;
pub mod recent_changes;
#[allow(non_camel_case_types)]
pub mod squeeze;
#[cfg(feature = "inference")]
pub mod router;
#[cfg(feature = "serve")]
pub mod serve;

#[cfg(feature = "autopilot")]
pub mod autopilot;

pub mod browser;

#[cfg(any(
    feature = "screenshot", feature = "mock", feature = "interface",
    feature = "video", feature = "demo", feature = "baked_demo",
    feature = "triple_sims", feature = "devtools", feature = "standards_check",
    feature = "checkpoint", feature = "compaction", feature = "dual_mode",
    feature = "perm_gate", feature = "harvest", feature = "ats_fixtures",
    feature = "cc_features", feature = "training_mine_tests",
    feature = "tool_call_parser", feature = "router_spec",
    feature = "agent_loop_tests", feature = "router_training_tests"
))]
pub mod exopack;



#[cfg(feature = "inference")]
pub mod agent_loop;
pub mod c2;
pub mod cargo_cmd;
pub mod context_mgr;
#[cfg(feature = "inference")]
pub mod cluster;
pub mod elicitor;
#[cfg(feature = "inference")]
pub mod factory;
#[cfg(feature = "inference")]
pub mod gauntlet;
pub mod git_cmd;
pub mod gpu_sched;
pub mod job_queue;
pub mod inspect;
#[cfg(feature = "inference")]
pub mod micro;
#[cfg(feature = "inference")]
pub mod moe;
pub mod node_cmd;
#[cfg(feature = "inference")]
pub mod codegen_moe;
#[cfg(feature = "rag")]
pub mod rag;
#[cfg(feature = "inference")]
pub mod repl;
pub mod ci;
#[cfg(feature = "inference")]
pub mod feedback;
pub mod mcp;
#[cfg(feature = "inference")]
pub mod providers;
#[cfg(feature = "inference")]
pub mod review;
pub mod ssh_ca;
pub mod syntax;
pub mod tools;
pub mod tokenization;
pub mod training_data;
pub mod training_mine;

#[cfg(test)]
mod test_utils;

/// f25 alias for integration tests.
pub fn load_backlog(p: &std::path::Path) -> anyhow::Result<t9> {
    f25(p)
}

/// f90=f315. Deploy quality gate: clippy, TRIPLE SIMS, release build, smoke, baked demo.
#[cfg(feature = "tests")]
pub fn f315() -> anyhow::Result<()> {
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let project = Path::new(env!("CARGO_MANIFEST_DIR"));

    /// Run subprocess with timeout. Kills process if it exceeds limit.
    fn run_with_timeout(cmd: &mut Command, secs: u64) -> anyhow::Result<bool> {
        let mut child = cmd.spawn()?;
        let pid = child.id();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let out = child.wait();
            let _ = tx.send(out);
        });
        match rx.recv_timeout(Duration::from_secs(secs)) {
            Ok(Ok(status)) => Ok(status.success()),
            Ok(Err(e)) => Err(e.into()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                anyhow::bail!("timed out after {}s", secs)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(anyhow::anyhow!("child thread panicked"))
            }
        }
    }

    fn run_cargo(project: &Path, args: &[&str]) -> (bool, String) {
        crate::cargo::f306(project, args)
    }

    println!("kova test: cargo clippy...");
    let (ok, stderr) = run_cargo(project, &["clippy", "--", "-D", "warnings"]);
    if !ok {
        anyhow::bail!("clippy failed:\n{}", stderr);
    }

    println!("kova test: TRIPLE SIMS (3 simulations)...");
    let (ok, summary) = crate::exopack::triple_sims::f61(project, 3);
    print!("{}", summary);
    if !ok {
        anyhow::bail!("TRIPLE SIMS: one or more simulations failed");
    }

    println!("kova test: cargo test -p kova-engine 3x...");
    let project_buf = project.to_path_buf();
    let (ok, stderr) = crate::exopack::triple_sims::f61_with_args(
        &project_buf,
        3,
        &["-p", "kova-engine", "--features", "tests"],
    );
    if !ok {
        anyhow::bail!("{}", stderr);
    }

    // Cross-compile for aarch64-apple-darwin on macOS (the Mac Mini deployment target).
    // On Linux/other, build native release — no cross-toolchain available.
    let on_macos = cfg!(target_os = "macos");
    let target_dir = project.join("target");
    let mut build_cmd = Command::new("cargo");
    build_cmd
        .args(["build", "--release", "--features", "serve,inference"])
        .current_dir(project)
        .env("CARGO_TARGET_DIR", &target_dir)
        // Skip WASM thin-client build: not needed for smoke/logic tests and
        // the wasm-bindgen CLI version must exactly match the Rust dep version.
        .env("KOVA_SKIP_WASM", "1");
    if on_macos {
        build_cmd.args(["--target", "aarch64-apple-darwin"]);
        println!("kova test: cargo build --release --features serve,inference --target aarch64-apple-darwin...");
    } else {
        println!("kova test: cargo build --release --features serve,inference (native)...");
    }
    let (ok, stderr) = match build_cmd.output() {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stderr).into_owned();
            (o.status.success(), s)
        }
        Err(e) => (false, e.to_string()),
    };
    if !ok {
        anyhow::bail!("release build failed:\n{}", stderr);
    }

    println!("kova test: release smoke (bootstrap + c2 nodes)...");
    let tmp = tempfile::TempDir::new()?;
    let home = tmp.path().to_path_buf();
    let kova_bin = if on_macos {
        project.join("target/aarch64-apple-darwin/release/kova")
    } else {
        project.join("target/release/kova")
    }
    .with_extension(std::env::consts::EXE_EXTENSION);
    if !kova_bin.exists() {
        anyhow::bail!("release binary not found: {:?}", kova_bin);
    }
    let ok = run_with_timeout(
        Command::new(&kova_bin).env("HOME", &home).arg("bootstrap"),
        120,
    )?;
    if !ok {
        anyhow::bail!("kova bootstrap failed (run manually to see stderr)");
    }
    let c2_out = {
        let mut cmd = Command::new(&kova_bin);
        cmd.env("HOME", &home)
            .args(["c2", "nodes"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = cmd.spawn()?;
        let pid = child.id();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let out = child.wait_with_output();
            let _ = tx.send(out);
        });
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return Err(e.into()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                anyhow::bail!("kova c2 nodes timed out after 15s");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("kova c2 nodes: child thread panicked")
            }
        }
    };
    let stdout = String::from_utf8_lossy(&c2_out.stdout);
    if !stdout.contains("lf") {
        anyhow::bail!("kova c2 nodes: expected lf in output, got:\n{}", stdout);
    }

    println!("kova test: cc_features (Claude Code replication via MCP stdio)...");
    let (cc_ok, cc_report) = crate::exopack::cc_features::f406(&kova_bin);
    print!("{}", cc_report);
    if !cc_ok {
        anyhow::bail!("cc_features: one or more Claude Code feature scenarios failed");
    }

    println!("kova test: training_mine_tests (transcript miner suite)...");
    let (tm_ok, tm_report) = crate::exopack::training_mine_tests::f417();
    print!("{}", tm_report);
    if !tm_ok {
        anyhow::bail!("training_mine_tests: one or more miner scenarios failed");
    }

    println!("kova test: tool_call_parser (f140 LLM-output parser)...");
    let (pp_ok, pp_report) = crate::exopack::tool_call_parser::f418();
    print!("{}", pp_report);
    if !pp_ok {
        anyhow::bail!("tool_call_parser: one or more parser scenarios failed");
    }

    println!("kova test: router_spec (tier-1 tool_router contract)...");
    let (_rs_ok, rs_report) = crate::exopack::router_spec::f419();
    print!("{}", rs_report);
    // router_spec is informational-only until the classifier exists; do not bail.

    println!("kova test: agent_loop_tests (end-to-end via mock inference)...");
    let (al_ok, al_report) = crate::exopack::agent_loop_tests::f423(&kova_bin);
    print!("{}", al_report);
    if !al_ok {
        anyhow::bail!("agent_loop_tests: one or more agent loop scenarios failed");
    }

    println!("kova test: router_training_tests (tier-1 tool_router classifier)...");
    let (rt_ok, rt_report) = crate::exopack::router_training_tests::f428();
    print!("{}", rt_report);
    if !rt_ok {
        anyhow::bail!("router_training_tests: one or more router scenarios failed");
    }

    if std::env::var("KOVA_SKIP_BAKED_DEMO").is_ok() || std::env::var("KOVA_BAKED_DEMO").is_err() {
        println!("kova test: baked demo (skipped; set KOVA_BAKED_DEMO=1 to run)");
    } else {
        println!("kova test: baked demo (full intended usage, no user input)...");
        let port = 19402u16;
        let kova_bin = kova_bin.to_path_buf();
        let home = home.to_path_buf();
        std::thread::scope(|s| {
            let (tx, rx) = std::sync::mpsc::channel();
            s.spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| anyhow::anyhow!("{}", e));
                let result = match rt {
                    Ok(r) => r
                        .block_on(crate::exopack::baked_demo::f95(&kova_bin, &home, port))
                        .map_err(|e| anyhow::anyhow!("{}", e)),
                    Err(e) => Err(e),
                };
                let _ = tx.send(result);
            });
            rx.recv().unwrap()
        })?;
    }

    println!("kova test: all checks passed");
    Ok(())
}