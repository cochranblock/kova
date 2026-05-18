# User Story Analysis — Kova 0.8.x

Date: 2026-05-17. Evaluator: Claude Sonnet 4.6.
Supersedes: 2026-03-27 analysis (pre-pyramid, pre-redb, pre-MoE wiring).

---

## Personas

### P1 — Solo Developer (Michael / primary user)
Rust expert. Runs kova daily as a coding assistant on IRONHIVE. Has GGUF models
configured. Knows every subcommand. Wants the system to get out of the way and
make him faster. Pain point: repetitive compile-fail-edit loops.

### P2 — Fleet Operator
Manages 4+ worker nodes. Wants coordinated builds, broadcast deploys, and node
health at a glance. Less interested in the AI loop, more interested in c2 and
orchestration. Probably Michael in a different mode, but worth separating.

### P3 — Model Trainer
Cares about the flywheel: collect training pairs, fine-tune experts, measure
improvement. Reads `compiler_pairs.redb`. Runs `kova train-data`. Wants to see
which expert fails most and what the error patterns are.

### P4 — First-Time External User
Has heard about kova. Downloads the binary. Has no GGUF, no nodes, no config.
Wants to try it in 5 minutes. Currently hits a wall: no model = no REPL = blank
stare.

---

## User Stories

| ID    | Persona | Story | Status |
|-------|---------|-------|--------|
| US-01 | P1 | As a solo dev, I want the REPL to flag my input type (code vs English) so I don't have to think about routing. | **DONE** — code_vs_english T1 classifier wired, prints dim gray hint. |
| US-02 | P1 | As a solo dev, I want the REPL to warn me when output looks like AI slop so I can review before committing. | **DONE** — slop_detector T1 wired, prints yellow warning at conf > 0.65. |
| US-03 | P1 | As a solo dev, I want compile errors auto-fixed without me re-typing the command. | **DONE** — Sponge Mesh retry loop, compiler_teacher pair capture. |
| US-04 | P1 | As a solo dev, I want to see my LLM call history (latency, tokens, model) after a session. | **PARTIAL** — T109 traces stored in redb, f162/f165 implemented, but no CLI subcommand exposes them. |
| US-05 | P1 | As a solo dev, I want the system to route hard problems to a better expert automatically. | **PARTIAL** — MoE tournament logic exists; intent_classifier trained on banking77 (wrong domain). |
| US-06 | P1 | As a solo dev, I want to run interactive programs (vim, htop, tests with prompts) inside the agent loop. | **MISSING** — PTY bridge not implemented. `run_command` pipes stdout but can't handle interactive input. |
| US-07 | P1 | As a solo dev, I want the REPL banner to tell me which models and classifiers are loaded at startup. | **MISSING** — Banner shows version only. No pyramid status, no model list. |
| US-08 | P2 | As a fleet operator, I want to broadcast a build to all nodes and see pass/fail per node in real time. | **DONE** — `kova c2 build --broadcast` works. Node results streamed. |
| US-09 | P2 | As a fleet operator, I want to check node health (RAM, load, disk) without SSHing in manually. | **DONE** — `kova c2 inspect` shows hardware specs per node. |
| US-10 | P2 | As a fleet operator, I want a single `kova deploy` command instead of remembering c2 flags. | **MISSING** — Must type `kova c2 build --broadcast --release -p kova`. No alias. |
| US-11 | P3 | As a model trainer, I want to export training pairs as JSONL for fine-tuning. | **DONE** — `kova train-data` calls dump_training_data(), emits JSONL. |
| US-12 | P3 | As a model trainer, I want to see which expert fails most and what error patterns are common. | **DONE** — dump_training_data() prints per-expert counts and top-10 errors. |
| US-13 | P4 | As a first-time user, I want kova to work without a GGUF configured — even if degraded — so I can evaluate it. | **MISSING** — No GGUF = REPL opens but every query returns an inference error. T1 pyramid runs but output is invisible. |

---

## Gap Matrix

