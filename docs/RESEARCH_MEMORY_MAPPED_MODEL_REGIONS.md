<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

# Research: Memory-Mapped Model Regions & Multiple Entry Points in Rust

**Question:** Can a self-contained Rust binary hook up to different memory-mapped regions of the same AI model, using near-memory regions to introduce different "entries" into iterating through the model?

**Short answer:** Yes, with caveats. The model weights can be memory-mapped (or embedded); different *logical* entry points (KV cache states, layer subsets) are well-supported. True "different memory regions" for different iteration paths is possible via mmap offset/len and NUMA binding, but the *iteration* through the model is typically fixed by architecture—what varies is *which prefix* (KV cache) we start from and *how many layers* we run.

---

## Prior Work (Academic & Industry)

### KV Cache Prefix Caching & Reuse

- **KVFlow** — Efficient prefix caching for multi-agent workflows; Agent Step Graph predicts future agent activation for cache eviction; overlapped KV prefetching; up to 2.19× speedup. [arXiv:2507.07400](https://arxiv.org/abs/2507.07400)
- **ChunkAttention** — Chunks KV tensors into prefix-tree structure; shares matching prompt prefixes across requests; 3.2–4.8× speedup for 1K–4K token prompts. [ACL 2024](https://aclanthology.org/2024.acl-long.623.pdf)
- **KVShare** — Cross-request KV cache reuse in multi-tenant serving; Dual-Stage High Deviation (DHD) algorithm; up to 9.39× TTFT reduction, 1.2× throughput. [arXiv:2503.16525](https://arxiv.org/abs/2503.16525)
- **PrefillShare** — Shares prefill + KV cache across multiple models in disaggregated serving; 4.5× lower p95 latency, 3.9× higher throughput. [arXiv:2602.12029](https://arxiv.org/abs/2602.12029)

### Multi-Tenant Shared Model & Memory Optimization

- **Oneiros** — Parameter remapping: repurposes model parameter memory for KV cache; 44.8–82.5% tail latency reduction, 20.7–99.3% TTFT improvement. [arXiv:2507.11507](https://arxiv.org/abs/2507.11507)
- **MemServe** — Context caching for disaggregated LLM serving; elastic memory pool (MemPool); inter/intra-request optimizations. [arXiv:2406.17565](https://arxiv.org/abs/2406.17565)
- **Activated LoRA** — Cross-model KV cache reuse between base and LoRA-adapted models; up to 58× latency reduction, 100× TTFT improvement. [arXiv:2512.17910](https://arxiv.org/abs/2512.17910)

### Early Exit & Layer Skipping

- **LayerSkip** (Meta) — Layer dropout + early exit loss; self-speculative decoding; up to 2.16× speedup (summarization), 1.82× (coding). [arXiv:2404.16710](https://arxiv.org/abs/2404.16710)
- **Middle Layer Skipping** — Learned gating to skip symmetric spans of central blocks; leverages redundancy in middle layers. [arXiv:2506.21103](https://arxiv.org/abs/2506.21103)
- **SkipBERT** — Skips shallow layers via n-gram lookup; 65% latency reduction. [ACL 2022](https://aclanthology.org/2022.acl-long.503.pdf)

### Memory-Mapped Model Loading

- **llama.cpp** — Uses mmap for model loading; discussion of tradeoffs (TLB shootdowns, TTFT vs throughput). [Issue #91](https://github.com/ggerganov/llama.cpp/issues/91)
- **Markaicode** — mmap for LLMs: up to 80% faster startup, lower RAM, shared memory across processes. [Markaicode](https://markaicode.com/memory-mapped-models-load-large-llms-faster/)
- **memmap2** (Rust) — Cross-platform mmap; `Mmap` / `MmapMut`; offset/len for partial mappings.

---

## 1. Self-Contained Model in Rust Binary

### 1.1 Embedding the Model

```rust
// Compile-time: model baked into .rodata section
static MODEL_GGUF: &[u8] = include_bytes!("../models/qwen2.5-coder-0.5b.gguf");
```

- `include_bytes!` yields `&'static [u8; N]` — contiguous, read-only, in process address space.
- No separate file at runtime; binary is self-contained.
- **Caveat:** Binary size = model size. A 0.5B GGUF (~300MB–1GB) makes the binary large but feasible.

### 1.2 Memory-Mapping an External File (Alternative)

If the model stays external (smaller binary, easier updates):

```rust
use memmap2::MmapOptions;
use std::fs::File;

let file = File::open("model.gguf")?;
let mmap = unsafe { MmapOptions::new().map(&file)? };
// mmap is &[u8] - zero-copy access to file contents
```

- **memmap2** (Rust 2024 edition compatible): cross-platform, `Send` + `Sync`.
- OS handles paging; only accessed pages are faulted in.

---

## 2. Multiple Memory-Mapped Regions from Same Source

### 2.1 Multiple Regions from a File

```rust
use memmap2::MmapOptions;

let file = File::open("model.gguf")?;

// Region 1: metadata + first N tensors
let region_meta = unsafe {
    MmapOptions::new().offset(0).len(1024 * 1024).map(&file)?
};

// Region 2: middle layers (e.g. layers 8–16)
let region_mid = unsafe {
    MmapOptions::new().offset(10_000_000).len(50_000_000).map(&file)?
};

// Region 3: later layers
let region_late = unsafe {
    MmapOptions::new().offset(60_000_000).len(50_000_000).map(&file)?
};
```

- Each region is an independent `Mmap`; same file, different offsets/lengths.
- Useful when you want to load/access only parts of the model (e.g. specific layer groups).
- **GGUF layout:** Tensors have byte offsets. You need the GGUF header/index to know where each tensor lives; then you can mmap per-tensor or per-layer-group.

### 2.2 Multiple Regions from Embedded Bytes

With `include_bytes!`, you don't get true mmap (it's already in memory). But you can create **slices** into different regions:

```rust
static MODEL: &[u8] = include_bytes!("model.gguf");

// Different "regions" = slices into the same buffer
let region_1 = &MODEL[0..1_000_000];
let region_2 = &MODEL[1_000_000..50_000_000];
let region_3 = &MODEL[50_000_000..];
```

- No extra allocation; just different `&[u8]` views.
- Same idea as mmap regions, but source is static data.

---

## 3. "Different Entries into Iterating Through the Model"

The model's forward pass iterates over layers in a fixed order. "Different entries" can mean:

### 3.1 Different KV Cache Prefixes (Prompt/Context)

- **Same weights**, different *context* (system prompt, few-shot examples).
- Precompute KV cache for each "persona" prefix; inject when switching.
- This is the **Regionalized Openings** approach from the RFC.
- **Entry point** = which cached prefix we start from. Iteration (layer loop) is unchanged.

### 3.2 Early Exit / Layer Subsets

- Run only layers 0–N for "fast path" (e.g. SyntaxValidator).
- Run full stack for "deep" path (e.g. ExecutiveRouter).
- **Entry point** = which layer index we start at (usually 0) and which we stop at.
- Implemented by short-circuiting the layer loop:

```rust
for (i, layer) in model.layers.iter().enumerate() {
    if i >= early_exit_layer { break; }
    // ...
}
```

### 3.3 Different Tensor Regions for Different Paths

- **Hypothesis:** Could we have "path A" use tensors from region 1 and "path B" use tensors from region 2?
- **Reality:** Transformer layers are sequential; layer N depends on layer N-1. You can't arbitrarily reorder.
- **What we can do:** Place different *layer groups* in different memory regions (e.g. via mmap offset) so that when we iterate, we touch different physical memory regions. This can help with:
  - **NUMA locality:** Bind region 1 to NUMA node 0, region 2 to node 1.
  - **Cache behavior:** Sequential access within a region may have better locality.

### 3.4 Near Memory / NUMA

On multi-socket or NUMA systems:

```rust
// libnuma or membase
mbind(region_1.as_ptr(), len, MPOL_BIND, nodemask, ...);
```

- Bind each mmap region to a specific NUMA node.
- On a single-socket Intel i9, you often have one node; NUMA matters less.
- Still useful for: huge pages, explicit memory placement, future multi-socket.

---

## 4. Practical Architecture: Regions as Entry Points

### 4.1 Model Layout (GGUF)

```
[Header | Metadata | Tensor Index]
[Tensor 0: embed]
[Tensor 1: layer.0.attn]
...
[Tensor N: layer.L-1.mlp]
[Output lm_head]
```

### 4.2 Region Strategy

| Region   | Content              | Use Case                    |
|----------|----------------------|-----------------------------|
| Meta     | Header + index       | Parse once, get tensor map  |
| Embed    | Embedding weights    | Shared by all paths         |
| Early    | Layers 0–7           | Fast path (early exit)      |
| Mid      | Layers 8–15          | Medium depth                |
| Late     | Layers 16–23         | Full depth                  |

### 4.3 Iteration with Multiple "Entries"

```rust
enum InferencePath {
    Fast,   // layers 0..8
    Medium, // layers 0..16
    Full,   // layers 0..24
}

fn run_inference(
    model: &Model,
    path: InferencePath,
    kv_prefix: Option<&KvCache>,  // precomputed system prompt
    input_ids: &[u32],
) -> Vec<u32> {
    let max_layer = match path {
        InferencePath::Fast => 8,
        InferencePath::Medium => 16,
        InferencePath::Full => 24,
    };
    let mut kv = kv_prefix.cloned().unwrap_or_default();
    for (i, layer) in model.layers.iter().enumerate() {
        if i >= max_layer { break; }
        // forward layer, update kv
    }
    // ...
}
```

- **Entry 1:** `InferencePath` (how many layers).
- **Entry 2:** `kv_prefix` (which cached context we start from).

---

## 5. Rust 2024 Edition Crates

| Crate      | Purpose                    | Notes                          |
|------------|----------------------------|--------------------------------|
| memmap2    | Memory-mapped files        | offset, len, multiple regions   |
| gguf-rs    | GGUF parsing              | Optional mmap feature           |
| gguf-rs-lib| GGUF with mmap support     | `GGUFFile::mmap()`              |
| candle     | Inference                  | Model loading, KV cache         |
| libnuma    | NUMA binding (Linux)      | mbind, set_mempolicy            |
| membase    | NUMA-aware allocation     | NumaPolicy enum                 |

---

## 6. Does It Work?

**Yes, with this interpretation:**

1. **Self-contained binary:** `include_bytes!` embeds the model; no external file.
2. **Multiple regions:** Slices or mmap with offset/len give different views into the same model.
3. **Different entries into iteration:**
   - **KV cache prefix** — different context/persona (zero TTFT with precompute).
   - **Layer subset** — early exit for different "depths" of reasoning.
   - **Memory region binding** — optional NUMA placement for locality.
4. **Near memory:** NUMA binding can pin regions to specific nodes; on single-socket i9 the benefit is smaller but the mechanism exists.

**Limitation:** The *order* of iteration (layer 0 → 1 → … → L) is fixed by the transformer architecture. We cannot "skip around" layers arbitrarily. What we *can* vary is:
- Where we *start* (context via KV cache).
- Where we *stop* (early exit).
- *Where in physical memory* each layer's weights live (mmap regions + NUMA).

**Relation to prior work:** The doc's approach (KV prefix + early exit + mmap regions) aligns with KVShare/ChunkAttention (prefix reuse), LayerSkip (early exit), and llama.cpp/mmap (model loading). The combination into a single Rust binary with regionalized entry points is the novel angle.

---

## 7. Suggested Next Steps

1. Use **gguf-rs-lib** with mmap to load the model; inspect tensor offsets.
2. Create 2–3 mmap regions (early/mid/late layers) and verify zero-copy tensor access.
3. Implement early-exit paths (Fast/Medium/Full) in the inference loop.
4. Add KV cache precompute for different system prompts (RFC approach).
5. On NUMA hardware, experiment with `mbind` on each region.