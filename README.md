<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

> **It's not the Mech — it's the pilot.**
>
> This repo is part of [CochranBlock](https://cochranblock.org) — 8 Unlicense Rust repositories that power an entire company on a **single <10MB binary**, a laptop, and a **$10/month** Cloudflare tunnel. No AWS. No Kubernetes. No six-figure DevOps team. Zero cloud.
>
> **[cochranblock.org](https://cochranblock.org)** is a live demo of this architecture. You're welcome to read every line of source code — it's all public domain.
>
> Every repo ships with **[Proof of Artifacts](PROOF_OF_ARTIFACTS.md)** (wire diagrams, screenshots, and build output proving the work is real) and a **[Timeline of Invention](TIMELINE_OF_INVENTION.md)** (dated commit-level record of what was built, when, and why — proving human-piloted AI development, not generated spaghetti).
>
> **Looking to cut your server bill by 90%?** → [Zero-Cloud Tech Intake Form](https://cochranblock.org/deploy)

---

<p align="center">
  <img src="https://raw.githubusercontent.com/cochranblock/kova/main/assets/logo.png" alt="Kova" width="120">
</p>

# Kova

Augment engine. Local-first agentic tool loop with dual-mode inference, swarm orchestration, and tokenized everything.

## What Works Today

### Agent Loop (`src/agent_loop.rs`)

Streaming agentic tool loop. LLM calls tools, gets results, repeats until done. Dual-mode inference via `KOVA_INFERENCE` env — local Kalosm GGUF or Anthropic API with SSE streaming. Context auto-compacts at 80% of budget using LLM-powered summarization. File checkpoints taken before every write/edit for undo support.

### Tools (`src/tools.rs`, 2,017 lines)

13 tools available to the agent:

| Tool | Purpose |
|------|---------|
| `read_file` | Read file contents with optional offset/limit |
| `write_file` | Write content to file, auto-creates dirs |
| `edit_file` | Find-and-replace exact text (must be unique match) |
| `exec` | Shell command execution via `$SHELL` (default `/bin/sh`) |
| `glob` | Find files matching glob patterns |
| `grep` | Search file contents for text patterns |
| `memory_write` | Append to persistent memory (`~/.kova/memory.md`) |
| `code_review` | LLM-powered code review with severity scoring |
| `code_outline` | Extract functions, structs, enums from Rust source |
| `record_failure` | Record challenge failure for training feedback loop |
| `undo_edit` | Restore file from last checkpoint (sled-backed) |
| `rag_search` | Semantic search over indexed codebase (requires `rag` feature) |
| `pixel_forge` | Generate pixel art sprites via Pixel Forge plugin |

Permission gates: `KOVA_PERMS=guarded` prompts before shell execution and git mutations. Default is `open` (no prompts).

### REPL (`src/repl.rs`)

Interactive chat. `kova` with no args starts the REPL. Loads system prompt from persona, project context, memory, and tool definitions. Routes through `f382` (local/remote/auto inference). Commands: `/exit`, `/clear`, `/project <path>`, `/tools`.

### C2 Swarm Orchestration (`src/c2.rs`, 1,309 lines)

Distributed build and command execution across 4 worker nodes:

- **Broadcast build**: One-command sync + `cargo build --release` on all nodes
- **tmux dispatch** (`f377`): Send to tmux pane with retry + exponential backoff
- **tmux broadcast** (`f378`): Send to all windows with stagger delay
- **sponge mesh** (`f379`): Fast pass + rate-limit-aware retry with backoff
- **Node commands** (`c1-c9`, `ci`): Tokenized SSH commands (status, specs, services, sync, build, deploy)
- **Wake-on-LAN**: Wake sleeping nodes
- **SSH host certificates**: Zero-churn host key management
- **Binary deploy**: rsync kova + models to nodes, restart services

### Inference (`src/inference/`)

Three backends, one dispatcher:

| Backend | Module | Method |
|---------|--------|--------|
| Local GGUF | `inference/local.rs` | Kalosm + candle, LRU model cache, streaming |
| Anthropic API | `inference/providers.rs` | SSE streaming, `content_block_delta` parsing |
| IRONHIVE cluster | `inference/cluster.rs` | Distributed dispatch across worker nodes |

`f382` (dual_stream) reads `KOVA_INFERENCE` env: `local`, `remote`, or `auto` (default — local if model exists, else Anthropic API). `KOVA_MODEL` overrides the remote model.

### Context Management (`src/context_mgr.rs`, 549 lines)

- Token estimation: chars/4 rough count
- Context compaction (`f380`): When conversation hits 80% of budget, older turns are sent to inference for LLM-powered summarization. Recent 4 turns kept intact. Falls back to static trim if needed.
- Tool output trimming: Head/tail with `[truncated]` marker
- File checkpointing (`f383`/`f384`): Snapshots file contents to sled before write/edit. `undo_edit` tool restores from last checkpoint.

### Micro Olympics (`src/micro/`)

Local LLM tournament system. Models compete across weight classes and event types.

- **Tournament** (`tournament.rs`): 6 event types (sprint, technical, freestyle, judged, endurance, anti-slop), weight class brackets, DQ mechanism
- **Training** (`candle_train.rs`): Pure Rust transformer training via candle. Three tiers (Spark 50K, Flame 500K, Blaze 2M). BPE tokenizer trained from scratch.
- **Quantization** (`quantize.rs`): TurboQuant — FWHT + mixed-precision 2/4-bit + QJL residual recovery
- **Routing** (`router.rs`): Epsilon-greedy bandit for template selection
- **Validation** (`validate.rs`): Completeness, coherence, format, confidence checks
- **Pipeline** (`pipe.rs`): Classify -> route -> run -> validate

### Code Generation Pipeline (`src/factory.rs`, `src/moe.rs`, `src/academy.rs`)

- **Factory** (`factory.rs`): 6-stage pipeline — classify, generate, compile, review, fix loop, output
- **MoE** (`moe.rs`): Fan-out to N expert nodes, compile all variants, score, pick winner
- **Academy** (`academy.rs`): Autonomous dev agent — task breakdown, code gen, test, commit
- **Gauntlet** (`gauntlet.rs`): 5-phase stress test (crawl, walk, run, fight, survive)

### Surfaces

| Surface | Module | Status |
|---------|--------|--------|
| TUI | `src/tui.rs` (1,672 lines) | Ratatui terminal UI — agent chat, visual QC |
| Native GUI | `src/gui.rs` (1,660 lines) | egui desktop — REPL, backlog, sprite QC |
| HTTP API | `src/serve.rs` (1,351 lines) | Axum + WebSocket streaming + embedded WASM client |
| MCP Server | `src/mcp.rs` (508 lines) | Model Context Protocol via JSON-RPC stdio |
| WASM Client | `src/web_client/` | egui in browser via `kova s` |

### Other Working Modules

| Module | Lines | Purpose |
|--------|-------|---------|
| `config.rs` | 791 | Config, paths, feature detection, model resolution |
| `rag.rs` | 749 | fastembed vectors, sled index, chunk + search Rust files |
| `feedback.rs` | 702 | Failure recording, harder challenge generation, DPO loop |
| `syntax.rs` | 636 | Symbol extraction from Rust source files |
| `review.rs` | 477 | LLM code review: staged, branch diff, severity scoring |
| `git_cmd.rs` | 450 | Tokenized git commands (g0-g9), compressed output |
| `ci.rs` | 387 | CI mode: headless quality gate, watch for changes |
| `imagegen.rs` | 383 | Image generation: Stable Diffusion, DALL-E dispatch |
| `training_data.rs` | 375 | Trace -> DPO/SFT/CSV export for fine-tuning |
| `tokenization.rs` | 308 | Compression protocol validator |

---

## Planned: Pyramid Architecture

> **Status: Design complete, not yet implemented.** See [`docs/PYRAMID_ARCHITECTURE.md`](docs/PYRAMID_ARCHITECTURE.md) for the full plan.

The next major initiative: replace external API dependency with a pyramid of locally-trained models.

- **Tier 1 — Subatomic** (sub-100K params): Hundreds of single-task specialists. Typo fix, binary classify, flag expand, token tag. Microsecond inference.
- **Tier 2 — Molecular** (100K-1M params): Coordinators with learned routing weights to subatomics. Intent routing, context summarization, tool selection.
- **Tier 3 — Cellular** (1M-10M params): Domain specialists. Code generation, conversation, planning.

All tiers share a single mmap'd weight blob called a **nanobyte**. Each model is a Rust function reading from different byte offsets. Cross-tier routing weights are trained, not hardcoded. Confidence gating means most requests never get past tier 1.

Claude trains its own replacement at every level via PTY bridge logging. End state: fully closed pyramid, zero external API dependency.

**What exists toward this goal:** candle training pipeline, tournament scoring, DPO/SFT export, TurboQuant quantization, epsilon-greedy routing, validation gates, circuit breakers. **What's not built yet:** nanobyte format, swarm.rs pyramid orchestrator, PTY bridge, discovery module, the trained models themselves.

---

## Crate Structure

```
kova/             — single crate, 103 Rust source files, ~41,600 lines
  src/            — all source
  src/web_client/ — WASM thin client (cross-compiled via wasm/)
exopack/          — test augmentation library (separate crate)
wasm/             — WASM build manifest
```

## Tokenization

100% compression protocol coverage. Every public function and type is tokenized.

```
$ kova tokens
tokenization: 100.0% (368/368)
  fn: 231/231 tokenized (highest: f384)
  ty: 137/137 tokenized (highest: T215)
```

Canonical map: [`docs/compression_map.md`](docs/compression_map.md)

## Worker Nodes

| Token | Host | Role |
|-------|------|------|
| n0/lf | kova-legion-forge | Primary build |
| n1/gd | kova-tunnel-god | Tunnel/relay |
| n2/bt | kova-thick-beast | Heavy compute |
| n3/st | kova-elite-support | Support/backup |

## Supported Platforms

| Platform | Binary | Size | Status |
|----------|--------|------|--------|
| macOS ARM64 (M1/M2/M3/M4) | `kova-macos-arm64` | 27 MB | Full (all features) |
| macOS x86_64 (Intel) | `kova-macos-x86_64` | 13 MB | No RAG (ort lacks x86 prebuilts) |
| Android arm64-v8a | `kova-android.apk` / `.aab` | 17 MB / 6.6 MB | GUI + mobile-llm |
| Linux x86_64 (Debian) | Build on node | ~25 MB | Full (build on st/gd) |
| iOS arm64 | `libkova_ios.a` | scaffold | staticlib, needs Xcode |
| Web (PWA) | WASM + service worker | ~2.5 MB | Offline-first, installable |
| Snap (Linux) | `snap/snapcraft.yaml` | — | core22, classic confinement |

## Binaries

| Binary | Features | Purpose |
|--------|----------|---------|
| `kova` | serve, inference, rag, tui | All-inclusive: TUI, GUI, HTTP, LLM, swarm, tools |
| `kova-test` | tests (exopack) | Quality gate: clippy, TRIPLE SIMS 3x, release build |

## Build

```sh
cargo build                          # default (serve + inference + rag + tui)
cargo build --release --features serve --target aarch64-apple-darwin
cargo run --features tests --bin kova-test   # quality gate
cargo test --release -p kova                 # 314 unit/integration tests
kova tokens                          # validate tokenization coverage
```

## Features

```toml
default    = ["serve", "inference", "rag", "tui"]
serve      = axum + tower + tracing (+ WASM thin client)
gui        = eframe + egui (native desktop)
tui        = ratatui + crossterm (terminal UI)
inference  = kalosm + reqwest + lru
mobile-llm = candle-core + candle-nn (on-device training/inference)
autopilot  = enigo (type into Cursor)
daemon     = capnp (worker node)
tests      = exopack (quality gate)
rag        = fastembed + ordered-float
```

## Environment Variables

| Variable | Values | Purpose |
|----------|--------|---------|
| `KOVA_INFERENCE` | `local`, `remote`, `auto` | Inference backend selection (default: auto) |
| `KOVA_MODEL` | model name | Override remote model (default: claude-sonnet-4-6) |
| `KOVA_PERMS` | `open`, `guarded` | Permission mode (default: open) |
| `ANTHROPIC_API_KEY` | API key | Required for remote inference |

## Tests

314 tests passing. Run with `cargo test --release -p kova`.

Coverage includes: tool dispatch, context compaction thresholds, checkpoint/undo roundtrips, permission gate logic, git mutation detection, tool parsing, code outline, file operations, CI pipeline, integration tests.

---

Unlicense — cochranblock.org