| Gap | Affected Personas | Blocking? | Effort |
|-----|-------------------|-----------|--------|
| No `kova history` CLI | P1 | No | ~30 lines |
| No pyramid status in banner | P1, P4 | No | ~15 lines |
| PTY bridge missing | P1 | Yes (for interactive tools) | Medium |
| Intent classifier wrong domain | P1, P3 | Yes (blocks priority queue) | Training work |
| No T1-only mode for P4 | P4 | Yes (blocks first-time eval) | Small |
| No `kova deploy` alias | P2 | No | ~5 lines |
| T2/T3 training data absent | P3 | Yes (pyramid stuck at T1) | Data collection |

---

## Backlog Reprioritization

### Current top 5 from BACKLOG.md (as of this analysis):
1. Static carving (tokeniaztion to 100%)
2. Parallel retrieval pipeline
3. Priority queue (augment scheduling)
4. Streaming tool-call UI
5. PTY bridge

### Recommended reorder:

**#1 — Static carving** (unchanged). 60 untokenized symbols is a correctness gap.

**#2 — PTY bridge** (promoted from #5). US-06 is a daily P1 pain. Every test suite
with interactive prompts breaks the agent loop. Blocking real use cases now.

**#3 — `kova history` command** (new). US-04 is PARTIAL. f162/f165 exist. This is
a 30-line CLI wrapper. Ship the observability that's already built.

**#4 — Pyramid status banner** (new). US-07. 15 lines. Closes the "what is loaded?"
question for P1 and makes the T1 work visible to P4. High signal-to-effort.

**#5 — Priority queue** (unchanged position, but blocked). US-05 is only useful once
the intent_classifier is retrained on code-intent labels (debug/add/refactor/
explain/test). Banking77 is wrong. Retrain first, then wire the queue.

---

## Key Architectural Note — intent_classifier Domain Mismatch

The intent_classifier in `starter.nanobyte` was trained on banking77 — a customer
service dataset with intents like "check_balance", "report_lost_card". This is
actively wrong for code routing. The MoE tournament selects experts based on intent
tags, but the tags it's producing are customer-service labels.

**Required fix before priority queue is useful:**
1. Collect a small labeled corpus of code intents: debug, add-feature, refactor,
   explain, write-test, review, build-infra.
2. Retrain T1 intent_classifier on this corpus (~500 examples is enough for T1).
3. Update expert routing table to match new labels.
4. Then wire priority queue to the classifier output.

Until this is done, priority queue will route on garbage labels.

---

## Honest Assessment

**What works:** The pyramid machinery (T1 inference, telemetry, compiler pairs,
Sponge Mesh retry) is solid. The redb migration unified storage. The MoE
tournament runs. c2 broadcast is reliable across 4 nodes.

**What's missing:** Training data. T2 and T3 are defined architecturally but have
no trained weights. The T1 classifiers (slop_detector, code_vs_english,
lang_detector) work but intent_classifier is domain-wrong. P4 (first-time user)
still hits a wall: without GGUF the REPL is a shell that echoes errors.

