# FIPS Compliance Status

**Product:** kova v0.7.0
**Date:** 2026-03-27

## Current Cryptographic Algorithms

| Algorithm | Use | FIPS Standard | Status |
|---|---|---|---|
| AES-256-GCM | Symmetric encryption (data at rest) | FIPS 197 / SP 800-38D | Algorithm compliant. Implementation not FIPS-validated. |
| HKDF | Key derivation | RFC 5869 (HMAC-based) | HMAC is FIPS 198-1. Implementation not FIPS-validated. |
| Argon2id | Password hashing | RFC 9106 | **Not FIPS-approved.** No FIPS standard covers Argon2. |

## FIPS 140-3 Module Status

**kova does not use a FIPS 140-3 validated cryptographic module.**

The cryptographic operations are implemented by Rust crates compiled from source. These implementations are algorithmically correct for AES-256-GCM and HKDF, but have not undergone CMVP (Cryptographic Module Validation Program) testing.

## Gap Analysis

### AES-256-GCM
- **Algorithm:** FIPS 197 approved.
- **Mode:** GCM is approved per SP 800-38D.
- **Gap:** The Rust implementation is not a validated module. Must be replaced with a FIPS-validated library.

### HKDF
- **Algorithm:** Based on HMAC (FIPS 198-1) with SHA-256 (FIPS 180-4).
- **Gap:** Same as AES — implementation not validated.

### Argon2id
- **Algorithm:** Not a FIPS-approved algorithm.
- **Gap:** Must be replaced with PBKDF2 (SP 800-132) or another FIPS-approved KDF for password-based key derivation.

## Path to FIPS Compliance

### Option 1: aws-lc-rs (Recommended)

Replace crypto backend with `aws-lc-rs`, which wraps AWS-LC — a FIPS 140-3 validated module (certificate pending/issued by AWS).

```toml
# Cargo.toml change
aws-lc-rs = { version = "1", features = ["fips"] }
```

Changes required:
1. Replace AES-256-GCM implementation with `aws-lc-rs` AEAD API.
2. Replace HKDF with `aws-lc-rs` HKDF.
3. Replace Argon2id with PBKDF2 via `aws-lc-rs`.
4. Configure `reqwest` to use `aws-lc-rs` as TLS backend.

### Option 2: ring with FIPS Module

Use `ring` crate, which uses BoringSSL (Google's FIPS-validated module in certain configurations).

### Option 3: OpenSSL FIPS Provider

Use `openssl` crate with OpenSSL 3.x FIPS provider. This introduces a C dependency (counter to kova's pure-Rust philosophy) but provides a well-established FIPS 140-3 validated module.

## TLS Considerations

kova uses `rustls` (via `reqwest` with `rustls-tls` feature). rustls is not FIPS-validated. For FIPS compliance, TLS must also use a validated module:

- `reqwest` supports `native-tls` feature (uses system TLS, which may be FIPS-validated on federal systems).
- Or use `aws-lc-rs` as the TLS backend for rustls.

**Source:** `Cargo.toml` line 58 (reqwest features).

## Impact Assessment

kova's crypto is used for:
1. Encrypting conversation history at rest (sled database).
2. Key derivation for secret management.

These are local-only operations on a single-user tool. No cryptographic operations occur over the network (TLS handled separately by rustls). The risk exposure is limited to the local `~/.kova/` directory.

## Recommendation

For federal deployments requiring FIPS 140-3:
1. Swap to `aws-lc-rs` with `fips` feature.
2. Replace Argon2id with PBKDF2.
3. Switch `reqwest` TLS backend.
4. Validate the resulting configuration against SP 800-131A (transitioning of crypto algorithms).

Estimated effort: 2-3 days of development, plus validation testing.
