// Copyright (c) 2026 The Cochran Block. All rights reserved.
//! kova c2 — Tokenized orchestration. f18–f23 local or broadcast.
//! run_build: one-command sync + broadcast with parallel execution.

#![allow(non_camel_case_types)]

use clap::ValueEnum;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Copy, ValueEnum)]
pub enum Token {
    #[value(name = "f18")]
    F18,
    #[value(name = "f19")]
    F19,
    #[value(name = "f20")]
    F20,
    #[value(name = "f21")]
    F21,
    #[value(name = "f22")]
    F22,
    #[value(name = "f23")]
    F23,
}

impl Token {
    pub fn to_intent(self, release: bool) -> kova_core::t0 {
        match self {
            Token::F18 => kova_core::t0::f18(release),
            Token::F19 => kova_core::t0::f19(),
            Token::F20 => kova_core::t0::f20(),
            Token::F21 => kova_core::t0::f21(),
            Token::F22 => kova_core::t0::f22(),
            Token::F23 => kova_core::t0::f23(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Token::F18 => "f18",
            Token::F19 => "f19",
            Token::F20 => "f20",
            Token::F21 => "f21",
            Token::F22 => "f22",
            Token::F23 => "f23",
        }
    }

    pub fn is_local_only(&self) -> bool {
        matches!(self, Token::F21 | Token::F22 | Token::F23)
    }
}

pub fn default_nodes() -> Vec<&'static str> {
    vec!["lf", "gd", "bt", "st"]
}

pub fn resolve_project(project: Option<PathBuf>) -> PathBuf {
    project
        .or_else(|| std::env::var("KOVA_PROJECT").ok().map(PathBuf::from))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn is_under_hive_vault(p: &Path) -> bool {
    if let Ok(home) = std::env::var("HOME") {
        let hive_vault = format!("{}/hive-vault", home);
        return p.to_string_lossy().starts_with(&hive_vault);
    }
    false
}

fn to_worker_path(p: &Path) -> PathBuf {
    to_worker_path_impl(p, &crate::config::hive_shared_base())
}

fn to_worker_path_local(p: &Path) -> PathBuf {
    to_worker_path_impl(p, &crate::config::hive_local_base())
}

fn to_worker_path_impl(p: &Path, base: &str) -> PathBuf {
    let s = p.to_string_lossy();
    if let Ok(home) = std::env::var("HOME") {
        let hive_vault = format!("{}/hive-vault", home);
        if s.starts_with(&hive_vault) {
            let rest = s.strip_prefix(&hive_vault).unwrap_or("");
            return PathBuf::from(base).join(rest.trim_start_matches('/'));
        }
    }
    p.to_path_buf()
}

fn run_local(plan: &crate::plan::t3) -> anyhow::Result<()> {
    let exec = crate::compute::t6;
    let results = exec.f15(plan)?;
    for r in &results {
        let mark = if r.s11 { "✓" } else { "✗" };
        eprintln!("{} {} {}", mark, r.s10, if r.s13.is_empty() { "" } else { &r.s13 });
    }
    let all_ok = results.iter().all(|r| r.s11);
    if !all_ok {
        anyhow::bail!("One or more actions failed");
    }
    Ok(())
}

/// f119=kova_c2_run. CLI orchestration. Local or broadcast.
pub fn run_command(
    token: Token,
    project: Option<PathBuf>,
    broadcast: bool,
    release: bool,
    nodes_override: Option<String>,
    local: bool,
) -> anyhow::Result<()> {
    let project_path = resolve_project(project);
    let intent = token.to_intent(release);
    let approuter_dir = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("approuter"));
    let plan = crate::plan::t3::f14(&intent, project_path.clone(), approuter_dir);

    if plan.s3.is_empty() {
        eprintln!("No actions for {}", token.name());
        return Ok(());
    }

    if token.is_local_only() {
        run_local(&plan)
    } else if broadcast {
        if !is_under_hive_vault(&project_path) {
            anyhow::bail!(
                "Project must be under ~/hive-vault for broadcast.\n\
                 Run: ln -s ~ ~/hive-vault/projects/workspace (or equivalent)\n\
                 Then: kova c2 run {} --broadcast --project ~/hive-vault/projects/workspace/...",
                token.name()
            );
        }
        // Delegate to run_build for shared sync + parallel broadcast logic.
        let nodes: Vec<String> = if let Some(s) = nodes_override {
            s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
        } else {
            let hosts = crate::inspect::run_inspect();
            hosts
                .iter()
                .filter(|h| h.id != "c2-core" && !h.unreachable)
                .map(|h| h.id.clone())
                .collect()
        };
        let nodes_opt = if nodes.is_empty() {
            None
        } else {
            Some(nodes.join(","))
        };
        run_build_with_plan(plan, local, false, nodes_opt)
    } else {
        run_local(&plan)
    }
}

