//! kova c2 — T212ized orchestration. f18–f23 local or broadcast.
//! f356: one-command sync + broadcast with parallel execution.
//! Sync: tar-stream for full sync (dir missing), rsync for incremental.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

#![allow(non_camel_case_types)]

use clap::ValueEnum;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Copy, ValueEnum)]
pub enum T212 {
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

impl T212 {
    pub fn f347(self, release: bool) -> crate::t0 {
        match self {
            T212::F18 => crate::t0::f18(release),
            T212::F19 => crate::t0::f19(),
            T212::F20 => crate::t0::f20(),
            T212::F21 => crate::t0::f21(),
            T212::F22 => crate::t0::f22(),
            T212::F23 => crate::t0::f23(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            T212::F18 => "f18",
            T212::F19 => "f19",
            T212::F20 => "f20",
            T212::F21 => "f21",
            T212::F22 => "f22",
            T212::F23 => "f23",
        }
    }

    pub fn f349(&self) -> bool {
        matches!(self, T212::F21 | T212::F22 | T212::F23)
    }
}

pub fn f350() -> Vec<&'static str> {
    vec!["lf", "gd", "bt", "st"]
}

/// MAC addresses for Wake-on-LAN. st has no WoL support.
pub fn f351(node: &str) -> Option<&'static str> {
    match node {
        "lf" | "n0" => Some("6c:24:08:df:7c:39"),
        "gd" | "n1" => Some("cc:96:e5:bd:01:3a"),
        "bt" | "n2" => Some("2c:f0:5d:55:3b:d3"),
        _ => None, // st/n3 has no WoL support
    }
}

