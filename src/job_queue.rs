//! kova c2 queue — Distributed job queue across kova nodes.
//! Inspired by mattbusel/tokio-prompt-orchestrator DAG pipeline pattern.
//!
//! Three-stage pipeline: Submit → Dispatch → Collect
//! - Submit: enqueue a job (train, generate, quantize, build, any command)
//! - Dispatch: pick best node (least-loaded or pinned), SSH execute
//! - Collect: stream results back, handle failure with circuit breaker
//!
//! Job state: ~/.kova/queue/jobs/<id>.json
//! Node health: ~/.kova/queue/health/<node>.json
//!
//! Usage:
//!   kova c2 queue submit "pixel-forge train --data data_v2_16 --epochs 300"
//!   kova c2 queue submit --node lf --tag train "cargo build --release"
//!   kova c2 queue status
//!   kova c2 queue drain
//!   kova c2 queue history
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ──────────────────────────────────────────────────

/// Job status lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Dead, // exhausted retries
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Queued => write!(f, "queued"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Done => write!(f, "done"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Dead => write!(f, "dead"),
        }
    }
}

/// A job in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub command: String,
    pub tag: String,
    pub node: Option<String>, // pinned node, or None for auto
    pub priority: u8,         // 0 = highest
    pub status: JobStatus,
    pub attempts: u32,
    pub max_retries: u32,
    pub assigned_node: Option<String>,
    pub submitted: u64,
    pub started: Option<u64>,
    pub finished: Option<u64>,
    pub exit_code: Option<i32>,
    pub output_tail: Option<String>, // last N lines of output
    pub project: String,             // working dir on node
}

/// Per-node health for circuit breaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node: String,
    pub consecutive_failures: u32,
    pub last_failure: Option<u64>,
    pub last_success: Option<u64>,
    pub total_jobs: u64,
    pub total_failures: u64,
    /// Circuit breaker state: closed (healthy), open (broken), half-open (probing)
    pub circuit: CircuitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,   // healthy — accept jobs
    Open,     // broken — reject jobs
    HalfOpen, // probing — accept one job to test
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "ok"),
            CircuitState::Open => write!(f, "OPEN"),
            CircuitState::HalfOpen => write!(f, "probe"),
        }
    }
}

/// Circuit breaker config.
const FAILURE_THRESHOLD: u32 = 3;  // open after N consecutive failures
const RECOVERY_SECS: u64 = 120;    // try half-open after N seconds

// ── Paths ──────────────────────────────────────────────────

fn queue_dir() -> PathBuf {
    crate::config::kova_dir().join("queue")
}

fn jobs_dir() -> PathBuf {
    queue_dir().join("jobs")
}

fn health_dir() -> PathBuf {
    queue_dir().join("health")
}

fn job_path(id: &str) -> PathBuf {
    jobs_dir().join(format!("{id}.json"))
}

fn health_path(node: &str) -> PathBuf {
    health_dir().join(format!("{node}.json"))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn short_id() -> String {
    let now = now_epoch();
    let pid = std::process::id();
    format!("j{:x}{:04x}", now & 0xFFFF, pid & 0xFFFF)
}

fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 { format!("{h}h{m}m") }
    else if m > 0 { format!("{m}m{s}s") }
    else { format!("{s}s") }
}

// ── Job CRUD ───────────────────────────────────────────────

fn save_job(job: &Job) -> Result<()> {
    fs::create_dir_all(jobs_dir())?;
    fs::write(job_path(&job.id), serde_json::to_string_pretty(job)?)?;
    Ok(())
}

fn load_job(id: &str) -> Result<Job> {
    let data = fs::read_to_string(job_path(id))?;
    Ok(serde_json::from_str(&data)?)
}

fn load_all_jobs() -> Result<Vec<Job>> {
    let dir = jobs_dir();
    if !dir.exists() { return Ok(vec![]); }

    let mut jobs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(data) = fs::read_to_string(entry.path())
            && let Ok(job) = serde_json::from_str::<Job>(&data)
        {
            jobs.push(job);
        }
    }
    jobs.sort_by_key(|j| (j.priority, j.submitted));
    Ok(jobs)
}

