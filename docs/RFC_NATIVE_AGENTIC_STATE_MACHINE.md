# RFC: Kova Native Agentic State Machine

**Status:** Draft / Architectural Blueprint  
**Environment:** Bare-metal Debian, Intel i9, 32GB RAM  
**Engine:** qwen2.5-coder:0.5b GGUF via Candle (candelabra)  
**Integration:** rig-core

---

## 1. Objective

Design a **deterministic state machine** and **agentic loop** natively in Rust, bypassing external HTTP APIs. A single 0.5B model is fragmented into a disciplined swarm of **regionalized micro-agents** (personas), each with a specialized system prompt. KV cache multiplexing achieves near-zero TTFT when switching regions.

---

## 2. Trait Design

### 2.1 Region Enum (Persona)

```rust
/// Each variant = one micro-agent with a specialized system prompt.
/// The enum is the switchboard: type-safe, exhaustive, deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    SyntaxValidator,
    LogicAnalyzer,
    ExecutiveRouter,
    // ... extensible
}

impl Region {
    /// System prompt for this region. Immutable, baked at compile/runtime.
    pub fn system_prompt(&self) -> &'static str { ... }

    /// Optional: routing rules (which region receives output of this one).
    pub fn next_region(&self, output: &str) -> Option<Region> { ... }
}
```

**Design principle:** The enum is the **single source of truth** for regions. Adding a region = adding a variant. No dynamic registration; compile-time exhaustiveness.

### 2.2 RegionExecutor Trait

```rust
/// A region can execute a task given its precomputed KV cache and user input.
pub trait RegionExecutor {
    type Cache: KvCacheSnapshot;  // Snapshot of KV state for this region

    /// Run inference from cached prefix (system prompt) + new tokens (user/context).
    fn execute(
        &self,
        cache: &Self::Cache,
        input: &str,
        max_tokens: u32,
    ) -> Result<String, InferenceError>;
}
```

### 2.3 Orchestrator Trait (State Machine)

```rust
/// The deterministic loop: route → execute → route → ...
pub trait Orchestrator {
    type State: Default + Clone;

    /// One step: given current region + state, produce (next_region, output, new_state).
    fn step(
        &self,
        region: Region,
        input: &str,
        state: &Self::State,
    ) -> Result<(Region, String, Self::State), OrchestrationError>;

    /// Full loop until termination condition (e.g. ExecutiveRouter says "DONE").
    fn run(&self, initial_input: &str) -> Result<String, OrchestrationError> {
        let mut region = Region::ExecutiveRouter;  // or configurable entry
        let mut input = initial_input.to_string();
        let mut state = Self::State::default();
        loop {
            let (next, output, new_state) = self.step(region, &input, &state)?;
            if self.is_terminal(&next, &output) { return Ok(output); }
            region = next;
            input = output;
            state = new_state;
        }
    }
}
```

**Determinism:** `step` is pure given (region, input, state). No external API calls. Randomness only via explicit `rng` param if sampling is needed.

### 2.4 KV Cache Snapshot Trait

```rust
/// Abstraction over a saved KV cache state.
/// Implementations: Candle's native cache, or a serialized blob.
pub trait KvCacheSnapshot: Send + Sync {
    /// Number of cached tokens (prefix length).
    fn len(&self) -> usize;
    /// Clone for injection into active inference. Must be cheap (Arc internally).
    fn clone_for_inference(&self) -> Box<dyn KvCacheSnapshot>;
}
```

---

## 3. Memory Strategy

### 3.1 The Problem

- Candle's KV cache is typically `Option<KvCache>` or similar, owned by the model during `forward()`.
- We need to **precompute** cache for each region's system prompt at init, then **inject** it when switching.
- Candle does not expose a stable "save/restore KV cache" API; we may need to fork or wrap.

### 3.2 Strategy A: Precompute at Init, Store Arc-Wrapped Tensors

```
Init:
  for region in Region::iter() {
      let cache = model.forward(system_prompt(region), kv_cache: None)?;
      let snapshot = KvSnapshot::from(cache);
      region_caches.insert(region, Arc::new(snapshot));
  }
```

