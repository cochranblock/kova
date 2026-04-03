# Kova Pyramid Architecture — Full Plan

## Context

Kova is a penetration testing methodology applied to development tooling. Enumerate, probe, exploit available resources, maintain persistence, pivot between nodes. The core differentiator: not competing on model size, competing on model SPEED and SPECIALIZATION. Hundreds of sub-1M parameter models chained in a pyramid, each doing one tiny job in microseconds, replacing the need for external API calls entirely.

The existing codebase already has: candle training (src/micro/candle_train.rs), tournament scoring, DPO/SFT export, quantization (TurboQuant), epsilon-greedy routing, validation gates, circuit breakers, C2 fleet dispatch, hardware discovery, context compaction (f380), dual-mode inference (f382), checkpoint/undo (f383/f384), and permission gates. This plan builds on all of it.

## Architecture: The Pyramid

```
         ┌───────────┐
         │  EXTINCT   │  Phase 4: Claude API deleted
         │  (Claude)  │  Pyramid seals shut
         └─────┬─────┘
               │ trained away
    ┌──────────┴──────────┐
    │   CELLULAR (T3)     │  1M-10M params. Code gen, conversation, planning.
    │   3-5 models        │  Routing maps to molecular layer below.
    └──────────┬──────────┘
               │ learned routing weights
    ┌──────────┴──────────┐
    │   MOLECULAR (T2)    │  100K-1M params. Intent routing, context summary,
    │   10-30 models      │  tool selection. Attention maps to subatomics.
    └──────────┬──────────┘
               │ learned routing weights
    ┌──────────┴──────────┐
    │   SUBATOMIC (T1)    │  Sub-100K params. Typo fix, binary classify,
    │   100+ models       │  flag expand, token tag. Microsecond inference.
    └─────────────────────┘
```

### Naming

- **Nanobyte** = the single mmap'd weight blob in RAM. One allocation. One file.
- **Subatomic Model** = a Rust function that reads from named offsets in the nanobyte blob
- **Molecular Model** = coordinator with routing weights to subatomics
- **Cellular Model** = domain specialist with routing maps to molecular layer

### Core Principle: Shared Memory, Zero I/O

One contiguous `.nanobyte` file. One `mmap()` call. Every model is just `fn forward(weights: &[f32], input: &[f32]) -> Vec<f32>` reading from different byte offsets. No model loading. No file I/O per model. No duplication. Nanosecond dispatch — pointer arithmetic and matrix math on already-resident memory. Small enough to live in CPU cache.

## Module Map

### New Modules

| Module | Purpose | Depends On |
|--------|---------|------------|
| `src/swarm.rs` | Pyramid orchestrator. MicroModel trait, pipeline chain, tier routing | nanobyte, discovery |
| `src/nanobyte.rs` | Nanobyte blob: mmap, manifest, offset registry, consolidation | candle, memmap2 |
| `src/discovery.rs` | Auto-detect hardware, SSH nodes, models, GPUs, ollama instances | inspect, config |
| `src/bridge.rs` | PTY bridge to claude. Log all interactions for training data | portable-pty |
| `src/swarm/subatomic.rs` | Tier 1 forward functions (typo_fix, intent_classify, etc.) | nanobyte |
| `src/swarm/molecular.rs` | Tier 2 coordinators with routing weights | nanobyte, subatomic |
| `src/swarm/cellular.rs` | Tier 3 domain specialists with routing maps | nanobyte, molecular |
| `src/swarm/train.rs` | Consolidated nanobyte training: individual runs -> merge | candle, training_data |

### Existing Modules (Reuse)

