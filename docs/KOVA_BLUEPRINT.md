# Kova Pyramid Blueprint

> Consolidated architecture. One document. Everything builds from this.

## What This Is

Kova replaces external AI APIs with a pyramid of locally-trained models. Not competing on model size — competing on speed and specialization. Hundreds of sub-10K param classifiers doing one job each in microseconds, coordinated by a human-intent-driven priority engine, trained on every Rust crate ever published.

The end state: zero API dependency. The pyramid seals shut. Kova runs 100% on local hardware.

---

## 1. The Pyramid

```
    EXTINCT (Claude API deleted — Phase 4)
         │ trained away
    ┌────┴────┐
    │ CELLULAR │  1M-10M params. Code gen, conversation, planning.
    │ 3-5      │  Routing maps to molecular layer.
    └────┬────┘
         │ learned routing weights
    ┌────┴─────┐
    │ MOLECULAR │  100K-1M params. Intent routing, context summary,
    │ 10-30     │  tool selection. Attention maps to subatomics.
    └────┬─────┘
         │ learned routing weights
    ┌────┴─────┐
    │ SUBATOMIC │  Sub-100K params. Binary/few-class classifiers.
    │ 66 unique │  Microsecond inference. Trigram hash → linear.
    └──────────┘
```

### Tier 1: Subatomic (sub-100K params)

Each model answers one question. `fn forward(weights: &[f32], input: &[f32]) -> Vec<f32>`. Character trigram hash features → fixed-dim vector → single linear layer → softmax. Sub-10K params typical. Microsecond inference on CPU.

**66 unique models** across 12 categories (functions, structs, enums, traits, match, closures, imports, unsafe, error handling, testing, meta, companion). **6 shared/universal models** work across all Rust constructs (visibility, doc-needed, lifetime-needed, naming-convention, complexity-flag, deprecated-pattern).

~55K total params. ~50KB quantized. Fits in L2 cache.

Full catalog: [`docs/SUBATOMIC_CATALOG.md`](docs/SUBATOMIC_CATALOG.md)

**Proven:** 3 models trained on bt's AMD RX 5700 XT via any-gpu Vulkan. slop_detector (514 params, 89.4% acc, ~5us inference), code_vs_english (514 params, 94.2% acc, ~4us), lang_detector (1,285 params, 97.0% acc, ~6us). Weights in [`assets/models/`](assets/models/).

Training infrastructure: [`src/swarm/train.rs`](src/swarm/train.rs) (f389-f392).

### Tier 2: Molecular (100K-1M params)

Coordinators. Each molecular model has a learned weight vector per subatomic — a trained attention map that says how much to trust each T1 output. These routing weights are in the same nanobyte blob.

| Model | Params | Routes To | Purpose |
|-------|--------|-----------|---------|
| intent_router | 200K | intent_classify, tool_tagger, tone_detector | Full intent resolution |
| context_summarizer | 500K | code_vs_english, urgency_scorer | Compress conversation |
| tool_selector | 300K | tool_tagger, path_predictor, flag_expander | Pick + parameterize tool |
| edit_planner | 500K | code_vs_english, path_predictor | Plan file edits |
| commit_writer | 500K | commit_classifier, slop_detector | Generate commit messages |

### Tier 3: Cellular (1M-10M params)

Domain specialists. Each has routing maps to the molecular layer below.

| Model | Params | Routes To | Purpose |
|-------|--------|-----------|---------|
| code_gen | 5M | edit_planner, tool_selector, context_summarizer | Generate code |
| conversation | 3M | intent_router, context_summarizer | Natural language response |
| planner | 2M | intent_router, edit_planner | Multi-step task planning |

### Confidence Gating

Each tier runs only if the tier below signals low confidence. Most requests never get past T1.

```rust
let t1 = pyramid.run_tier1(input);
if t1.confidence >= 0.7 { return t1; }

let t2 = pyramid.run_tier2(input, &t1);
if t2.confidence >= 0.7 { return t2; }

let t3 = pyramid.run_tier3(input, &t2);
if t3.confidence >= 0.7 { return t3; }

// During migration: escalate to Claude
// Post-migration: T3 retries with expanded context
```

---

## 2. Nanobyte Format

One contiguous file. One `mmap()` call. Every model reads from different byte offsets. No model loading. No file I/O per model. Pointer arithmetic and matrix math on already-resident memory.

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
  Per model:
    name: [u8; 32]
    tier: u8              // 1=subatomic, 2=molecular, 3=cellular
    offset: u64           // byte offset into weight region
    size: u64             // weight byte count
    num_classes: u32      // output dimension
    feature_dim: u32      // input dimension
    routing_offset: u64   // T2/T3: offset to routing weights
    routing_size: u64

