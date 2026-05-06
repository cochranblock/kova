<!-- Unlicense — cochranblock.org -->

# Gap Analysis Sim — Subatomic Pyramid (2026-05-06)

> Research-backed gap analysis on the work shipped in commits `44caf37` and `01d5533`:
> nanobyte format, subatomic inference, embedded starter, REPL telemetry.
> Extends [`PLAN_GAP_ANALYSIS_SIMULATION.md`](PLAN_GAP_ANALYSIS_SIMULATION.md) (2026-03-09)
> with five new sims (Sim 11–15) focused on what's been built since.

**Method.** Each sim's **Expected** is sourced from primary external research (papers, format specs, OpenSSF/NIST guidance) — not from kova's own docs, so it's a real outside-in measurement. **Observed** is the current codebase. **Gap** carries a severity (High / Medium / Low / None). **Recommendation** says fix-now, defer, or accept.

**Severity bar.**

| Tier | Definition |
|------|------------|
| **High** | Blocks downstream consumers, breaks a compliance/security baseline, or invalidates a documented claim. |
| **Medium** | Design gap that caps reach or quality but does not break current users. |
| **Low** | Known/planned item already on BACKLOG in proper sequence. |

---

## Sim 11 — Nanobyte File Format

**Expected (research):**
- *safetensors:* 8-byte LE size + JSON header carrying per-tensor `dtype` / `shape` / `data_offsets` plus optional `__metadata__` strings. F64/F32/F16/BF16/I64/I32/I16/I8/U8/BOOL dtypes.
- *GGUF (llama.cpp):* magic `0x46554747` + version + KV-metadata block (tokenizer, chat template, RoPE, ctx size) + tensor info. Quantization: 1.58-bit → F32 plus block-quantized Q4_0 / Q4_K / Q8_K with embedded scales.
- *ONNX:* protobuf `ModelProto` carrying a full `GraphProto` op DAG (not just weights).

**Observed (`src/nanobyte.rs`):**
- 64-byte header: `NANO` magic + `version: u32` + `num_models: u32` + manifest off/size + total_weights + 28B reserved.
- 80-byte/entry manifest: name (32B) + tier (u8) + num_classes / feature_dim (u32 each) + weights off/size + routing off/size (u64 each).
- f32 weight blob, contiguous, row-major.
- 36-byte NSIG trailer (4B `NSIG` + 32B BLAKE3).

**Gaps:**