pub fn run_nodes() {
    for n in default_nodes() {
        println!("{}", n);
    }
}

/// f121=run_build. One-command sync + broadcast. Parallel execution.
pub fn run_build(
    broadcast: bool,
    release: bool,
    no_sync: bool,
    local: bool,
    nodes_override: Option<String>,
    project: Option<PathBuf>,
) -> anyhow::Result<()> {
    if !broadcast {
        anyhow::bail!("kova c2 build requires --broadcast. For local build, use: kova c2 run f20");
    }

    let project_path = resolve_project(project);
    if !is_under_hive_vault(&project_path) {
        anyhow::bail!(
            "Project must be under ~/hive-vault for broadcast.\n\
             Run: ln -s ~ ~/hive-vault/projects/workspace (or equivalent)\n\
             Then: kova c2 build --broadcast --project ~/hive-vault/projects/workspace/..."
        );
    }

    let intent = kova_core::t0::f18(release);
    let approuter_dir = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("approuter"));
    let plan = crate::plan::t3::f14(&intent, project_path.clone(), approuter_dir);
    run_build_with_plan(plan, local, no_sync, nodes_override)
        .map(|_| ())
}

/// Shared sync + broadcast. Used by run_build and run_command --broadcast.
fn run_build_with_plan(
    plan: crate::plan::t3,
    local: bool,
    force_skip_sync: bool,
    nodes_override: Option<String>,
) -> anyhow::Result<()> {
    let nodes: Vec<String> = if let Some(s) = nodes_override {
        s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
    } else {
        let hosts = crate::inspect::run_inspect();
        hosts
            .iter()
            .filter(|h| h.id != "c2-core" && !h.unreachable)
            .map(|h| h.id.clone())
            .collect()
    };

    if nodes.is_empty() {
        anyhow::bail!("No reachable workers. Run: kova c2 inspect");
    }

    let skip_sync = if force_skip_sync {
        true
    } else {
        let worker_path = if local {
            to_worker_path_local(&plan.s4)
        } else {
            to_worker_path(&plan.s4)
        };
        let preflight = Command::new("ssh")
            .args(["-o", "ConnectTimeout=5", &nodes[0]])
            .arg(format!("test -d {}", worker_path.display()))
            .status();
        preflight.as_ref().map(|s| s.success()).unwrap_or(false)
    };

    if !skip_sync {
        eprintln!("[build] Syncing to {} workers (parallel)...", nodes.len());
        sync_parallel(&nodes, local)?;
    }

    eprintln!("[build] Broadcasting to {} workers (parallel)...", nodes.len());
    broadcast_parallel(&plan, &nodes, local)?;

    eprintln!("[build] Done.");
    Ok(())
}

/// Parallel sync: one thread per node. Each runs rsync to that node.
fn sync_parallel(nodes: &[String], local: bool) -> anyhow::Result<()> {
    let (hive_workspace, hive_projects) = hive_paths(local);
    let root = kova_root();
    let rsync_args = vec!["-avz", "--exclude", "target", "--exclude", "node_modules"];

    let handles: Vec<_> = nodes
        .iter()
        .map(|t| {
            let t = t.clone();
            let root = root.clone();
            let hive_workspace = hive_workspace.to_string();
            let hive_projects = hive_projects.to_string();
            let rsync_args = rsync_args.clone();
            thread::spawn(move || sync_one_node(&t, &root, &hive_workspace, &hive_projects, &rsync_args))
        })
        .collect();

    for h in handles {
        h.join().map_err(|_| anyhow::anyhow!("Sync thread panicked"))??;
    }
    Ok(())
}

