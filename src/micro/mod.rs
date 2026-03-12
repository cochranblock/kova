// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! micro — Thousands of tiny AI-enabled units. Each kova function gets a purpose-built
//! micro-model: a small binary with a baked system prompt, few-shot examples, and
//! input/output schema. Shared model weights via mmap. Coordinated by a learned router.
//!
//! Architecture inspired by:
//!   - Mattbusel/tokio-prompt-orchestrator: bounded-channel pipeline, learned routing
//!     (epsilon-greedy bandit), semantic dedup, circuit breakers, self-tuning PID
//!   - Mattbusel/llm-stream et al: single-purpose micro-library pattern (26 C++ libs,
//!     each does ONE thing, zero deps, drop-in)
//!   - Mattbusel/llm_affector: focused async analysis functions (detect_hallucination,
//!     critique_code) — the micro-function model
//!   - Mattbusel/tokio-llm: provider-agnostic client with circuit breaker + budget
//!   - Mattbusel/LLM-Hallucination-Detection-Script: multi-method validation gates
//!
//! MIT-licensed concepts from github.com/Mattbusel adapted with attribution.

pub mod bench;
pub mod pipe;
pub mod registry;
pub mod router;
pub mod runner;
pub mod stats;
pub mod template;
pub mod validate;
