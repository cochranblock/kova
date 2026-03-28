# Security Posture

**Product:** kova v0.7.0
**Date:** 2026-03-27

## Cryptographic Controls

### Algorithms

| Function | Algorithm | Standard | Implementation |
|---|---|---|---|
| Symmetric encryption | AES-256-GCM | FIPS 197, NIST SP 800-38D | Rust crate (not FIPS-validated) |
| Key derivation | HKDF | RFC 5869 | Rust crate |
| Password hashing | Argon2id | RFC 9106 | Rust crate |

### Secret Management

- No plaintext secrets in source code (P17 protocol).
- Conversation history encrypted at rest via AES-256-GCM in sled database.
- Key material derived via HKDF. Password-based keys use Argon2id.
- All secret operations designed for `src/secrets.rs`.

**Source:** `CLAUDE.md` (P17), crypto design for `src/secrets.rs`.

## Attack Surface Analysis

### 1. HTTP API (axum)

**Exposure:** `kova serve` binds to configurable address (default localhost).
**Controls:**
- CORS enforcement via `tower-http` (`Cargo.toml` line 52: `features = ["cors"]`).
- Type-safe request handling via axum extractors (no raw string parsing).
- WebSocket support for real-time communication (`Cargo.toml` line 49: `features = ["ws"]`).
- No authentication layer in Phase 1 (local development tool).

**Source:** `src/serve.rs`, `Cargo.toml` line 49.

### 2. SSH to Worker Nodes

**Exposure:** SSH connections to 4 worker nodes on local network (192.168.1.43-47).
**Controls:**
- Certificate authority for host verification (`src/ssh_ca.rs`).
- Node principals bound to specific hostnames and IPs.
- CA key stored at `~/.ssh/kova-host-ca` (standard SSH key permissions).
- No password authentication; key-based only.

**Source:** `src/ssh_ca.rs` lines 12-17, `src/c2.rs`, `src/node_cmd.rs`.

### 3. Local sled Database

**Exposure:** Filesystem access at `~/.kova/`.
**Controls:**
- sled is an embedded database (no network listener).
- Data serialized via bincode, compressed via zstd (`src/storage.rs`).
- Encryption at rest via AES-256-GCM for sensitive data.
- Standard Unix file permissions protect the data directory.

**Source:** `src/storage.rs` (t12 struct, f39 open function).

### 4. CLI Input

**Exposure:** Local user input via terminal.
**Controls:**
- Type-safe CLI parsing via clap derive macros (`src/main.rs`).
- 30+ subcommands, each with typed arguments. No raw `env::args()` parsing.
- `--help` available on every subcommand.

**Source:** `src/main.rs` lines 8-110.

### 5. WASM Thin Client

**Exposure:** Browser access via `kova serve`.
**Controls:**
- WASM client is a thin rendering layer; all logic runs server-side.
- Same CORS controls as the HTTP API.
- No local storage or cookies in the WASM client.

**Source:** `src/web_client/mod.rs`, `src/web_client/app.rs`.

## Error Handling

- Library errors: `thiserror` with structured variants (`src/storage.rs` E0, `src/error.rs`).
- Application errors: `anyhow` for context-rich error chains.
- No `unwrap()` in production paths. `panic = 'abort'` in release profile prevents unwinding.
- Error types include context (sled failures, bincode failures, zstd failures mapped to distinct variants).

**Source:** `src/storage.rs` lines 11-24, `Cargo.toml` line 100.

## Memory Safety

- Written in Rust. Memory safety enforced at compile time.
- No `unsafe` blocks in application code (verified by clippy).
- TLS via rustls (pure Rust, no C FFI for TLS).
- `reqwest` configured with `rustls-tls`, eliminating OpenSSL as a dependency.

## Network Isolation

- Default operation: fully offline. No network calls unless user explicitly:
  - Runs `kova serve` (binds HTTP listener).
  - Configures cluster nodes in `~/.kova/config.toml`.
  - Downloads models via `kova model install`.
- No telemetry. No analytics. No phone-home behavior.
- Cloudflare tunnel is optional and configured separately via approuter.

## Deployment Security

- Single binary: no shared libraries, no runtime dependencies, no interpreter.
- `strip = true` removes debug symbols from release binary.
- No setuid/setgid. No elevated privileges required.
- No daemon processes in Phase 1. Process runs in foreground, terminates cleanly.