| Module | What It Provides | How It's Used |
|--------|-----------------|---------------|
| `src/micro/candle_train.rs` | Transformer arch, BPE tokenizer, AdamW, cosine LR | Base training loop for all tiers |
| `src/micro/quantize.rs` | TurboQuant (FWHT + 2/4-bit + QJL residual) | Compress nanobyte blob |
| `src/micro/kova_model.rs` | Spark/Flame/Blaze tiers, forward pass | Architecture templates for T1/T2/T3 |
| `src/micro/router.rs` | Epsilon-greedy bandit | Warm-start molecular routing |
| `src/micro/tournament.rs` | Olympic competition, failure recording | Validation and scoring |
| `src/micro/validate.rs` | Completeness/coherence/format checks | Output gates per tier |
| `src/micro/pipe.rs` | Bounded-channel pipeline | Chain execution |
| `src/training_data.rs` | Trace -> DPO/SFT export | Bridge log conversion |
| `src/trace.rs` | LLM call logging (T109) | Capture bridge interactions |
| `src/feedback.rs` | Failure recording, challenge gen | Curriculum from failures |
| `src/inference/providers.rs` | f381 Anthropic SSE streaming | Claude bridge backend |
| `src/inference/mod.rs` | f382 dual dispatch | Route local vs remote |
| `src/inspect.rs` | T205 hardware snapshot, SSH probe | Discovery foundation |
| `src/c2.rs` | tmux dispatch, sponge mesh, broadcast | Fleet orchestration |
| `src/context_mgr.rs` | f380 context compaction | Tier 3 context management |

## Implementation: src/nanobyte.rs

### Nanobyte File Format

```
[HEADER: 64 bytes]
  magic: "NANO" (4 bytes)
  version: u32
  num_models: u32
  manifest_offset: u64
  manifest_size: u64
  total_weights: u64
  _reserved: 28 bytes

[MANIFEST: variable]
  For each model:
    name: [u8; 32]       // "typo_fixer\0..."
    tier: u8              // 1=subatomic, 2=molecular, 3=cellular
    offset: u64           // byte offset into weight region
    size: u64             // weight byte count
    vocab_size: u32
    embed_dim: u32
    num_heads: u32
    num_layers: u32
    ff_dim: u32
    max_seq: u32
    num_classes: u32      // output dimension
    routing_offset: u64   // offset to routing weights (T2/T3 only)
    routing_size: u64     // routing weight byte count

[WEIGHTS: contiguous f32 blob]
  All model weights packed sequentially.
  Each model's forward() reads from its offset.
  Routing weights for T2/T3 models stored inline.
```

### Key Functions

```rust
// Load nanobyte file via mmap. Zero-copy. One syscall.
pub fn load(path: &Path) -> Result<Nanobyte, Error>

// Get a model's weight slice (pointer arithmetic, no copy)
pub fn weights(&self, model_name: &str) -> &[f32]

// Get routing weights for T2/T3 models
pub fn routing(&self, model_name: &str) -> &[f32]

// Manifest listing
pub fn models(&self) -> Vec<ModelEntry>

// Consolidate individual .safetensors into one .nanobyte
pub fn consolidate(models: &[(String, PathBuf)], output: &Path) -> Result<()>
```

### Dependencies

Add to Cargo.toml:
```toml
memmap2 = "0.9"
```

Candle's safetensors already handles the individual model training output. `consolidate()` reads trained safetensors, packs into the nanobyte format.

## Implementation: src/swarm.rs

### MicroModel Trait

```rust
pub trait MicroModel: Send + Sync {
    fn name(&self) -> &str;
    fn tier(&self) -> Tier;
    fn forward(&self, weights: &[f32], input: &[f32]) -> Vec<f32>;
    fn confidence(&self, output: &[f32]) -> f32;
}
```

### Tier Routing