| # | Gap | Severity |
|---|-----|----------|
| 11.1 | **No quantization.** F32 only. GGUF ships Q4_K block-quant for the same architectures; ours is 4× bloated already, will be much worse for T2/T3. | **Medium** |
| 11.2 | **No class names in format.** Caller must know them out-of-band (we hardcoded `STARTER_CLASS_NAMES` for the 3 starter models). Any third-party reader cannot recover class semantics. | **High** |
| 11.3 | **No per-tensor dtype.** Implicit f32. Adding f16/bf16/int8 later requires a format bump or a sidecar. | **Medium** |
| 11.4 | **No extensible KV-metadata** (vs GGUF's `kv_pairs`). No place to put tokenizer config, training hyperparams, dataset hash, base model ID. | **Medium** |
| 11.5 | **`version: u32` exists but no forward-compat strategy.** Unknown-version → hard reject. No "skip unknown manifest fields" rule. | **Low** (revisit at V2) |
| 11.6 | **No architecture identifier.** Manifest doesn't say "linear classifier" vs "MLP" vs "transformer". Inference path is hardcoded to trigram-linear. | **Medium** |

**Recommendation.** Land **11.2** in V1.1 (add a `class_names_offset` + UTF-8 string table region between manifest and weights). Defer **11.1 / 11.3 / 11.6** to V2 — block-quantized + dtype-tagged, modeled on GGUF. Document the V2 plan in `docs/NANOBYTE_FORMAT_V2.md` so the deferral is explicit.

---

## Sim 12 — Subatomic Inference Quality

**Expected (research):**
- *FastText (Joulin 2016):* hashing trick, default **2M buckets**, averages word/n-gram embeddings → linear. AG News **92.5%**, DBpedia **98.6%**, Yelp Polarity **95.7%**, Yelp Full **63.9%**. Smallest distributed variant `lid.176.ftz` is **<1MB / ~250K params**.
- *Feature hashing tail bound (Weinberger 2009):* unbiased inner-product preservation; collision rate is the dominant cost.
- *Distillation baselines:* TinyBERT 14.5M params recovers **96–97%** of teacher GLUE accuracy. MiniLM 33M.
- *No peer-reviewed benchmark exists for trigram-hash linear classifiers at <2K params.* This is below any documented baseline.

**Observed (`src/swarm/train.rs` + 3 packed models):**
- 256-dim trigram hash → linear → softmax. **L2-normalized features.**
- slop_detector: 514 params, 89.4% claimed accuracy.
- code_vs_english: 514 params, 94.2% claimed.
- lang_detector: 1,285 params, 97.0% claimed.
- Accuracy figures are training-set self-reports stored in `assets/models/*/config.json::best_accuracy`.

**Gaps:**

| # | Gap | Severity |
|---|-----|----------|
| 12.1 | **256-dim hash → ~98% collision rate** with English trigrams (~17K typical) per Weinberger's tail bound. Production hashing-trick systems use ≥10K–100K buckets. This is a hard quality ceiling for kova subatomics. | **High** |
| 12.2 | **No standard-benchmark validation.** AG News, banking77, DBpedia are public; a 30-line bin could measure kova vs FastText baselines. Without it, "89/94/97%" is unanchored. | **High** |
| 12.3 | **Param count is 100–500× below the smallest published baseline** (FastText `lid.176.ftz` ≈ 250K). Quality ceiling for kova T1 isn't defined; we don't know if 514 params can do anything useful in the steady state. | **Medium** |
| 12.4 | **Train/test split unknown.** `best_accuracy` could be train-set fit, validation, or held-out test — not recorded in the config schema. | **Medium** |
| 12.5 | **No calibration.** GATEKEEPER (2025) and Cascade-Aware Training note small models are *systematically overconfident* — confidence values feed cascade routing decisions, so this matters for T2. | **Medium** (becomes High when T2 lands) |

**Recommendation.** Top priority is **12.1** (bump hash to 4096 or 8192 dims) before training more starter models, since it caps everything downstream. Then **12.2** — add `kova bench classify` that runs all starter models against AG News + DBpedia + a banking-intent subset, prints accuracy table. **12.5** can wait until T2 routing exists.

---

## Sim 13 — NSIG Model Signing

**Expected (research):**
- *Sigstore Model Transparency v1.0* (OpenSSF AI/ML WG, Apr 2025): in-toto attestations, Fulcio short-lived certs, Rekor transparency log. Used by NVIDIA NGC, Cohere uploads on HuggingFace.
- *NIST SP 800-218A* (SSDF profile for generative AI): mandates integrity verification of training data (PW.3); does NOT yet mandate weight signing.
- *EO 14028 §4e* maps to SSDF tasks but specifies neither the signing scheme nor weight-level coverage.
- A bare hash gives **integrity only**: no signer identity, no revocation, no log of who signed when. Hash defends against post-build tampering but **not** poisoned training data, backdoors, or weight-space trojans.

**Observed (`src/nanobyte.rs` + `docs/NANOSIGN.md`):**
- 4-byte `NSIG` magic + 32-byte BLAKE3 of all preceding bytes, appended.
- Reject on hash mismatch. Reject on missing magic.
- No signer identity, no key, no timestamp, no log.
- Same scheme used unchanged by `pixel-forge/src/nanosign.rs`.

**Gaps:**

| # | Gap | Severity |
|---|-----|----------|
| 13.1 | **Authenticity ≠ integrity.** Anyone can re-hash a tampered nanobyte and produce a "valid" trailer. The current spec proves nothing about provenance. | **Medium** (High in production) |
| 13.2 | **No transparency log.** Sigstore + Rekor is now the OpenSSF baseline. Kova ships zero supply-chain attestation. | **Medium** |
| 13.3 | **Hash misframed in docs.** `NANOSIGN.md` reads as a signing standard but it's an integrity standard. Wording risks compliance gap if a federal evaluator reads it as authenticity. | **High** (correct the framing now — costs nothing) |
| 13.4 | **No threat model for what NSIG defends against.** Doc lists what it does mechanically but not what attacks it stops vs accepts. | **Medium** |

**Recommendation.** **13.3** is a 5-minute doc fix — relabel "AI Model Signing" → "AI Model Integrity Hash"; explicitly say "does not provide authenticity, signer identity, or transparency log." **13.1 / 13.2** are real but Phase-2: NSIG-V2 = NSIG + optional Ed25519 detached signature + optional Rekor log entry hash, all backward-compatible. Tracks BACKLOG #8 (publish `nanosign` crate) — do the rename and threat-model write-up *before* the crate publish so we don't ship a misframed standard to crates.io.

---

## Sim 14 — Tiered / Cascade Routing (Pyramid)

**Expected (research):**
- *BranchyNet (Teerapittayanon ICPR 2016):* side-branch classifiers exit early when softmax entropy < τ. Direct structural precedent.
- *Switch Transformer (Fedus 2021):* top-1 routing over experts, **parallel** (constant FLOPs at huge param count). Different shape than kova's sequential tiers.
- *FrugalGPT / GATEKEEPER / Cascade-Aware Training (2024–25):* formalized confidence-thresholded tiny→big routing. **Key finding: small models are systematically overconfident; raw softmax is unsafe as a routing signal without temperature scaling or a learned proxy.**
- *Viola-Jones (2001):* the OG cascade — sequential weak classifiers with rejection thresholds.

**Observed:**
- T1 (Subatomic): 3 of 11 starter / 66 catalog models trained. Inference path verified. Embedded via `include_bytes!`.
- T2 (Molecular): not implemented. BACKLOG #17.
- T3 (Cellular): not implemented. BACKLOG #18.
- No cascade-routing logic — `nanobyte::infer` returns confidence but nothing reads it to decide tier escalation.
- REPL classifies every input/response with all 3 T1 models in parallel (BACKLOG #3 silent telemetry first pass) — but this is "run all" not "cascade."

**Gaps:**

| # | Gap | Severity |
|---|-----|----------|
| 14.1 | **T2 / T3 absent.** Documented as planned (BACKLOG #17, #18). | **Low** (planned) |
| 14.2 | **No confidence calibration plan.** GATEKEEPER's overconfidence finding implies kova's confidence values shouldn't be used as routing signals as-is. Need temperature scaling or a learned routing model. Not on BACKLOG. | **Medium** |
| 14.3 | **No cascade orchestrator.** Even with T2/T3 trained, nothing wires confidence-threshold escalation. Architecture doc gestures at this but doesn't specify the orchestrator API. | **Medium** |
| 14.4 | **REPL runs all 3 T1 in parallel (`classify_with_starters`).** Cheap today, but scales linearly with starter count. At 11 models this is fine; at 66 it's wasteful when only 1–3 are relevant per input. Intent classifier (catalog §3) should gate which T1s run. | **Medium** |

**Recommendation.** Add a new BACKLOG item: **"Confidence calibration for T1 starters — temperature scaling on a held-out set; persist calibrated softmax temp in nanobyte manifest"** (slot before BACKLOG #17 since T2 routing decisions depend on it). Defer the cascade orchestrator design until T2 has at least one trained model.

---

## Sim 15 — `pyramid` Naming Collision

**Expected:** One module per concept. Cross-doc consistency.

**Observed:**
- `src/pyramid/` is a hierarchical **code-gen MoE** with `Router` decomposes task → `Assembler` per-subtask → `Expert` (Claude API calls today) → `SpongeMesh` retry. Inspired by DeepSeek-MoE / THOR-MoE / Expert Choice. Actively shipping.
- `docs/PYRAMID_ARCHITECTURE.md` documents a *different* pyramid: T1 Subatomic / T2 Molecular / T3 Cellular nanobyte tiers. This is what we just shipped phase 1 of. Lives in `src/swarm/` + `src/nanobyte.rs`.
- `src/swarm/mod.rs` doc says: *"Subatomic model training and inference."* No mention of pyramid.
- `KOVA_BLUEPRINT.md` §2 Nanobyte Format describes the subatomic-pyramid file we built.

**Gaps:**

| # | Gap | Severity |
|---|-----|----------|
| 15.1 | **Two distinct architectures both called "pyramid"** in code, docs, and conversation. Onboarding hazard. | **Medium** |
| 15.2 | **PYRAMID_ARCHITECTURE.md describes the subatomic pyramid but the matching code is in `src/swarm/`, not `src/pyramid/`.** Doc-to-code mapping is wrong. | **Medium** |

**Recommendation.** Two-step rename, no behavior change:

1. `src/pyramid/` → `src/codegen_moe/` (the Router/Assembler/Expert MoE). Update `pub mod pyramid` → `pub mod codegen_moe` in `lib.rs`. Update docs that reference `src/pyramid`.
2. Keep `pyramid` as the umbrella term for **subatomic T1/T2/T3** going forward — that matches `PYRAMID_ARCHITECTURE.md` and `KOVA_BLUEPRINT.md`. Long-term, the `codegen_moe` experts become T3 cellular models in the unified pyramid (per blueprint Phase 4).

This rename is reversible, clarifies onboarding, and aligns code with the strategic doc.

---

## Aggregated Gap Table — by severity

### High (fix soon)

| ID | Gap | Effort |
|----|-----|--------|
| **11.2** | No class names in nanobyte format | M (V1.1 format bump) |
| **12.1** | 256-dim hash → ~98% trigram collision | S (constant change + retrain 3 models) |
| **12.2** | No standard-benchmark validation | M (`kova bench classify` bin + datasets) |
| **13.3** | NSIG misframed as "signing" — it's integrity | XS (doc edit) |

### Medium (plan + sequence)

| ID | Gap | Effort |
|----|-----|--------|
| 11.1 | No quantization | L (V2 format) |
| 11.3 | No per-tensor dtype | L (V2 format) |
| 11.4 | No KV-metadata block | M (V2 format) |
| 11.6 | No architecture identifier | S (V2 format) |
| 12.3 | Param count below FastText baseline | M (architecture review) |
| 12.4 | Train/test split unrecorded | S (config schema) |
| 12.5 | No confidence calibration | M (held-out set + temp scaling) |
| 13.1 | NSIG = integrity, not authenticity | L (Ed25519 detached sig in V2) |
| 13.2 | No Rekor / transparency log | L (Sigstore integration) |
| 13.4 | No threat model written | S (doc) |
| 14.2 | No calibration plan for cascade | S (BACKLOG addition) |
| 14.3 | No cascade orchestrator | M (defer to post-T2) |
| 14.4 | `classify_with_starters` runs all T1s | M (intent gate from catalog §3) |
| 15.1 | "pyramid" name collision | XS (rename) |
| 15.2 | Doc-to-code mismatch | XS (rename + doc edit) |

### Low (already on BACKLOG in correct sequence)

| ID | Gap | BACKLOG # |
|----|-----|-----------|
| 11.5 | Format forward-compat | revisit at V2 |
| 14.1 | T2 / T3 not built | #17 / #18 |

---

## Open questions the research couldn't answer

(Empirical work needed on our side — not external reading.)

1. What is kova's **measured** accuracy on a standard benchmark (AG News / banking77 / DBpedia)?
2. What is the **empirical** trigram-hash collision rate at 256 dims vs the actual training corpus?
3. Is mmap-loading **actually faster** than safetensors-loading for kova's sizes? (The README claims it; never measured.)
4. Are softmax confidences from kova's T1 models **calibrated**, or systematically overconfident as the cascade literature predicts?

---

## Recommended next moves (proposed sequence)

| Order | Item | Severity | Why first |
|-------|------|----------|-----------|
| 1 | **13.3** — relabel NSIG as integrity-hash, write threat model | High | 5-min doc edit; blocks BACKLOG #8 (`nanosign` crate publish) from shipping a misframed standard. |
| 2 | **15.1 + 15.2** — rename `src/pyramid/` → `src/codegen_moe/` | Medium | XS effort. Removes onboarding hazard before more code lands. |
| 3 | **12.2** — `kova bench classify` against AG News / banking77 | High | Anchors every other quality conversation in a real number. Answers open question #1. |
| 4 | **12.1** — bump hash dim to 8192, retrain 3 starter models | High | Caps quality ceiling for everything downstream. Cheap retrain (~30s on bt). |
| 5 | **11.2** — V1.1 format with class-name string table | High | Unblocks third-party readers and cleans up the `STARTER_CLASS_NAMES` const. |
| 6 | **14.2** — add BACKLOG entry for confidence calibration | Medium | Pre-requisite for T2 routing; one-line BACKLOG addition. |
| 7 | Then: continue toward BACKLOG #4 (static carving) → catalog model expansion. | — | This unblocks training the other 60+ subatomics. |

---

*Sims 1–10: see [`PLAN_GAP_ANALYSIS_SIMULATION.md`](PLAN_GAP_ANALYSIS_SIMULATION.md) (2026-03-09). This document extends with Sim 11–15.*
