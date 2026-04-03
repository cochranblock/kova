# Subatomic Model Catalog

> Every Rust construct decomposed into the smallest learnable tasks. Hundreds of models, each sub-10K params, each microsecond inference. Trained on 240K crates from crates.io (34GB, harvested to bt `/mnt/data/crates/`).

## Core Principles

### 1. One Task = One Model

Each subatomic model answers exactly one question about one aspect of code. Binary or few-class classifier. Sub-10K params. Trigram hash features → linear layer. Microsecond inference on CPU.

### 2. Shared Models Across Constructs

A `visibility-classifier` works for functions AND structs AND enums AND traits. Don't train 4 separate models — train ONE that works across all Rust constructs. Universal models:

| Shared Model | Works On | Question |
|-------------|----------|----------|
| visibility | fn, struct, enum, trait, mod, const | pub / pub(crate) / private? |
| doc-needed | anything pub | Does this need a doc comment? |
| lifetime-needed | fn, struct, impl, trait | Does this need explicit lifetimes? |
| naming-convention | everything | snake_case / CamelCase / SCREAMING correct? |
| complexity-flag | fn, impl, match | Too complex? Should be split? |
| deprecated-pattern | fn, struct, trait | Uses a deprecated Rust pattern? |

Fewer models, more reuse, same coverage.

### 3. Intent-Driven Priority Engine

Human intent drives the sled priority queue. Not heuristics. Not access patterns. What the human is DOING determines which models are hot.

```
Human types 'fix the bug'    → error-fixer, borrow-checker, lifetime models → top
Human types 'add a struct'   → field-count, derive-needed, visibility      → hot
Human types 'write tests'    → test-pattern, assertion-style, coverage     → promote
Human types 'deploy'         → build-time, binary-size, optimization       → load
Human opens unsafe block     → unsafe-analysis, ffi-pattern, raw-pointer   → page in
```

The intent classifier (itself a subatomic model) watches input and updates sled priority scores in real-time. The human never waits — models they need are already warm by the time inference runs.

**Intent → priority score → sled key update → OS pages in the weights → microsecond inference.**

The human is the cache controller. They don't know it. They just type and the right models are already there.

### 4. One Sled, One Priority Queue

No three-zone bookkeeping. One sled DB. Key format: `{priority_score}:{model_name}`. sled is a B-tree — ordered iteration is native. Hot models have high priority scores, cold models have low.

The batter-up model updates priority scores based on context. sled handles the rest — hot data naturally stays in the OS page cache (RAM), cold data lives on disk. No manual memory management. No mmap zones.

**sled + Linux page cache = the entire memory hierarchy for free.**

The dugout/on-deck/at-bat metaphor still holds:
- **At bat** (L1/L2 cache) — models actively inferring. OS keeps them hot.
- **On deck** (RAM/page cache) — high-priority models. sled touched them recently, OS keeps pages warm.
- **In the dugout** (disk) — low-priority models. sled keys exist but pages are cold. First access pages them in.

One sled tree, one priority score per model, let the kernel manage the rest.

---

## Data Carving Strategies

### Static Carving (Parse .rs Files)

Extract training data by parsing source code. No compilation needed. Runs on the 240K crate corpus.

