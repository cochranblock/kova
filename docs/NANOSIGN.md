# NanoSign — Universal AI Model Integrity Hash

> 36 bytes. Any model format. One hash. Zero infrastructure.

## What This Is

A **content-addressed integrity stamp** for model files. Append 36 bytes (`NSIG` magic + BLAKE3 hash) to any model file. Re-hash on load. Reject mismatches. That's it.

## What This Is **Not**

NanoSign is **not** a signing scheme in the cryptographic sense — there is no signer identity, no private key, no transparency log, no revocation. The name "NanoSign" is historical; semantically it is an *integrity hash trailer*, not a signature. See [Scope and Threat Model](#scope-and-threat-model) below.

For authenticity / signer-identity / transparency — what you want is **[Sigstore Model Transparency v1.0](https://blog.sigstore.dev/model-transparency-v1.0/)** (OpenSSF AI/ML WG, April 2025). NSIG can compose underneath it as the content hash that gets signed, but NSIG alone does not provide those properties.

## The Problem (Tampering, Not Authenticity)

Model files ship without integrity verification. You download a `.safetensors` or `.gguf` and trust the bytes match what the publisher uploaded. Network corruption, mirror tampering, or a compromised CDN can silently substitute weights. Most existing solutions (GPG signatures, X.509 certs, full Sigstore deployments) require infrastructure that nobody bothers to set up for model files in practice.

NanoSign solves the smallest version of this problem: *the bytes I'm reading now are bit-identical to the bytes whoever stamped this file intended to ship.* That's a useful primitive even though it's strictly weaker than authenticity.

## The Solution

Append 36 bytes to any model file. Done.

```
[MODEL FILE: any format, any size]
  .safetensors, .gguf, .onnx, .nanobyte, .pt, .bin — doesn't matter

[NANOSIGN: 36 bytes, always last]
  magic: "NSIG" (4 bytes, ASCII)
  hash:  BLAKE3 (32 bytes) — hash of everything before these 36 bytes
```

The file is self-verifying. No external registry. No key server. No setup.

## Spec

### Version 1

**Format:** The last 36 bytes of a NanoSign-protected file are:

| Offset from EOF | Size | Content |
|----------------|------|---------|
| -36 | 4 bytes | `NSIG` (ASCII: `0x4E 0x53 0x49 0x47`) |
| -32 | 32 bytes | BLAKE3 hash of bytes `[0..len-36]` |

**Stamping (bake):**
1. Read or generate the model file
2. Compute `blake3::hash(file_bytes)`
3. Append `b"NSIG"` + hash (36 bytes) to the file

**Verification (load):**
1. Read last 36 bytes of the file
2. First 4 bytes must be `NSIG` — if not, file is unstamped (not an error, just unstamped)
3. Remaining 32 bytes = expected hash
4. Compute `blake3::hash(file_bytes[0..len-36])`
5. Compare. Match = integrity-verified. Mismatch = tampered or corrupted. Reject.

**Properties:**
- Self-contained: the trailer is IN the file, not beside it
- Format-agnostic: works with any binary format (safetensors, GGUF, ONNX, PyTorch, nanobyte)
- Backward-compatible: existing tools ignore trailing bytes they don't recognize
- Fast: BLAKE3 runs at memory bandwidth (~6 GB/s). A 4GB model verifies in <1 second
- Tiny: 36 bytes overhead. On a 4GB model, that's 0.0000009% size increase
- No dependencies beyond `blake3` (pure Rust, no C, no OpenSSL)
- **Integrity, not authenticity** — see [Scope and Threat Model](#scope-and-threat-model)

### Why BLAKE3

- Fastest cryptographic hash: 6+ GB/s on modern CPUs, faster than SHA-256 by 5-14x
- 256-bit output: collision-resistant for any practical purpose
- Pure Rust implementation: `blake3` crate, no C bindings, no system dependencies
- Streaming: can hash files larger than RAM via incremental API
- Proven: used by IPFS, Bao, multiple OS package managers

### Why Not SHA-256

SHA-256 would also work. BLAKE3 is chosen because:
1. ~5-14x faster on the same hardware
2. Supports SIMD acceleration out of the box
3. The `blake3` Rust crate is smaller and has fewer dependencies than `ring` or `sha2`
4. For multi-GB model files, the speed difference is significant (0.7s vs 5s for a 4GB file)

SHA-256 is acceptable if you already depend on it. The format is hash-agnostic in spirit — but the spec says BLAKE3 to avoid negotiation overhead. One hash, no options, no confusion.

## Scope and Threat Model

NanoSign is an unkeyed content hash. Knowing what it does and does *not* protect is the difference between a useful primitive and false security.

### What NanoSign defends against

- **Network/transit corruption.** A flipped bit between mirror and disk → hash mismatch → rejected.
- **Storage corruption.** Bitrot on the disk holding the model → mismatch → rejected.
- **Accidental modification.** Someone edits the file with a hex editor and forgets to re-stamp → rejected.
- **CDN / mirror substitution by an attacker who lacks the originator's stamping pipeline.** If the attacker also re-hashes, see below.

### What NanoSign does **not** defend against

- **Authenticity / forgery.** There is no key. *Anyone* who tampers with a file can also re-hash it and ship a "valid" trailer. A valid NSIG trailer proves only that *someone* hashed the bytes — not *who*.
- **Adversarial training data.** A model trained on poisoned examples produces a perfectly valid hash. NSIG sees bytes; it cannot see semantics.
- **Weight-space backdoors / trojans.** Malicious weights pass an integrity check trivially. Detecting these requires behavioral evaluation, not hashing.
- **Supply-chain compromise of the stamping pipeline itself.** If an attacker compromises the originator's build system, they get to stamp arbitrary weights with the originator's blessing. NSIG has no log to detect this after the fact.
- **Revocation.** A known-bad hash cannot be marked "do not load" centrally — there is no transparency log to consult.

### When NSIG is enough

- Self-distributed models inside a trusted boundary (your laptop, your cluster, your CI).
- Embedded models inside a binary you already trust (e.g. `include_bytes!` on `assets/starter.nanobyte` — the hash protects against build-time corruption and detects if someone alters the binary's `.rodata` post-link).
- A first-pass integrity check before more expensive validation.

### When NSIG is **not** enough

- You want to know *who* trained / packaged the model.
- You're shipping models across organizational trust boundaries.
- Compliance requires authenticated provenance (FedRAMP, FIPS, SLSA Level ≥ 2, NIST SP 800-218A authenticated provisioning).
- You need to revoke a poisoned release without re-issuing all consumers.

### The compose-with-Sigstore upgrade path

NSIG and Sigstore Model Transparency v1.0 are complementary, not alternatives:

```
[ MODEL BYTES + NSIG TRAILER ]   ← integrity (kova / pixel-forge)
              │
              ▼
[ in-toto attestation referencing the BLAKE3 hash above ]
              │
              ▼
[ Fulcio short-lived cert + Rekor log entry ]   ← authenticity (Sigstore)
```

The 32-byte BLAKE3 from NSIG becomes the content digest inside the in-toto attestation. NSIG handles "the bytes haven't changed since stamp time"; Sigstore handles "and the stamp came from this identity, logged at this time, and you can verify both later." Future `nanosign-v2` will document this composition.

### Compliance framing

- **NIST SP 800-218A** (SSDF profile for generative AI, Apr 2024) requires integrity verification of training/fine-tuning data (PW.3) but does *not* yet specify a weight-signing scheme. NSIG addresses the integrity half; Sigstore composition addresses authenticity.
- **EO 14028 §4(e)** maps to SSDF tasks but does not (as of 2026-05) mandate weight-level signing — only software supply chain. Document NSIG as integrity, document Sigstore composition as the authenticity story.

## Reference Implementation

> **Note on terminology in the API.** The functions are named `sign` / `verify` / `strip` — these names are sticky (already used by `kova`, `pixel-forge`, and any future consumers) and refer mechanically to "append the integrity trailer" / "check the integrity trailer" / "remove the integrity trailer." The names predate the cryptographic-signing reframing in this doc and are preserved for compatibility. Read every occurrence of "sign" in the API as "stamp" or "hash-and-append."

### Rust (3 lines)

```rust
// Sign a model file
fn nanosign_sign(path: &Path) -> std::io::Result<()> {
    let data = std::fs::read(path)?;
    let hash = blake3::hash(&data);
    let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
    f.write_all(b"NSIG")?;
    f.write_all(hash.as_bytes())?;
    Ok(())
}

// Verify a model file
fn nanosign_verify(path: &Path) -> std::io::Result<bool> {
    let data = std::fs::read(path)?;
    if data.len() < 36 { return Ok(false); }
    let (payload, sig) = data.split_at(data.len() - 36);
    if &sig[..4] != b"NSIG" { return Ok(false); } // unsigned
    let expected = &sig[4..];
    let actual = blake3::hash(payload);
    Ok(actual.as_bytes() == expected)
}

// Strip signature (for tools that choke on trailing bytes)
fn nanosign_strip(path: &Path) -> std::io::Result<()> {
    let data = std::fs::read(path)?;
    if data.len() >= 36 && &data[data.len()-36..data.len()-32] == b"NSIG" {
        std::fs::write(path, &data[..data.len()-36])?;
    }
    Ok(())
}
```

### CLI

```bash
# Sign
nanosign sign model.safetensors

# Verify
nanosign verify model.safetensors
# Output: VERIFIED (blake3: a1b2c3d4...)
# or:     FAILED (expected: a1b2..., got: f5e6...)
# or:     UNSIGNED (no NSIG marker)

# Strip (remove signature)
nanosign strip model.safetensors

# Hash only (print without signing)
nanosign hash model.safetensors
# Output: a1b2c3d4e5f6...
```

### Python

```python
import blake3, struct

def nanosign_verify(path: str) -> bool:
    with open(path, 'rb') as f:
        data = f.read()
    if len(data) < 36:
        return False
    if data[-36:-32] != b'NSIG':
        return False  # unsigned
    expected = data[-32:]
    actual = blake3.blake3(data[:-36]).digest()
    return actual == expected
```

## Compatibility

### safetensors

safetensors files start with a header length (u64 LE) followed by JSON metadata, then raw tensor data. The format reads forward from the start and knows its own length from the header. Trailing bytes after the tensor data are ignored by all safetensors parsers. NanoSign appends after the tensor data — existing tools continue to work.

### GGUF

GGUF files have a header with magic, version, tensor count, and metadata count. Tensor data follows. The format is self-describing and reads forward. Trailing bytes are ignored by llama.cpp, ollama, and other GGUF consumers. NanoSign is invisible to them.

### ONNX

ONNX uses protobuf, which is length-delimited and reads only what the schema describes. Trailing bytes are ignored. NanoSign is compatible.

### PyTorch (.pt/.bin)

PyTorch uses Python pickle or safetensors. Pickle reads forward from a stream and stops when complete. Trailing bytes are ignored. NanoSign is compatible.

## Optional: Registry

For environments that want a known-good hash database:

```rust
// sled-backed registry (kova uses this)
let db = sled::open("model_hashes.sled")?;
let hash = blake3::hash(&payload);
db.insert(model_name.as_bytes(), hash.as_bytes())?;

// On load: verify file hash AND check it matches registry
let stored = db.get(model_name.as_bytes())?;
if stored.as_deref() != Some(hash.as_bytes()) {
    // File changed since last known-good load
}
```

This is optional. The file is self-verifying without any registry.

## Crate: `nanosign`

```toml
[package]
name = "nanosign"
version = "0.1.0"
description = "Universal AI model integrity hash trailer. 36 bytes. Any format."
license = "Unlicense"
repository = "https://github.com/cochranblock/nanosign"

[dependencies]
blake3 = "1"
```

Public API:

```rust
pub fn sign(path: &Path) -> io::Result<()>;
pub fn verify(path: &Path) -> io::Result<NanoSignResult>;
pub fn strip(path: &Path) -> io::Result<()>;
pub fn hash(path: &Path) -> io::Result<blake3::Hash>;

pub enum NanoSignResult {
    Verified(blake3::Hash),
    Failed { expected: blake3::Hash, actual: blake3::Hash },
    Unsigned,
}
```

3 lines to sign. 3 lines to verify. `cargo add nanosign`.

---

*Part of the [Kova](https://github.com/cochranblock/kova) augment engine. [Unlicense](https://unlicense.org) — public domain.*
