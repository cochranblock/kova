# Next Work — kova (saved 2026-05-17)

## Session context
Previous session: all 49 kova tests passing.
This session: wired pyramid into REPL (backlog item 3).

Changes this session:
- `src/repl.rs`: T1 classification now active (not just telemetry)
  - Before inference: `code_vs_english` prints `[input: code/english, 0.94]` in dim gray
  - After inference: `slop_detector` prints `[slop: 0.81 — review output]` in yellow if conf > 0.65
  - Full telemetry bank still stored to sled unchanged

Items 1–3 + 14 (embedded starter) are all done.

## Backlog top items (from BACKLOG.md)

1. **[build] Static carving: syn parse the corpus** — `scripts/carve_static.rs`
   Parse .rs files with `syn`. Extract fn sigs, struct/enum defs, trait impls,
   match arms, error patterns. Generate labeled JSONL per model.

2. **[feature] Sled priority queue** — `src/swarm/priority.rs`
   Key format: `{score}:{model_name}`. Intent classifier updates scores.

3. **[feature] Implement f393 (P23 Triple Lens)** — `kova c2 research <topic>`
   dispatches optimist/pessimist/paranoia to 3 idle panes.

4. **[build] Publish nanosign crate** — Extract from `docs/NANOSIGN.md`,
   `sign()`, `verify()`, `strip()`, `hash()`. Publish to crates.io.

## Warnings to fix (not blocking, dev build only)
- `src/micro/mod.rs`: unexpected cfg `mobile-llm` (4 instances)
- `src/codegen_moe/mesh.rs:143`: irrefutable if-let (parse::<String>())
- `src/codegen_moe/mod.rs:111`: unused variable `mesh`
- `src/inference/local.rs:39`: private_interfaces warning (CachedModel)
