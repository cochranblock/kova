# Supply Chain Integrity

**Product:** kova v0.7.0
**Date:** 2026-03-27

## Dependency Provenance

All 31 direct dependencies are sourced from **crates.io**, the official Rust package registry. No dependencies are fetched from private registries, git repositories, or vendored sources (except one `[patch.crates-io]` for Android build compatibility).

### Version Pinning

Every dependency (direct and transitive) is pinned in `Cargo.lock`, which is committed to version control. Builds are deterministic: the same `Cargo.lock` produces the same dependency tree on any machine with the same Rust toolchain.

**Source:** `Cargo.lock` (repository root), `Cargo.toml` lines 26-71.

### No Vendored Binaries

The repository contains zero pre-built binaries, shared libraries, or object files. All code is compiled from Rust source by `rustc`. The release binary is built with:

```
opt-level = 'z'
lto = true          # link-time-optimization across all crates
codegen-units = 1   # single codegen unit for determinism
strip = true        # no debug symbols
```

**Source:** `Cargo.toml` lines 96-101.

## Build Reproducibility

Given identical inputs (`Cargo.lock`, Rust toolchain version, target triple), the build produces a functionally identical binary. LTO and `codegen-units = 1` reduce non-determinism in code generation.

### Build Command

```bash
cargo build --release -p kova --features serve
```

### Verification

```bash
# Check dependency tree
cargo tree -p kova --depth 1

# Audit for known vulnerabilities
cargo audit

# Verify no unexpected network calls in binary
strings target/release/kova | grep -i "http://"
```

## TLS and Network Stack

- HTTP client: `reqwest` with `rustls-tls` feature (no OpenSSL dependency).
- HTTP server: `axum` with `tower-http` for CORS.
- No system TLS libraries linked. Pure Rust TLS via rustls.

**Source:** `Cargo.toml` line 58 (`reqwest` features).

## Crypto Supply Chain

Cryptographic operations use Rust crate implementations:
- AES-256-GCM for symmetric encryption
- HKDF for key derivation
- Argon2id for password hashing

These are Rust implementations, not wrappers around C libraries (unlike OpenSSL). This eliminates C-level memory safety vulnerabilities in the crypto stack.

## Inference Model Supply Chain

Local LLM inference uses:
- **kalosm** (v0.4.0): downloads GGUF model files from Hugging Face Hub on first use.
- **candle** (v0.9): loads safetensors model files for on-device inference.

Model weights are stored locally in `~/.kova/models/`. No model files are bundled in the binary. Users control which models are downloaded via `kova model install`.

**Source:** `src/inference/local.rs`, `src/model.rs`, `src/mobile_llm.rs`.

## Worker Node Trust

SSH access to worker nodes (n0-n3) uses a certificate authority managed by `kova c2 ssh-ca`:
- CA key stored at `~/.ssh/kova-host-ca`
- Host certificates signed with node-specific principals (hostname + IP)
- Eliminates trust-on-first-use (TOFU) for SSH host keys

Node principals hardcoded in `src/ssh_ca.rs` lines 12-17:
- lf (kova-legion-forge, 192.168.1.47)
- gd (kova-tunnel-god, 192.168.1.44)
- bt (kova-thick-beast, 192.168.1.45)
- st (kova-elite-support, 192.168.1.43)

## Patch Management

One `[patch.crates-io]` override exists:
- `android-activity`: patched to upstream `main` branch for Android NDK compatibility.

This patch tracks the official repository (`rust-mobile/android-activity`) and is only activated for Android target builds.

**Source:** `Cargo.toml` lines 76-77.