// ── Node Health / Circuit Breaker ──────────────────────────

fn load_health(node: &str) -> NodeHealth {
    let path = health_path(node);
    if path.exists()
        && let Ok(data) = fs::read_to_string(&path)
        && let Ok(h) = serde_json::from_str::<NodeHealth>(&data)
    {
        return h;
    }
    NodeHealth {
        node: node.to_string(),
        consecutive_failures: 0,
        last_failure: None,
        last_success: None,
        total_jobs: 0,
        total_failures: 0,
        circuit: CircuitState::Closed,
    }
}

fn save_health(h: &NodeHealth) -> Result<()> {
    fs::create_dir_all(health_dir())?;
    fs::write(health_path(&h.node), serde_json::to_string_pretty(h)?)?;
    Ok(())
}

fn record_success(node: &str) -> Result<()> {
    let mut h = load_health(node);
    h.consecutive_failures = 0;
    h.last_success = Some(now_epoch());
    h.total_jobs += 1;
    h.circuit = CircuitState::Closed;
    save_health(&h)
}

fn record_failure(node: &str) -> Result<()> {
    let mut h = load_health(node);
    h.consecutive_failures += 1;
    h.last_failure = Some(now_epoch());
    h.total_jobs += 1;
    h.total_failures += 1;

    if h.consecutive_failures >= FAILURE_THRESHOLD {
        h.circuit = CircuitState::Open;
        eprintln!("circuit OPEN for {node} — {} consecutive failures", h.consecutive_failures);
    }

    save_health(&h)
}

/// Check if a node is available (circuit breaker).
fn node_available(node: &str) -> bool {
    let mut h = load_health(node);
    match h.circuit {
        CircuitState::Closed => true,
        CircuitState::HalfOpen => true, // allow one probe
        CircuitState::Open => {
            // Check if recovery timeout elapsed
            let since_fail = h.last_failure
                .map(|t| now_epoch().saturating_sub(t))
                .unwrap_or(999);
            if since_fail >= RECOVERY_SECS {
                h.circuit = CircuitState::HalfOpen;
                let _ = save_health(&h);
                eprintln!("{node}: circuit half-open — probing");
                true
            } else {
                false
            }
        }
    }
}

// ── Node Selection (LeastLoaded) ───────────────────────────

/// Pick the best available node. Prefers pinned, then least loaded.
fn pick_node(pinned: Option<&str>) -> Result<String> {
    if let Some(node) = pinned {
        if node_available(node) {
            return Ok(node.to_string());
        }
        bail!("node {node} circuit is open — try another or wait");
    }

    // Get load from all nodes in parallel
    let nodes = crate::c2::f350();
    let mut candidates: Vec<(String, f64)> = Vec::new();

    for &node in &nodes {
        if !node_available(node) { continue; }

        // Quick SSH load check
        let output = Command::new("ssh")
            .args(["-o", "ConnectTimeout=3", node,
                   "cat /proc/loadavg 2>/dev/null | cut -d' ' -f1"])
            .output();

        let load = match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(99.0)
            }
            _ => {
                // SSH failed — mark and skip
                let _ = record_failure(node);
                continue;
            }
        };

        // Penalize nodes with running jobs
        let running = load_all_jobs()?
            .iter()
            .filter(|j| j.status == JobStatus::Running && j.assigned_node.as_deref() == Some(node))
            .count() as f64;

        candidates.push((node.to_string(), load + running * 2.0));
    }

    if candidates.is_empty() {
        bail!("no nodes available — all circuits open or SSH failed");
    }

    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    Ok(candidates[0].0.clone())
}

// ── Public API ─────────────────────────────────────────────