```rust
pub struct Pyramid {
    nanobyte: Nanobyte,
    subatomics: Vec<Box<dyn MicroModel>>,   // T1: hundreds
    moleculars: Vec<Box<dyn MicroModel>>,    // T2: tens
    cellulars: Vec<Box<dyn MicroModel>>,     // T3: few
    confidence_threshold: f32,               // escalation gate (default 0.7)
}

impl Pyramid {
    /// Process input through the pyramid. Escalate on low confidence.
    /// Most requests never get past T1.
    pub fn process(&self, input: &str) -> PyramidResult {
        // T1: subatomic pass
        let t1_results = self.run_tier1(input);
        if t1_results.confidence >= self.confidence_threshold {
            return PyramidResult::resolved(1, t1_results);
        }

        // T2: molecular coordination (uses T1 outputs + routing weights)
        let t2_results = self.run_tier2(input, &t1_results);
        if t2_results.confidence >= self.confidence_threshold {
            return PyramidResult::resolved(2, t2_results);
        }

        // T3: cellular specialist (uses T2 outputs + routing weights)
        let t3_results = self.run_tier3(input, &t2_results);
        if t3_results.confidence >= self.confidence_threshold {
            return PyramidResult::resolved(3, t3_results);
        }

        // Escalation: during migration, Claude handles this.
        // Post-migration: T3 retries with expanded context.
        PyramidResult::escalate(t3_results)
    }
}
```

### Cross-Tier Routing Weights

Molecular models have a learned weight vector per subatomic:
```rust
// In T2 molecular model's forward():
// routing_weights: [f32; num_subatomics] — how much to trust each T1 output
// These are TRAINED, not hardcoded.
let weighted_input: Vec<f32> = t1_outputs.iter()
    .zip(routing_weights.iter())
    .map(|(output, weight)| output * weight)
    .collect();
// Then feed weighted_input through molecular model's own layers
```

Same pattern from T3 -> T2.

### Subatomic Model Registry (Tier 1)

| Model | Params | Input | Output | Purpose |
|-------|--------|-------|--------|---------|
| typo_fixer | 50K | token | corrected token | Fix obvious typos |
| intent_classify | 50K | sentence | 8-class logits | Question/command/code/conversation |
| code_vs_english | 20K | token | binary | Is this code or natural language |
| flag_expander | 30K | token | expanded token | pf->pixel-forge, cb->cochranblock |
| tone_detector | 50K | sentence | 4-class logits | Urgent/casual/frustrated/neutral |
| path_predictor | 80K | partial path | completion candidates | File path autocomplete |
| tool_tagger | 50K | sentence | tool_id logits | Which tool for this task |
| commit_classifier | 30K | diff summary | commit type | feat/fix/refactor/docs/test |
| slop_detector | 20K | sentence | binary | P12 banned word detection |
| urgency_scorer | 20K | sentence | float | Priority 0.0-1.0 |
| **noodle** | **30K** | **session context** | **short string** | **Noodle the penguin — companion AI, personality quips** |

#### Noodle the Penguin (First Demo Subatomic)

> Inspired by Claude Code's companion/buddy system. Credit: the concept of a small personality model reacting to session events originates from Claude Code's design.

**Noodle** is kova's mascot and the first subatomic model proof-of-concept. A tiny penguin personality that validates the entire T1 pipeline end-to-end: train a model, pack it into the nanobyte, run inference in microseconds, output a short string.

**What it does:** Watches session context — what tools ran, did tests pass/fail, is a build running, how long has the user been working — and produces one-liner personality-driven reactions. Not intelligence, just personality. Noodle is a penguin. Penguins are concise.

**Input features (context vector):**
- Last tool name (one-hot encoded across tool registry)
- Tool success/failure (binary)
- Build running (binary)
- Tests passed/failed/count
- Session duration bucket (fresh/working/marathon)
- Time since last user input
- Error count in last N turns

**Output:** Index into a vocabulary of ~200 short quips, bucketed by situation:
- Build success: "clean build", "ship it", "zero warnings"
- Test failure: "ouch", "close one", "try again"
- Long session: "still here?", "hydrate", "touch grass"
- Edit streak: "on a roll", "flow state", "keep going"
- Error recovery: "fixed it", "persistence pays", "back on track"

**Training data:** Generated from session logs — label each (context_vector, appropriate_quip) pair. Augment with synthetic pairs. 30K params is more than enough for a lookup-with-personality.

