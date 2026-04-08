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

## Human Revelations — Invented Techniques

*Novel ideas that came from human insight, not AI suggestion. These are original contributions to the field.*

### P13 Compression Mapping (March 2026)

**Invention:** A human-designed tokenization scheme that compresses all public symbols in a Rust codebase to short tokens (f0-fN for functions, t0-tN for types, s0-sN for fields), reducing AI context consumption by 40-60% per session.

**The Problem:** AI coding assistants consume tokens proportional to symbol length. A function named `handle_cargo_build_with_features` costs 6x more context than `f134`. Across a session with hundreds of references, this blows through context windows and increases latency.

**The Insight:** Military communications have used brevity codes for decades — "WILCO" instead of "Will comply with your last transmission." The same principle applies to AI-human code collaboration. If every symbol in the codebase has a short, unique token, AI sessions run faster, cost less, and fit more context.

**The Technique:**
1. Every public function gets `f` + sequential number (f0, f1, ... f392)
2. Every public type gets `t` + number (t0, t1, ... t108)
3. Every struct field gets `s` + number
4. A canonical `compression_map.md` per project maps tokens to human-readable names
5. Cargo commands become `x0`-`x9`, git commands `g0`-`g9`, node commands `c1`-`c9`
6. Applied uniformly across 16+ repositories

**Result:** 40-60% reduction in tokens consumed per AI session. Same codebase, same functionality, dramatically lower AI operational cost. Every CochranBlock project uses P13.

**Named:** P13 Compression Mapping
**Commit:** See `1012a05` (first kova tokenization) and per-project compression commits
**Origin:** Military brevity codes (WILCO, SITREP, CASEVAC). Michael Cochran's 13 years in Army signals/cyber (17C). Applied to AI-human collaboration as a token economy measure.

### Agentic Tool Loop with Tokenized Commands (March 2026)

**Invention:** An AI agent loop where the LLM calls tools (read/write/edit/bash/glob/grep) iteratively until the task is complete, with all system commands pre-compressed into tokenized aliases that produce compressed output.

**The Problem:** AI coding agents either (a) generate code and hope it works, or (b) run tools but consume massive context with verbose command output. `cargo build` dumps hundreds of lines. `git status` is chatty. Every tool call eats context budget.

**The Insight:** Combine two ideas: (1) let the AI call tools in a loop until it decides it's done (agentic), and (2) make every tool call produce the minimum possible output (tokenized). The agent doesn't need `cargo build`'s full output — it needs "pass" or "fail + first error."

**The Technique:**
1. `agent_loop.rs` (f147-f148): LLM calls tools, receives results, decides next action, repeats until done
2. `cargo_cmd.rs` (f133-f136): `kova x x0` through `x9` — cargo commands that parse output into compressed JSON
3. `git_cmd.rs` (f156-f160): `kova git g0` through `g9` — git commands with compressed output
4. `node_cmd.rs` (f122-f132): `kova c2 ncmd c1` through `c9` — SSH node commands with compressed output
5. Every tool targets <100 tokens of output (P25)

**Result:** An AI agent that can build, test, deploy, and manage infrastructure while consuming 5-10x fewer tokens than raw command output. The agent works faster because it processes less noise.

**Named:** Tokenized Agentic Loop
**Commit:** See initial commit and `agent_loop.rs`, `cargo_cmd.rs`, `git_cmd.rs`
**Origin:** Frustration with AI assistants that dump 500 lines of `cargo build` output into context. Michael Cochran realized the AI doesn't need the output — it needs the result.

### C2 Swarm Orchestration (March 2026)

**Invention:** A command-and-control system that treats 4 heterogeneous worker nodes (different GPUs, different architectures) as a single distributed compute fabric, orchestrated from a Mac Mini over SSH with circuit breakers and job deduplication.

**The Problem:** Solo developers with multiple machines (desktop, NUC, old laptop) can't easily distribute work across them. Cloud GPU is expensive. The machines sit idle 90% of the time.

**The Insight:** Military C2 (command and control) systems don't require homogeneous units. A squad has riflemen, a SAW gunner, a grenadier — different capabilities, one mission. Apply the same model to heterogeneous compute nodes: each node has different GPU/CPU/RAM, but the C2 layer abstracts that into "capabilities" and routes work accordingly.