| Carving | Method | Trains Model | Data Format |
|---------|--------|-------------|-------------|
| Function signatures | `syn` AST parse → extract `fn` items | function-predictor, return-type, arg-count | `{sig: "fn foo(x: i32) -> bool", ...labels}` |
| Match arms | Extract `match` expressions → arm patterns | pattern-completer, exhaustive-detector | `{match_expr, arms, has_wildcard}` |
| Error handling | Grep `Result`, `Option`, `unwrap`, `expect`, `?` | error-style-classifier | `{line, pattern: result/option/unwrap/expect/?}` |
| Trait impl blocks | `syn` parse → extract `impl Trait for Type` | trait-usage-predictor | `{trait_name, type_name, method_count}` |
| Import graphs | Extract `use` statements per file | dependency-recommender | `{file_type, imports[]}` |
| Type annotations | Extract all `: Type` patterns | type-predictor | `{context, type_annotation}` |
| Doc comments | Extract `///` and `//!` blocks | doc-quality, slop-detector | `{comment_text, has_examples, label}` |
| Attribute usage | Extract `#[derive()]`, `#[cfg()]`, etc. | derive-predictor, cfg-detector | `{item_kind, attributes[]}` |
| Unsafe blocks | Extract `unsafe { }` and `unsafe fn` | unsafe-necessity, ffi-pattern | `{code, reason, is_sound}` |
| Closure patterns | Extract `|args| body` and `move ||` | capture-mode, fn-pointer-candidate | `{closure_text, captures_ref, captures_move}` |

### Dynamic Carving (Compile Each Crate)

Extract training data from compiler output. Requires building crates. More expensive but produces unique signal.

| Carving | Method | Trains Model | Data Format |
|---------|--------|-------------|-------------|
| Clippy lints | `cargo clippy --message-format=json` per crate | lint-predictor | `{code_span, lint_name, suggestion}` |
| Compile errors | Delete random line, compile, capture error | error-fixer | `{error_msg, missing_line, file_context}` |
| Build timings | `cargo build --timings` per crate | build-time-estimator | `{crate_size, dep_count, build_secs}` |
| Type inference | `RUSTC_LOG=rustc_typeck` traces | type-inference model | `{expression, inferred_type}` |
| MIR patterns | `rustc -Z unpretty=mir` on key files | complexity-estimator | `{fn_name, mir_block_count, mir_statement_count}` |

---

## Full Subatomic Decomposition by Rust Construct

### Functions (10+ models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| return-type-predictor | fn signature context | Result/Option/void/primitive/custom | ~1.5K | No |
| arg-count-predictor | fn name + context | 0/1/2/3/4+ | ~1.5K | No |
| visibility-classifier | item text | pub/pub(crate)/private | ~1K | **Yes** — all items |
| async-detector | fn signature | async/sync | ~514 | No |
| result-vs-option | fn with error handling | Result/Option/neither | ~1K | No |
| lifetime-needed | fn signature | yes/no | ~514 | **Yes** — fns, structs, impls |
| generic-count | fn signature | 0/1/2/3+ | ~1.3K | No |
| self-receiver | fn in impl block | &self/&mut self/self/none | ~1.3K | No |
| must-use-predictor | fn signature + return type | should have #[must_use]? | ~514 | No |
| doc-needed | pub fn signature | needs doc comment? | ~514 | **Yes** — all pub items |
| error-handling-style | fn body | ?-operator / match / unwrap / expect | ~1.3K | No |
| complexity-flag | fn body | simple/medium/complex/split-it | ~1.3K | **Yes** — fns, impls |

### Structs (8 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| field-count | struct definition | 1-3/4-8/9+ | ~1K | No |
| derive-needed | struct with fields | which derives? (Debug/Clone/Serialize/...) | ~2K | No |
| pub-fields | struct context | all-pub/mixed/all-private | ~1K | No |
| repr-needed | struct definition | #[repr(C)]/#[repr(transparent)]/none | ~1K | No |
| builder-pattern | struct with 4+ fields | needs builder? | ~514 | No |
| newtype-candidate | struct with 1 field | is this a newtype wrapper? | ~514 | No |
| default-impl | struct definition | should impl Default? | ~514 | No |
| serde-needed | struct context (use statements, field types) | needs Serialize/Deserialize? | ~514 | No |

### Enums (6 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| variant-count | enum definition | 2-4/5-10/11+ | ~1K | No |
| enum-vs-struct | type definition context | should be enum/should be struct | ~514 | No |
| error-enum-detector | enum definition | is this an error type? | ~514 | No |
| non-exhaustive-needed | pub enum | should have #[non_exhaustive]? | ~514 | No |
| variant-data | enum variants | unit/tuple/struct variants | ~1K | No |
| display-impl-needed | enum definition | needs Display impl? | ~514 | No |

