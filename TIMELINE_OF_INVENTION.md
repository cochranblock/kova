<!-- Unlicense — cochranblock.org -->

# Timeline of Invention

*Dated, commit-level record of what was built, when, and why. Proves human-piloted AI development — not generated spaghetti.*

> Every entry below maps to real commits. Run `git log --oneline` to verify.

## How to Read This Document

Each entry follows this format:
- **Date**: When the work shipped
- **What**: Concrete deliverable
- **Why**: Business or technical reason
- **AI Role**: What the AI did vs. what the human directed

---

## Entries

### 2026-03-23 — Evolve Loop: Synthetic Data + Shuffled Training

**What:** Added synthetic data generation and shuffled training to the evolve loop. New CLI commands for training orchestration.
**Why:** Model quality depends on data diversity — synthetic augmentation + shuffle prevents overfitting.
**Commit:** `63cc6cd`
**AI Role:** AI implemented training loop changes. Human directed the data strategy and validated convergence.

### 2026-03-22 — MoE Tournament: Spark Routes + Cascade on Failure

**What:** MoE tournament routing — Spark model routes queries, cascade to stronger models on failure. KovaMoE competes as a composite.
**Commit:** `89353b3`
**AI Role:** AI built routing logic. Human designed the cascade strategy and failure thresholds.

### 2026-03-21 — Cluster Infrastructure: SSH Tunnels + Health Checks

**What:** Raw TCP health check for cluster nodes, VPN-safe SSH tunnels.
**Commit:** `b4211c5`
**AI Role:** AI wrote SSH tunnel code. Human designed the network topology and security model.

### 2026-03-20 — BPE Tokenizer + C2 Deploy Command

**What:** Custom BPE tokenizer for kova models. New `kova c2 deploy` command for pushing binaries to worker nodes.
**Commit:** `428009b`
**AI Role:** AI implemented BPE algorithm. Human specified vocabulary size and deployment targets.

### 2026-03-20 — From-Scratch Transformer Models (Spark/Flame/Blaze)

**What:** kova_model crate — three tiers of from-scratch transformer models for local inference.
**Commit:** `7e099af`
**AI Role:** AI generated model architecture. Human directed tier sizing and training data pipeline.

### 2026-03-19 — Distributed Job Queue with Circuit Breaker

**What:** C2 queue system — distributed job scheduling across 4 nodes with circuit breaker and dedup.
**Commit:** `c1265c9`
**AI Role:** AI built queue implementation. Human designed the circuit breaker thresholds and dedup logic.

### 2026-03-18 — GUI Welcome + Help Panel

**What:** egui GUI gets welcome message on first run and help panel. Phase 7 polish.
**Commit:** `248dfe5`
**AI Role:** AI generated egui layout. Human directed UX flow and copy.

### 2026-03-16 — GPU Scheduling for Training Jobs

**What:** `kova c2 gpu` — GPU scheduling across cluster nodes for training job allocation.
**Commit:** `c986dbc`
**AI Role:** AI implemented scheduler. Human designed priority system and resource constraints.

### 2026-03-10 — Initial Commit

**What:** Full augment engine: REPL, agent loop, tools, inference, cargo/git tokenization, node commands, pipeline, C2 orchestration.
**Why:** Replace fragmented AI tooling with a single, self-contained binary that does everything.
**AI Role:** AI generated code across all modules. Human architected every subsystem, directed integration, verified each component works end-to-end.

---

*170 commits in 13 days. Every decision human-directed. Every output AI-executed and human-verified.*

*Part of the [CochranBlock](https://cochranblock.org) zero-cloud architecture. All source under the Unlicense.*