**The Technique:**
1. 4 nodes: lf (RTX 3070), gd (RTX 3050 Ti), bt (RX 5700 XT), st (CPU-only)
2. `c2.rs` (f119-f121): distributed job queue with circuit breaker, dedup, priority scheduling
3. `node_cmd.rs`: tokenized SSH commands (c1=status, c2=specs, ci=inspect)
4. GPU scheduling: file-based lock + priority queue across nodes
5. Health checks, WoL (Wake-on-LAN) for sleeping nodes, autossh tunnels

**Result:** A solo developer with 4 consumer machines has a private GPU cluster. Training jobs route to the right node. Builds compile on the right architecture. No cloud, no Kubernetes, no orchestration framework — just SSH and Rust.

**Named:** IRONHIVE C2
**Commit:** `c1265c9` (distributed job queue), `c986dbc` (GPU scheduling), `b4211c5` (cluster infra)
**Origin:** Army C2 doctrine applied to consumer hardware. Michael Cochran's experience as a 17C (Cyber Operations Specialist) managing distributed assets.

### tmuxisfree Fleet Mesh (April 2026)

**Invention:** A tmux session manager that treats AI agent panes as a fleet of workers — dispatching tasks, broadcasting commands, detecting rate limits, and auto-retrying with exponential backoff across multiple AI coding agents running in parallel.

**The Problem:** Running multiple AI coding agents (Claude Code, Cursor, etc.) in tmux panes requires manual window switching, copy-pasting prompts, and monitoring each pane for completion. Rate limits from AI providers cause agents to stall silently.

**The Insight:** This is a fleet management problem, not a terminal multiplexer problem. Each AI agent pane is a "vehicle" that can be IDLE or WORKING. A fleet dispatcher should be able to send tasks to idle vehicles, broadcast to all, and handle the fact that AI providers rate-limit — which means some vehicles will stall and need retry.

**The Technique:**
1. `tf0` (status): scan all panes, report IDLE/WORK state
2. `tf1` (dispatch): send task to one pane with retry + backoff
3. `tf3` (sponge): mesh broadcast — skip rate-limited panes, retry later
4. `tf5` (unblock): daemon that auto-approves prompts and flushes paste buffers
5. `tfp`/`tfpp`/`tfdr` (push/pop/drain): backlog queue per pane with auto-dispatch

**Result:** A solo developer can run 4-8 AI agents in parallel, dispatch tasks from a C2 pane, and the fleet self-manages rate limits and retries. Multiplies AI throughput by the number of available panes.

**Named:** Sponge Mesh Broadcast (tf3)
**Commit:** See tmuxisfree repo
**Origin:** Military convoy operations — vehicles that break down get skipped, convoy continues, recovery vehicle comes back for them. Applied to AI agent fleet management.

### 2026-04-08 — Human Revelations Documentation Pass

**What:** Documented novel human-invented techniques across the full CochranBlock portfolio. Added Human Revelations sections to all 13 project TOIs (kova, pixel-forge, approuter, exopack, ghost-fabric, whyyoulying, call-shield, rogue-repo, oakilydokily, ronin-sites, wowasticker, pocket-server, provenance-docs).
**Commit:** See git log
**AI Role:** AI formatted and wrote the sections. Human identified which techniques were genuinely novel, provided the origin stories, and directed the documentation pass.

---

## Entries

### 2026-04-03 — Subatomic Models Trained + NanoSign + P23 + Blueprint

**What:** Trained first 3 subatomic models on bt's AMD RX 5700 XT via any-gpu Vulkan: slop_detector (514 params, 89.4%), code_vs_english (514 params, 94.2%), lang_detector (1,285 params, 97.0%). Built swarm training infrastructure ([`src/swarm/train.rs`](src/swarm/train.rs), f389-f392). Harvested 240,596 crates from crates.io to bt `/mnt/data/crates/` (34GB). Published NanoSign spec ([`docs/NANOSIGN.md`](docs/NANOSIGN.md)) — universal AI model signing (36 bytes, BLAKE3, any format). Created P23 Triple Lens Research Protocol. Published 66-model subatomic catalog ([`docs/SUBATOMIC_CATALOG.md`](docs/SUBATOMIC_CATALOG.md)). Consolidated full blueprint ([`docs/KOVA_BLUEPRINT.md`](docs/KOVA_BLUEPRINT.md)). Added C2 fleet commands: status (f385), peek (f386), unblock daemon (f387), QA sweep (f388).
**Commits:** `a4da3f6`, `80965f6`, `2c6d647`, `f244483`, `5671ca3`, `c36064c`, `96f8245`, `329c7cd`
**AI Role:** AI implemented training pipeline, ran GPU training on bt, designed NanoSign spec, wrote P23 protocol, consolidated blueprint. Human directed model selection, architecture principles (sled priority queue, intent-driven priority, shared models, one-document consolidation), NanoSign as open standard.