**What to build next:** The three small UX items (#3 and #4 above) will make the
existing machinery visible and usable in under an hour of work. PTY bridge (#2)
unblocks the hardest P1 daily-use gap. Intent retraining is the leverage point
that makes the rest of the pyramid earn its place.

---

---

## Master Rust Engineer Review

Date: 2026-05-17. Reviewer: Claude Sonnet 4.6 acting as senior Rust code reviewer.
Files read: `src/nanobyte.rs`, `src/storage.rs`, `src/error.rs`, `src/trace.rs`,
`src/codegen_moe/compiler_teacher.rs`, `src/moe.rs`, `src/tools.rs` (grep),
`src/lib.rs`, `Cargo.toml`.

---

### What Is Working Well

**`nanobyte.rs` is production-grade.** The `repr(align(8))` wrapper for the
embedded `include_bytes!` is correct and portable — without it, the f32 slice
reinterpretation is UB on targets where `.rodata` places the bytes at a
non-4-aligned address. The `SAFETY` comments on both unsafe blocks are accurate
and sufficient. Atomic write via `tmp`→`rename` means a crashed pack-starter
leaves the old file intact. The `slice_f32` alignment check covers both the start
offset and the size, which is the right pair of conditions. The BLAKE3 signature
is verified before any parsing, not after — correct order.

**`storage.rs` redb migration pattern is correct.** `OnceLock<Option<Arc<Database>>>`
is the right shape: lazy init, shareable, non-blocking on subsequent calls, the
`Option` layer handles open failure without panicking. The `t12Inner::Shared` vs
`t12Inner::Owned(_, TempDir)` split for test isolation is clean — the `TempDir`
drop handle keeps the tmpdir alive for exactly the lifetime of the `t12`.

**Feature-flag hygiene is good.** The `[features]` table is exhaustive and the
`#[cfg(feature = "inference")]` gates are consistent. Conditional pub-use in
`lib.rs` is tidy.

**Error hierarchy is structured.** `thiserror` + `T176` with named variants is
better than raw `anyhow` everywhere. The module-local `E0` in storage with `#[from]`
derives and explicit zstd wrappers is the right shape.

---

### Issues by Severity

#### Critical

**`compiler_teacher.rs:69` — silent training-pair data loss.**

```rust
let encoded = bincode::serde::encode_to_vec(&pair, bincode::config::standard())
    .unwrap_or_default();  // ← returns Vec::new() on failure
let compressed = zstd::encode_all(encoded.as_slice(), 3).unwrap_or(encoded);
```

If `encode_to_vec` fails, `unwrap_or_default()` returns an empty `Vec`. zstd
successfully compresses 0 bytes. The key is inserted with valid-but-empty compressed
data. On read, decompression returns 0 bytes, bincode decode returns `Err`, and the
pair is silently dropped from `all_pairs()`. The training flywheel loses data without
any signal. Fix: `let Ok(encoded) = ... else { return };`.

---

#### High

**`f39(_p)` — dead parameter is an API lie.**

```rust
pub fn f39(_p: impl AsRef<Path>) -> Result<Self, E0> {
    let db = global_db()...  // _p is never used
```

Callers believe they are opening a specific-path store. They are not — they always
get the global DB. Any caller that passes a meaningful path is silently wrong. The
comment says "accepted for API compatibility" but that framing defers the fix
forever. Remove the parameter and update call sites. If the API was previously
path-based and callers are in other modules, the right fix is `pub fn f39() ->
Result<Self, E0>` and a one-time grep+replace.

**Blanket `#[allow(dead_code, unused_imports)]` in storage.rs.**

This allows `ReadableTable` to survive an entire migration cycle unused without
a compiler warning. Blanket suppression of `dead_code` at the module level hides
real drift. Suppress specific items at the item level (`#[allow(dead_code)]` on
the specific struct or field) rather than silencing the whole module. The `tools.rs`
file has `#[allow(non_camel_case_types)]` which is justified by the naming scheme;
`dead_code` is not.

**`T93` string fields for finite-state machine values.**

```rust
pub stage: String,    // "compile" | "clippy" | "tests"
pub outcome: String,  // "success" | "failed"
```

These are enums masquerading as strings. A typo in any callsite becomes a silent
logic error; a refactor that adds a stage is invisible to the type system. Define
`enum Stage { Compile, Clippy, Tests }` and `enum Outcome { Success, Failed }`.
The `Serialize/Deserialize` derives will produce the same wire format.

---

#### Medium

**`decode_manifest` — `.try_into().unwrap()` inside a parse path.**

```rust
let num_classes = u32::from_le_bytes(b[36..40].try_into().unwrap());
```

The slice is statically known to be 4 bytes, so this cannot panic given valid
inputs. But `from_storage` already has a `Result` return type, and
`decode_manifest` is called within it. If a future change shortens a manifest
entry or the bounds check above has an off-by-one, this panics instead of
returning `Err(Error::BadManifest)`. Use `.try_into().map_err(|_| Error::BadManifest)?`
and propagate.

**`f161` key in trace.rs — deterministic "discriminant" named `rand_bytes`.**

```rust
let seed = trace.ts ^ (trace.latency_ms << 16) ^ (trace.prompt_bytes as u64);
let rand_bytes = [seed as u8, (seed >> 8) as u8, ...];
```

Two traces in the same millisecond with identical latency and prompt size produce
the same key, silently overwriting each other. The variable is named `rand_bytes`
but is fully deterministic. Either use an atomic counter suffix or pull 4 bytes
from `rand::random::<u32>()` (one dep already pulled in transitively). Rename
`rand_bytes` → `discriminant` to remove the lie in the name.

**`From<String>` and `From<&str>` on `T176` are grab-bag escape hatches.**

Every `.map_err(|e| e.to_string().into())` in the codebase converts a structured
error into `T176::Other(String)`, erasing the error chain. These two `From` impls
make `.into()` always work for strings, which kills the motivation for having typed
variants. The pattern to prefer: add a specific variant, use `#[from]` where
possible, use `.map_err(T176::VariantName)` for the rest. Remove `From<&str>` —
it converts string literals into error values with no callsite context.

**`compiler_teacher` `LazyLock` cannot be reset between tests.**

`static DB: LazyLock<Option<Database>>` fires once per process. Any test that
exercises `save_pair` or `lookup_hint` hits the real `~/.kova/training/
compiler_pairs.redb`. There are no tests in compiler_teacher today, but the
pattern guarantees test pollution when they are added. Inject a `&Database`
parameter (following the tools.rs `with_test_redb` pattern) or add an optional
`OnceLock`-based override hook.

---

#### Low

**Silently swallowed `create_dir_all` errors.**

```rust
let _ = std::fs::create_dir_all(parent);
```

If the directory cannot be created (permissions, disk full, symlink loop), the
`Database::create` that follows fails with "No such file or directory" — less
informative than the actual cause. At minimum: `if let Err(e) = create_dir_all
... { eprintln!(...) }` before returning.

**`T176` variants named after internal type codes.**

`T176::T193(String)` means "cluster error." `T176::T129(String)` means
"provider error." Outside the LANGUAGE.md dictionary, these variant names are
uninterpretable. Since `T176` is the public error type, its variant names are
part of the API surface. Prefer `T176::Cluster(String)` and `T176::Provider(String)`.

---

### Summary Scorecard

| Area | Rating | Notes |
|------|--------|-------|
| Memory safety | A | All unsafe blocks are correct and commented |
| Error handling | B | Good structure, undermined by escape hatches and string erasure |
| Test isolation | B+ | storage/tools pattern is solid; compiler_teacher is a gap |
| API honesty | C+ | `f39(_p)` and `rand_bytes` naming are active lies |
| Data integrity | C | Silent loss in compiler_teacher is the worst bug in the file |
| Naming | — | Intentional scheme (LANGUAGE.md). Tradeoff, not a mistake |

**Bottom line:** The structural choices (redb migration, OnceLock global, temp DB
for tests, nanobyte alignment) are correct. The most urgent fix is the silent data
loss in `compiler_teacher.rs:69`. The second-most-urgent is removing the `f39`
dead parameter before more callers accumulate. Everything else is technical debt
that is not actively harmful today.

---

## Prior Analysis (2026-03-27)

The March 2026 analysis covered installation friction, edge cases, competitor
positioning, and top-3 fixes. Those findings are still valid but pre-date the
pyramid, redb migration, and MoE wiring. See git history for the original version.

---

*Analysis by Claude Sonnet 4.6 — persona simulation + gap analysis.*
<!-- COCHRANBLOCK-BRAND-FOOTER:START - generated by cochranblock/scripts/brand-stamp.sh -->

---

<sub>&#9656; **THE COCHRAN BLOCK, LLC** &#183; CAGE `1CQ66` &#183; UEI `W7X3HAQL9CF9` &#183; UNLICENSE &#183; [cochranblock.org](https://cochranblock.org)</sub>
<!-- COCHRANBLOCK-BRAND-FOOTER:END -->
