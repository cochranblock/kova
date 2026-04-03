# Backlog

> Prioritized stack. Top item = next work. Max 20 items. Stale items (2+ weeks untouched) get dropped. Re-read on idle.

## Interrupt Protocol

The backlog is the interrupt handler. When a pane gets interrupted mid-task:

1. **New task arrives** → Escape interrupts current work
2. **Auto-push** → Current work description prepended as item #1, tagged `[interrupted]`
3. **New task executes** → Runs to completion
4. **Auto-pop** → When done, pop BACKLOG.md #1 (`[interrupted]` item) and resume where left off

Nothing gets lost. Interrupted work always goes on top of the stack. Context switches are lossless. This is standard for all panes.

## Idle = Work the Stack

When a pane finishes a task and has no new dispatch:

1. **Read** BACKLOG.md
2. **Pop** item #1
3. **Work it** to completion
4. **Loop** — pop next item, keep going
5. **Stop** only when backlog is empty or a new dispatch interrupts

The fleet is never idle. If there's work on the stack, do it. No waiting for orders.

1. [security] **Fix shell injection in c2.rs hive tar-stream sync** — Line 382: `format!("cat {} | ssh {} \"...\"", tar_path, node, extract_dir)` passes user-controlled strings into `sh -c`. Replace with direct `Command::new("ssh").args([...])` — no shell interpolation, args as separate items. Also replace `/tmp/hive-build.tar` with `tempfile::NamedTempFile` to close race condition. P23 paranoia: highest-severity hole in the fleet.

2. [build] **Add pre-push git hook: cargo test gate** — `.git/hooks/pre-push` running `cargo test -p kova --lib`. The f158 lookahead regex was a runtime panic that lived in main for 5 commits because there was no push gate. 2-second test run eliminates the whole class. Also add to `scripts/install-kova.sh` so new clones get it automatically.

3. [build] **Implement nanobyte format** — `src/nanobyte.rs`. Header (64B), manifest, weight region, BLAKE3 signature (36B NanoSign). Load via memmap2. `weights()` returns `&[f32]` at offset. `consolidate()` packs trained models. This is the critical path: items 4, 5, 16 are all blocked on this file existing. Ref: [`docs/KOVA_BLUEPRINT.md`](docs/KOVA_BLUEPRINT.md) section 2.

4. [build] **Pack 3 proven models into first .nanobyte** — Consolidate slop_detector + code_vs_english + lang_detector from `assets/models/` into `starter.nanobyte`. Verify load/infer roundtrip. First real nanobyte file.

5. [feature] **Wire pyramid into REPL** — After nanobyte loads, run subatomic classifiers as preprocessing before main inference. slop_detector flags AI slop in output. code_vs_english classifies input. Intent classifier routes to correct tool.

6. [build] **Static carving: syn parse the corpus** — `scripts/carve_static.rs`. Parse .rs files with `syn` crate. Extract function signatures, struct defs, enum defs, trait impls, match arms, error handling patterns. Generate labeled JSONL per model. Trains: return-type-predictor, field-count, derive-needed, etc.

7. [build] **P13 tokenize swarm module** — Rename public symbols in `src/swarm/train.rs`: Example→t216, SubatomicConfig→t217, featurize→f394, train_starter→f395, predict→f396. Update compression_map.md. Verify build.

8. [feature] **Sled priority queue** — `src/swarm/priority.rs`. Key format: `{score}:{model_name}`. Intent classifier updates scores. OS page cache handles memory hierarchy. Wire into pyramid orchestrator.

9. [feature] **Implement f393 (P23 Triple Lens)** — `kova c2 research <topic>` dispatches optimist/pessimist/paranoia to 3 idle panes, waits, peeks results, dispatches synthesis to 4th. Uses existing f377/f385/f386.

10. [build] **Publish nanosign crate** — Extract NanoSign spec from `docs/NANOSIGN.md` into standalone crate. `sign()`, `verify()`, `strip()`, `hash()`. Publish to crates.io. Ref: [`docs/NANOSIGN.md`](docs/NANOSIGN.md).

11. [feature] **PTY bridge to Claude** — `src/bridge.rs`. Spawn `claude` via portable-pty. Pipe input, stream output, log everything to sled for training data. `KOVA_MODE=bridge|native|hybrid`. Ref: [`docs/KOVA_BLUEPRINT.md`](docs/KOVA_BLUEPRINT.md) section 8.

12. [feature] **Discovery module** — `src/discovery.rs`. Auto-detect local hardware (CPU, RAM, GPU), SSH nodes, model files, ollama instances. Resource map with periodic re-probe. REPL startup banner. Ref: blueprint section 9.

13. [build] **Dynamic carving: clippy on corpus** — Run `cargo clippy --message-format=json` on compilable crates from corpus. Every lint warning = labeled training data for lint-predictor model. Store in `/mnt/data/training/clippy/`.

14. [research] **Train shared/universal models** — Train 6 shared models (visibility, doc-needed, lifetime-needed, naming-convention, complexity-flag, deprecated-pattern) on carved corpus data. These work across all Rust constructs. Ref: [`docs/SUBATOMIC_CATALOG.md`](docs/SUBATOMIC_CATALOG.md).

15. [feature] **Noodle companion model** — Train 30K param personality model on session context. Pack into starter nanobyte. Wire into REPL — print quip after each tool result. First visible proof the pyramid works end-to-end.

16. [build] **Starter nanobyte embedded in binary** — `include_bytes!("../assets/starter.nanobyte")`. 11 models, <2MB. Zero-setup pyramid on `cargo install kova`. Ref: blueprint section 2.

17. [test] **Gauntlet validation for subatomics** — Run gauntlet phases 1-3 with subatomic preprocessing. Measure: does T1 classification improve code gen accuracy? Does slop detection improve output quality?

18. [docs] **Cross-project BACKLOG.md** — Create BACKLOG.md for any-gpu, pixel-forge, cochranblock. Link cross-project dependencies (any-gpu autograd needed for GPU training, pixel-forge sprite data for visual models).

19. [feature] **Molecular layer (T2)** — `src/swarm/molecular.rs`. Routing weight vectors. Train intent_router and tool_selector on Claude bridge logs. Consolidate T1+T2 into combined nanobyte.

20. [research] **Cellular layer feasibility** — Can 5-10M param code gen models run on bt's 5700 XT with 8GB VRAM? Benchmark candle transformer training at that scale. Determine if fleet distribution needed.
