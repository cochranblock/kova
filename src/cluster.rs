// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Cluster inference — distributed model dispatch across IRONHIVE nodes.
//! Routes tasks to the best available node based on role, model tier, and load.

use crate::ollama;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

/// Model tier — determines which tasks a node can handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    /// 32B models — high quality code gen and fix
    Heavy,
    /// 14B models — fast review, test writing
    Mid,
    /// 7B models — routing, classification, quick tasks
    Light,
    /// 3B models — ultra-fast routing only
    Router,
}

/// Numeric rank for tier comparison. Higher = more capable.
fn tier_rank(t: ModelTier) -> u8 {
    match t {
        ModelTier::Router => 0,
        ModelTier::Light => 1,
        ModelTier::Mid => 2,
        ModelTier::Heavy => 3,
    }
}

/// Node role in the factory pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Primary code generation (32B)
    PrimaryGen,
    /// Secondary code gen + heavy compilation (32B)
    SecondaryGen,
    /// Code review + test writing (14B)
    Reviewer,
    /// Batch/overflow tasks (14B CPU)
    Batch,
    /// Coordinator — routing, classification (7B)
    Coordinator,
}

/// Task type for dispatch routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    CodeGen,
    CodeReview,
    TestWrite,
    FixCompile,
    ClippyFix,
    Classify,
    General,
}

/// A node in the inference cluster.
#[derive(Debug, Clone)]
pub struct InferNode {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub model: String,
    pub general_model: Option<String>,
    pub role: NodeRole,
    pub tier: ModelTier,
    pub busy: Arc<AtomicBool>,
}

impl InferNode {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn set_busy(&self, val: bool) {
        self.busy.store(val, Ordering::SeqCst);
    }
}

/// The cluster state — all inference nodes.
pub struct Cluster {
    pub nodes: Vec<InferNode>,
}