fn sync_one_node(
    target: &str,
    root: &Path,
    hive_workspace: &str,
    hive_projects: &str,
    rsync_args: &[&str],
) -> anyhow::Result<()> {
    let check = Command::new("ssh")
        .args(["-o", "ConnectTimeout=5", target])
        .arg(format!(
            "mkdir -p {} {}/ronin-sites {}/rogue-repo",
            hive_workspace, hive_projects, hive_projects
        ))
        .status();
    if let Ok(status) = check {
        if !status.success() {
            anyhow::bail!("Cannot create dirs on {}. Check SSH.", target);
        }
    } else {
        anyhow::bail!("Cannot reach {}. Check SSH.", target);
    }

    for crate_name in WORKSPACE_CRATES {
        let src = root.join(crate_name);
        if src.is_dir() {
            let status = Command::new("rsync")
                .args(rsync_args)
                .arg(src)
                .arg(format!("{}:{}/", target, hive_workspace))
                .status()?;
            if !status.success() {
                anyhow::bail!("rsync {} failed on {}", crate_name, target);
            }
        }
    }
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.is_file() {
        let status = Command::new("rsync")
            .args(rsync_args)
            .arg(&cargo_toml)
            .arg(format!("{}:{}/", target, hive_workspace))
            .status()?;
        if !status.success() {
            anyhow::bail!("rsync Cargo.toml failed on {}", target);
        }
    }
    for dir in ["ronin-sites", "rogue-repo"] {
        let src = root.join(dir);
        if src.is_dir() {
            let status = Command::new("rsync")
                .args(rsync_args)
                .arg(&src)
                .arg(format!("{}:{}/", target, hive_projects))
                .status()?;
            if !status.success() {
                anyhow::bail!("rsync {} failed on {}", dir, target);
            }
        }
    }
    Ok(())
}

/// Parallel broadcast: one thread per node. Stream output with [node] prefix.
fn broadcast_parallel(plan: &crate::plan::t3, nodes: &[String], local: bool) -> anyhow::Result<()> {
    let worker_path = if local {
        to_worker_path_local(&plan.s4)
    } else {
        to_worker_path(&plan.s4)
    };

    for step in &plan.s3 {
        let cmd = match &step.s6 {
            crate::plan::t5::CargoCheck => "cargo check",
            crate::plan::t5::CargoTest => "cargo test",
            crate::plan::t5::CargoBuild { release } => {
                if *release {
                    "cargo build --release"
                } else {
                    "cargo build"
                }
            }
            _ => continue,
        };

        let (tx, rx) = mpsc::channel::<(String, String)>();
        let handles: Vec<_> = nodes
            .iter()
            .map(|node| {
                let node = node.clone();
                let tx = tx.clone();
                let worker_path = worker_path.clone();
                let cmd = cmd.to_string();
                thread::spawn(move || {
                    let child = Command::new("ssh")
                        .arg(&node)
                        .arg(format!("cd {} && {}", worker_path.display(), cmd))
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn();
                    let ok = match child {
                        Ok(mut c) => {
                            if let Some(out) = c.stdout.take() {
                                for line in BufReader::new(out).lines().filter_map(|l| l.ok()) {
                                    let _ = tx.send((node.clone(), line));
                                }
                            }
                            if let Some(err) = c.stderr.take() {
                                for line in BufReader::new(err).lines().filter_map(|l| l.ok()) {
                                    let _ = tx.send((node.clone(), line));
                                }
                            }
                            c.wait().map(|s| s.success()).unwrap_or(false)
                        }
                        Err(e) => {
                            let _ = tx.send((node.clone(), format!("ssh failed: {}", e)));
                            false
                        }
                    };
                    ok
                })
            })
            .collect();

        drop(tx);
        for (n, line) in rx {
            eprintln!("[{}] {}", n, line);
        }
        for h in handles {
            let ok = h.join().map_err(|_| anyhow::anyhow!("Broadcast thread panicked"))?;
            if !ok {
                anyhow::bail!("{} failed on at least one node", cmd);
            }
        }
    }
    Ok(())
}