[WEIGHTS: contiguous f32 blob]
  All model weights packed sequentially.
```

### Key Operations

- `load(path)` — mmap the file. One syscall. Zero copy.
- `weights(model_name)` — return `&[f32]` slice at the model's offset.
- `routing(model_name)` — return routing weight slice for T2/T3 models.
- `consolidate(models, output)` — pack individual trained models into one nanobyte.

### Starter Nanobyte

11 subatomic models ship embedded in the kova binary via `include_bytes!`. Working pyramid on first run. Zero setup.

| # | Model | Params | Purpose |
|---|-------|--------|---------|
| 1 | shitty_test_detector | 30K | REAL/SMOKE/MISSING test classifier |
| 2 | claim_verifier | 40K | Flag unsourced doc claims |
| 3 | noodle | 30K | Companion penguin (Claude Code inspired) |
| 4 | typo_fixer | 50K | Typo correction |
| 5 | code_vs_english | 20K | Binary: code or english |
| 6 | intent_classify | 50K | Question/command/code/conversation |
| 7 | slop_detector | 20K | P12 banned word detection |
| 8 | lang_detector | 30K | Rust lib/bin/test/macro/build |
| 9 | sentiment | 20K | Positive/negative/neutral |
| 10 | commit_msg_scorer | 30K | Commit message quality |
| 11 | filename_predictor | 50K | Suggest filename from content |

~370K total params. <2MB quantized. Ships with kova and any-gpu.

User-supplied `.nanobyte` files in `~/.kova/models/` are discovered on startup and merged into the pyramid at runtime.

---

## 3. Memory Architecture: One Sled, One Priority Queue

No three-zone bookkeeping. No manual mmap management. One sled DB. The OS handles everything else.

**Key format:** `{priority_score}:{model_name}` in sled's B-tree. Ordered iteration is native. Hot models have high scores, cold models have low scores.

**The kernel does the work:**
- **At bat** (L1/L2 cache) — models actively inferring. OS keeps recently-touched pages hot.
- **On deck** (RAM/page cache) — high-priority models. sled accessed them recently, pages stay warm.
- **In the dugout** (disk) — low-priority models. sled keys exist but pages are cold. First read pages them in.

sled + Linux page cache = the entire memory hierarchy for free.

**bt has 48GB RAM.** That's the prefetch buffer. The batter-up model (a subatomic intent classifier) predicts which models will be needed based on what the human just typed, touches those sled keys, and the OS pages in the weights before inference runs.

---

## 4. Intent-Driven Priority Engine

**The human is the cache controller. They don't know it.**

The intent classifier (a T1 subatomic model) watches every human input and updates sled priority scores in real-time:

```
Human types 'fix the bug'    → error-fixer, lifetime, borrow-checker → priority 100
Human types 'add a struct'   → field-count, derive-needed, visibility → priority 100
Human types 'write tests'    → test-quality, assertion-style          → priority 100
Human types 'deploy'         → build-time, binary-size                → priority 100
Human opens unsafe block     → unsafe-analysis, ffi-pattern           → priority 100
```

Intent → priority score update → sled key write → OS pages in the weights → microsecond inference.

The models the human needs are already warm by the time the pyramid runs. Zero-wait inference because the prefetch happened during the time between the human pressing Enter and the pyramid starting to process.

---

## 5. Training Corpus: Every Rust Crate

**240,596 crates from crates.io.** 34GB of `.crate` tarballs. Latest version of every crate. Harvested to bt at `/mnt/data/crates/` (870GB dedicated drive).

Tool: `get-all-crates --latest` with crates.io-index clone.

### Data Carving

Two axes of extraction:

**Static carving** (parse .rs files, no compilation):

| Carving | Method | Models Fed |
|---------|--------|-----------|
| Function signatures | `syn` AST parse | return-type, arg-count, async-detector, self-receiver, generic-count |
| Struct definitions | `syn` parse | field-count, derive-needed, pub-fields, repr-needed |
| Enum definitions | `syn` parse | variant-count, error-enum, variant-data |
| Trait definitions | `syn` parse | method-count, object-safe, supertraits |
| Match expressions | `syn` parse | exhaustive, wildcard, guard-complexity |
| Error handling | grep `Result`/`Option`/`unwrap`/`?` | error-type-choice, unwrap-safety, ?-candidate |
| Doc comments | grep `///`/`//!` | doc-needed, slop-detector, code-vs-english |
| Unsafe blocks | grep `unsafe` | unsafe-necessity, ffi-pattern |
| Import graphs | grep `use` | unused-import, glob-flag, reorder |
| Attribute usage | grep `#[derive]`/`#[cfg]` | derive-predictor, cfg-detector |
| Closure patterns | grep `\|args\|`/`move \|\|` | capture-mode, fn-pointer-candidate |