/// Send Wake-on-LAN magic packet to a node.
pub fn f352(node: &str) -> Result<(), String> {
    let mac = f351(node).ok_or_else(|| format!("{}: no WoL MAC (st has no WoL support)", node))?;
    let output = std::process::Command::new("wakeonlan")
        .arg(mac)
        .output()
        .map_err(|e| format!("wakeonlan: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("wakeonlan failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

pub fn f353(project: Option<PathBuf>) -> PathBuf {
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

pub(crate) fn to_worker_path(p: &Path) -> PathBuf {
    to_worker_path_impl(p, &crate::config::hive_shared_base())
}

pub(crate) fn to_worker_path_local(p: &Path) -> PathBuf {
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
        eprintln!(
            "{} {} {}",
            mark,
            r.s10,
            if r.s13.is_empty() { "" } else { &r.s13 }
        );
    }
    let all_ok = results.iter().all(|r| r.s11);
    if !all_ok {
        anyhow::bail!("One or more actions failed");
    }
    Ok(())
}

/// f119=kova_c2_run. CLI orchestration. Local or broadcast.
pub fn f354(
    token: T212,
    project: Option<PathBuf>,
    broadcast: bool,
    release: bool,
    nodes_override: Option<String>,
    local: bool,
) -> anyhow::Result<()> {
    let project_path = f353(project);
    let intent = token.f347(release);
    let approuter_dir = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("approuter"));
    let plan = crate::plan::t3::f14(&intent, project_path.clone(), approuter_dir);

    if plan.s3.is_empty() {
        eprintln!("No actions for {}", token.name());
        return Ok(());
    }

    if token.f349() {
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
        // Delegate to f356 for shared sync + parallel broadcast logic.
        let nodes: Vec<String> = if let Some(s) = nodes_override {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        } else {
            let hosts = crate::inspect::f359();
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

pub fn f355() {
    for n in f350() {
        println!("{}", n);
    }
}

/// f121=f356. One-command sync + broadcast. Parallel execution.
pub fn f356(
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

    let project_path = f353(project);
    if !is_under_hive_vault(&project_path) {
        anyhow::bail!(
            "Project must be under ~/hive-vault for broadcast.\n\
             Run: ln -s ~ ~/hive-vault/projects/workspace (or equivalent)\n\
             Then: kova c2 build --broadcast --project ~/hive-vault/projects/workspace/..."
        );
    }

    let intent = crate::t0::f18(release);
    let approuter_dir = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("approuter"));
    let plan = crate::plan::t3::f14(&intent, project_path.clone(), approuter_dir);
    run_build_with_plan(plan, local, no_sync, nodes_override).map(|_| ())
}

/// Shared sync + broadcast. Used by f356 and f354 --broadcast.
fn run_build_with_plan(
    plan: crate::plan::t3,
    local: bool,
    force_skip_sync: bool,
    nodes_override: Option<String>,
) -> anyhow::Result<()> {
    let nodes: Vec<String> = if let Some(s) = nodes_override {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    } else {
        let hosts = crate::inspect::f359();
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
        eprintln!(
            "[build] Syncing to {} workers (parallel, tar-stream)...",
            nodes.len()
        );
        f357(&nodes, local, true)?;
    }

    eprintln!(
        "[build] Broadcasting to {} workers (parallel)...",
        nodes.len()
    );
    broadcast_parallel(&plan, &nodes, local)?;

    eprintln!("[build] Done.");
    Ok(())
}

/// Parallel sync. full_sync=true: tar-stream (faster for first sync). full_sync=false: rsync (incremental).
pub fn f357(nodes: &[String], local: bool, full_sync: bool) -> anyhow::Result<()> {
    if full_sync {
        sync_tar_stream(nodes, local)
    } else {
        sync_rsync_parallel(nodes, local)
    }
}

/// Tar workspace once, stream to each node in parallel. Best for full sync (dir missing).
fn sync_tar_stream(nodes: &[String], local: bool) -> anyhow::Result<()> {
    let root = kova_root();
    let base = if local {
        crate::config::hive_local_base()
    } else {
        crate::config::hive_shared_base()
    };

    let tmp = std::env::temp_dir().join(format!("kova-sync-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| anyhow::anyhow!("Cannot create temp dir: {}", e))?;
    let _cleanup = TempDirGuard(tmp.clone());

    let projects = tmp.join("projects");
    let workspace_dir = projects.join("workspace");
    std::fs::create_dir_all(&workspace_dir)?;

    for crate_name in WORKSPACE_CRATES {
        let src = root.join(crate_name);
        if src.is_dir() {
            let dst = workspace_dir.join(crate_name);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&src, &dst)
                .map_err(|e| anyhow::anyhow!("symlink {}: {}", crate_name, e))?;
            #[cfg(not(unix))]
            {
                copy_dir_all(&src, &dst)?;
            }
        }
    }
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.is_file() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&cargo_toml, workspace_dir.join("Cargo.toml"))
            .map_err(|e| anyhow::anyhow!("symlink Cargo.toml: {}", e))?;
        #[cfg(not(unix))]
        std::fs::copy(&cargo_toml, workspace_dir.join("Cargo.toml"))?;
    }
    for dir in ["ronin-sites", "rogue-repo"] {
        let src = root.join(dir);
        if src.is_dir() {
            let dst = projects.join(dir);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&src, &dst)
                .map_err(|e| anyhow::anyhow!("symlink {}: {}", dir, e))?;
            #[cfg(not(unix))]
            copy_dir_all(&src, &dst)?;
        }
    }

    let tar_path = std::env::temp_dir().join(format!("kova-sync-{}.tar", std::process::id()));
    let status = Command::new("tar")
        .args([
            "-chf",
            tar_path.to_str().unwrap(),
            "--exclude",
            "target",
            "--exclude",
            ".git",
            "--exclude",
            "node_modules",
        ])
        .arg("-C")
        .arg(&tmp)
        .arg("projects")
        .status()?;
    if !status.success() {
        anyhow::bail!("tar create failed");
    }
    let _tar_cleanup = TempFileGuard(tar_path.clone());

    let extract_dir = base;
    let handles: Vec<_> = nodes
        .iter()
        .map(|node| {
            let node = node.clone();
            let tar_path = tar_path.clone();
            let extract_dir = extract_dir.clone();
            thread::spawn(move || {
                let sh = format!(
                    "cat {} | ssh -o ConnectTimeout=5 {} \"mkdir -p {} && cat > /tmp/hive-build.tar && cd {} && tar xf /tmp/hive-build.tar && rm -f /tmp/hive-build.tar\"",
                    tar_path.display(),
                    node,
                    extract_dir,
                    extract_dir
                );
                let status = Command::new("sh").args(["-c", &sh]).status();
                status.map(|s| s.success()).unwrap_or(false)
            })
        })
        .collect();

    let mut all_ok = true;
    for h in handles {
        if !h
            .join()
            .map_err(|_| anyhow::anyhow!("Tar-stream sync thread panicked"))?
        {
            all_ok = false;
        }
    }
    if !all_ok {
        anyhow::bail!("Tar-stream sync failed on at least one node");
    }
    Ok(())
}

#[cfg(not(unix))]
fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let p = e.path();
        let name = e.file_name();
        let d = dst.join(&name);
        if p.is_dir() {
            copy_dir_all(&p, &d)?;
        } else {
            std::fs::copy(&p, &d)?;
        }
    }
    Ok(())
}

