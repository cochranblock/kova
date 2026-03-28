# NIST SP 800-218 — Secure Software Development Framework (SSDF)

**Product:** kova v0.7.0
**Date:** 2026-03-27

This document maps kova's development practices to the NIST SSDF (SP 800-218) practice groups.

---

## PS: Prepare the Organization

### PS.1 — Define Security Requirements

- Security requirements documented in `CLAUDE.md` (project root): crypto standards (AES-256-GCM, HKDF, Argon2id), no plaintext secrets, embedded secrets policy (P17).
- Error handling mandate: `thiserror` for library errors (`src/error.rs`, `src/storage.rs`), `anyhow` for application errors. No panics in production paths.
- Anti-pattern list enforced: no circular dependencies (P15), no external test frameworks (P16).

### PS.2 — Implement Roles and Responsibilities

- Build process documented in `CLAUDE.md` and `Cargo.toml`.
- Contributor list in file headers: `Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3`.
- All source files carry Unlicense header.

### PS.3 — Implement Supporting Toolchains

- Rust toolchain: `rustc` + `cargo` (edition 2024).
- Linter: `clippy -D warnings` (zero tolerance for warnings).
- Dependencies: all from crates.io, pinned via `Cargo.lock`.
- No vendored binaries. No pre-built artifacts in the repository.

**Source:** `Cargo.toml` (lines 1-102), `CLAUDE.md`.

---

## PW: Protect the Software

### PW.1 — Design Software to Meet Security Requirements

- Single binary architecture: one attack surface, one deployment artifact.
- Local-only storage via sled (`src/storage.rs`). No cloud database. No network-accessible data store.
- Crypto in `src/secrets.rs` (design) using AES-256-GCM for encryption, HKDF for key derivation, Argon2id for password hashing.
- SSH node access via certificate authority (`src/ssh_ca.rs`), eliminating host key trust-on-first-use.

### PW.4 — Reuse Existing, Well-Secured Software

- 31 direct dependencies, all from crates.io (see `govdocs/SBOM.md`).
- Crypto primitives from established Rust crates, not hand-rolled.
- TLS via `rustls` (no OpenSSL). Configured in `reqwest` with `rustls-tls` feature.

### PW.5 — Create Source Code by Adhering to Secure Coding Practices

- `Cargo.toml` line 97-101: release profile uses `panic = 'abort'` (no unwinding exploits), `strip = true` (no debug symbols in release).
- `clippy -D warnings` enforced. Warnings are build failures.
- Type-safe CLI via `clap` derive macros (`src/main.rs`). No raw string parsing for user input.
- Error types via `thiserror` (`src/storage.rs` E0, `src/error.rs`). Structured error propagation, no string-based errors.

### PW.6 — Configure the Compilation, Interpreter, and Build Processes

- `Cargo.lock` committed to repository. All builds are reproducible.
- Release profile (`Cargo.toml` lines 96-101): `opt-level = 'z'`, `lto = true`, `codegen-units = 1`.
- Feature gates isolate optional functionality: `serve`, `gui`, `tui`, `inference`, `rag`, `browser`, `daemon`, `mobile-llm`.

### PW.7 — Review and/or Analyze Human-Readable Code

- LLM-assisted code review via `kova review` command (`src/review.rs`).
- TRIPLE SIMS test gate: three independent `cargo test` runs to catch flaky tests.
- Quality gate binary: `kova-test` (`src/bin/kova-test.rs`) runs clippy + TRIPLE SIMS + release build + smoke tests.

### PW.9 — Test Executable Code

- Test pipeline (P16): compilation, unit tests, integration tests, HTTP tests, exit code verification.
- Test binary IS the CI pipeline: `cargo run -p kova --bin kova-test --features tests`.
- No external test frameworks. The test harness is `exopack` (workspace crate, `src/bin/kova-test.rs`).
- `tempfile` crate used for test isolation (real filesystem, temp directories).

**Source:** `Cargo.toml` (features, profile), `src/bin/kova-test.rs`, `src/storage.rs`, `src/ssh_ca.rs`.

---

## RV: Respond to Vulnerabilities

### RV.1 — Identify and Confirm Vulnerabilities

- GitHub Issues for vulnerability reporting (public repository).
- `cargo audit` available for known CVE scanning against `Cargo.lock`.
- Dependency versions pinned; updates are explicit and reviewed.

### RV.2 — Assess, Prioritize, and Remediate Vulnerabilities

- Single binary simplifies patch deployment: rebuild and replace one file.
- `cargo update` + test gate validates dependency updates before release.

### RV.3 — Analyze Vulnerabilities to Identify Root Causes

- Automated test pipeline via exopack catches regressions.
- `kova gauntlet` stress-tests the AI pipeline across 5 phases.
- `kova feedback` tracks failure data from tournament runs for root-cause analysis.

**Source:** `src/bin/kova-test.rs`, `src/gauntlet.rs`, `src/feedback.rs`.

---

## PO: Protect Operations

### PO.1 — Secure Environments for Build and Deploy

- Single binary deployment: `scp` the binary, run it. No installer, no daemon, no package manager.
- Worker nodes accessed via SSH with certificate authority (`src/ssh_ca.rs`). CA key at `~/.ssh/kova-host-ca`.
- Node principals defined in code: lf, gd, bt, st with hostnames and IPs (`src/ssh_ca.rs` lines 12-17).
- Deploy command: `kova deploy` syncs and builds on worker nodes.

### PO.2 — Secure Software Distribution

- Binary built from source on deployment target (or cross-compiled and transferred via SSH).
- No package registry distribution. Direct binary transfer.
- `Cargo.lock` ensures identical dependency resolution across build environments.

**Source:** `src/ssh_ca.rs`, `src/c2.rs`, `src/main.rs` (Deploy subcommand).
