# Kova Intent Recalibration

**Purpose:** Map every kova-related Cursor prompt and stated intent to what was actually done. Recalibrate so future work targets your real goals.

**Date:** 2026-03-09

---

## 1. Stated Intents (from prompts, docs, transcripts)

### From CURSOR_PROMPTS_FROM_OTHER_WINDOWS.md (Kova section)

| Prompt | Intent |
|--------|--------|
| "You are Kova. Execute my intent." | Kova executes; human directs. No endless clarification. |
| "Assume I want you to do more work than you think I want you to want to do." | Bias toward doing more, not less. |
| "Make functional products, don't mock stuff." | Real implementations, not stubs. |
| **ROLE:** Senior Systems Architect. Human directs; you execute. Build fast, ship fast. | Execute, don't over-clarify. |

### From USER_STORY_PERFECT_RUST.md

| Vision | Intent |
|--------|--------|
| Understand → Plan → Validate → Refine → Print | Full pipeline: parse intent, generate, validate (cargo check/test/clippy), fix loop, output. |
| "Perfect" Rust | Compiles, tests pass, clippy clean, matches project style (compression_map, fN/tN/sN). |
| Senior engineer success | "I said 'add a retry with exponential backoff' and got code that compiles and fits our style." |
| Architect success | "New engineers get correct patterns because Kova uses our prompts." |

### From TRIPLE_SIMS_KOVA.md (evaluation vs intent)

| Gap | Severity | Intent |
|-----|----------|--------|
| GUI CodeGen omits cursor_prompts | High | Architect: conventions (compression_map, .cursor/rules) must reach Coder in GUI. |
| Serve: no diff, Apply, Copy | Medium | Senior engineer: diff before apply, one-click copy. |
| Serve: no backlog add | Low | "Run later" → backlog from serve. |

### From PLAN_GUI_CURSOR_PROMPTS.md

- **Goal:** Align GUI with serve — inject Cursor rules into all Coder paths (CodeGen, use_coder, direct send).

### From agent-intent-prompt.txt (scripts/)

- **Scope:** ronin-sites only. Non-breaking changes. Not kova.

---

## 2. What "run it and iterate" actually did

| Action | Result |
|--------|--------|
| Install Rust (macOS arm64) | ✓ stable-aarch64-apple-darwin |
| Fix borrow checker (gui.rs) | ✓ |
| Fix clippy (config, cursor_prompts, gui, pipeline, main, kova-c2) | ✓ |
| Run kova-test | ✓ All passed |

**kova-test runs:** clippy, cargo test 3×, release build, bootstrap smoke, kova-c2 nodes smoke, serve smoke.

---

## 3. Alignment check

| Intent | Addressed? | Notes |
|--------|------------|-------|
| Execute my intent | ✓ | kova-test runs; pipeline, serve, kova-c2 work. |
| Build fast, ship fast | ✓ | Build succeeds; no blocking issues. |
| Perfect Rust (compiles, tests, clippy) | ✓ | kova-test enforces clippy + cargo test. |
| Conventions in Coder (cursor_prompts) | ✓ | GUI uses `build_system_prompt()` → `load_cursor_prompts(&project)`. CodeGen and use_coder paths both use it. |
| TRIPLE_SIMS Gap #1 (GUI cursor_prompts) | ✓ | Fixed in PLAN_GUI_CURSOR_PROMPTS; implemented. |
| TRIPLE_SIMS Gap #2 (Serve diff/Apply/Copy) | ✗ | Not addressed. kova-test does not validate this. |
| TRIPLE_SIMS Gap #3 (Serve backlog) | ✗ | Not addressed. |
| "Assume I want more work" | ? | Subagent fixed what was broken; did not proactively expand scope. |
| "Make functional products, don't mock" | ✓ | kova-test runs real flows (bootstrap, serve, kova-c2). |

---

## 4. Potential misalignment

| Issue | Explanation |
|-------|-------------|
| **Iterate vs. pass** | "run it and use it to iterate" could mean: use test results to fix gaps, not just make tests pass. The subagent fixed build/compile errors; it did not address TRIPLE_SIMS Gaps #2–3 or re-run the AI-assisted eval (Sim 1–3). |
| **Intent scope** | "Execute my intent" in Kova context is about the *human* directing Kova; the AI agent (Cursor) was asked to "run it and iterate" on kova. The agent ran kova-test and fixed build issues. That aligns with "execute" but not necessarily with "iterate on gaps from TRIPLE_SIMS." |
| **kova-test vs. TRIPLE_SIMS** | kova-test is deterministic (clippy, test, smoke). TRIPLE_SIMS is an AI-assisted evaluation (Sim 1–3) that walks through USER_STORY_PERFECT_RUST and checks Expected/Observed, Gaps. The "iterate" loop may have been intended to include that. |

---

## 5. Intent sequence (clarified)

**Order:** A → B → C (sequential, not parallel)

| Step | Intent | Status |
|------|--------|--------|
| A | Run kova-test, fix failures, get it green | ✓ Done |
| B | Use kova-test to drive closing TRIPLE_SIMS gaps (Gaps #2–3), re-evaluate | Next |
| C | Run AI-assisted eval (Sim 1–3), update Expected/Observed, iterate | After B |

## 6. Recommendations

1. **B:** Close Gaps #2 (serve diff/Apply/Copy), #3 (serve backlog add). Re-run kova-test after each.
2. **C:** Run "EVALUATE KOVA AGAINST ORIGINAL INTENT WITH TRIPLE SIMS" — walk Sim 1–3, update tables, propose fixes.
3. **Recalibration:** Keep this file. Update when intent or priorities change.

---

## 7. Source locations

| Source | Path |
|--------|------|
| Kova prompts (extracted) | `.archive/docs/archive/CURSOR_PROMPTS_FROM_OTHER_WINDOWS.md` |
| User story (Perfect Rust) | `kova/docs/USER_STORY_PERFECT_RUST.md` |
| Triple Sims eval | `kova/docs/TRIPLE_SIMS_KOVA.md` |
| GUI plan | `kova/docs/PLAN_GUI_CURSOR_PROMPTS.md` |
| Agent intent (ronin-sites) | `scripts/agent-intent-prompt.txt` |