**Why Noodle is first:** Trivially small, no downstream dependencies, immediately visible to the user, and exercises every part of the nanobyte pipeline (mmap, offset read, forward pass, output decode) without any risk to the agentic loop. If the penguin can quip, the pyramid works.

### Molecular Model Registry (Tier 2)

| Model | Params | Routing To | Purpose |
|-------|--------|-----------|---------|
| intent_router | 200K | intent_classify, tool_tagger, tone_detector | Full intent resolution |
| context_summarizer | 500K | code_vs_english, urgency_scorer | Compress conversation |
| tool_selector | 300K | tool_tagger, path_predictor, flag_expander | Pick + parameterize tool |
| edit_planner | 500K | code_vs_english, path_predictor, intent_classify | Plan file edits |
| commit_writer | 500K | commit_classifier, slop_detector | Generate commit messages |

### Cellular Model Registry (Tier 3)

| Model | Params | Routing To | Purpose |
|-------|--------|-----------|---------|
| code_gen | 5M | edit_planner, tool_selector, context_summarizer | Generate code |
| conversation | 3M | intent_router, context_summarizer | Natural language response |
| planner | 2M | intent_router, edit_planner | Multi-step task planning |

## Implementation: src/discovery.rs

### Auto-Detection on Startup

```rust
pub struct ResourceMap {
    pub local: LocalResources,
    pub nodes: Vec<RemoteNode>,
    pub models: Vec<DiscoveredModel>,
    pub last_probe: Instant,
}

pub struct LocalResources {
    pub cpu_cores: u32,
    pub ram_gb: u32,
    pub gpu: Option<GpuInfo>,      // Metal chipset, VRAM
    pub ollama: Option<String>,     // localhost:11434 if running
    pub nanobytes: Vec<PathBuf>,    // .nanobyte files found
    pub gguf_models: Vec<PathBuf>,  // .gguf files found
}

pub struct RemoteNode {
    pub host: String,
    pub ssh_alias: String,
    pub cpu_cores: u32,
    pub ram_gb: u32,
    pub gpu: Option<GpuInfo>,
    pub ollama: Option<String>,
    pub reachable: bool,
}
```

### Scan Sources

1. **Local hardware**: `sysctl` (macOS) / `/proc` (Linux) for CPU/RAM. `system_profiler SPDisplaysDataType` / `nvidia-smi` for GPU
2. **Local models**: Scan `~/.kova/models/`, `~/.cache/huggingface/`, `~/models/` for `.gguf`, `.safetensors`, `.nanobyte`
3. **Local services**: Probe `localhost:11434` (ollama), `localhost:3006` (kalosm)
4. **SSH nodes**: Parse `~/.ssh/config` for known hosts. Probe each with 3s timeout: `ssh host 'uname -m && free -h && which ollama && nvidia-smi 2>/dev/null'`
5. **Re-probe**: Background thread every 60s. Hot add/drop nodes

### REPL Startup Banner

```
nodes: local(M4/16G/Metal) n0(20c/15G) n1(20c/31G/ollama) n3(14c/30G) n2(down)
models: kova-spark.nanobyte(48K) qwen-0.5b.gguf(380M)
pyramid: 12 subatomic | 4 molecular | 1 cellular | claude(fallback)
```

### Inference Routing

```
Local nanobyte pyramid > Local GGUF (Kalosm) > Remote GPU node (IRONHIVE) > Claude API
```

Discovery feeds into f382 (dual_stream) — extend to check pyramid first:

```rust
pub fn f382(...) -> Receiver<Arc<str>> {
    // 1. Try pyramid (microseconds)
    if let Some(result) = pyramid.process(input).resolved() {
        return immediate_channel(result);
    }
    // 2. Try local GGUF
    // 3. Try remote node
    // 4. Try Claude API (during migration only)
}
```

## Implementation: src/bridge.rs

### PTY Bridge to Claude