struct TempDirGuard(PathBuf);
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct TempFileGuard(PathBuf);
impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Parallel rsync: one thread per node. Best for incremental sync.
fn sync_rsync_parallel(nodes: &[String], local: bool) -> anyhow::Result<()> {
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
            thread::spawn(move || {
                sync_one_node(&t, &root, &hive_workspace, &hive_projects, &rsync_args)
            })
        })
        .collect();

    for h in handles {
        h.join()
            .map_err(|_| anyhow::anyhow!("Sync thread panicked"))??;
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
                    match child {
                        Ok(mut c) => {
                            if let Some(out) = c.stdout.take() {
                                for line in BufReader::new(out).lines().map_while(Result::ok) {
                                    let _ = tx.send((node.clone(), line));
                                }
                            }
                            if let Some(err) = c.stderr.take() {
                                for line in BufReader::new(err).lines().map_while(Result::ok) {
                                    let _ = tx.send((node.clone(), line));
                                }
                            }
                            c.wait().map(|s| s.success()).unwrap_or(false)
                        }
                        Err(e) => {
                            let _ = tx.send((node.clone(), format!("ssh failed: {}", e)));
                            false
                        }
                    }
                })
            })
            .collect();

        drop(tx);
        for (n, line) in rx {
            eprintln!("[{}] {}", n, line);
        }
        for h in handles {
            let ok = h
                .join()
                .map_err(|_| anyhow::anyhow!("Broadcast thread panicked"))?;
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
    "approuter",
    "cochranblock",
    "oakilydokily",
    "kova",
    "exopack",
    "whyyoulying",
    "wowasticker",
    "railgun",
    "ironhive",
    "vendor",
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
pub fn f358(
    dry_run: bool,
    target: &str,
    local: bool,
    all: bool,
    full: bool,
) -> anyhow::Result<()> {
    let nodes: Vec<String> = if all {
        f350().into_iter().map(String::from).collect()
    } else {
        vec![target.to_string()]
    };

    if !dry_run && !nodes.is_empty() {
        eprintln!(
            "[sync] Syncing to {} workers (parallel, {})...",
            nodes.len(),
            if full { "tar-stream" } else { "rsync" }
        );
        f357(&nodes, local, full)?;
        eprintln!("[sync] Done.");
        return Ok(());
    }

    let root = kova_root();
    let (hive_workspace, hive_projects) = hive_paths(local);
    let targets: Vec<&str> = nodes.iter().map(|s| s.as_str()).collect();

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
        eprintln!(
            "[sync] Syncing ronin-sites, rogue-repo to {}:{}/",
            t, hive_projects
        );
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

// ── Offload: archive build artifacts to worker node, free local disk ──

/// Disk usage as percentage (0-100). Uses statvfs.
fn disk_usage_percent() -> u8 {
    // Parse df output to get usage percentage
    if let Ok(output) = std::process::Command::new("df").arg("-k").arg("/").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = text.lines().nth(1) {
            // df -k output: Filesystem 1K-blocks Used Available Use% Mounted
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5
                && let Some(pct) = parts[4].strip_suffix('%')
                && let Ok(n) = pct.parse::<u8>()
            {
                return n;
            }
        }
    }
    50 // fallback: assume 50% if stat fails
}

/// Find all target/ dirs under workspace root.
fn find_target_dirs() -> Vec<(String, PathBuf)> {
    let root = kova_root();
    let mut targets = Vec::new();

    for crate_name in WORKSPACE_CRATES {
        let target = root.join(crate_name).join("target");
        if target.is_dir() {
            targets.push((crate_name.to_string(), target));
        }
    }

    // Also check android/target inside kova
    let android_target = root.join("kova").join("android").join("target");
    if android_target.is_dir() {
        targets.push(("kova-android".to_string(), android_target));
    }

    // Root workspace target
    let root_target = root.join("target");
    if root_target.is_dir() {
        targets.push(("workspace".to_string(), root_target));
    }

    targets
}

/// Get dir size in bytes (recursive).
fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0}M", bytes as f64 / 1_048_576.0)
    } else {
        format!("{}K", bytes / 1024)
    }
}