**Dynamic carving** (compile each crate, capture output):

| Carving | Method | Models Fed |
|---------|--------|-----------|
| Clippy lints | `cargo clippy --message-format=json` | lint-predictor |
| Compile errors | Delete random line, compile, capture error | error-fixer |
| Build timings | `cargo build --timings` | build-time-estimator |
| Type inference | `RUSTC_LOG=rustc_typeck` traces | type-inference model |
| MIR patterns | `rustc -Z unpretty=mir` | complexity-estimator |

### Extraction Pipeline

```
/mnt/data/crates/     → 240K .crate tarballs (34GB)
/mnt/data/corpus/     → extracted .rs files per crate
/mnt/data/training/   → JSONL training data per model
/mnt/data/models/     → trained nanobyte blobs
```

Scripts: [`scripts/extract_corpus.sh`](scripts/extract_corpus.sh), [`scripts/build_training_data.sh`](scripts/build_training_data.sh).

---

## 6. Shared Models Principle

A visibility-classifier works for functions AND structs AND enums AND traits. Train ONE model across all constructs, not N duplicates.

| Universal Model | Works On | Question |
|----------------|----------|----------|
| visibility | fn, struct, enum, trait, mod, const | pub / pub(crate) / private? |
| doc-needed | anything pub | Needs a doc comment? |
| lifetime-needed | fn, struct, impl, trait | Needs explicit lifetimes? |
| naming-convention | all identifiers | Correct case convention? |
| complexity-flag | fn, impl, match | Too complex? Split it? |
| deprecated-pattern | fn, struct, trait | Uses a deprecated Rust pattern? |

6 shared models replace ~24 construct-specific duplicates. Fewer models, more reuse, same coverage.

---

## 7. any-gpu Integration

Training on AMD/NVIDIA/Intel GPUs via any-gpu's wgpu tensor ops. Proven on bt's AMD RX 5700 XT (RADV Vulkan).

**Current state:** any-gpu has forward pass ops (matmul, add, sub, scale, relu, sigmoid, softmax). No autograd yet (Sprint 4). Training uses manual gradient computation — forward matmul on GPU, backward matmul on GPU, weight update on CPU.

**Pipeline proven:**
1. Generate training data (CPU)
2. Featurize via trigram hash (CPU)
3. Forward pass: `gpu.matmul()` + `gpu.softmax()` (GPU)
4. Loss + gradient computation (GPU download → CPU)
5. Backward pass: `gpu.matmul()` for grad_w (GPU)
6. Weight update (CPU, sub-microsecond for <1.3K params)
7. Save weights as binary (CPU)
8. Inference: pure CPU, ~5 microseconds

Results: [`assets/models/`](assets/models/) — 3 trained models, 2,313 total params, 40KB on disk.

---

## 8. Claude Migration Path

Claude occupies all tiers above T1 on day one. As each tier's models train (using Claude's own outputs as training data), Claude retreats up one tier.

### Phase 1: Subatomics Online (now → 2 months)

- Claude handles tiers 2-4 via PTY bridge ([`src/bridge.rs`](src/bridge.rs), planned)
- Train T1 subatomics on crates.io corpus + Claude bridge logs
- Validate via Micro Olympics gauntlet
- **Gate:** T1 handles >90% of classification/tagging without escalation

### Phase 2: Molecular Models (2-4 months)

- Extract Claude's routing/planning behavior from bridge logs
- Train T2 molecular models with learned routing weights to T1
- **Gate:** T2 handles >80% of routing/planning without escalation

### Phase 3: Cellular Models (4-8 months)

- Extract Claude's code gen/conversation patterns from bridge logs
- Train T3 cellular models (5-10M params) on IRONHIVE nodes
- **Gate:** T3 handles >70% of code gen/conversation without escalation

### Phase 4: Pyramid Seals Shut (8-12 months)

- Train final cellular models on Claude's hardest edge cases
- Full gauntlet pass with zero external API calls
- Delete ANTHROPIC_API_KEY
- **No subscription. No external calls. No dependency.**

Bridge logging: every Claude interaction = labeled training data for the tier below. Claude trains its own replacement at every level.

---

## 9. Infrastructure

### What Exists (built this session)