Default mode until pyramid is self-sufficient. Spawn `claude` as child process via PTY. Pipe user input, stream output. Log everything.

```rust
pub struct Bridge {
    pty: PtyPair,            // portable-pty
    logger: BridgeLogger,    // logs to sled for training
    session_id: String,
}

impl Bridge {
    pub fn spawn() -> Result<Self>    // spawn claude CLI in PTY
    pub fn send(&self, input: &str)   // write to PTY stdin
    pub fn recv(&self) -> String      // read from PTY stdout (streaming)
}
```

### Logging for Training

Every bridge interaction logged as labeled training data:

```rust
pub struct BridgeLog {
    pub session_id: String,
    pub timestamp: u64,
    pub tier: u8,              // which tier would have handled this
    pub input: String,
    pub output: String,
    pub tool_calls: Vec<String>,
    pub routing_decision: String,
    pub latency_ms: u64,
}
```

Stored in sled `bridge_logs` tree. Exported via existing training_data.rs pipeline (f181) for DPO/SFT.

### Dependencies

Add to Cargo.toml:
```toml
portable-pty = "0.8"
```

### KOVA_MODE env

```
KOVA_MODE=bridge   # PTY bridge to claude (default during migration)
KOVA_MODE=native   # Kova's own inference via pyramid + f382
KOVA_MODE=hybrid   # Pyramid for T1/T2, bridge for T3+ (transition mode)
```

## Claude Migration Timeline

### Phase 1: Subatomics Online (Now -> 2 months)

- Claude handles tiers 2-4 via PTY bridge
- Train T1 subatomics using:
  - Existing tournament data
  - Claude bridge logs (input classification, tool tagging, etc.)
  - Synthetic data from candle_train.rs
- Consolidate T1 models into first `.nanobyte` blob
- Validate via Micro Olympics (gauntlet.rs)
- **Gate**: T1 handles >90% of classification/tagging tasks without escalation

### Phase 2: Molecular Models Replace Claude T2 (2-4 months)

- Extract Claude's tier 2 behavior from bridge logs:
  - How Claude routes between tools
  - How Claude summarizes context
  - How Claude plans edits
- Train T2 molecular models with routing weights to T1
- Cross-tier routing weights learned during training
- **Gate**: T2 handles >80% of routing/planning without escalation to T3

### Phase 3: Cellular Models Replace Claude T3 (4-8 months)

- Extract Claude's tier 3 behavior from bridge logs:
  - Code generation patterns
  - Conversation style
  - Multi-step planning
- Train T3 cellular models (5-10M params) via candle
- Deploy to IRONHIVE nodes for GPU-accelerated training
- **Gate**: T3 handles >70% of code gen/conversation without escalation

### Phase 4: Pyramid Seals Shut (8-12 months)

- Extract Claude's remaining tier 4 edge case handling from bridge logs
- Train final cellular models on the hardest cases
- Run gauntlet at maximum difficulty — all 5 phases, no API calls
- **Gate**: Full gauntlet pass with zero external API calls
- Delete ANTHROPIC_API_KEY
- Remove bridge.rs from default build
- Kova runs 100% on local hardware
- **No subscription. No external calls. No dependency.**

## C2 Integration

### Single Binary Drop

`kova` binary already deploys via `kova c2 deploy`. Extend:

1. `rsync` binary to any box
2. On startup, `discovery.rs` scans local hardware
3. Finds `.nanobyte` files, loads pyramid
4. Probes for other kova instances via SSH
5. Joins the mesh — reports resources to coordinator
6. Receives work via tmux dispatch (f377/f378/f379)

### Fleet Training

Distribute training across IRONHIVE:
- n0 (lf): Train T1 subatomics (small, CPU-friendly)
- n1 (gd): Train T2 molecular (31G RAM, biggest node)
- n3 (st): Train T3 cellular (30G RAM)
- Coordinator (local Mac M4): Consolidate nanobytes, run validation

### Sponge Mesh for Inference

