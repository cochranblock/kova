# Changelog

All notable changes to Kova are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
