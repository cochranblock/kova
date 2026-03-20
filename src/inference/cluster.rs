//! T193 inference — distributed model dispatch across IRONHIVE nodes.
//! Routes tasks to the best available node based on role, model tier, and load.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::providers::{self, T129};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

/// Model tier — determines which tasks a node can handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T189 {
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
fn tier_rank(t: T189) -> u8 {
    match t {
        T189::Router => 0,
        T189::Light => 1,
        T189::Mid => 2,
        T189::Heavy => 3,
    }
}

/// Node role in the factory pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T190 {
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
pub enum T191 {
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
pub struct T192 {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub model: String,
    pub general_model: Option<String>,
    pub role: T190,
    pub tier: T189,
    pub busy: Arc<AtomicBool>,
}

impl T192 {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Get provider for this node's HTTP inference endpoint.
    /// Uses OpenAI-compat — kova serve acts as inference server on each node.
    pub fn provider(&self) -> T129 {
        T129::OpenAiCompat {
            url: self.base_url(),
            api_key: String::new(), // local LAN, no auth needed
            model: self.model.clone(),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn set_busy(&self, val: bool) {
        self.busy.store(val, Ordering::SeqCst);
    }
}

/// The cluster state — all inference nodes.
pub struct T193 {
    pub nodes: Vec<T192>,
}

fn parse_tier(s: &str) -> T189 {
    match s.to_lowercase().as_str() {
        "heavy" | "32b" => T189::Heavy,
        "mid" | "14b" => T189::Mid,
        "light" | "7b" => T189::Light,
        "router" | "3b" => T189::Router,
        _ => T189::Mid,
    }
}

fn parse_role(s: &str) -> T190 {
    match s.to_lowercase().as_str() {
        "primary" | "primarygen" => T190::PrimaryGen,
        "secondary" | "secondarygen" => T190::SecondaryGen,
        "reviewer" | "review" => T190::Reviewer,
        "batch" => T190::Batch,
        "coordinator" | "coord" => T190::Coordinator,
        _ => T190::Batch,
    }
}

impl T193 {
    /// Load cluster from config. Falls back to hardcoded defaults if not configured.
    pub fn default_hive() -> Self {
        let config_nodes = crate::config::cluster_nodes();
        if !config_nodes.is_empty() {
            return Self::from_config(&config_nodes);
        }
        Self::hardcoded_defaults()
    }

    /// Build cluster from config entries.
    fn from_config(nodes: &[crate::config::ClusterNodeConfig]) -> Self {
        T193 {
            nodes: nodes.iter().map(|n| T192 {
                id: n.id.clone(),
                host: n.host.clone(),
                port: n.port,
                model: n.model.clone(),
                general_model: n.general_model.clone(),
                role: parse_role(&n.role),
                tier: parse_tier(&n.tier),
                busy: Arc::new(AtomicBool::new(false)),
            }).collect(),
        }
    }

    /// Original hardcoded IRONHIVE cluster (LAN IPs).
    fn hardcoded_defaults() -> Self {
        T193 {
            nodes: vec![
                T192 {
                    id: "n0".into(),
                    host: "192.168.1.47".into(),
                    port: 3002,
                    model: "qwen2.5-coder:14b".into(),
                    general_model: Some("qwen2.5:7b".into()),
                    role: T190::PrimaryGen,
                    tier: T189::Mid,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                T192 {
                    id: "n1".into(),
                    host: "192.168.1.44".into(),
                    port: 3002,
                    model: "qwen2.5-coder:14b".into(),
                    general_model: Some("qwen2.5:7b".into()),
                    role: T190::Reviewer,
                    tier: T189::Mid,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                T192 {
                    id: "n2".into(),
                    host: "192.168.1.45".into(),
                    port: 3002,
                    model: "qwen2.5-coder:32b".into(),
                    general_model: Some("starcoder2:15b".into()),
                    role: T190::SecondaryGen,
                    tier: T189::Heavy,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                T192 {
                    id: "n3".into(),
                    host: "192.168.1.43".into(),
                    port: 3002,
                    model: "qwen2.5-coder:14b".into(),
                    general_model: Some("qwen2.5:14b".into()),
                    role: T190::Batch,
                    tier: T189::Mid,
                    busy: Arc::new(AtomicBool::new(false)),
                },
                T192 {
                    id: "c2".into(),
                    host: "localhost".into(),
                    port: 3002,
                    model: "qwen2.5-coder:7b".into(),
                    general_model: Some("qwen2.5:3b".into()),
                    role: T190::Coordinator,
                    tier: T189::Light,
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
                let prov = node.provider();
                std::thread::spawn(move || {
                    let online = providers::f334(&prov);
                    let ver = if online { providers::f335(&prov) } else { None };
                    (id, online, ver)
                })
            })
            .collect();

        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    }

    /// Get all online nodes.
    pub fn online_nodes(&self) -> Vec<&T192> {
        self.nodes
            .iter()
            .filter(|n| providers::f334(&n.provider()))
            .collect()
    }

    /// Pick the best node for a given task kind.
    /// Enforces minimum tier: code gen/fix tasks never fall back to Light/Router nodes.
    pub fn pick_node(&self, task: T191) -> Option<&T192> {
        let preferred_roles: &[T190] = match task {
            T191::CodeGen => &[T190::PrimaryGen, T190::SecondaryGen],
            T191::CodeReview => &[T190::Reviewer, T190::Batch, T190::Coordinator],
            T191::TestWrite => &[T190::Reviewer, T190::Batch],
            T191::FixCompile => &[T190::PrimaryGen, T190::SecondaryGen],
            T191::ClippyFix => &[T190::Reviewer, T190::Batch],
            T191::Classify => &[T190::Coordinator, T190::Reviewer],
            T191::General => &[T190::Reviewer, T190::PrimaryGen, T190::Batch],
        };

        // Minimum tier for the task — prevents weak nodes grabbing heavy work
        let min_tier = match task {
            T191::CodeGen | T191::FixCompile => T189::Heavy,
            T191::CodeReview | T191::TestWrite | T191::ClippyFix => T189::Mid,
            T191::Classify | T191::General => T189::Router,
        };

        // Try preferred roles in order, pick first non-busy online node
        for role in preferred_roles {
            for node in &self.nodes {
                if node.role == *role && !node.is_busy() && providers::f334(&node.provider()) {
                    return Some(node);
                }
            }
        }

        // Fallback: any online non-busy node that meets the minimum tier
        self.nodes.iter().find(|n| {
            !n.is_busy()
                && tier_rank(n.tier) >= tier_rank(min_tier)
                && providers::f334(&n.provider())
        })
    }

    /// Dispatch inference to the best node for a task.
    /// Returns (node_id, response).
    pub fn dispatch(
        &self,
        task: T191,
        system: &str,
        prompt: &str,
        _num_ctx: Option<u32>,
    ) -> Result<(String, String), String> {
        let node = self.pick_node(task).ok_or("no available nodes")?;
        let node_id = node.id.clone();

        // Use coding model for code tasks, general model for others
        let model = match task {
            T191::CodeGen
            | T191::FixCompile
            | T191::TestWrite
            | T191::ClippyFix => &node.model,
            T191::Classify | T191::General | T191::CodeReview => {
                node.general_model.as_deref().unwrap_or(&node.model)
            }
        };

        node.set_busy(true);
        let result = providers::f199(&node.provider(), model, system, prompt);
        node.set_busy(false);

        result.map(|r| (node_id, r.text))
    }

    /// Dispatch with streaming. Returns (node_id, receiver).
    pub fn dispatch_stream(
        &self,
        task: T191,
        system: &str,
        prompt: &str,
        _num_ctx: Option<u32>,
    ) -> Result<(String, mpsc::Receiver<Arc<str>>), String> {
        let node = self.pick_node(task).ok_or("no available nodes")?;
        let node_id = node.id.clone();
        let model = &node.model;

        node.set_busy(true);
        let rx = providers::f337(&node.provider(), model, system, prompt);
        // Note: busy flag should be cleared when stream ends — caller responsibility
        Ok((node_id, rx))
    }

    /// Fan-out: send same prompt to multiple nodes, return first response (speculative).
    pub fn speculative_dispatch(
        &self,
        task: T191,
        system: &str,
        prompt: &str,
        num_ctx: Option<u32>,
    ) -> Result<(String, String), String> {
        let preferred_roles: &[T190] = match task {
            T191::CodeGen | T191::FixCompile => {
                &[T190::PrimaryGen, T190::SecondaryGen]
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
                    let provider = T129::OpenAiCompat {
                        url,
                        api_key: String::new(),
                        model: model.clone(),
                    };
                    let result = providers::f199(&provider, &model, &system, &prompt)
                        .map(|r| r.text);
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
        out.push_str("IRONHIVE T193 Status\n");
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
                match providers::f336(&node.provider()) {
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
pub fn f338(system: &str, prompt: &str) -> Result<String, String> {
    let cluster = T193::default_hive();
    cluster
        .dispatch(T191::CodeGen, system, prompt, Some(8192))
        .map(|(_, r)| r)
}

/// Convenience: create default cluster and dispatch a code review request.
pub fn f339(system: &str, code: &str) -> Result<String, String> {
    let cluster = T193::default_hive();
    let prompt = format!("Review this Rust code for correctness, idiom violations, and potential issues:\n\n```rust\n{}\n```", code);
    cluster
        .dispatch(T191::CodeReview, system, &prompt, Some(8192))
        .map(|(_, r)| r)
}

/// Convenience: dispatch a fix-compile request with error context.
pub fn f340(system: &str, code: &str, error: &str) -> Result<String, String> {
    let cluster = T193::default_hive();
    let prompt = format!(
        "Fix this Rust code. The compiler error is:\n```\n{}\n```\n\nCode:\n```rust\n{}\n```\n\nReturn only the fixed code.",
        error, code
    );
    cluster
        .dispatch(T191::FixCompile, system, &prompt, Some(8192))
        .map(|(_, r)| r)
}