### Traits (6 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| method-count | trait definition | 1-2/3-5/6+ | ~1K | No |
| has-default-impl | trait method | has default implementation? | ~514 | No |
| object-safe | trait definition | is object-safe? (dyn Trait) | ~514 | No |
| supertraits-needed | trait definition | needs supertraits? which? | ~1.5K | No |
| sealed-trait | trait + visibility | should be sealed? | ~514 | No |
| blanket-impl | trait definition | good candidate for blanket impl? | ~514 | No |

### Match Expressions (5 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| exhaustive-detector | match expression | exhaustive/needs-wildcard | ~514 | No |
| wildcard-needed | match with enum | _ arm needed? | ~514 | No |
| guard-complexity | match arm | simple/has-guard/complex-guard | ~1K | No |
| match-vs-if-let | match expression | better as if-let? | ~514 | No |
| arm-count | match expression | 2-3/4-6/7+ | ~1K | No |

### Closures (4 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| capture-mode | closure expression | move/ref/implicit | ~1K | No |
| fn-pointer-candidate | closure expression | could be fn pointer instead? | ~514 | No |
| closure-vs-fn | closure context | should be extracted to named fn? | ~514 | No |
| async-closure | closure + context | should be async? | ~514 | No |

### Use/Imports (5 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| unused-import | use statement + file body | used/unused | ~514 | No |
| missing-import | code with unresolved name | suggest import | ~2K | No |
| reorder-needed | import block | needs reordering? | ~514 | No |
| glob-import-flag | use statement | glob import bad practice? | ~514 | No |
| prelude-candidate | frequently used imports | should be in prelude? | ~514 | No |

### Unsafe (4 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| unsafe-necessity | unsafe block | necessary/removable | ~514 | No |
| ffi-pattern | unsafe code | FFI/pointer-arith/transmute/other | ~1.3K | No |
| raw-pointer-safety | unsafe with pointers | sound/unsound-pattern | ~514 | No |
| unsafe-fn-needed | fn with unsafe body | should fn be unsafe? | ~514 | No |

### Error Handling (4 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| error-type-choice | error context | thiserror/anyhow/custom/std | ~1.3K | No |
| unwrap-safety | .unwrap() call | safe/should-use-expect/should-handle | ~1K | No |
| question-mark-candidate | match/if-let on Result | should use ? operator? | ~514 | No |
| error-chain-depth | error handling code | flat/one-level/deep-chain | ~1K | No |

### Testing (5 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| test-quality (shitty-test-detector) | test fn body | REAL/SMOKE/MISSING | ~1K | No |
| assertion-style | test assertions | assert_eq/assert/matches/custom | ~1.3K | No |
| test-coverage-gap | pub fn without test | needs test? priority? | ~514 | No |
| fixture-needed | test fn | needs setup/teardown? | ~514 | No |
| proptest-candidate | test fn | good for property-based testing? | ~514 | No |

### Meta/Project (5 models)

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| slop-detector | doc text | clean/slop | ~514 | No |
| code-vs-english | text line | code/english | ~514 | No |
| claim-verifier | doc sentence | sourced/unsourced/numeric | ~1K | No |
| commit-msg-quality | commit message | good/bad/needs-body | ~1K | No |
| naming-convention | identifier | correct/wrong convention | ~514 | **Yes** — all identifiers |

### Companion

| Model | Input | Output | Params | Shared? |
|-------|-------|--------|--------|---------|
| noodle | session context vector | quip index | ~1K | No |

---

## Summary

