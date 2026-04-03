# NanoSign — Universal AI Model Signing

> 36 bytes. Any model format. One hash. Zero infrastructure.

## The Problem

AI model files ship unsigned. You download a `.safetensors` or `.gguf` from the internet and trust it blindly. No integrity verification. No tamper detection. Existing solutions (GPG signatures, X.509 certificates, sigstore) require infrastructure, key management, and ceremony. Nobody uses them for model files.

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

**Signing (bake):**
1. Read or generate the model file
2. Compute `blake3::hash(file_bytes)`
3. Append `b"NSIG"` + hash (36 bytes) to the file

**Verification (load):**
1. Read last 36 bytes of the file
2. First 4 bytes must be `NSIG` — if not, file is unsigned (not an error, just unsigned)
3. Remaining 32 bytes = expected hash
4. Compute `blake3::hash(file_bytes[0..len-36])`
5. Compare. Match = verified. Mismatch = tampered or corrupted. Reject.

**Properties:**
- Self-contained: the signature is IN the file, not beside it
- Format-agnostic: works with any binary format (safetensors, GGUF, ONNX, PyTorch, nanobyte)
- Backward-compatible: existing tools ignore trailing bytes they don't recognize
- Fast: BLAKE3 runs at memory bandwidth (~6 GB/s). A 4GB model verifies in <1 second
- Tiny: 36 bytes overhead. On a 4GB model, that's 0.0000009% size increase
- No dependencies beyond `blake3` (pure Rust, no C, no OpenSSL)

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

## Reference Implementation

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
description = "Universal AI model signing. 36 bytes. Any format."
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