/// Submit a job to the queue.
pub fn submit(
    command: &str,
    node: Option<&str>,
    tag: &str,
    priority: u8,
    project: &str,
    max_retries: u32,
) -> Result<String> {
    // Dedup: reject if identical command is already queued or running
    let existing = load_all_jobs()?;
    for j in &existing {
        if j.command == command && matches!(j.status, JobStatus::Queued | JobStatus::Running) {
            bail!("duplicate: job {} already {} — {}", j.id, j.status, j.command);
        }
    }

    let job = Job {
        id: short_id(),
        command: command.to_string(),
        tag: tag.to_string(),
        node: node.map(String::from),
        priority,
        status: JobStatus::Queued,
        attempts: 0,
        max_retries,
        assigned_node: None,
        submitted: now_epoch(),
        started: None,
        finished: None,
        exit_code: None,
        output_tail: None,
        project: project.to_string(),
    };

    save_job(&job)?;
    println!("submitted {} p{} [{}] — {}", job.id, priority, tag, command);
    Ok(job.id)
}

/// Drain: pick next queued job, dispatch to best node, run via SSH.
pub fn drain_next() -> Result<Option<String>> {
    let jobs = load_all_jobs()?;
    let next = jobs.iter().find(|j| j.status == JobStatus::Queued);

    let job = match next {
        Some(j) => j.clone(),
        None => {
            println!("queue empty");
            return Ok(None);
        }
    };

    // Pick node
    let node = pick_node(job.node.as_deref())?;
    println!("{}: dispatching {} [{}] — {}", node, job.id, job.tag, job.command);

    // Update job state
    let mut job = job;
    job.status = JobStatus::Running;
    job.assigned_node = Some(node.clone());
    job.started = Some(now_epoch());
    job.attempts += 1;
    save_job(&job)?;

    // SSH execute
    let ssh_cmd = format!(
        "source ~/.cargo/env 2>/dev/null; cd ~/{} && {}",
        job.project, job.command
    );

    let output = Command::new("ssh")
        .args([&node, &ssh_cmd])
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);

            // Capture last 20 lines of output
            let all_output = format!("{}{}", stdout, stderr);
            let tail: Vec<&str> = all_output.lines().rev().take(20).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            job.output_tail = Some(tail.join("\n"));

            if o.status.success() {
                job.status = JobStatus::Done;
                job.exit_code = Some(0);
                job.finished = Some(now_epoch());
                record_success(&node)?;

                let elapsed = job.finished.unwrap() - job.started.unwrap_or(0);
                println!("{}: {} done in {} — {}", node, job.id, fmt_duration(elapsed), job.tag);
            } else {
                let code = o.status.code().unwrap_or(-1);
                job.exit_code = Some(code);
                record_failure(&node)?;

                if job.attempts < job.max_retries {
                    job.status = JobStatus::Queued; // re-queue for retry
                    job.assigned_node = None;
                    eprintln!("{}: {} failed (exit {}), retry {}/{}", node, job.id, code, job.attempts, job.max_retries);
                } else {
                    job.status = JobStatus::Dead;
                    job.finished = Some(now_epoch());
                    eprintln!("{}: {} dead after {} attempts", node, job.id, job.attempts);
                }
            }
            save_job(&job)?;

            // Print output
            if !stdout.is_empty() { print!("{stdout}"); }
            if !stderr.is_empty() { eprint!("{stderr}"); }
        }
        Err(e) => {
            record_failure(&node)?;
            job.status = JobStatus::Queued;
            job.assigned_node = None;
            save_job(&job)?;
            bail!("{node}: SSH error — {e}");
        }
    }

    // Auto-drain next if more queued
    let remaining = load_all_jobs()?.iter().filter(|j| j.status == JobStatus::Queued).count();
    if remaining > 0 {
        println!("{remaining} more jobs queued — run `kova c2 queue drain` again");
    }

    Ok(Some(job.id))
}

/// Drain all queued jobs sequentially.
pub fn drain_all() -> Result<()> {
    while drain_next()?.is_some() {}
    Ok(())
}