| Category | Models | Shared | Total Unique |
|----------|--------|--------|-------------|
| Functions | 12 | 4 shared | 8 unique |
| Structs | 8 | 0 | 8 |
| Enums | 6 | 0 | 6 |
| Traits | 6 | 0 | 6 |
| Match | 5 | 0 | 5 |
| Closures | 4 | 0 | 4 |
| Imports | 5 | 0 | 5 |
| Unsafe | 4 | 0 | 4 |
| Error Handling | 4 | 0 | 4 |
| Testing | 5 | 0 | 5 |
| Meta/Project | 5 | 1 shared | 4 unique |
| Companion | 1 | 0 | 1 |
| **Shared (universal)** | **6** | — | **6** |
| **Total** | **71** | **6 shared across categories** | **66 unique** |

**Estimated total params:** ~55K across all 66 models. ~220KB raw, ~50KB quantized. Fits in L2 cache.

## Training Data Sources

All from the crates.io harvest on bt (`/mnt/data/crates/`, 240,596 crates, 34GB):

| Source | Carving | Models Fed |
|--------|---------|-----------|
| `lib.rs` files | Static: filename | rust-kind (library) |
| `main.rs` files | Static: filename | rust-kind (binary) |
| `build.rs` files | Static: filename | rust-kind (build) |
| `#[test]` blocks | Static: grep | rust-kind (test), test-quality, assertion-style |
| `macro_rules!` blocks | Static: grep | rust-kind (macro) |
| `pub fn` signatures | Static: syn parse | return-type, arg-count, async-detector, self-receiver, generic-count |
| `struct` definitions | Static: syn parse | field-count, derive-needed, pub-fields, repr-needed |
| `enum` definitions | Static: syn parse | variant-count, error-enum, variant-data |
| `trait` definitions | Static: syn parse | method-count, object-safe, supertraits |
| `match` expressions | Static: syn parse | exhaustive, wildcard, guard-complexity |
| `unsafe` blocks | Static: grep | unsafe-necessity, ffi-pattern |
| `use` statements | Static: grep | unused-import, glob-flag, reorder |
| `///` doc comments | Static: grep | doc-needed, slop-detector, code-vs-english |
| `Result`/`Option` usage | Static: grep | error-type-choice, unwrap-safety, ?-candidate |
| Clippy output | Dynamic: compile | lint-predictor |
| Compile errors | Dynamic: compile | error-fixer |
| Build timings | Dynamic: compile | build-time-estimator |

## Priority Engine: Intent → Model Selection

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────┐
│ Human Input │ ──→ │   Intent     │ ──→ │ Priority     │ ──→ │  sled    │
│ "fix bug"   │     │  Classifier  │     │ Score Update │     │ B-tree   │
└─────────────┘     │ (subatomic)  │     │ hot models↑  │     │ ordered  │
                    └──────────────┘     │ cold models↓ │     │ by score │
                                         └──────────────┘     └──────────┘
                                                                    │
                                                              OS page cache
                                                              handles the rest
```

The intent classifier maps human input to model groups:

| Intent Signal | Model Group Promoted |
|--------------|---------------------|
| "fix", "bug", "error", "crash" | error-fixer, unwrap-safety, lifetime-needed, borrow-checker |
| "add", "new", "create", "struct" | field-count, derive-needed, visibility, doc-needed |
| "test", "coverage", "assert" | test-quality, assertion-style, coverage-gap, fixture-needed |
| "deploy", "release", "build" | build-time, binary-size, complexity-flag |
| "refactor", "clean", "simplify" | complexity-flag, naming-convention, closure-vs-fn |
| "unsafe", "ffi", "extern" | unsafe-necessity, ffi-pattern, raw-pointer-safety |
| "trait", "impl", "dyn" | object-safe, method-count, supertraits, blanket-impl |
| "match", "enum", "pattern" | exhaustive-detector, wildcard-needed, variant-count |
| "async", "await", "spawn" | async-detector, async-closure, capture-mode |
| "import", "use", "dep" | unused-import, missing-import, glob-flag |

One sled tree. One priority score per model. The kernel manages L1→L2→disk promotion via page cache access patterns. The human drives priority. The models are already warm before inference runs.