/// f360=offload. Archive build artifacts to worker node, clean local.
pub fn f360(
    dry_run: bool,
    threshold: u8,
    target_node: Option<String>,
) -> anyhow::Result<()> {
    let usage = disk_usage_percent();
    eprintln!("[offload] disk usage: {}%", usage);

    if usage < threshold && !dry_run {
        eprintln!("[offload] below threshold ({}%), nothing to do", threshold);
        return Ok(());
    }

    let targets = find_target_dirs();
    if targets.is_empty() {
        eprintln!("[offload] no target/ dirs found");
        return Ok(());
    }

    let node = target_node.unwrap_or_else(|| {
        crate::config::offload_target_node()
    });
    let archive_base = crate::config::offload_archive_base();

    let mut total_size = 0u64;
    eprintln!("\n{:<20} {:<10} Path", "Crate", "Size");
    eprintln!("{}", "─".repeat(60));
    for (name, path) in &targets {
        let size = dir_size(path);
        total_size += size;
        eprintln!("{:<20} {:<10} {}", name, format_size(size), path.display());
    }
    eprintln!("{}", "─".repeat(60));
    eprintln!("{:<20} {}", "Total", format_size(total_size));

    if dry_run {
        eprintln!("\n[offload] --dry-run: would sync to {} and clean {} of artifacts", node, format_size(total_size));
        return Ok(());
    }

    // Sync each target dir to archive on worker node
    eprintln!("\n[offload] syncing to {}:{}/ ...", node, archive_base);

    for (name, path) in &targets {
        let remote_dir = format!("{}:{}/{}/", node, archive_base, name);
        eprintln!("[offload] {} → {}", name, remote_dir);

        // Ensure remote dir exists
        let mkdir = Command::new("ssh")
            .args(["-o", "ConnectTimeout=5", &node])
            .arg(format!("mkdir -p {}/{}", archive_base, name))
            .status();
        if let Ok(s) = mkdir {
            if !s.success() {
                eprintln!("[offload] WARNING: cannot create dir on {}", node);
                continue;
            }
        } else {
            eprintln!("[offload] WARNING: cannot reach {}", node);
            continue;
        }

        // Rsync
        let status = Command::new("rsync")
            .args(["-az", "--delete", "--exclude", ".git"])
            .arg(format!("{}/", path.display()))
            .arg(&remote_dir)
            .status();

        match status {
            Ok(s) if s.success() => {
                // Clean local
                eprintln!("[offload] cleaning {}", path.display());
                let _ = std::fs::remove_dir_all(path);
            }
            Ok(_) => {
                eprintln!("[offload] WARNING: rsync failed for {}, keeping local", name);
            }
            Err(e) => {
                eprintln!("[offload] WARNING: rsync error for {}: {}", name, e);
            }
        }
    }

    let new_usage = disk_usage_percent();
    eprintln!("\n[offload] done. disk: {}% → {}%", usage, new_usage);
    Ok(())
}