/// Show queue status.
pub fn status() -> Result<()> {
    let jobs = load_all_jobs()?;
    if jobs.is_empty() {
        println!("queue empty");
        return Ok(());
    }

    let queued = jobs.iter().filter(|j| j.status == JobStatus::Queued).count();
    let running = jobs.iter().filter(|j| j.status == JobStatus::Running).count();
    let done = jobs.iter().filter(|j| j.status == JobStatus::Done).count();
    let failed = jobs.iter().filter(|j| matches!(j.status, JobStatus::Failed | JobStatus::Dead)).count();

    println!("jobs: {} queued, {} running, {} done, {} failed", queued, running, done, failed);
    println!();

    for j in &jobs {
        if matches!(j.status, JobStatus::Done) { continue; } // skip completed

        let node = j.assigned_node.as_deref().unwrap_or("-");
        let age = fmt_duration(now_epoch().saturating_sub(j.submitted));
        let run_time = j.started.map(|s| fmt_duration(now_epoch().saturating_sub(s))).unwrap_or_default();

        match j.status {
            JobStatus::Running => {
                println!("  {} {} on {} [{}] ({}) — {}", j.id, j.status, node, j.tag, run_time, j.command);
            }
            JobStatus::Queued => {
                println!("  {} {} p{} [{}] (waiting {}) — {}", j.id, j.status, j.priority, j.tag, age, j.command);
            }
            _ => {
                println!("  {} {} [{}] attempt {}/{} — {}", j.id, j.status, j.tag, j.attempts, j.max_retries, j.command);
            }
        }
    }

    // Node health
    println!();
    for &node in &crate::c2::f350() {
        let h = load_health(node);
        println!("  {}: circuit={} jobs={} failures={}", node, h.circuit, h.total_jobs, h.total_failures);
    }

    Ok(())
}

/// Show completed job history.
pub fn history(limit: usize) -> Result<()> {
    let mut jobs = load_all_jobs()?;
    jobs.retain(|j| matches!(j.status, JobStatus::Done | JobStatus::Dead));
    jobs.sort_by_key(|j| std::cmp::Reverse(j.finished.unwrap_or(0)));
    jobs.truncate(limit);

    if jobs.is_empty() {
        println!("no completed jobs");
        return Ok(());
    }

    for j in &jobs {
        let node = j.assigned_node.as_deref().unwrap_or("?");
        let duration = match (j.started, j.finished) {
            (Some(s), Some(f)) => fmt_duration(f.saturating_sub(s)),
            _ => "?".to_string(),
        };
        let icon = if j.status == JobStatus::Done { "ok" } else { "DEAD" };
        println!("  {} {} {} on {} ({}) [{}] — {}", j.id, icon, j.status, node, duration, j.tag, j.command);
    }
    Ok(())
}

/// Cancel a queued job.
pub fn cancel(id: &str) -> Result<()> {
    let mut job = load_job(id)?;
    if job.status != JobStatus::Queued {
        bail!("{id} is {} — can only cancel queued jobs", job.status);
    }
    job.status = JobStatus::Dead;
    job.finished = Some(now_epoch());
    save_job(&job)?;
    println!("cancelled {id} — {}", job.command);
    Ok(())
}

/// Purge completed/dead jobs older than N hours.
pub fn purge(hours: u64) -> Result<()> {
    let cutoff = now_epoch().saturating_sub(hours * 3600);
    let jobs = load_all_jobs()?;
    let mut removed = 0;

    for j in &jobs {
        if matches!(j.status, JobStatus::Done | JobStatus::Dead)
            && j.finished.unwrap_or(0) < cutoff
        {
            let _ = fs::remove_file(job_path(&j.id));
            removed += 1;
        }
    }
    println!("purged {removed} jobs older than {hours}h");
    Ok(())
}

/// Reset circuit breaker for a node.
pub fn reset_circuit(node: &str) -> Result<()> {
    let mut h = load_health(node);
    h.circuit = CircuitState::Closed;
    h.consecutive_failures = 0;
    save_health(&h)?;
    println!("{node}: circuit reset to closed");
    Ok(())
}
