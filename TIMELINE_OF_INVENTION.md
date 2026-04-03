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