fn hive_paths(local: bool) -> (String, String) {
    let base = if local {
        crate::config::hive_local_base()
    } else {
        crate::config::hive_shared_base()
    };
    let workspace = format!("{}/projects/workspace", base);
    let projects = format!("{}/projects", base);
    (workspace, projects)
}

/// Workspace crates to sync (per KOVA_PROJECT_PLACEMENT).
const WORKSPACE_CRATES: &[&str] = &[
    "approuter", "cochranblock", "oakilydokily", "kova", "kova-core", "kova-web",
    "exopack", "whyyoulying", "wowasticker", "vendor",
];

fn kova_root() -> PathBuf {
    std::env::var("KOVA_ROOT")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Sync workspace from c2-core to workers. Replaces sync-hive.sh.
pub fn run_sync(dry_run: bool, target: &str, local: bool, all: bool) -> anyhow::Result<()> {
    let root = kova_root();
    let (hive_workspace, hive_projects) = hive_paths(local);

    let targets: Vec<&str> = if all {
        default_nodes().into_iter().collect()
    } else {
        vec![target]
    };

    let mut rsync_args = vec!["-avz", "--exclude", "target", "--exclude", "node_modules"];
    if dry_run {
        rsync_args.push("--dry-run");
    }

    for t in &targets {
        // 1. Ensure hive dir exists on target
        let check = if local {
            Command::new("ssh")
                .args(["-o", "ConnectTimeout=5", t])
                .arg(format!(
                    "mkdir -p {} {}/ronin-sites {}/rogue-repo",
                    hive_workspace, hive_projects, hive_projects
                ))
                .status()
        } else {
            Command::new("ssh")
                .args(["-o", "ConnectTimeout=5", t])
                .arg(format!("test -d {}", hive_workspace))
                .status()
        };

        if let Ok(status) = check {
            if !status.success() {
                if local {
                    anyhow::bail!("Cannot create dirs on {}. Check SSH.", t);
                } else {
                    anyhow::bail!(
                        "Hive not ready. Run on target:\n  ssh {} \"sudo mkdir -p {} {}/ronin-sites {}/rogue-repo && sudo chown -R $(whoami):$(whoami) {}\"",
                        t, hive_workspace, hive_projects, hive_projects, hive_projects
                    );
                }
            }
        } else {
            anyhow::bail!("Cannot reach {}. Check SSH.", t);
        }

        // 2. Rsync workspace crates
        eprintln!("[sync] Syncing workspace to {}:{}/", t, hive_workspace);
        for crate_name in WORKSPACE_CRATES {
            let src = root.join(crate_name);
            if src.is_dir() {
                let status = Command::new("rsync")
                    .args(&rsync_args)
                    .arg(&src)
                    .arg(format!("{}:{}/", t, hive_workspace))
                    .status()?;
                if !status.success() {
                    anyhow::bail!("rsync {} failed", crate_name);
                }
            }
        }
        let cargo_toml = root.join("Cargo.toml");
        if cargo_toml.is_file() {
            let status = Command::new("rsync")
                .args(&rsync_args)
                .arg(&cargo_toml)
                .arg(format!("{}:{}/", t, hive_workspace))
                .status()?;
            if !status.success() {
                anyhow::bail!("rsync Cargo.toml failed");
            }
        }

        // 3. Rsync ronin-sites, rogue-repo (outside workspace)
        eprintln!("[sync] Syncing ronin-sites, rogue-repo to {}:{}/", t, hive_projects);
        for dir in ["ronin-sites", "rogue-repo"] {
            let src = root.join(dir);
            if src.is_dir() {
                let status = Command::new("rsync")
                    .args(&rsync_args)
                    .arg(&src)
                    .arg(format!("{}:{}/", t, hive_projects))
                    .status()?;
                if !status.success() {
                    anyhow::bail!("rsync {} failed", dir);
                }
            }
        }
    }

    eprintln!("[sync] Done.");
    Ok(())
}