**Storage:** `HashMap<Region, Arc<KvSnapshot>>`. Each snapshot holds the tensor data for the system prompt prefix. Clone = `Arc::clone`, O(1).

**Inference:** When routing to `SyntaxValidator`, we `Arc::clone` the snapshot and pass it to `model.forward(..., kv_cache: Some(snapshot.as_cache()))`. The model continues from that prefix.

### 3.3 Strategy B: Lazy Precompute + LRU Eviction

If 32GB is tight and we have many regions:

- Precompute on first use, cache in `DashMap<Region, Arc<KvSnapshot>>`.
- LRU eviction: drop least-recently-used snapshots when memory exceeds threshold.
- Trade-off: First use of a region pays TTFT; subsequent uses are instant.

### 3.4 Borrow Checker and Ownership

- **Model:** Single `Llama` (or Qwen) instance. We cannot share it across threads without `Arc<Mutex<Model>>` or similar.
- **KV snapshots:** `Arc<KvSnapshot>` — shared, immutable. No mutable borrow of model during snapshot storage.
- **Inference thread:** Holds `&mut Model` (or `MutexGuard`) only during `forward()`. Snapshot is consumed/copied into the cache struct Candle expects.

**Key insight:** The snapshot is **read-only**. We never mutate it after creation. Candle's `forward` typically takes `&mut self` for the model and `Option<&mut KvCache>` for the cache. We need an API that accepts a **prefilled** cache. If Candle doesn't support that, we need a wrapper that:
1. Creates a fresh `KvCache` with the same shape as our snapshot.
2. Copies tensor data from snapshot into the cache.
3. Calls `forward` with that cache.

### 3.5 Memory Bounds

- One 0.5B model in FP16/INT4: ~300MB–1GB.
- KV cache per region: ~(2 * layers * heads * dim * seq_len) per region. For 0.5B, seq_len of 512 tokens, estimate ~50–100MB per region.
- 5 regions ≈ 250–500MB. Total ~1.5GB. 32GB is ample.

---

## 4. Rig-Core Integration

### 4.1 Rig's Abstractions

Rig provides:
- `CompletionModel` — low-level LLM interface (prompt → completion)
- `Agent` — high-level: context, tools, RAG
- `Pipeline` / `Op` — sequential or conditional operations

Rig's providers are HTTP-based (Ollama, OpenAI). We need a **native** completion path.

### 4.2 Option A: Custom CompletionModel

Implement `CompletionModel` for our native engine:

```rust
pub struct NativeCandleModel {
    model: Arc<Mutex<QwenModel>>,
    region_caches: HashMap<Region, Arc<KvSnapshot>>,
    active_region: Region,
}

impl CompletionModel for NativeCandleModel {
    async fn complete(&self, prompt: impl Into<Prompt>) -> Result<CompletionResponse> {
        let cache = self.region_caches.get(&self.active_region).unwrap();
        let output = self.model.lock().unwrap()
            .generate_with_cache(cache, prompt.into())?;
        Ok(CompletionResponse::new(output))
    }
}
```

Rig's `Agent` and `Pipeline` can then use this as the backing model. **Caveat:** Rig's Agent expects async, tool loops, etc. Our state machine is synchronous and deterministic. We may use Rig only for the **pipeline** (conditional, parallel ops) and keep our own orchestration loop.

### 4.3 Option B: Rig Pipeline as Orchestrator

Rig's `conditional!` macro dispatches based on enum variant. We could:

1. Define an `Op` for each region that calls our native inference.
2. Use `conditional!` to route based on `Region`.
3. Each `Op` receives the previous step's output as input.

**Challenge:** Rig's pipeline is designed for async, external calls. Our ops would be sync, in-process. We might need a thin async wrapper that spawns a blocking task.

### 4.4 Option C: Rig for Tools, Native for Core Loop

- Use Rig's `Tool` / `ToolSet` for any external tools (e.g. file read, API call).
- Our **orchestrator** is custom: deterministic state machine + Candle inference.
- When a region needs a tool, we pass control to Rig's tool executor, then feed the result back into our loop.