| Component | Location | Status |
|-----------|----------|--------|
| Agent loop | [`src/agent_loop.rs`](src/agent_loop.rs) (f147/f148) | Working |
| Context compaction | [`src/context_mgr.rs`](src/context_mgr.rs) (f380) | Working |
| Dual-mode inference | [`src/inference/mod.rs`](src/inference/mod.rs) (f382) | Working |
| Anthropic SSE streaming | [`src/inference/providers.rs`](src/inference/providers.rs) (f381) | Working |
| Checkpoint/undo | [`src/tools.rs`](src/tools.rs) (f383/f384) | Working |
| Permission gates | [`src/tools.rs`](src/tools.rs) (is_guarded/perm_gate) | Working |
| C2 fleet commands | [`src/c2.rs`](src/c2.rs) (f385-f388) | Working |
| Swarm training | [`src/swarm/train.rs`](src/swarm/train.rs) (f389-f392) | Working |
| 3 trained models | [`assets/models/`](assets/models/) | Proven on AMD GPU |
| Crates.io corpus | bt `/mnt/data/crates/` (240K crates, 34GB) | Harvested |
| 314 tests | `cargo test --release -p kova` | Passing |

### What Needs Building

| Component | Module | Depends On |
|-----------|--------|------------|
| Nanobyte format | `src/nanobyte.rs` | memmap2 |
| Pyramid orchestrator | `src/swarm/mod.rs` (expand) | nanobyte |
| Subatomic forward fns | `src/swarm/subatomic.rs` | nanobyte |
| Molecular routing | `src/swarm/molecular.rs` | nanobyte, subatomic |
| Cellular specialists | `src/swarm/cellular.rs` | nanobyte, molecular |
| PTY bridge | `src/bridge.rs` | portable-pty |
| Discovery | `src/discovery.rs` | inspect, config |
| Sled priority queue | `src/swarm/priority.rs` | sled |
| Corpus extraction | expand `scripts/extract_corpus.sh` | crates.io harvest |
| Static carving (syn) | `scripts/carve_static.rs` | syn crate |
| Dynamic carving | `scripts/carve_dynamic.sh` | rustc, clippy |

### Existing Modules to Reuse

| Module | Provides | Used For |
|--------|----------|---------|
| [`src/micro/candle_train.rs`](src/micro/candle_train.rs) | Transformer training, BPE tokenizer | T2/T3 model training |
| [`src/micro/quantize.rs`](src/micro/quantize.rs) | TurboQuant (FWHT + 2/4-bit) | Nanobyte compression |
| [`src/micro/tournament.rs`](src/micro/tournament.rs) | Olympic competition | Model validation |
| [`src/micro/router.rs`](src/micro/router.rs) | Epsilon-greedy bandit | Warm-start molecular routing |
| [`src/micro/validate.rs`](src/micro/validate.rs) | Output validation gates | Per-tier output checks |
| [`src/training_data.rs`](src/training_data.rs) | Trace → DPO/SFT export | Bridge log conversion |
| [`src/feedback.rs`](src/feedback.rs) | Failure recording | Curriculum from failures |
| [`src/c2.rs`](src/c2.rs) | Fleet dispatch, sponge mesh | Distributed training |
| [`src/inspect.rs`](src/inspect.rs) | Hardware detection | Discovery foundation |

### Fleet Resources

| Node | Hardware | Storage | Role |
|------|----------|---------|------|
| Local (Mac M4) | 16G RAM, Metal | SSD | Coordinator, validation |
| n0/lf | 20 cores, 15G RAM | 744G | T1 training (CPU) |
| n1/gd | 20 cores, 31G RAM | 757G | T2 training (biggest RAM) |
| n2/bt | 20 cores, 48G RAM, RX 5700 XT | 870G /mnt/data | GPU training, corpus storage |
| n3/st | 14 cores, 30G RAM | 762G | T3 training |

---

## 10. Implementation Order

### Sprint 1: Nanobyte Format

Build `src/nanobyte.rs`. Header, manifest, weight region. Load via memmap2. `weights()` returns `&[f32]` at offset. `consolidate()` packs individual models. Test: create 2-model nanobyte, load, verify.

### Sprint 2: Corpus Extraction + Real Training Data

Run `extract_corpus.sh` on bt. Run `build_training_data.sh`. Generate JSONL for all 3 proven models. Retrain on real crates.io data (not synthetic). Measure accuracy improvement.

### Sprint 3: Pyramid Orchestrator + Priority Engine

Expand `src/swarm/mod.rs`. MicroModel trait. Pyramid struct with tier routing. Sled priority queue in `src/swarm/priority.rs`. Intent classifier updates scores. Wire into REPL.

### Sprint 4: Discovery + Bridge

`src/discovery.rs` — hardware scan, SSH probe, model discovery. `src/bridge.rs` — PTY bridge to Claude with logging. KOVA_MODE env routing.

### Sprint 5: Molecular + Cellular + Closure

T2 routing weights. T3 code gen. Full pyramid. Gauntlet validation. Delete API key.

---

## Deliverable

When this plan executes: `docs/KOVA_BLUEPRINT.md` — one clean doc in the repo that replaces the scattered PYRAMID_ARCHITECTURE.md + SUBATOMIC_CATALOG.md with a single consolidated reference. Every claim source-linked. Every number verifiable.
