//! kova c2 gpu — File-based GPU lock + priority queue for training jobs.
//! Any process can acquire/check/release. Prevents mid-inference swaps.
//!
//! Lock file: ~/.kova/gpu/<node>.lock
//! Queue file: ~/.kova/gpu/<node>.queue
//!
//! Usage:
//!   kova c2 gpu lock lf "expert training 20 epochs"
//!   kova c2 gpu status
//!   kova c2 gpu queue lf "quench retrain" -c "cargo run ..."
//!   kova c2 gpu release lf
//!   kova c2 gpu drain lf

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

/// Active GPU lock on a node.
#[derive(Serialize, Deserialize, Debug)]
pub struct GpuLock {
    pub node: String,
    pub job: String,
    pub pid: u32,
    pub started: u64,
}

/// Queued GPU job.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueueEntry {
    pub job: String,
    pub command: String,
    pub priority: u8,
    pub added: u64,
}

fn gpu_dir() -> PathBuf {
    crate::config::kova_dir().join("gpu")
}

fn lock_path(node: &str) -> PathBuf {
    gpu_dir().join(format!("{node}.lock"))
}

fn queue_path(node: &str) -> PathBuf {
    gpu_dir().join(format!("{node}.queue"))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 { format!("{h}h{m}m") }
    else if m > 0 { format!("{m}m{s}s") }
    else { format!("{s}s") }
}

/// Acquire GPU lock. Fails if already held.
pub fn acquire(node: &str, job: &str) -> Result<()> {
    fs::create_dir_all(gpu_dir())?;
    let path = lock_path(node);

    if path.exists() {
        let existing: GpuLock = serde_json::from_str(&fs::read_to_string(&path)?)?;
        let age = fmt_duration(now_epoch().saturating_sub(existing.started));
        bail!("{node} locked: {} (pid {}, {age})", existing.job, existing.pid);
    }

    let lock = GpuLock {
        node: node.to_string(),
        job: job.to_string(),
        pid: std::process::id(),
        started: now_epoch(),
    };
    fs::write(&path, serde_json::to_string_pretty(&lock)?)?;
    println!("{node}: locked — {job}");
    Ok(())
}

/// Release GPU lock.
pub fn release(node: &str) -> Result<()> {
    let path = lock_path(node);
    if path.exists() {
        let lock: GpuLock = serde_json::from_str(&fs::read_to_string(&path)?)?;
        let age = fmt_duration(now_epoch().saturating_sub(lock.started));
        fs::remove_file(&path)?;
        println!("{node}: released (was: {}, ran {age})", lock.job);
    } else {
        println!("{node}: no lock held");
    }
    Ok(())
}

/// Show lock + queue status for all nodes or a specific one.
pub fn status(node_filter: Option<&str>) -> Result<()> {
    let dir = gpu_dir();
    if !dir.exists() {
        println!("no GPU state (run kova c2 gpu lock first)");
        return Ok(());
    }

    let nodes = crate::c2::f350();
    let mut found = false;

    for node in &nodes {
        if let Some(filter) = node_filter
            && *node != filter { continue; }

        let lock = lock_path(node);
        let queue = load_queue(node)?;

        if lock.exists() {
            let l: GpuLock = serde_json::from_str(&fs::read_to_string(&lock)?)?;
            let age = fmt_duration(now_epoch().saturating_sub(l.started));
            println!("{node}: LOCKED — {} (pid {}, {age})", l.job, l.pid);
            found = true;
        } else {
            println!("{node}: idle");
            found = true;
        }

        for (i, entry) in queue.iter().enumerate() {
            println!("  q[{i}] p{}: {}", entry.priority, entry.job);
        }
    }

    if !found {
        println!("no matching nodes");
    }
    Ok(())
}

/// Add job to priority queue.
pub fn enqueue(node: &str, job: &str, command: &str, priority: u8) -> Result<()> {
    fs::create_dir_all(gpu_dir())?;
    let mut queue = load_queue(node)?;
    queue.push(QueueEntry {
        job: job.to_string(),
        command: command.to_string(),
        priority,
        added: now_epoch(),
    });
    queue.sort_by_key(|e| e.priority);
    save_queue(node, &queue)?;
    println!("{node}: queued p{priority} — {job} [{} total]", queue.len());
    Ok(())
}

/// Pop next job from queue. Optionally run it via SSH.
pub fn drain(node: &str, run: bool) -> Result<Option<QueueEntry>> {
    let mut queue = load_queue(node)?;
    if queue.is_empty() {
        println!("{node}: queue empty");
        return Ok(None);
    }

    let next = queue.remove(0);
    save_queue(node, &queue)?;
    println!("{node}: dequeued — {}", next.job);
    println!("  cmd: {}", next.command);

    if run {
        acquire(node, &next.job)?;
        println!("{node}: running via ssh...");
        let output = Command::new("ssh")
            .args([node, &format!("source ~/.cargo/env && cd ~/pixel-forge && {}", next.command)])
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() { print!("{stdout}"); }
        if !stderr.is_empty() { eprint!("{stderr}"); }
        release(node)?;

        // Auto-drain next if available
        if !load_queue(node)?.is_empty() {
            println!("{node}: more jobs queued, run `kova c2 gpu drain {node} --run` to continue");
        }
    }

    Ok(Some(next))
}

/// Query live GPU VRAM usage on a node via nvidia-smi over SSH.
pub fn vram(node: &str) -> Result<()> {
    let output = Command::new("ssh")
        .args([node, "nvidia-smi --query-gpu=name,memory.used,memory.total,utilization.gpu --format=csv,noheader,nounits 2>/dev/null || echo 'no-gpu'"])
        .output()?;
    let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if out == "no-gpu" || out.is_empty() {
        println!("{node}: no GPU detected");
    } else {
        println!("{node}: {out}");
    }
    Ok(())
}

/// Query VRAM across all GPU nodes.
pub fn vram_all() -> Result<()> {
    let nodes = crate::c2::f350();
    for node in &nodes {
        vram(node)?;
    }
    Ok(())
}

/// Check if GPU is available on a node.
pub fn is_available(node: &str) -> bool {
    !lock_path(node).exists()
}

fn load_queue(node: &str) -> Result<Vec<QueueEntry>> {
    let path = queue_path(node);
    if !path.exists() { return Ok(Vec::new()); }
    Ok(serde_json::from_str(&fs::read_to_string(&path)?).unwrap_or_default())
}

fn save_queue(node: &str, queue: &[QueueEntry]) -> Result<()> {
    let path = queue_path(node);
    fs::write(&path, serde_json::to_string_pretty(queue)?)?;
    Ok(())
}