/// f370=deploy. Deploy kova binary + models to all nodes, restart kova-serve.
/// Pattern: local build → scp binary → scp models → restart systemd.
pub fn f370(
    nodes: Option<Vec<String>>,
    skip_build: bool,
    skip_models: bool,
) -> Result<(), String> {
    let targets: Vec<String> = nodes
        .unwrap_or_else(|| f350().iter().map(|s| s.to_string()).collect());

    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/mcochran".into());
    let local_binary = format!("{}/target/aarch64-apple-darwin/release/kova", home);
    // Fallback: check if bt has a fresh build
    let bt_binary = "/home/mcochran/target/release/kova";

    // Step 1: Find binary
    let (binary_source, via_node) = if std::path::Path::new(&local_binary).exists() && !skip_build {
        eprintln!("[deploy] using local binary: {}", local_binary);
        (local_binary.clone(), None)
    } else {
        eprintln!("[deploy] using bt binary: {}", bt_binary);
        (bt_binary.to_string(), Some("bt"))
    };

    // Step 2: Copy binary to all nodes
    eprintln!("[deploy] copying binary to {} nodes...", targets.len());
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    for node in &targets {
        let node = node.clone();
        let src = binary_source.clone();
        let via = via_node.map(|s| s.to_string());
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let result = if let Some(via_node) = via {
                // Copy from bt to target node
                if node == via_node {
                    // bt → bt: just copy locally
                    let status = Command::new("ssh")
                        .args([&node, &format!("cp {} /home/mcochran/bin/kova", src)])
                        .status();
                    status.map(|s| s.success()).unwrap_or(false)
                } else {
                    let status = Command::new("scp")
                        .args([&format!("{}:{}", via_node, src), &format!("{}:/home/mcochran/bin/kova", node)])
                        .status();
                    status.map(|s| s.success()).unwrap_or(false)
                }
            } else {
                // Copy local binary to node
                let status = Command::new("scp")
                    .args([&src, &format!("{}:/home/mcochran/bin/kova", node)])
                    .status();
                status.map(|s| s.success()).unwrap_or(false)
            };
            tx.send((node, "binary", result)).ok();
        }));
    }
    drop(tx);
    for (node, what, ok) in rx.iter() {
        if ok {
            eprintln!("  {}: {} deployed", node, what);
        } else {
            eprintln!("  {}: {} FAILED", node, what);
        }
    }
    for h in handles { let _ = h.join(); }

    // Step 3: Copy trained models (safetensors)
    if !skip_models {
        let models_dir = crate::config::models_dir();
        let kova_models: Vec<_> = std::fs::read_dir(&models_dir)
            .ok()
            .map(|entries| {
                entries.flatten()
                    .filter(|e| e.path().is_dir())
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.starts_with("kova-") && e.path().join("model.safetensors").exists()
                    })
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default();

        if !kova_models.is_empty() {
            eprintln!("[deploy] syncing {} trained models...", kova_models.len());
            for model_dir in &kova_models {
                let name = model_dir.file_name().unwrap().to_string_lossy();
                let (tx, rx) = mpsc::channel();
                let mut handles = Vec::new();
                for node in &targets {
                    let node = node.clone();
                    let src = model_dir.display().to_string();
                    let name = name.to_string();
                    let tx = tx.clone();
                    handles.push(thread::spawn(move || {
                        let dest = format!("{}:/home/mcochran/.kova/models/{}/", node, name);
                        // Ensure dir exists
                        let _ = Command::new("ssh")
                            .args([&node, &format!("mkdir -p /home/mcochran/.kova/models/{}", name)])
                            .status();
                        let ok = Command::new("rsync")
                            .args(["-avz", &format!("{}/", src), &dest])
                            .stdout(Stdio::null())
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false);
                        tx.send((node, ok)).ok();
                    }));
                }
                drop(tx);
                for (node, ok) in rx.iter() {
                    if ok {
                        eprintln!("  {}: {} synced", node, name);
                    } else {
                        eprintln!("  {}: {} FAILED", node, name);
                    }
                }
                for h in handles { let _ = h.join(); }
            }
        } else {
            eprintln!("[deploy] no trained kova models to sync");
        }
    }

    // Step 4: Symlink + restart kova-serve
    eprintln!("[deploy] restarting kova-serve on all nodes...");
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    for node in &targets {
        let node = node.clone();
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let ok = Command::new("ssh")
                .args([
                    &node,
                    "ln -sf /home/mcochran/bin/kova /home/mcochran/kova-bin && systemctl --user restart kova-serve",
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            tx.send((node, ok)).ok();
        }));
    }
    drop(tx);
    for (node, ok) in rx.iter() {
        if ok {
            eprintln!("  {}: restarted", node);
        } else {
            eprintln!("  {}: restart FAILED", node);
        }
    }
    for h in handles { let _ = h.join(); }

    // Step 5: Verify
    eprintln!("[deploy] verifying...");
    std::thread::sleep(std::time::Duration::from_secs(2));
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    for node in &targets {
        let node = node.clone();
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let output = Command::new("ssh")
                .args([&node, "/home/mcochran/bin/kova --version"])
                .output();
            let version = output.ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "?".into());
            tx.send((node, version)).ok();
        }));
    }
    drop(tx);
    for (node, version) in rx.iter() {
        eprintln!("  {}: {}", node, version);
    }
    for h in handles { let _ = h.join(); }

    eprintln!("[deploy] done");
    Ok(())
}