**Recommended:** Start with Option A — implement `CompletionModel` for our native engine. This gives maximum control. Rig's higher-level features (RAG, tools) can be added later if needed.

---

## 5. Structural Blueprint

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Kova Native Agentic Engine                       │
├─────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌─────────────────────────────────────────────┐   │
│  │   Region     │    │  RegionCacheStore                            │   │
│  │   (Enum)     │───▶│  HashMap<Region, Arc<KvSnapshot>>            │   │
│  │              │    │  Precomputed at init from system prompts     │   │
│  └──────────────┘    └─────────────────────────────────────────────┘   │
│           │                              │                              │
│           ▼                              ▼                              │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │                    Orchestrator (State Machine)                     │ │
│  │  step(region, input, state) → (next_region, output, new_state)     │ │
│  │  Deterministic routing rules; no external calls                    │ │
│  └──────────────────────────────────────────────────────────────────┘ │
│           │                                                             │
│           ▼                                                             │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │                    InferenceRuntime                               │ │
│  │  - Single Qwen model (Candle)                                     │ │
│  │  - Injects KvSnapshot for active region                           │ │
│  │  - Generates completion from (cached_prefix + input)               │ │
│  └──────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────────┐
                    │  rig-core (optional)           │
                    │  CompletionModel impl          │
                    │  or Pipeline/Op for tools      │
                    └───────────────────────────────┘
```

### 5.1 Module Layout (Proposed)

```
kova/src/
  native/
    mod.rs
    region.rs       # Region enum, system_prompt()
    cache.rs        # KvSnapshot, RegionCacheStore
    inference.rs    # Candle model wrapper, generate_with_cache
    orchestrator.rs # Orchestrator trait, default impl
    rig_bridge.rs   # CompletionModel impl for rig (optional)
```

### 5.2 Init Sequence

1. Load GGUF model via Candle.
2. For each `Region`, run a single forward pass with `system_prompt(region)` and no prior cache.
3. Extract KV cache from the forward pass; wrap in `KvSnapshot`; store in `RegionCacheStore`.
4. Orchestrator is ready. On first `step(region, input)`, inference uses the precomputed cache for that region.

### 5.3 Candle KV Cache Reality Check

Candle's GGUF/Qwen support may not expose "inject prefilled cache" directly. We need to:

1. Inspect `candle-transformers` / `candle-nn` for the actual KV cache type and `forward` signature.
2. If injection is unsupported, consider:
   - **Fork Candle** and add a `forward_from_cache(cache)` method.
   - **Precompute and serialize** cache to disk, load on demand (slower init per region, but no Candle changes).
   - **Use a different backend** (e.g. llama-cpp-rs) if it has better cache control.

---

## 6. Open Questions

1. **Candle API:** Does Candle's Qwen/GGUF implementation allow passing a pre-filled KV cache into `forward`? If not, what is the minimal patch?

2. **Rig async:** Our inference is sync. Rig's `CompletionModel` is async. Do we `spawn_blocking` or implement a sync-only path?

3. **Termination:** How does the orchestrator know when to stop? Explicit "DONE" token from ExecutiveRouter? Max iterations? Both?

4. **Streaming:** Can we stream tokens from a region while the orchestrator waits? Or is full completion required before routing?

---

## 7. Summary

| Component        | Design                                                                 |
|------------------|------------------------------------------------------------------------|
| **Region**       | Enum with `system_prompt()`, `next_region()`. Type-safe, exhaustive.   |
| **Orchestrator** | Trait with `step()` and `run()`. Pure, deterministic.                  |
| **KV Cache**     | Precompute at init, store `Arc<KvSnapshot>` per region. Inject on use.  |
| **Memory**       | ~1.5GB for model + 5 regions. Arc for sharing, no leaks.              |
| **Rig**          | Implement `CompletionModel` for native engine; optional tool bridge.   |

This blueprint provides the structural foundation. Implementation will require Candle API verification and possibly a small fork or wrapper for KV cache injection.
