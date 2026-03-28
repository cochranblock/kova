# Software Bill of Materials (SBOM)

**Product:** kova v0.7.0
**Binary:** `kova` (single binary, 27 MB release, aarch64-apple-darwin)
**License:** Unlicense (public domain)
**Generated:** 2026-03-27
**Format:** EO 14028 (Executive Order on Improving the Nation's Cybersecurity) compliant

## Build Profile

```
opt-level = 'z'
lto = true
codegen-units = 1
panic = 'abort'
strip = true
```

All versions pinned via `Cargo.lock`. No vendored binaries. All dependencies sourced from crates.io.

## Direct Dependencies (31)

| Component | Version | License | Source |
|---|---|---|---|
| anyhow | 1.0.102 | MIT OR Apache-2.0 | crates.io |
| axum | 0.7.9 | MIT | crates.io |
| bincode | 2.0.1 | MIT | crates.io |
| clap | 4.6.0 | MIT OR Apache-2.0 | crates.io |
| crossterm | 0.28.1 | MIT | crates.io |
| csv | 1.4.0 | MIT OR Unlicense | crates.io |
| dirs | 5.0.1 | MIT OR Apache-2.0 | crates.io |
| fastembed | 5.13.0 | Apache-2.0 | crates.io |
| flate2 | 1.1.9 | MIT OR Apache-2.0 | crates.io |
| futures-util | 0.3.32 | MIT OR Apache-2.0 | crates.io |
| glob | 0.3.3 | MIT OR Apache-2.0 | crates.io |
| kalosm | 0.4.0 | MIT OR Apache-2.0 | crates.io |
| kalosm-sample | 0.4.1 | MIT OR Apache-2.0 | crates.io |
| lru | 0.16.3 | MIT | crates.io |
| ordered-float | 4.6.0 | MIT | crates.io |
| pdf-extract | 0.10.0 | Apache-2.0 | crates.io |
| ratatui | 0.29.0 | MIT | crates.io |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | crates.io |
| serde | 1.0.228 | MIT OR Apache-2.0 | crates.io |
| serde_json | 1.0.149 | MIT OR Apache-2.0 | crates.io |
| similar | 2.7.0 | Apache-2.0 | crates.io |
| sled | 0.34.7 | MIT OR Apache-2.0 | crates.io |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | crates.io |
| thiserror | 2.0.18 | MIT OR Apache-2.0 | crates.io |
| tokio | 1.50.0 | MIT | crates.io |
| toml | 0.8.23 | MIT OR Apache-2.0 | crates.io |
| tower | 0.5.3 | MIT | crates.io |
| tower-http | 0.6.8 | MIT | crates.io |
| tracing | 0.1.44 | MIT | crates.io |
| tracing-subscriber | 0.3.23 | MIT | crates.io |
| zstd | 0.13.3 | MIT | crates.io |

## License Summary

| License | Count |
|---|---|
| MIT | 31 (sole or dual) |
| Apache-2.0 | 20 (sole or dual) |
| Unlicense | 1 (csv, dual with MIT) |

All dependencies use OSI-approved licenses. No copyleft (GPL/LGPL/AGPL). No proprietary dependencies.

## Transitive Dependencies

Full transitive dependency tree is captured in `Cargo.lock` at repository root. The lock file is committed to version control and used for all builds to ensure reproducible binary output.

## Verification

```bash
# Reproduce this SBOM from source
cargo tree -p kova --depth 1 --format "{p} {l}"
# Verify lock file integrity
sha256sum Cargo.lock
```

## Supply Chain Notes

- No pre-built binaries. All code compiled from source via `rustc`.
- `reqwest` configured with `rustls-tls` (no OpenSSL dependency).
- `kalosm` and `candle-*` crates provide local inference (no API calls).
- One `[patch.crates-io]` entry: `android-activity` patched to upstream git for Android build compatibility.