For requests that need distributed inference:
- f379 (sponge mesh) dispatches to best available node
- Rate-limit aware — auto-backoff and retry
- Hot failover — if node drops, reroute to next

## Implementation Order

### Sprint 1: Foundation (src/nanobyte.rs + src/swarm.rs)

1. Nanobyte file format: header, manifest, weight region
2. `load()` via memmap2 — zero-copy mmap
3. `weights()` and `routing()` — offset-based slice access
4. MicroModel trait definition
5. Pyramid struct with tier routing
6. `consolidate()` — pack safetensors into nanobyte
7. **Files**: `src/nanobyte.rs`, `src/swarm.rs`, `src/swarm/mod.rs`
8. **Test**: Create a 2-model nanobyte, load, verify weight access

### Sprint 2: Subatomic Models (src/swarm/subatomic.rs)

1. **Noodle first** — train 30K param penguin personality on synthetic session context data
2. Pack Noodle into first `.nanobyte` via consolidate()
3. Wire Noodle into REPL loop — after each tool result, run forward(), print quip
4. Validate full pipeline: train -> safetensors -> nanobyte -> mmap -> forward() -> output string
5. Then implement: intent_classify, code_vs_english, slop_detector
6. Train all using existing candle_train.rs pipeline, consolidate into combined nanobyte
7. Wire classifiers into REPL preprocessing (before inference)
8. **Files**: `src/swarm/subatomic.rs`
9. **Test**: Noodle produces quip in <100us. Classifiers: 100 inputs, <1ms each

### Sprint 3: Discovery + Bridge (src/discovery.rs + src/bridge.rs)

1. Local hardware scan (CPU, RAM, GPU, models)
2. SSH node probe with 3s timeout
3. Resource map with periodic re-probe
4. REPL startup banner
5. PTY bridge to claude via portable-pty
6. Bridge logging to sled
7. KOVA_MODE env var routing
8. **Files**: `src/discovery.rs`, `src/bridge.rs`
9. **Test**: Discovery finds local hardware. Bridge spawns claude, sends input, gets output

### Sprint 4: Molecular Layer (src/swarm/molecular.rs)

1. Implement routing weight vectors (learned, not hardcoded)
2. Train intent_router using Claude bridge logs
3. Train tool_selector using tool call logs
4. Consolidate T1+T2 into combined nanobyte
5. **Files**: `src/swarm/molecular.rs`, `src/swarm/train.rs`
6. **Test**: Intent routing accuracy >90% vs Claude baseline

### Sprint 5: Cellular Layer + Closure (src/swarm/cellular.rs)

1. Implement code_gen cellular model (5M params)
2. Train on Claude's code generation logs
3. Train conversation model on Claude's chat logs
4. Full pyramid integration — T1 -> T2 -> T3 with confidence gating
5. Gauntlet validation at each phase
6. **Files**: `src/swarm/cellular.rs`
7. **Gate**: Full gauntlet pass, zero API calls

## Verification

### Unit Tests

- Nanobyte: load/save roundtrip, offset arithmetic, manifest parsing
- Subatomic: forward pass produces correct output dimensions
- Routing: confidence gating escalates correctly
- Consolidation: multiple safetensors pack into valid nanobyte

### Integration Tests

- Pyramid processes simple input end-to-end without escalation
- Bridge logs capture complete interaction for training
- Discovery finds local hardware and reports correctly
- KOVA_MODE switches between bridge/native/hybrid

### Gauntlet Gate

Run `cargo run -p kova --bin kova-test --features tests` at each sprint:
- Sprint 1-2: Existing tests pass + nanobyte tests
- Sprint 3: Bridge smoke test
- Sprint 4-5: Gauntlet phases 1-3 without API calls

### Migration Metrics

Track per-tier:
- Escalation rate (% of requests that go to next tier)
- Latency (microseconds for T1, milliseconds for T2, seconds for T3)
- Accuracy vs Claude baseline (measured by tournament)
- API call count (should trend to zero)