impl Cluster {
    /// Default IRONHIVE cluster configuration.
    pub fn default_hive() -> Self {
        Cluster {
            nodes: vec![
                InferNode {
                    id: "n0".into(),
                    host: "192.168.1.47".into(),
                    port: 11434, // lf — direct LAN
                    model: "qwen2.5-coder:14b".into(),
                    general_model: Some("qwen2.5:7b".into()),
                    role: NodeRole::PrimaryGen,
                    tier: ModelTier::Mid,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                InferNode {
                    id: "n1".into(),
                    host: "192.168.1.44".into(),
                    port: 11434, // gd — direct LAN
                    model: "qwen2.5-coder:14b".into(),
                    general_model: Some("qwen2.5:7b".into()),
                    role: NodeRole::Reviewer,
                    tier: ModelTier::Mid,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                InferNode {
                    id: "n2".into(),
                    host: "192.168.1.45".into(),
                    port: 11434, // bt — direct LAN (150W muzzle)
                    model: "qwen2.5-coder:32b".into(),
                    general_model: Some("starcoder2:15b".into()),
                    role: NodeRole::SecondaryGen,
                    tier: ModelTier::Heavy,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                InferNode {
                    id: "n3".into(),
                    host: "192.168.1.43".into(),
                    port: 11434, // st — direct LAN
                    model: "qwen2.5-coder:14b".into(),
                    general_model: Some("qwen2.5:14b".into()),
                    role: NodeRole::Batch,
                    tier: ModelTier::Mid,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                InferNode {
                    id: "c2".into(),
                    host: "localhost".into(),
                    port: 11434, // local ollama
                    model: "qwen2.5-coder:7b".into(),
                    general_model: Some("qwen2.5:3b".into()),
                    role: NodeRole::Coordinator,
                    tier: ModelTier::Light,
                    busy: Arc::new(AtomicBool::new(false)),
                },
            ],
        }
    }

    /// Check health of all nodes. Returns vec of (node_id, online, version).
    pub fn health_check(&self) -> Vec<(String, bool, Option<String>)> {
        let handles: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                let id = node.id.clone();
                let url = node.base_url();
                std::thread::spawn(move || {
                    let online = ollama::health(&url);
                    let ver = if online { ollama::version(&url) } else { None };
                    (id, online, ver)
                })
            })
            .collect();

        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    }

    /// Get all online nodes.
    pub fn online_nodes(&self) -> Vec<&InferNode> {
        self.nodes
            .iter()
            .filter(|n| ollama::health(&n.base_url()))
            .collect()
    }

    /// Pick the best node for a given task kind.
    /// Enforces minimum tier: code gen/fix tasks never fall back to Light/Router nodes.
    pub fn pick_node(&self, task: TaskKind) -> Option<&InferNode> {
        let preferred_roles: &[NodeRole] = match task {
            TaskKind::CodeGen => &[NodeRole::PrimaryGen, NodeRole::SecondaryGen],
            TaskKind::CodeReview => &[NodeRole::Reviewer, NodeRole::Batch, NodeRole::Coordinator],
            TaskKind::TestWrite => &[NodeRole::Reviewer, NodeRole::Batch],
            TaskKind::FixCompile => &[NodeRole::PrimaryGen, NodeRole::SecondaryGen],
            TaskKind::ClippyFix => &[NodeRole::Reviewer, NodeRole::Batch],
            TaskKind::Classify => &[NodeRole::Coordinator, NodeRole::Reviewer],
            TaskKind::General => &[NodeRole::Reviewer, NodeRole::PrimaryGen, NodeRole::Batch],
        };

        // Minimum tier for the task — prevents weak nodes grabbing heavy work
        let min_tier = match task {
            TaskKind::CodeGen | TaskKind::FixCompile => ModelTier::Heavy,
            TaskKind::CodeReview | TaskKind::TestWrite | TaskKind::ClippyFix => ModelTier::Mid,
            TaskKind::Classify | TaskKind::General => ModelTier::Router,
        };

        // Try preferred roles in order, pick first non-busy online node
        for role in preferred_roles {
            for node in &self.nodes {
                if node.role == *role && !node.is_busy() && ollama::health(&node.base_url()) {
                    return Some(node);
                }
            }
        }

        // Fallback: any online non-busy node that meets the minimum tier
        self.nodes.iter().find(|n| {
            !n.is_busy()
                && tier_rank(n.tier) >= tier_rank(min_tier)
                && ollama::health(&n.base_url())
        })
    }

    /// Dispatch inference to the best node for a task.
    /// Returns (node_id, response).
    pub fn dispatch(
        &self,
        task: TaskKind,
        system: &str,
        prompt: &str,
        num_ctx: Option<u32>,
    ) -> Result<(String, String), String> {
        let node = self.pick_node(task).ok_or("no available nodes")?;
        let node_id = node.id.clone();
        let url = node.base_url();

        // Use coding model for code tasks, general model for others
        let model = match task {
            TaskKind::CodeGen
            | TaskKind::FixCompile
            | TaskKind::TestWrite
            | TaskKind::ClippyFix => &node.model,
            TaskKind::Classify | TaskKind::General | TaskKind::CodeReview => {
                node.general_model.as_deref().unwrap_or(&node.model)
            }
        };

        node.set_busy(true);
        let result = ollama::generate(&url, model, system, prompt, num_ctx);
        node.set_busy(false);

        result.map(|r| (node_id, r))
    }

    /// Dispatch with streaming. Returns (node_id, receiver).
    pub fn dispatch_stream(
        &self,
        task: TaskKind,
        system: &str,
        prompt: &str,
        num_ctx: Option<u32>,
    ) -> Result<(String, mpsc::Receiver<Arc<str>>), String> {
        let node = self.pick_node(task).ok_or("no available nodes")?;
        let node_id = node.id.clone();
        let url = node.base_url();
        let model = &node.model;

        node.set_busy(true);
        let rx = ollama::generate_stream(&url, model, system, prompt, num_ctx);
        // Note: busy flag should be cleared when stream ends — caller responsibility
        Ok((node_id, rx))
    }

    /// Fan-out: send same prompt to multiple nodes, return first response (speculative).
    pub fn speculative_dispatch(
        &self,
        task: TaskKind,
        system: &str,
        prompt: &str,
        num_ctx: Option<u32>,
    ) -> Result<(String, String), String> {
        let preferred_roles: &[NodeRole] = match task {
            TaskKind::CodeGen | TaskKind::FixCompile => {
                &[NodeRole::PrimaryGen, NodeRole::SecondaryGen]
            }
            _ => return self.dispatch(task, system, prompt, num_ctx),
        };

        let candidates: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| preferred_roles.contains(&n.role) && !n.is_busy())
            .collect();

        if candidates.is_empty() {
            return self.dispatch(task, system, prompt, num_ctx);
        }

        // Race all candidates
        let (tx, rx) = mpsc::channel();
        let _handles: Vec<_> = candidates
            .iter()
            .map(|node| {
                let tx = tx.clone();
                let url = node.base_url();
                let model = node.model.clone();
                let id = node.id.clone();
                let system = system.to_string();
                let prompt = prompt.to_string();
                let busy = Arc::clone(&node.busy);

                std::thread::spawn(move || {
                    busy.store(true, Ordering::SeqCst);
                    let result = ollama::generate(&url, &model, &system, &prompt, num_ctx);
                    busy.store(false, Ordering::SeqCst);
                    let _ = tx.send((id, result));
                })
            })
            .collect();
        drop(tx);

        // Take first successful result
        for (id, result) in rx {
            if let Ok(response) = result {
                return Ok((id, response));
            }
        }

        Err("all speculative nodes failed".into())
    }

    /// Print cluster status table.
    pub fn status(&self) -> String {
        let mut out = String::new();
        out.push_str("IRONHIVE Cluster Status\n");
        out.push_str("─────────────────────────────────────────────────────────────────\n");
        out.push_str(&format!(
            "{:<5} {:<14} {:<22} {:<10} {:<8} {}\n",
            "Node", "Role", "Model", "Tier", "Status", "Models"
        ));
        out.push_str("─────────────────────────────────────────────────────────────────\n");

        let health = self.health_check();
        for node in &self.nodes {
            let (_, online, _ver) = health
                .iter()
                .find(|(id, _, _)| id == &node.id)
                .cloned()
                .unwrap_or((node.id.clone(), false, None));

            let status = if !online {
                "OFFLINE"
            } else if node.is_busy() {
                "BUSY"
            } else {
                "READY"
            };

            let role = format!("{:?}", node.role);
            let tier = format!("{:?}", node.tier);

            let models = if online {
                match ollama::list_models(&node.base_url()) {
                    Ok(ms) => ms
                        .iter()
                        .map(|m| m.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                    Err(_) => "?".into(),
                }
            } else {
                "-".into()
            };

            out.push_str(&format!(
                "{:<5} {:<14} {:<22} {:<10} {:<8} {}\n",
                node.id, role, node.model, tier, status, models
            ));
        }

        if let Some(ver) = health
            .iter()
            .find_map(|(_, online, v)| if *online { v.clone() } else { None })
        {
            out.push_str(&format!("\nollama version: {}\n", ver));
        }

        out
    }
}

/// Convenience: create default cluster and dispatch a code gen request.
pub fn quick_gen(system: &str, prompt: &str) -> Result<String, String> {
    let cluster = Cluster::default_hive();
    cluster
        .dispatch(TaskKind::CodeGen, system, prompt, Some(8192))
        .map(|(_, r)| r)
}

/// Convenience: create default cluster and dispatch a code review request.
pub fn quick_review(system: &str, code: &str) -> Result<String, String> {
    let cluster = Cluster::default_hive();
    let prompt = format!("Review this Rust code for correctness, idiom violations, and potential issues:\n\n```rust\n{}\n```", code);
    cluster
        .dispatch(TaskKind::CodeReview, system, &prompt, Some(8192))
        .map(|(_, r)| r)
}

/// Convenience: dispatch a fix-compile request with error context.
pub fn quick_fix(system: &str, code: &str, error: &str) -> Result<String, String> {
    let cluster = Cluster::default_hive();
    let prompt = format!(
        "Fix this Rust code. The compiler error is:\n```\n{}\n```\n\nCode:\n```rust\n{}\n```\n\nReturn only the fixed code.",
        error, code
    );
    cluster
        .dispatch(TaskKind::FixCompile, system, &prompt, Some(8192))
        .map(|(_, r)| r)
}
