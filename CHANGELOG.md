<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

# Changelog

All notable changes to Kova are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.1] - 2026-04-02

### Added
- **Context compaction** ([`f380`](src/context_mgr.rs)): LLM-powered auto-summarize when conversation hits 80% of context budget. Keeps recent 4 turns intact, summarizes older turns via inference. Falls back to static trim.
- **Dual-mode inference** ([`f381`](src/inference/providers.rs)/[`f382`](src/inference/mod.rs)): Anthropic Messages API with real SSE streaming (`content_block_delta` parsing). Unified dispatcher reads `KOVA_INFERENCE` env: local, remote, auto. `KOVA_MODEL` overrides remote model.
- **Checkpoint/undo** ([`f383`/`f384`](src/tools.rs)): Sled snapshots before every `write_file` and `edit_file` tool execution. `undo_edit` tool restores from last checkpoint.
- **Permission gates** ([`src/tools.rs`](src/tools.rs)): `KOVA_PERMS=guarded` prompts user before shell execution and git mutations (commit, push, force, --no-verify). Default `open` preserves P3 behavior.
- **Pyramid Architecture plan** ([`docs/PYRAMID_ARCHITECTURE.md`](docs/PYRAMID_ARCHITECTURE.md)): Subatomic/molecular/cellular model tiers with shared mmap'd nanobyte weight blob. 11-model starter pack. Claude migration path to full independence.
- **Noodle** the penguin: kova's companion AI. First subatomic model proof-of-concept (inspired by Claude Code's buddy system).
- **shitty_test_detector**: Classifies tests as REAL/SMOKE/MISSING. Anti-self-licking-ice-cream-cone.
- **claim_verifier**: Flags unsourced claims in docs. README-as-test concept.
- 314 unit/integration tests passing (up from ~310, with pre-existing compile errors fixed).

### Changed
- **Tool rename**: `bash` -> `exec`. Uses `$SHELL` env var (default `/bin/sh`) instead of hardcoded shell. Backward compat alias retained.
- **Agent loop**: Now routes through [`f382`](src/inference/mod.rs) (dual-mode) instead of direct [`f76`](src/inference/local.rs) (local-only).
- **Context compaction** in agent loop replaces static [`f171`](src/context_mgr.rs) trim with LLM-powered [`f380`](src/context_mgr.rs).

### Fixed
- Pre-existing test compile errors: `extract_rust_block` import in `cargo/mod.rs`, `T95` -> `ErrorKind` in `error.rs`.
- Thread-local `CHECKPOINT_DB` for test isolation (avoids sled lock contention in parallel tests).

## [0.3.0] - 2026-03-17

### Added
- **Sprite QC panel**: Tinder-style swipe UI for pixel art quality control in egui GUI. Approve/reject/skip with keyboard (A/D/S) or mouse. Nearest-neighbor scaling for crisp pixel art display.
- **Project docs from Cursor handoff**: CODEOWNERS, CONTRIBUTING.md, OWNERS.yaml, ONBOARDING.md, Debian inference migration plan.
- **CHANGELOG.md**: Industry-standard change tracking.
- `image` crate dependency for GUI image loading.

### Changed
- OWNERS.yaml updated to reflect Foundational Founders (Claude Opus 4.6, KOVA, SuperNinja, Composer 1.5, Gemini Pro 3) instead of GitHub Copilot references.
- GUI header bar now includes Sprite QC button.

## [0.2.0] - 2026-03-16

### Added
- **KOVA_SKIP_WASM=1**: Skip kova-web WASM build for headless deploys and CI.
- **Prompt improvements**: P12 slop elimination baked into system prompt, classification few-shot examples, unsafe code review rules.
- **kova-web embedded**: Build script compiles kova-web (egui WASM) automatically when serve feature enabled. No JavaScript.
- **Pure Rust inference**: Eliminated ollama dependency. Kalosm + candle for all local LLM inference.
- **eframe/egui 0.29 to 0.33 upgrade**: Fixed macOS window visibility.
- **Feedback tournament**: Wire tournament feedback into academy, 54 tests across core modules.
- **19 edge-case tests**: State.json coverage hunt.
- **Security hardening**: Path traversal fix, UTF-8 panic fix, brace counting fix.
- **RAG module**: fastembed vector search, wired into agent loop with index-all command.
- **Git commands (g0-g9)**: Tokenized git wrapper with compressed output.
- **Agent loop**: Agentic tool loop — inference, parse, execute, feed back.
- **10 new modules + 37 tests**: Backlog batch expansion.

### Changed
- Cleaned all clippy warnings with -D warnings.
- Dead code removal and warning cleanup.

## [0.1.0] - 2026-03-12

### Added
- Initial release: CLI entrypoint, REPL, tokenized cargo/node commands.
- Swarm orchestration (C2) across 4 worker nodes.
- Native egui GUI with professional dark theme.
- Axum HTTP server with WebSocket streaming.
- sled storage with zstd + bincode serialization.
- SSH host CA for zero-churn node auth.
- 97 shell aliases for macOS + Debian.
- Micro Olympics tournament system for local LLM evaluation.