### 2026-04-02 — Pyramid Architecture + Docs Audit

**What:** Designed full Pyramid Architecture ([`docs/PYRAMID_ARCHITECTURE.md`](docs/PYRAMID_ARCHITECTURE.md)): subatomic/molecular/cellular model tiers with shared mmap'd nanobyte weight blob. Named Noodle the penguin as companion AI (inspired by Claude Code's buddy system). Added shitty_test_detector (REAL/SMOKE/MISSING classifier), claim_verifier (README-as-test), 11-model starter nanobyte pack. Cross-referenced every README claim to source file. Full docs audit.
**Commits:** `43a7c4d`, `373edd9`, `a24106d`, `558da7f`, `07dbc4a`, `8ec4709`, `c2e2e92`, `cda7462`
**AI Role:** AI wrote architecture plan, model specs, README cross-references. Human directed pyramid vision, naming, and model selection.

### 2026-04-01 — Claude Code Architecture Fusion

**What:** Fused Claude Code patterns into kova. Context compaction ([`f380`](src/context_mgr.rs)): LLM-powered auto-summarize at 80% context threshold. Dual-mode inference ([`f381`](src/inference/providers.rs)/[`f382`](src/inference/mod.rs)): Anthropic SSE streaming + local/remote/auto dispatch via `KOVA_INFERENCE` env. Checkpoint/undo ([`f383`/`f384`](src/tools.rs)): sled snapshots before every file write/edit. Exec tool rename (bash->exec) + permission gates (`KOVA_PERMS=guarded`). Fixed pre-existing test compile errors. 314 tests passing.
**Commits:** `6efab67`, `3fa3080`, `6091dad`, `e8a0cb0`, `a79be81`
**AI Role:** AI implemented all features, fixed test infrastructure. Human directed priorities and architecture decisions.

### 2026-03-29 — Multi-Architecture Release

**What:** Built for macOS ARM64 (27 MB), macOS x86_64 (13 MB), Android AAB (6.6 MB), Android APK (17 MB). iOS scaffold (staticlib crate). PWA manifest + service worker. build-all-targets.sh. All artifacts uploaded to GitHub Release v0.7.0.
**Commit:** `c876698`
**AI Role:** AI built cross-compile pipeline, diagnosed ort-sys x86_64 incompatibility. Human directed target list.

### 2026-03-29 — Snap Store Packaging

**What:** snapcraft.yaml for kova (core22, classic confinement, amd64+arm64). cochranblock-stack snap for approuter repo (3 daemons: approuter + cochranblock + cloudflared).
**Commit:** `93f3102`
**AI Role:** AI wrote both snapcraft configs. Human directed snap architecture.

### 2026-03-28 — C2 CLI Tools + Govdocs Subcommand

**What:** 5 new c2 subcommands: dispatch (single node SSH), broadcast (parallel fan-out), status (health check), monitor (continuous poll), fleet (project overview). Govdocs subcommand: 11 compliance docs baked into binary via include_str!.
**Commit:** `0398ed4`
**AI Role:** AI implemented all handlers. Human designed the C2 dispatch architecture.

### 2026-03-27 — Federal Compliance Documentation

**What:** 11 govdocs created: SBOM (EO 14028), SSDF (NIST 800-218), supply chain, security posture, Section 508 accessibility, privacy impact, FIPS gap analysis, FedRAMP notes, CMMC mapping, ITAR/EAR export control, federal agency use cases.
**Commit:** `7c474da`
**AI Role:** AI generated all documents. Human directed scope (which frameworks matter) and verified claims trace to source.

### 2026-03-27 — User Story Analysis + TTY Fix + Deploy Alias

**What:** Full user walkthrough as new user (scored 6/10). Fixed: REPL TTY error message ("Device not configured" → helpful message), added `kova deploy` subcommand (shortcut for c2 build --broadcast --release).
**Commit:** `4228073`
**AI Role:** AI simulated real user, identified pain points, implemented fixes. Human reviewed and approved.

### 2026-03-27 — P13 Tokenization + Binary Size Optimization

**What:** Tokenized quantize.rs (f366-f376, T214-T215). Release profile: opt-level=z, LTO, codegen-units=1, panic=abort, strip. Binary: 54 MB → 27 MB (48% reduction).
**Commit:** `1012a05`
**AI Role:** AI renamed symbols and configured profile. Human directed compression map numbering.

### 2026-03-27 — Clippy Clean + QA Round 2

**What:** Fixed deprecated screen_rect, removed dead fields, collapsed if-let chains across 5 files. QA Round 2: clean build, clippy -D warnings pass on all feature sets.
**Commit:** `9937c7f`
**AI Role:** AI fixed all clippy issues. Human ran QA gate.

### 2026-03-27 — Bash Tool Timeout Enforcement

**What:** f145 bash tool now enforces timeout — polls with try_wait in 50ms loop, kills after deadline. Default 120s. Previously parsed but never used.
**Commit:** `739bd65`
**AI Role:** AI found the bug during audit and implemented fix. Human directed the audit.

### 2026-03-27 — Google Play Deploy Pipeline

**What:** android/deploy-play.sh: cargo ndk → copy .so → bundleRelease → fastlane supply. Upload keystore generated. AAB: 6.6 MB.
**Commit:** `3ead4ae`
**AI Role:** AI wrote the script and gradle config. Human directed the pipeline architecture.

### 2026-03-27 — GUI Mobile UX: Bottom Tab Bar + Node Cards + Toasts

**What:** Bottom tab bar (Chat/Deploy/MoE, 56pt buttons), node cards (72pt, toggle+WoL), toast notifications (3s auto-dismiss), offline handling (Retry buttons), periodic 30s cluster re-check, landscape detection, proof card collapsed by default.
**Commit:** `357235f`
**AI Role:** AI implemented all UI changes. Human designed the UX specifications.

### 2026-03-27 — Android APK: Patch android-activity for Rust 1.94

**What:** Patched android-activity 0.6.0 Arc::as_ptr inference bug. Switched to game-activity backend. Fixed micro_train feature gates. .so: 17 MB, APK: 17 MB.
**Commit:** `fb0b380`
**AI Role:** AI diagnosed and patched the upstream Rust toolchain incompatibility. Human directed the investigation.

### 2026-03-27 — GUI Tabs: Chat, Deploy, MoE

**What:** Tab bar with 3 tabs. Deploy tab: node grid, command builder, WoL buttons, c2::f354 wiring. MoE tab: prompt + remote_moe() + results grid. Theme update: BG=#050508, PRIMARY=#00d9ff.
**Commit:** `8e75231`
**AI Role:** AI wrote all egui layout code. Human designed tab structure and feature set.

### 2026-03-27 — Browser Feature Gates + Clippy Clean

**What:** Gated all browser-only functions behind #[cfg(feature = "browser")]. Fixed collapsible_if across 12 files. Switched exopack to path dependency.
**Commits:** `23ed702`, `366230b`
**AI Role:** AI fixed all compile errors and clippy warnings. Human directed the feature gate strategy.

### 2026-03-27 — MoE Phase 2-5: Training + Routing + TurboQuant

**What:** Phase 4: dropout, padding mask, class-weighted loss. Phase 3b: confidence-weighted routing. Phase 2: evolve-full pipeline, model versioning, mine-classifier. Phase 5: quantize.rs — FWHT, mixed-precision, QJL residual. New commands: evolve-full, mine-classifier, quantize.
**Commit:** `1dee3af`
**AI Role:** AI implemented all phases. Human directed the TurboQuant technique selection and MoE architecture.

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

*185+ commits in 17 days. Every decision human-directed. Every output AI-executed and human-verified.*

*Part of the [CochranBlock](https://cochranblock.org) zero-cloud architecture. All source under the Unlicense.*
