# User Story Analysis — Kova 0.7.x
<!-- Supersedes 2026-05-17 analysis. Date: 2026-05-26. Evaluator: Claude Sonnet 4.6. -->

---

## Phase 1 — Repository Topology

**Single crate** (`kova-engine` v0.7.0), 103 Rust source files, ~41,629 lines. One `<10 MB` binary. No workspace peers — Android/iOS/edge are separate `Cargo.toml` targets, not workspace members.

Key source modules:
| Module | Lines | Purpose |
|--------|-------|---------|
| `src/tools.rs` | 2,017 | 13 agent tools; dispatch `f141`; perm gate |
| `src/tui.rs` | 1,672 | Ratatui terminal UI (chat + visual QC) |
| `src/c2.rs` | 1,309 | IRONHIVE C2 swarm — build/sync/tmux/WoL |
| `src/serve.rs` | 1,351 | Axum HTTP + WebSocket + embedded WASM |
| `src/config.rs` | 791 | Config, paths, feature detection |
| `src/rag.rs` | 749 | fastembed vectors, sled index |
| `src/main.rs` | 3,211 | Full CLI (all subcommands via clap) |
| `src/mcp.rs` | 508 | JSON-RPC stdio MCP server |
| `src/agent_loop.rs` | — | `f147` single turn; `f148` agent loop |
| `src/nanobyte.rs` | — | BLAKE3-signed model format; baked T1 classifiers |
| `src/swarm/priority.rs` | — | redb priority queue `f430–f434` |
| `src/swarm/t2.rs` | — | T2 molecular coordinator `f410/f411` |
| `src/bridge.rs` | — | PTY proxy `f403` (logs to tele/ for retraining) |
| `src/daemon.rs` | — | **STUB** — Cap'n Proto schema loaded, listener deferred |

Binaries: `kova` (main), `kova-test` (TRIPLE SIMS gate), `pack-starter`, `train-intent`, `bench-classify`, `retrain-starters`, `carve-static`.

Features: `default=[tui]`, `serve`, `inference`, `rag`, `autopilot`, `browser`, `tests`, `carve`, plus 20 exopack test features.

---

## Phase 2 — Ecosystem Map

CochranBlock portfolio. Every repo is Unlicense, single-binary, zero-cloud.

| Repo | Version | Role | kova relationship |
|------|---------|------|-------------------|
| `cochranblock` | 1.0.3 | Zero-cloud website binary (Axum, 13 MB) | kova generates/reviews its code; `kova rag` indexes it |
| `approuter` | 0.2.0 | Reverse proxy + Cloudflare tunnel registration | kova deploys it via `kova c2 build`; `approuter-client` wires apps in |
| `whobelooking` | 0.3.0 | Federal API scout (surveillance detector) | kova builds/reviews it; part of SDVOSB product suite |
| `cochranblock-mail` | 0.1.0 | Email system (server + shared + frontend) | kova generates its code; monitoring target |
| `any-gpu` | 0.7.1 | wgpu tensor engine (AMD/NVIDIA/Intel/Apple) | kova trains subatomic models via any-gpu Vulkan compute |
| `r8r` | 0.1.0 | Rust port of n8n (585 nodes, 24 working) | workflow orchestration layer that can invoke kova |
| `exopack` | 0.3.0 | Test augmentation (TRIPLE SIMS, screenshot, mock) | absorbed into kova as feature flags; `kova-test` binary |
| `nanobyte` | standalone | Packed model format crate | kova's `starter.nanobyte` is the reference implementation |
| `header-writer` | — | Unlicense + Contributors header injection | runs post-AI on cochranblock/approuter/oakilydokily |
| `pixel-forge` | defunct | Pixel art sprite generation | referenced in `pixel_forge` tool but project is dead |

Cross-repo data flows:
- `kova bridge` logs Claude Code PTY sessions → `~/.kova/tele/` → `kova export tele` → retrain intent classifier
- `kova c2 build --broadcast` → rsync + cargo on n0/n1/n2/n3 → ships cochranblock/approuter binaries
- `any-gpu` Vulkan compute → trains subatomic models → `pack-starter` packs into `starter.nanobyte` → `include_bytes!` baked into kova binary
- `kova mcp` JSON-RPC stdio → Claude Desktop tool calls → kova executes tools against any project

---

## Phase 3 — Personas

### P1 — Solo Developer / Daily Pilot
**Who:** Michael Cochran. Rust expert. Runs kova on IRONHIVE daily as a coding assistant.
**Context:** Has GGUF models configured, knows every subcommand, wants the system to get out of the way.
**Level:** Expert Rust, expert kova.
**Goal:** Ship features faster with AI in the loop; maintain IRONHIVE fleet without manual SSH.

### P2 — Fleet Operator
**Who:** Same user, different mode. Manages 5 worker nodes (lf/gd/bt/st/mm).
**Context:** Needs coordinated builds, broadcast deploys, node health at a glance.
**Level:** Expert SSH/tmux/infra.
**Goal:** Zero-friction fleet ops; never SSH manually when kova can do it.

### P3 — ML Engineer / Model Trainer
**Who:** Wears the training hat. Runs corpus carving, retrains starters, packs nanobytes.
**Context:** 240K crate corpus on `/mnt/data/crates/`. GPU is bt's RX 5700 XT (8 GB VRAM). Wants to close the training flywheel.
**Level:** Expert ML, comfortable with candle/wgpu internals.
**Goal:** Working pyramid — T1 routes correctly, T2 cuts inference cost, T3 replaces API calls.

### P4 — First-Time External Evaluator
**Who:** Rust developer who heard about kova. Downloads the binary. Has no GGUF, no nodes, no config.
**Context:** Wants to try it in 5 minutes. Currently hits a wall if ANTHROPIC_API_KEY is absent.
**Level:** Intermediate Rust dev. Zero kova knowledge.
**Goal:** Evaluate whether kova is useful before investing setup time.

### P5 — Federal Contractor / Government Procurement Officer
**Who:** GovCon evaluating SDVOSB-sourced tooling. CAGE 1CQ66 supplier.
**Context:** Needs compliance docs, supply chain transparency, air-gap capability.
**Level:** Non-technical to moderate. Cares about CMMC/FedRAMP/ITAR/SBOM.
**Goal:** Verify kova meets procurement requirements without reading source code.

### P6 — Open Source Contributor / Future Maintainer
**Who:** External Rust developer who wants to contribute or fork.
**Context:** Reads CONTRIBUTING.md, explores the codebase, wants to add a feature or fix a bug.
**Level:** Intermediate to expert Rust.
**Goal:** Understand the codebase architecture, submit a correct PR, not break the TRIPLE SIMS gate.

### P7 — Cross-Repo App Developer
**Who:** Developer building one of the CochranBlock apps (cochranblock, approuter, whobelooking).
**Context:** Uses kova as the AI brain: code review, code gen, fleet deploys, MCP tool access.
**Level:** Expert Rust, familiar with kova's API surface.
**Goal:** Integrate kova's capabilities into a product without fighting the tool.

### P8 — CI/CD Automation
**Who:** The `kova test` / TRIPLE SIMS gate itself. Automated process, not a human.
**Context:** Runs on every potential deploy. 12-core bt node. Must complete in under 20 minutes.
**Level:** N/A (automated).
**Goal:** Block bad deploys. Catch regressions before they reach production.

### P9 — SDVOSB / Compliance Auditor
**Who:** Independent auditor reviewing kova for federal contract eligibility.
**Context:** Checks CAGE/UEI, SBOM, CMMC posture, ITAR classification, air-gap capability.
**Level:** Non-technical. Uses `kova govdocs` and reads generated docs.
**Goal:** Produce a compliance memo that confirms or denies procurement eligibility.

### P10 — Adversary
**Who:** Nation-state APT, disgruntled insider, opportunistic script kiddie — three sub-types.
**Context:** kova has execution authority over the entire fleet. A compromised kova is a fleet-wide worm.
**Level:** Nation-state = expert. Insider = knows the codebase. Script kiddie = automated tools.
**Goal:** Escalate privileges, exfiltrate data, or degrade the training flywheel.

---

## Phase 4 + 5 — User Stories and Acceptance Criteria

Format: story, then three ACs in `Given / When / Then` form with specific metrics.

---

### P1 — Solo Developer

**P1-01** As a solo developer, I want the REPL to respond to my prompt within 2 seconds of hitting Enter (remote inference, sub-50-token response) so that the interactive loop feels as fast as a shell.
- **AC1:** Given `KOVA_INFERENCE=remote` and Anthropic API is reachable, when I submit a short prompt, then first token streams to stdout within 2 s.
- **AC2:** Given a slow network, when TTFT exceeds 5 s, then kova prints `[inference: slow — 5.2s]` in dim gray so I know the delay is external.
- **AC3:** Given any inference error, when it occurs, then exit code is non-zero and stderr contains the HTTP status or `connection refused`; stdout is not polluted.

**P1-02** As a solo developer, I want compile errors automatically fixed via the Sponge Mesh retry loop so that I don't have to re-type the build command after every failure.
- **AC1:** Given a Rust compile error in `src/`, when the agent loop's fix loop (`src/factory.rs` `T181`) triggers, then it re-runs `cargo build`, captures stderr, generates a patch, and applies it without user input.
- **AC2:** Given the fix succeeds on the second attempt, when the loop exits, then stdout shows `fix: attempt 2 — ok` and the final file on disk compiles clean.
- **AC3:** Given the fix fails after `orchestration_max_fix_retries` attempts (configurable via `config.rs`), then the loop exits with a non-zero code and prints the final compiler error so I can fix it manually.

**P1-03** As a solo developer, I want the REPL to classify my input as code vs. English before sending it to inference so that I can see the routing decision and trust the system.
- **AC1:** Given the `code_vs_english` T1 classifier (accuracy 97.1%, baked in `STARTER_NANOBYTE`), when I type a Rust snippet, then the REPL prints `[input: code, 0.97]` in dim gray before the inference call — inference latency is unaffected.
- **AC2:** Given I type plain English, then the label reads `[input: english, 0.94]`.
- **AC3:** Given the classifier confidence is below 0.55, then no label is printed (low-confidence outputs are silently dropped, not printed as noise).

**P1-04** As a solo developer, I want the slop detector to flag AI output that looks like generated filler so that I review it before committing.
- **AC1:** Given `slop_detector` confidence > 0.65 on the response, when the agent loop returns, then kova prints `[slop: 0.81 — review output]` in yellow to stderr.
- **AC2:** Given slop confidence ≤ 0.65, then nothing is printed (no false-positive noise for good output).
- **AC3:** Given I set `KOVA_SLOP_THRESHOLD=0.9` in env, then the threshold is respected and yellow warnings appear only above 0.9.

**P1-05** As a solo developer, I want `kova traces recent -n 20` to show my last 20 LLM calls with latency, token counts, and model name so that I can audit my inference usage.
- **AC1:** Given at least one completed inference call logged to redb `tele/` namespace (`f161` in `src/trace.rs`), when I run `kova traces recent`, then output contains rows with columns: timestamp, model, latency_ms, prompt_tokens, completion_tokens.
- **AC2:** Given no traces exist yet, when I run `kova traces stats`, then output prints `no traces recorded yet` and exits 0.
- **AC3:** Given two calls in the same millisecond with identical prompt size and latency, when I read the traces, then both appear as distinct rows (the `discriminant` suffix in `f161` prevents key collision).

**P1-06** As a solo developer, I want `undo_edit` to restore any file I asked the agent to modify so that I can recover from a bad AI edit in one command.
- **AC1:** Given the agent called `write_file` or `edit_file` on `src/foo.rs`, when I call `undo_edit` with the same path, then the file on disk matches its pre-edit content byte-for-byte (checkpoint stored in redb via `f383`).
- **AC2:** Given no checkpoint exists for the path, when I call `undo_edit`, then stderr prints `no checkpoint for src/foo.rs` and exit code is 1.
- **AC3:** Given I call undo twice on the same file, then the second call returns the same checkpoint (idempotent — checkpoint is not consumed on first undo).

**P1-07** As a solo developer, I want context compaction to kick in automatically when the conversation hits 80% of the context budget so that long sessions don't fail with a context-exceeded error.
- **AC1:** Given conversation token count reaches 80% of `ctx` (`f380` in `src/context_mgr.rs`), when the next turn fires, then the 4 most recent turns are preserved verbatim and older turns are summarized via LLM before being discarded.
- **AC2:** Given compaction fires, when it completes, then the reduced context is still valid JSON (`[{role, content}]` array) and the next inference call succeeds.
- **AC3:** Given the inference call for summarization fails (network down), then kova falls back to static trim (`f171`) — hard-cutting oldest turns — rather than crashing.

**P1-08** As a solo developer, I want `kova rag search "async trait object lifetime"` to return the 5 most semantically relevant code chunks from my project so that the agent has accurate context for the query.
- **AC1:** Given `kova rag index .` was run and `.rs` files are indexed via fastembed, when I search, then output lists 5 results with file path, line range, and similarity score ≥ 0.0.
- **AC2:** Given the index is empty or stale, when I search, then output prints `index empty — run kova rag index first` and exits 1.
- **AC3:** Given a query that matches no file above 0.3 similarity, when results are returned, then each result's score is shown so I can judge relevance.

**P1-09** As a solo developer, I want `kova tui` to show me the agent chat on the left and a visual QC diff panel on the right so that I can review generated code without switching windows.
- **AC1:** Given `--features tui` is compiled in, when I run `kova tui`, then ratatui renders a two-pane layout: left = conversation, right = last file diff or output.
- **AC2:** Given the terminal is narrower than 80 columns, when tui starts, then it gracefully falls back to single-pane chat without crashing.
- **AC3:** Given I press Ctrl+C, when the signal is received, then the terminal is restored to normal mode (no leftover raw-mode artifacts) and exit code is 0.

**P1-10** As a solo developer, I want `kova git g1` (tokenized `git diff`) to compress the output so that I can paste it into an LLM prompt without wasting tokens.
- **AC1:** Given a dirty working tree, when I run `kova git g1`, then output is the git diff with unchanged function bodies replaced by `[+N/-M lines]` markers (compression from `src/git_cmd.rs`).
- **AC2:** Given no changes, when I run `kova git g0` (status), then output is `nothing to commit` and exit code is 0.
- **AC3:** Given `--expand` flag, when I run `kova git g1 --expand`, then rN tokens in output are replaced with human-readable names before printing.

**P1-11** As a solo developer, I want `kova x x1` (tokenized `cargo check`) to run on the current project and stream errors in real time so that I get feedback before committing.
- **AC1:** Given a valid `Cargo.toml` in cwd, when I run `kova x x1`, then `cargo check` runs and stderr is streamed line-by-line to the terminal.
- **AC2:** Given `--chain x1,x3` flag, when x1 passes, then x3 (clippy) runs automatically; if x1 fails, x3 is skipped.
- **AC3:** Given `--project p2` flag (cochranblock token), when I run it, then the command executes in the cochranblock directory resolved by `config.rs`.

**P1-12** As a solo developer, I want `kova review staged` to run LLM code review against my staged changes and print severity-scored findings before I push so that bad code doesn't reach the repo.
- **AC1:** Given staged changes via `git diff --staged`, when `kova review staged` runs, then output contains findings grouped by severity: CRITICAL, HIGH, MEDIUM, LOW.
- **AC2:** Given no staged changes, when the command runs, then it prints `no staged changes` and exits 0.
- **AC3:** Given findings include a CRITICAL item, then exit code is non-zero so CI can block on it.

**P1-13** As a solo developer, I want `KOVA_PERMS=guarded` to prompt me before executing shell commands or git mutations so that the agent can't silently delete files or force-push.
- **AC1:** Given `KOVA_PERMS=guarded`, when the agent calls `exec` with any command, then kova prints `[perm] exec: rm -rf foo — allow? [y/N]` and waits for input before executing.
- **AC2:** Given I type `N`, when the prompt is rejected, then the tool returns `"permission denied"` to the agent and the command is not executed.
- **AC3:** Given `KOVA_PERMS=open` (default), then no prompts appear and all tools execute immediately.

**P1-14** As a solo developer, I want `kova mcp` to expose all 13 tools to Claude Desktop via JSON-RPC stdio so that I can use kova's filesystem tools from within Claude.
- **AC1:** Given `kova mcp` is configured as an MCP server in Claude Desktop, when Claude calls `read_file` with a path, then kova reads the file and returns contents as a JSON-RPC response within 500 ms.
- **AC2:** Given an invalid tool name in the JSON-RPC request, when the request arrives, then kova returns a JSON-RPC error with code `-32601` (`method not found`).
- **AC3:** Given `kova mcp --project /path/to/project`, when file tools resolve relative paths, then they resolve relative to `/path/to/project`, not cwd.

**P1-15** As a solo developer, I want `kova factory "add a rate limiter to the HTTP handler"` to run the full classify→generate→compile→review→fix pipeline so that I get a working Rust implementation without manually iterating.
- **AC1:** Given the factory command (requires `--features inference`), when it runs to completion, then the generated code compiles clean via `cargo check` and the fix loop (`T181`) ran at most `retries` times.
- **AC2:** Given `--no-review` flag, then the code review stage is skipped and the file is written after the first passing compile.
- **AC3:** Given the fix loop exhausts all retries, then exit code is non-zero, the final failing compiler error is printed, and no partial file is left on disk.

**P1-16** As a solo developer, I want `kova moe "implement a retry policy" --experts 3` to fan out to 3 expert variants, compile all, and return the one with the best review score so that I get the highest-quality implementation.
- **AC1:** Given 3 experts requested, when the MoE runs, then exactly 3 variants are generated, each compiled independently, and the winner is selected by `f341` in `src/moe.rs` based on review score.
- **AC2:** Given all 3 variants fail to compile, then exit code is non-zero and a summary of all 3 errors is printed.
- **AC3:** Given `--save` flag, then the winning variant is written to `~/.kova/experts/<prompt-hash>.rs`.

**P1-17** As a solo developer, I want `kova squeeze` to analyze my shell history and suggest kova aliases for commands I type repeatedly so that I can compress my workflow further.
- **AC1:** Given `~/.zsh_history` or `~/.bash_history` exists, when `kova squeeze` runs, then output shows up to 20 command patterns with suggested aliases and frequency counts.
- **AC2:** Given `--apply` flag, then suggested aliases are appended to `~/.kova-aliases` and a summary of added aliases is printed.
- **AC3:** Given `--remote` flag, when remote node histories are scanned via SSH, then patterns from all reachable nodes are merged before ranking.

**P1-18** As a solo developer, I want the REPL banner to show which T1 classifiers are loaded and which inference backend is active so that I know the system state at a glance without running a separate status command.
- **AC1:** Given kova starts (any inference mode), when the REPL banner prints, then it includes: binary version, inference backend (`local:model-name` or `remote:claude-sonnet-4-6`), and T1 classifiers loaded from `STARTER_NANOBYTE`.
- **AC2:** Given `KOVA_INFERENCE=local` but no model file is found, then the banner prints `[inference: local — no model found, falling back to remote]`.
- **AC3:** Given `STARTER_NANOBYTE` BLAKE3 signature fails verification, then startup aborts with `fatal: starter.nanobyte signature invalid` before the REPL opens.

**P1-19** As a solo developer, I want `kova recent -m 30` to show me a tokenized diff of every file changed in the last 30 minutes so that I can quickly reconstruct what the AI did in my last session.
- **AC1:** Given files modified in the last 30 minutes in cwd, when `kova recent` runs, then output lists each file with a compressed diff (same compression as `kova git g1`).
- **AC2:** Given no files changed, when the command runs, then output is `no changes in the last 30 minutes` and exits 0.
- **AC3:** Given `--minutes 5` flag, then only files changed in the last 5 minutes appear.

**P1-20** As a solo developer, I want `kova bridge` to spawn Claude Code CLI in a PTY and log every session to redb `tele/` so that those interactions feed back into my T1 training data.
- **AC1:** Given `kova bridge` runs (`f403` in `src/bridge.rs`), when I interact with Claude Code normally, then terminal behavior is identical to running `claude` directly (raw mode, correct window size propagation).
- **AC2:** Given the session ends, when I run `kova export tele`, then a JSONL file appears at `~/.kova/training_data/tele.jsonl` containing (timestamp, input, output) pairs from the session.
- **AC3:** Given Claude Code crashes mid-session, then kova bridge cleans up the PTY (`RawMode` drop impl) and exits without leaving the terminal in raw mode.

**P1-21** As a solo developer, I want `kova academy "add pagination to the API"` to autonomously break the task into steps, generate code, test it, and commit so that I can hand off a well-scoped feature and review the result.
- **AC1:** Given a task description, when academy (`f301` in `src/academy.rs`) runs, then it produces a plan, generates code per step, runs `cargo test` after each step, and commits with a message referencing the task.
- **AC2:** Given `--dry-run` flag, then the plan is printed but no files are written or committed.
- **AC3:** Given `--no-commit` flag, then code is generated and tests pass but no git commit is made.

**P1-22** As a solo developer, I want `kova tokens` to confirm 100% tokenization coverage so that I know no public symbol slipped through the compression protocol.
- **AC1:** Given the codebase is built, when `kova tokens` runs, then output shows `tokenization: 100.0% (N/N)` where N matches the symbol count in `src/tokenization.rs`.
- **AC2:** Given a new public function `f999` is added but not registered in `compression_map.md`, then `kova tokens` prints `MISSING: f999` and exits non-zero.
- **AC3:** Given all symbols are covered, then exit code is 0 and the output is a single summary line.

**P1-23** As a solo developer, I want `kova c2 research "should I use sled or redb for the priority queue"` to run the P23 triple-lens analysis (optimist / pessimist / paranoia) and return a structured synthesis so that I get adversarial coverage before an architectural decision.
- **AC1:** Given a topic string, when `f393` in `src/c2.rs` runs, then three tmux panes receive the optimist/pessimist/paranoia prompts in parallel, and a synthesis prompt is dispatched to a fourth pane after all three complete.
- **AC2:** Given one pane is rate-limited, then that lens is retried via the sponge mesh backoff before synthesis proceeds.
- **AC3:** Given all four panes succeed, when synthesis completes, then the structured report is printed to stdout with section headings for each lens plus a final RECOMMENDATION section.

**P1-24** As a solo developer, I want `kova ci watch --interval 5` to re-run check/clippy/test on every file save so that errors surface within 10 seconds of a bad edit.
- **AC1:** Given `notify` watcher (`src/ci.rs` `f178`) is running, when a `.rs` file is saved, then the CI run starts within 5 s and results print to stdout.
- **AC2:** Given the CI run fails, then the failed phase (check/clippy/test) is highlighted in red and subsequent phases are skipped.
- **AC3:** Given `--no-clippy` flag, then clippy is skipped and only check + test run.

**P1-25** As a solo developer, I want the T2 molecular coordinator (`f411` in `src/swarm/t2.rs`) to route my REPL input to `code_responder` or `prose_responder` system prefix automatically, so that code questions get terse code-first responses and prose questions get narrative explanations.
- **AC1:** Given T2 is wired into the REPL inference path, when I type a Rust snippet, then `f411` routes to `CodeResponder` and the system prompt prepend is `## Mode: code\nPrioritize working code...`.
- **AC2:** Given I type "explain what a borrow checker does", then T2 routes to `ProseResponder` and the system prompt prepend is `## Mode: prose\nPrioritize clarity...`.
- **AC3:** Given the T2 route confidence is below 0.5, then `CodeResponder` is used as the safe default.

---

### P2 — Fleet Operator

**P2-01** As a fleet operator, I want `kova c2 build --broadcast --release` to sync the workspace and build kova on all 5 nodes in parallel so that a fleet-wide deploy takes one command and under 10 minutes.
- **AC1:** Given all nodes are reachable via SSH, when `f356` in `src/c2.rs` runs, then rsync fires in parallel to all nodes, followed by `cargo build --release` on each; per-node pass/fail is streamed with `[node]` prefixes.
- **AC2:** Given one node is unreachable (SSH timeout), when the broadcast runs, then that node is skipped with `[lf] SKIP: unreachable` and other nodes proceed normally.
- **AC3:** Given `--nodes lf,bt` flag, then only lf and bt receive the build; gd/st/mm are not touched.

**P2-02** As a fleet operator, I want `kova c2 inspect` to show CPU cores, RAM, disk free, and GPU model for every node in under 5 seconds so that I can make placement decisions without manual SSH.
- **AC1:** Given all nodes reachable, when `f359` in `src/inspect.rs` runs, then output is a table: `Host | Cores | RAM | Disk(GB) | GPU` printed in under 5 s.
- **AC2:** Given `--json` flag, then output is valid JSON array with the same fields, suitable for scripting.
- **AC3:** Given a node is down, then its row shows `offline` in every field and the command exits 0 (not 1 — a down node is not a failure mode for inspect).

**P2-03** As a fleet operator, I want `kova c2 wake lf` to send a Wake-on-LAN packet to the Legion Forge node so that I can power it on remotely without physical access.
- **AC1:** Given bt's MAC address is configured (`f352` in `src/c2.rs`), when `kova c2 wake lf` runs, then a WoL magic packet is sent to the broadcast address and `WoL sent to lf` prints.
- **AC2:** Given the node identifier is unknown (e.g., `kova c2 wake zz`), then stderr prints `unknown node: zz` and exit code is 1.
- **AC3:** Given WoL succeeds, when I run `kova c2 inspect` 60 s later, then the node is reachable (this is environment-dependent; acceptance is on the WoL packet send, not node reachability).

**P2-04** As a fleet operator, I want `kova c2 status` to show a green/red dot per node with load average and memory in a single line per node so that I can assess fleet health at a glance.
- **AC1:** Given all nodes online, when `kova c2 status` runs (inline SSH, `ConnectTimeout=3`), then each node prints `● node  load=0.45  3200M/46000M` with ANSI green dot; offline nodes print `○ node  offline` with dim dot.
- **AC2:** Given the command runs, then it completes in under 5 s (parallel SSH with 3 s timeout per node).
- **AC3:** Given output is piped (`kova c2 status | grep offline`), then ANSI escape codes do not corrupt the grep output (detect TTY and strip when not a TTY).

**P2-05** As a fleet operator, I want `kova c2 monitor --interval 3` to continuously poll all nodes and print lines only when state changes so that I can leave it running in a pane without scroll noise.
- **AC1:** Given monitor is running and node loads are stable, when no load changes, then no new output is printed (silence = all clear).
- **AC2:** Given a node goes from online to offline, then kova immediately prints `○ lf  offline` on the next poll cycle (within `interval` seconds).
- **AC3:** Given Ctrl+C, when the signal is received, then the command exits cleanly with exit code 0.

**P2-06** As a fleet operator, I want `kova c2 tmux-sponge "cargo build"` to send the command to all active tmux panes with rate-limit-aware retry so that panes blocked by Claude Code rate limits are not lost.
- **AC1:** Given a tmux session `kova-c2` with 4 panes, when `f379` runs, then the fast first pass sends to all unblocked panes; rate-limited panes are queued for retry with exponential backoff starting at 1 s.
- **AC2:** Given a pane is rate-limited for more than 60 s, then kova logs `[pane 2] still blocked after 60s` and moves on rather than hanging indefinitely.
- **AC3:** Given all panes receive the command, then exit code is 0 and a summary `4/4 panes delivered` prints.

**P2-07** As a fleet operator, I want `kova c2 unblock --interval 3` to run as a daemon that auto-approves Claude Code prompts and flushes pasted text in blocked panes so that the fleet resumes work without manual intervention.
- **AC1:** Given a pane shows a `[y/N]` prompt, when `f387` detects it (pane peek), then it sends `y\n` automatically.
- **AC2:** Given a pane is stuck with incomplete pasted text (no newline), when the daemon detects the stall, then it sends `\n` to flush.
- **AC3:** Given `--interval 3`, then the daemon polls every 3 s; CPU usage stays under 1% on bt during the poll loop.

**P2-08** As a fleet operator, I want `kova c2 gpu lock bt gpu_training` to exclusively reserve bt's GPU for a training job so that concurrent jobs don't fight for VRAM and crash.
- **AC1:** Given no lock exists on bt, when `acquire` in `src/gpu_sched.rs` runs, then the lock is written to redb and `[bt] GPU locked: gpu_training` prints.
- **AC2:** Given a lock already exists, when another job calls `lock`, then it returns `gpu busy: locked by gpu_training` and exit code is 1.
- **AC3:** Given `kova c2 gpu release bt`, then the lock is cleared and the next queued job (if any) is eligible to run.

**P2-09** As a fleet operator, I want `kova c2 queue submit "cargo run --bin carve-static" --node bt --tag corpus` to enqueue a job that runs when bt is free so that I can fire-and-forget long corpus carving runs.
- **AC1:** Given the job is submitted, when `job_queue::submit` writes to redb, then `kova c2 queue status` shows the job with status `queued` and the correct tag.
- **AC2:** Given `kova c2 queue drain`, when it runs, then the highest-priority queued job is dispatched via SSH to its target node.
- **AC3:** Given the job fails on the node (non-zero exit), when the retry count is exhausted (default: 2), then the job status is marked `dead` and `kova c2 queue history` shows it with the error.

**P2-10** As a fleet operator, I want `kova c2 offload --threshold 90` to automatically move build artifacts from bt's disk to the archive node when disk usage hits 90% so that I never lose a build to a full disk.
- **AC1:** Given bt disk usage ≥ 90%, when `f360` runs, then `target/` directories older than 7 days are rsynced to the archive node and removed locally.
- **AC2:** Given `--dry-run`, then a list of would-be-moved paths is printed but nothing is moved.
- **AC3:** Given disk usage is below threshold, when the command runs, then it prints `disk at 72% — no offload needed` and exits 0.

**P2-11** As a fleet operator, I want `kova c2 ssh-ca setup` to initialize the CA key and sign certs for all workers so that I never get a host key changed warning when a node's IP rotates.
- **AC1:** Given no CA exists, when `f300` in `src/ssh_ca.rs` runs, then a CA keypair is created at `~/.kova/ca/`, and each worker's host key is signed and deployed via SSH.
- **AC2:** Given the CA already exists, when `setup` runs again, then it signs and deploys new certs without overwriting the CA keypair.
- **AC3:** Given a new node is added, when `kova c2 ssh-ca sign new-node` runs, then only that node's cert is signed and deployed.

**P2-12** As a fleet operator, I want `kova c2 fleet` to show all CochranBlock projects with build status, binary size, and last commit per project in a single table so that I can see the full fleet at a glance.
- **AC1:** Given `discover_projects()` returns paths for all projects with `.kova` marker files, when `C2Cmd::Fleet` runs, then output is a table with columns: `project | status | binary | last commit`.
- **AC2:** Given a project has no `target/release/<name>` binary, then the status column shows `clean` in dim gray.
- **AC3:** Given a project's git log fails (not a git repo), then the last-commit column shows `-` and the command continues.

**P2-13** As a fleet operator, I want `kova c2 ncmd ci --oneline` to show a one-line summary of every node's CPU/memory/load so that I can run this from any pane and get instant fleet state.
- **AC1:** Given all nodes reachable, when `f132` in `src/node_cmd.rs` runs with `oneline=true`, then output is `n0:4c/2G/0.5 n1:8c/3G/0.2 n2:12c/4G/0.1 ...` on a single line.
- **AC2:** Given `--idle` flag, then only the least-loaded reachable node is returned (useful for `kova c2 ncmd c6 --idle` to build on the idlest node).
- **AC3:** Given `--nodes n0,n2`, then only n0 and n2 are queried.

**P2-14** As a fleet operator, I want `kova deploy --nodes lf,bt` to sync and build kova on only lf and bt then restart `kova-serve` on those nodes so that I can roll out to a subset for canary testing.
- **AC1:** Given `--nodes lf,bt`, when `f370` in `src/c2.rs` runs, then rsync, build, and service restart happen only on lf and bt; gd/st/mm are untouched.
- **AC2:** Given `--skip-build`, then rsync still runs but `cargo build` is skipped (useful when deploying a pre-built binary from bt).
- **AC3:** Given a node fails the restart step, then other nodes continue and the failed node is reported at the end with its error.

**P2-15** As a fleet operator, I want `kova c2 init --scan ~/ --session kova-c2` to scan my home directory for `.kova` marker files and create one tmux pane per project so that my fleet session is always in sync with my active projects.
- **AC1:** Given `.kova` marker files in `~/kova`, `~/cochranblock`, `~/approuter`, when `f400` runs, then a tmux session `kova-c2` is created with one window per project, each `cd`'d to the project root.
- **AC2:** Given `--no-agent`, then windows are created but no kova REPL is launched in them.
- **AC3:** Given the session already exists, when init runs, then it prints `session kova-c2 already exists — attach with tmux attach -t kova-c2` and exits 0 without clobbering the existing session.

**P2-16** As a fleet operator, I want `kova c2 ncmd c6 --release --extra kova-engine` to trigger a release build of kova on all reachable nodes so that I can confirm the build is green everywhere before shipping.
- **AC1:** Given `c6` token dispatches to all reachable nodes with the release flag, when `f132` runs, then `cargo build --release -p kova-engine` runs on each node in parallel and per-node output is streamed.
- **AC2:** Given one node fails the build, then that node's stderr is captured and printed with `[node] BUILD FAIL:` prefix.
- **AC3:** Given `--nodes n2` filter, then only bt receives the build command.

**P2-17** As a fleet operator, I want `kova c2 broadcast "kova test"` to run the TRIPLE SIMS gate on all nodes simultaneously so that I can confirm all nodes are green before a release.
- **AC1:** Given 4 nodes online, when broadcast fires `kova test` in parallel (threading `f370`-style), then each node's pass/fail is reported with `[node]` prefix.
- **AC2:** Given one node returns exit code 1, then the overall broadcast exit code is 1 and that node's output is preserved.
- **AC3:** Given `--nodes lf`, then only lf runs the gate.

**P2-18** As a fleet operator, I want `kova c2 dispatch bt "tail -n 50 ~/.kova/kova.log"` to stream the last 50 log lines from bt in real time so that I can debug a service issue without SSHing in.
- **AC1:** Given bt is reachable, when the SSH command runs, then stdout from the remote command is streamed locally in real time (not buffered to completion).
- **AC2:** Given bt is unreachable, then stderr prints `[bt] ssh error: connection refused` and exit code is 1.
- **AC3:** Given the remote command exits non-zero, then the local exit code matches the remote exit code.

**P2-19** As a fleet operator, I want `kova c2 peek 2 --lines 30` to show the last 30 lines of tmux pane 2 so that I can check progress without switching panes.
- **AC1:** Given pane 2 exists in session `kova-c2`, when `f386` runs, then the last 30 visible lines are printed to stdout.
- **AC2:** Given the pane number doesn't exist, then stderr prints `pane 2 not found in session kova-c2` and exit code is 1.
- **AC3:** Given `--session custom-name`, then the named session is used instead of the default.

**P2-20** As a fleet operator, I want `kova c2 tmux-status` to show which panes are working, idle, blocked, or rate-limited so that I can identify stuck panes without reading each one.
- **AC1:** Given a 4-pane session, when `f385` runs, then each pane shows one of: `WORKING`, `IDLE`, `RATE_LIMITED`, `BLOCKED` based on the last visible output.
- **AC2:** Given a pane shows a `Rate limit` string, then it is classified `RATE_LIMITED`.
- **AC3:** Given all panes are idle, then overall status prints `fleet idle — all 4 panes ready`.

**P2-21** As a fleet operator, I want `kova c2 tmux-broadcast "kova c2 ncmd ci"` with a 5-second stagger so that panes don't all fire the SSH command simultaneously and overwhelm the cluster.
- **AC1:** Given `--stagger 5`, when `f378` runs, then pane 0 receives the command at t=0, pane 1 at t=5s, pane 2 at t=10s, etc.
- **AC2:** Given `--stagger 0`, then all panes receive simultaneously.
- **AC3:** Given a tmux `send-keys` failure, then the failed pane is logged but subsequent panes still receive the command.

**P2-22** As a fleet operator, I want `kova c2 qa` to broadcast build + clippy + status to all panes in one command so that a full QA sweep requires one keystroke.
- **AC1:** Given `f388` runs, then it broadcasts `kova x x1 && kova x x3` to all panes via sponge mesh.
- **AC2:** Given panes complete, then `kova c2 tmux-status` is run and the summary is printed to stdout.
- **AC3:** Given any pane returns a clippy error, then the QA result is `FAIL` and exit code is 1.

**P2-23** As a fleet operator, I want `kova c2 sync --all --full` to tar-stream the entire workspace to all worker nodes so that a fresh node gets full content on first setup.
- **AC1:** Given `--full` flag, when `f358` runs, then a tar archive of the workspace is streamed via SSH to all nodes (not incremental rsync).
- **AC2:** Given `--dry-run`, then the tar command is printed but not executed.
- **AC3:** Given incremental mode (no `--full`), then rsync delta is used and only changed files are transferred.

**P2-24** As a fleet operator, I want `kova c2 recommend` to read `kova c2 inspect` output and print placement recommendations (e.g., "run corpus carving on bt — most RAM") so that I don't have to interpret resource tables manually.
- **AC1:** Given inspect data, when `f361` runs, then it prints at least one recommendation in the form `[recommendation] run X on Y: reason`.
- **AC2:** Given all nodes are loaded above 80%, then the recommendation includes a warning: `all nodes loaded — consider scheduling during off-hours`.
- **AC3:** Given only one node is online, then the recommendation names that node for all workloads.

**P2-25** As a fleet operator, I want `kova c2 gpu vram` to query live VRAM usage on all GPU nodes so that I know whether bt's 8 GB is free before queuing a training job.
- **AC1:** Given bt is online and has an AMD GPU, when `vram_all` in `src/gpu_sched.rs` runs, then output shows `bt: 2.1 GB / 8.0 GB used`.
- **AC2:** Given the node is offline, then its entry shows `bt: unavailable`.
- **AC3:** Given `--node bt`, then only bt is queried.

---

### P3 — ML Engineer / Model Trainer

**P3-01** As an ML engineer, I want `kova export training --format dpo` to emit `~/.kova/training_data/dpo.jsonl` from my LLM trace history so that I can use those pairs to fine-tune a model.
- **AC1:** Given at least one trace in redb `tele/` namespace, when `f181` in `src/training_data.rs` runs with `--format dpo`, then output is JSONL where each line is `{"prompt": ..., "chosen": ..., "rejected": ...}` valid UTF-8.
- **AC2:** Given `--output /tmp/custom.jsonl`, then output is written there instead of the default path.
- **AC3:** Given zero traces, when the command runs, then it prints `no traces to export` and exits 0 (not 1).

**P3-02** As an ML engineer, I want `cargo run --bin retrain-starters` to retrain the `slop_detector`, `code_vs_english`, and `lang_detector` classifiers on the corpus and emit new `.nanobyte`-ready weight files so that I can close the retraining loop.
- **AC1:** Given training data files under `~/.kova/training/`, when `src/bin/retrain-starters.rs` runs, then three weight files are written to `assets/models/` and each prints accuracy on held-out set.
- **AC2:** Given `--feature-dim 8192` (current default), then the classifier has 16,386 params and is compatible with the existing `STARTER_NANOBYTE` manifest format.
- **AC3:** Given training fails (e.g., empty dataset), then the command exits non-zero with an error message; no partial weight file is written.

**P3-03** As an ML engineer, I want `cargo run --bin pack-starter` to pack all four starter models (`slop_detector`, `code_vs_english`, `lang_detector`, `intent_classifier`) into a single `assets/starter.nanobyte` with a BLAKE3 signature so that the binary always ships with verified weights.
- **AC1:** Given four weight files in `assets/models/`, when pack-starter runs, then `assets/starter.nanobyte` is written with the 64B header + 320B manifest + weights + 36B NSIG trailer.
- **AC2:** Given `Nanobyte::verify()` is called on the output, then it returns `Ok(())` (BLAKE3 matches).
- **AC3:** Given any weight file is missing, then pack-starter exits non-zero with `missing model: slop_detector` rather than writing a partial file.

**P3-04** As an ML engineer, I want `cargo run --bin bench-classify` to evaluate the intent classifier on the banking77 held-out test set and print accuracy, macro-F1, lowest-F1 classes, and top confusions so that I know whether the retrain improved or regressed.
- **AC1:** Given `assets/models/intent_classifier/` contains weights, when bench-classify runs, then it prints: overall accuracy (e.g., `80.36%`), macro F1 (e.g., `81.10%`), bottom-5 class F1 scores, top-5 confusion pairs.
- **AC2:** Given accuracy regresses by more than 2 pp from the previous baseline (80.36%), then exit code is 1 so CI can block the pack.
- **AC3:** Given a non-existent model directory, then bench-classify prints `model not found` and exits 1.

**P3-05** As an ML engineer, I want the `compiler_teacher` pair capture to never silently lose a pair due to serialization failure so that the training flywheel doesn't degrade under load.
- **AC1:** Given `bincode::serde::encode_to_vec` fails on a pair, when the current code runs, then — **BUG** — `unwrap_or_default()` returns `Vec::new()`, a zero-byte entry is written, and the pair is silently lost. **Fix required:** return early with a logged error instead. After fix: a failure to encode logs `[compiler_teacher] encode error: ...` to stderr and returns without writing.
- **AC2:** Given the fix is applied, when `all_pairs()` is called, then it returns zero pairs for that key rather than a corrupt entry, and no panic occurs.
- **AC3:** Given 1000 rapid pair captures under concurrent load, when `all_pairs()` is queried, then every successfully serialized pair appears in the results with no data corruption.

**P3-06** As an ML engineer, I want `kova export tele` to dump REPL telemetry (every T1 classification on every input/response) as JSONL so that I can use it to retrain the intent classifier on real code-intent distribution.
- **AC1:** Given REPL sessions stored in redb `tele/{ts}/{i|o}`, when `ExportCmd::Tele` runs, then JSONL output contains one row per exchange: `{"ts": ..., "input": ..., "output": ..., "classifications": {...}}`.
- **AC2:** Given `--output /tmp/tele.jsonl`, then output is written there.
- **AC3:** Given zero tele entries, then output is an empty file and exit code is 0.

**P3-07** As an ML engineer, I want `kova train-router --mine-projects --mix-synthetic` to train the T1 `tool_router` classifier from real Claude Code transcripts plus synthetic pairs so that it generalizes beyond the Bash-heavy transcript distribution.
- **AC1:** Given `~/.claude/projects/` exists with session transcripts, when train-router runs, then it parses transcripts, extracts (prompt→tool) pairs, mixes in `SYNTH_ROUTER_PAIRS`, trains a trigram hash classifier, and writes the model to `~/.kova/models/tool_router/`.
- **AC2:** Given `--max-per-class 200`, then no class has more than 200 training examples (undersampling the Bash-biased distribution).
- **AC3:** Given `--min-per-class 50`, then classes with fewer than 50 examples are oversampled by cycling.

**P3-08** As an ML engineer, I want `kova micro forge --tier spark` to train a 50K-param Spark model from scratch in pure Rust via candle so that I have a locally trained model with zero external dependency.
- **AC1:** Given training data exists, when `src/micro/candle_train.rs` runs for `KovaClassifier::spark()`, then a model with ≤50K params is trained and weights are written to `~/.kova/models/kova-spark/`.
- **AC2:** Given `--epochs 200 --lr 0.01`, then the model trains for exactly 200 epochs with that learning rate.
- **AC3:** Given CUDA/Metal is unavailable, then candle falls back to CPU without erroring (no GPU hard requirement for Spark tier).

**P3-09** As an ML engineer, I want `kova micro quantize spark --outlier-frac 0.25` to apply TurboQuant (FWHT + mixed-precision 2/4-bit + QJL residual) to a trained Spark model so that it fits in the `starter.nanobyte` without significant accuracy loss.
- **AC1:** Given a trained Spark model in `~/.kova/models/kova-spark/`, when `f371` in `src/micro/quantize.rs` runs, then a quantized weight file is emitted with ≤25% of rows at 4-bit precision (the outlier fraction).
- **AC2:** Given quantization, when the quantized model is benchmarked against the held-out set, then accuracy degrades by less than 3 pp from the full-precision baseline.
- **AC3:** Given `--outlier-frac 0.0` (all 2-bit), then the file size is minimized and accuracy regression is logged as a warning if > 5 pp.

**P3-10** As an ML engineer, I want `kova micro tournament` to run 42 models against 45 challenges across 6 event types and emit a tournament result so that I know which model wins which weight class.
- **AC1:** Given a cluster with ≥1 reachable node, when `f250` in `src/micro/tournament.rs` runs, then results include pass/fail per (model, challenge) pair and per-category winners.
- **AC2:** Given a tournament checkpoint exists from a previous interrupted run, when tournament runs again, then it resumes from the checkpoint rather than starting over.
- **AC3:** Given `kova micro tournament-clear`, then the checkpoint is deleted and the next run starts fresh.

**P3-11** As an ML engineer, I want the priority queue (`swarm/priority.rs`) to bump a model's score when the T1 classifier signals relevance and decay all scores on a tick so that frequently used models stay hot in redb's page cache.
- **AC1:** Given `f430(model)` is called, when the score is bumped by `BUMP_AMOUNT=1000`, then `f433(model)` returns the new score and the redb B-tree key reflects the inverted score for fast iteration.
- **AC2:** Given `f431` (decay tick) is called, then every model's score is multiplied by `DECAY_FACTOR=0.95` and the B-tree is updated.
- **AC3:** Given `f432(3)`, then the top-3 models by score are returned in descending score order.

**P3-12** As an ML engineer, I want `kova micro evolve --epochs 200` to run the Synth→Retrain loop in one command so that I can improve the Spark classifier without manually orchestrating the steps.
- **AC1:** Given no existing Spark model, when `MicroCmd::Evolve` runs, then it calls `Synth` to generate training data, then `Forge` to train Spark, emitting a model in `~/.kova/models/kova-spark/`.
- **AC2:** Given an existing model, when evolve runs again, then the previous model is overwritten with the new one.
- **AC3:** Given `--epochs 200`, then exactly 200 training epochs are used.

**P3-13** As an ML engineer, I want `kova micro evolve-full` to run the complete tournament→export→synth→retrain→MoE validation pipeline in one command so that the full quality-improvement loop is automated.
- **AC1:** Given prior tournament results, when `MicroCmd::EvolveFull` runs, then all four stages complete in sequence and the MoE validation result is printed at the end.
- **AC2:** Given tournament results are stale (older than 24 h), then a warning is printed: `[evolve-full] tournament results are X hours old — consider re-running tournament`.
- **AC3:** Given `--max-cascade 3`, then MoE validation allows up to 3 cascade attempts per challenge.

**P3-14** As an ML engineer, I want `kova micro mine-export` to mine conversation logs and export them as training JSONL so that I have real interaction data for fine-tuning.
- **AC1:** Given `~/.claude/projects/` contains session logs, when `logmine::f237` runs, then (prompt, response) pairs are extracted and statistics printed.
- **AC2:** Given `logmine::f238`, when it runs, then a JSONL file is written to `~/.kova/training_data/mined.jsonl`.
- **AC3:** Given no conversation logs exist, then the command exits 0 with `no logs found to mine`.

**P3-15** As an ML engineer, I want `cargo run --bin carve-static` to parse 240K `.crate` files from the corpus and emit 15 labeled JSONL training datasets (async, arg_count, return_type, etc.) so that subatomic models have training data.
- **AC1:** Given `--features carve` and corpus at `/mnt/data/crates/`, when `src/bin/carve_static.rs` runs, then 15 JSONL files appear in the output directory, one per model type.
- **AC2:** Given a malformed crate (unparseable `lib.rs`), then it is skipped with a warning and other crates continue processing.
- **AC3:** Given the output directory already exists, then the run overwrites existing files rather than appending (idempotent).

**P3-16** As an ML engineer, I want the T2 molecular coordinator (`f410/f411`) to be wired into the REPL inference path so that the T1 feature vector feeds the T2 routing decision before every LLM call.
- **AC1:** Given T2 is wired, when I type a Rust snippet in the REPL, then `f410` computes the 8-float feature vector, `f411` returns `Route::CodeResponder`, and the system prompt prepend is applied before inference.
- **AC2:** Given T2 weight file exists at `~/.kova/models/t2/`, then T2 uses learned weights; otherwise it falls back to the hardcoded linear classifier in `f411`.
- **AC3:** Given the T2 route decision, then it is logged as a telemetry entry alongside the T1 classification so I can audit routing accuracy.

**P3-17** As an ML engineer, I want the `f39(_p)` dead parameter bug in `storage.rs` to be fixed so that callers who pass a path actually open the DB at that path.
- **AC1:** Given the fix is applied (parameter renamed to `path`, used instead of `global_db()`), when `t12::new(custom_path)` is called, then a database is opened at `custom_path`, not the global path.
- **AC2:** Given two concurrent calls with different paths, then two independent databases are opened with no cross-contamination.
- **AC3:** Given the public API change, when all internal callers are updated, then `kova tokens` still returns 100%.

**P3-18** As an ML engineer, I want `kova micro academy` to analyze tournament results, detect which categories have the lowest pass rates, and recommend curriculum additions so that I know where to invest training effort next.
- **AC1:** Given tournament results in `~/.kova/micro/tournament_result.json`, when `f230` in `src/micro/academy.rs` runs, then the report identifies categories with < 50% pass rate.
- **AC2:** Given the report is generated, when `f232` writes it to disk, then it appears at `~/.kova/micro/academy_report.json`.
- **AC3:** Given all categories have > 80% pass rate, then the report still prints and notes `no curriculum gaps detected`.

**P3-19** As an ML engineer, I want `kova micro train --format dpo` to invoke LoRA fine-tuning via `mlx_lm` so that I can fine-tune a model on Apple Silicon without writing training scripts.
- **AC1:** Given tournament DPO pairs are exported, when `MicroCmd::Train` with `--format dpo` runs, then the `mlx_lm` command is constructed with correct paths and executed.
- **AC2:** Given `--dry-run`, then the command is printed but not executed.
- **AC3:** Given `mlx_lm` is not installed, then an error `mlx_lm not found — install via pip install mlx-lm` is printed and exit code is 1.

**P3-20** As an ML engineer, I want the `rand_bytes` naming lie in `trace.rs` `f161` to be fixed and the discriminant made truly unique so that two traces in the same millisecond with identical size/latency don't overwrite each other.
- **AC1:** Given the fix uses an atomic counter suffix (e.g., `static COUNTER: AtomicU64`), when two traces arrive in the same millisecond, then both are written to distinct redb keys and both appear in `kova traces recent`.
- **AC2:** Given the renamed variable is called `discriminant`, then the naming lie is resolved and a reader can understand the intent.
- **AC3:** Given the counter wraps at `u64::MAX` (unlikely), then the behavior is documented in a comment; no silent data loss occurs.

**P3-21** As an ML engineer, I want `kova micro validate "classify_intent" "debug: the borrow checker is complaining"` to run the output validator and print pass/fail per check so that I can verify model outputs meet quality criteria.
- **AC1:** Given the template exists in the registry, when `f263` in `src/micro/validate.rs` runs, then output lists each check: completeness, coherence, format, confidence.
- **AC2:** Given the response contains `todo!()` or `unimplemented!()`, then the completeness check fails.
- **AC3:** Given all checks pass, then overall status is `PASS` and exit code is 0.

**P3-22** As an ML engineer, I want `kova micro stats` to show per-template pass rate, average latency, and token throughput from the last bench/tournament run so that I can track model quality over time.
- **AC1:** Given stats file exists at the path from `stats::f246()`, when `T158::print()` runs, then output is a table: template | pass_rate | avg_latency_ms | avg_tokens.
- **AC2:** Given no stats exist, then output prints `No stats yet. Run kova micro run or kova micro bench first.` and exits 0.
- **AC3:** Given a new run adds data, then stats accumulate (not replace) prior runs.

**P3-23** As an ML engineer, I want `kova micro synth` to generate synthetic (prompt → tool) training examples for all 8 classifier categories so that I have balanced training data even for rare tool types.
- **AC1:** Given `MicroCmd::Synth` runs (`src/micro/candle_train.rs` synthetic data gen), then JSONL output is written to `~/.kova/training_data/synth.jsonl` with at least 100 examples per category.
- **AC2:** Given the synthetic data mixes with real transcripts via `--mix-synthetic` in train-router, then the combined dataset is balanced across categories.
- **AC3:** Given the output file already exists, then it is overwritten (not appended).

**P3-24** As an ML engineer, I want the intent classifier to be retrained on code-intent labels (debug/add/refactor/explain/test/review/build-infra) rather than banking77 so that routing decisions in the REPL are semantically correct.
- **AC1:** Given the new labeled corpus (≥500 examples per category), when `cargo run --bin train-kova-intent` runs, then a new classifier is trained and its held-out accuracy on code intents (not banking77) is reported.
- **AC2:** Given the new model is packed into `starter.nanobyte`, when the REPL classifies "fix the lifetime error on line 42", then the intent label is `debug` (not `check_balance`).
- **AC3:** Given the old banking77 model is still present for comparison, when bench-classify runs on both, then the code-intent model has ≥ 80% accuracy on the code-intent test set.

**P3-25** As an ML engineer, I want the `compiler_teacher` LazyLock global to be replaceable with an injected `&Database` in test contexts so that tests for `save_pair`/`lookup_hint` don't pollute `~/.kova/training/compiler_pairs.redb`.
- **AC1:** Given a `with_test_db` parameter pattern (matching `tools.rs` `with_test_redb`), when `save_pair` is called in a test, then data is written to the temp DB, not the production path.
- **AC2:** Given the test DB is dropped, then `~/.kova/training/compiler_pairs.redb` is unchanged.
- **AC3:** Given `lookup_hint` is called with the temp DB, then it reads from the temp DB only.

---

### P4 — First-Time External Evaluator

**P4-01** As a first-time user, I want `kova bootstrap` to create `~/.kova/` and write a minimal config in under 3 seconds so that I can start using kova without reading documentation.
- **AC1:** Given no `~/.kova/` directory, when `bootstrap` runs, then `~/.kova/config.toml`, `~/.kova/memory.md`, and `~/.kova/prompts/` are created within 3 s.
- **AC2:** Given `~/.kova/` already exists, when bootstrap runs again, then existing files are untouched (idempotent).
- **AC3:** Given no write permission to `~HOME`, then bootstrap exits 1 with `cannot create ~/.kova: permission denied`.

**P4-02** As a first-time user, I want `kova` with no arguments to open a working chat session even without a local GGUF model — falling back to the Anthropic API if `ANTHROPIC_API_KEY` is set — so that I can evaluate kova without installing a local model.
- **AC1:** Given `KOVA_INFERENCE=auto` and no local model but a valid API key, when `kova` runs, then the REPL opens with `[inference: remote — claude-sonnet-4-6]` in the banner.
- **AC2:** Given no API key and no local model, when `kova` runs, then the REPL opens with a clear message: `[inference: none — set ANTHROPIC_API_KEY or configure a local model in ~/.kova/config.toml]` and queries return an actionable error rather than a cryptic panic.
- **AC3:** Given the API key is present but invalid (403 from Anthropic), then the first query returns `inference error: 403 Unauthorized — check ANTHROPIC_API_KEY` rather than a crash.

**P4-03** As a first-time user, I want `kova --help` to show all subcommands grouped by category (AUGMENTATION / BUILD & CI / FLEET / INTELLIGENCE / COMPLIANCE / SYSTEM) so that I can find the right command without reading the codebase.
- **AC1:** Given any terminal width, when `kova --help` runs, then the after-help section shows the 6 category headers with subcommands listed under each.
- **AC2:** Given `kova c2 --help`, then c2 subcommands are listed with one-line descriptions.
- **AC3:** Given `kova chat --help`, then the chat subcommand's options are shown (`--project`).

**P4-04** As a first-time user, I want `kova serve` to start an HTTP server and print the URL so that I can try the web interface in a browser without reading documentation.
- **AC1:** Given `--features serve` compiled in, when `kova serve` runs, then it prints `listening on http://127.0.0.1:PORT` within 1 s of startup.
- **AC2:** Given I open that URL in a browser, then an embedded WASM client loads and renders a chat interface.
- **AC3:** Given port conflict (port already in use), then kova prints `error: port 8080 already in use` and exits 1.

**P4-05** As a first-time user, I want `kova demo` to exercise every CLI subcommand and HTTP endpoint automatically (zero user input) so that I can see what kova does in 2 minutes.
- **AC1:** Given the release binary exists, when `kova demo` runs, then it outputs a structured log of each step (bootstrap, serve, c2 nodes, etc.) and completes within 120 s.
- **AC2:** Given all steps pass, then exit code is 0 and the final line is `demo: all checks passed`.
- **AC3:** Given a step fails, then the failure is printed with the step name and the demo continues rather than aborting (best-effort coverage).

**P4-06** As a first-time user, I want `kova govdocs list` to show all available compliance documents and `kova govdocs sbom` to print the SBOM so that I can verify compliance without reading source code.
- **AC1:** Given the govdocs feature, when `kova govdocs list` runs, then it prints: `sbom security ssdf supply-chain accessibility privacy fips fedramp cmmc itar use-cases`.
- **AC2:** Given `kova govdocs sbom`, then the SBOM document is printed to stdout and contains the string `CAGE 1CQ66`.
- **AC3:** Given `kova govdocs unknown`, then it prints `unknown document: unknown — valid: sbom security ssdf ...` and exits 1.

**P4-07** As a first-time user, I want the binary to run on macOS ARM64, macOS x86_64, and Linux x86_64 without any installation step beyond downloading so that I can evaluate it on my machine regardless of OS.
- **AC1:** Given the macOS ARM64 binary downloaded from GitHub Releases, when I run `./kova --help`, then it executes without Gatekeeper blocking (signed with Apple notarization or user allows it).
- **AC2:** Given the Linux x86_64 binary, when I run `./kova bootstrap`, then it completes without any missing shared library error (`ldd` shows all deps satisfied or binary is statically linked).
- **AC3:** Given `file ./kova` on each platform, then output confirms correct architecture (ARM64/x86_64/ELF).

**P4-08** As a first-time user, I want `kova c2 nodes` to print the known cluster nodes even when none are reachable so that I understand the cluster topology without SSH access.
- **AC1:** Given no nodes are reachable, when `f355` runs, then output lists `lf gd bt st` with an indication that they are currently unreachable.
- **AC2:** Given nodes are reachable, then each node shows its hostname and IP.
- **AC3:** Given the first run (no config), then the static node list from `f350` is used as the default.

**P4-09** As a first-time user, I want `kova tokens` to show a human-readable summary of tokenization coverage so that I understand the compression protocol without reading LANGUAGE.md.
- **AC1:** Given the binary, when `kova tokens` runs, then output is `tokenization: 100.0% (N/N)\n  fn: M/M tokenized\n  ty: K/K tokenized`.
- **AC2:** Given coverage is < 100%, then the missing symbols are listed and exit code is 1.
- **AC3:** Given `kova tokens` runs without being in the kova repo, then it still succeeds (coverage data is baked into the binary, not read from disk).

**P4-10** As a first-time user, I want errors to print a single actionable message to stderr and exit non-zero so that I know exactly what to fix without parsing a stack trace.
- **AC1:** Given `ANTHROPIC_API_KEY` is absent, when an inference call fails, then stderr shows `inference error: ANTHROPIC_API_KEY not set — export it or use KOVA_INFERENCE=local` and the REPL continues.
- **AC2:** Given a file path not found, when `read_file` is called via the agent, then the tool returns `"error: file not found: /path/to/file"` to the agent (not a panic).
- **AC3:** Given any `T176::Other(String)` error variant, when it propagates to the CLI, then it is printed as a one-line human message (not `T176::Other(...)`).

**P4-11** As a first-time user, I want the Android APK (17 MB) to install on Android 11+ and open a chat UI so that I can evaluate kova on mobile.
- **AC1:** Given the APK from GitHub Releases v0.7.0, when installed on an Android 11 device (arm64-v8a), then the app launches and renders the egui chat interface.
- **AC2:** Given no internet connection, when the app opens, then the T1 classifiers (baked in STARTER_NANOBYTE) still run and the UI is responsive.
- **AC3:** Given internet connection and `ANTHROPIC_API_KEY` in the app config, when a chat message is sent, then inference completes and a response appears within 5 s.

**P4-12** As a first-time user, I want `kova tui` to open and be navigable with arrow keys and Enter without reading a manual so that I can start chatting immediately.
- **AC1:** Given `--features tui`, when `kova tui` runs, then a two-pane ratatui layout renders with an input field at the bottom.
- **AC2:** Given I type text and press Enter, then my message appears in the chat pane and inference is triggered.
- **AC3:** Given I press `Ctrl+C`, then the TUI exits cleanly (terminal restored, exit code 0).

**P4-13** As a first-time user, I want clear documentation of every environment variable in `--help` output or a README section so that I know how to customize inference without guessing.
- **AC1:** Given `kova --help` or `kova chat --help`, then the output references `KOVA_INFERENCE`, `KOVA_MODEL`, `KOVA_PERMS`, and `ANTHROPIC_API_KEY` with one-line descriptions.
- **AC2:** Given the PROOF_OF_ARTIFACTS.md env vars table, then all 4 documented vars match the actual behavior in `src/inference/mod.rs`.
- **AC3:** Given an undocumented env var is set, then kova ignores it silently (no cryptic parse errors).

**P4-14** As a first-time user, I want the WASM client at `kova serve` to work in Chrome, Firefox, and Safari (current versions) so that I can use the web UI on any modern browser.
- **AC1:** Given the WASM client loaded via `kova serve`, when I open it in Chrome 120+, then the chat interface renders and sends/receives messages.
- **AC2:** Given Firefox 120+, then same behavior.
- **AC3:** Given Safari 17+, then same behavior (no WASM feature gaps).

**P4-15** As a first-time user, I want the binary size to be under 30 MB so that downloading and copying the binary is fast even on a slow connection.
- **AC1:** Given the release build with `lto="fat"`, `codegen-units=1`, `strip=true`, `panic='abort'`, then `ls -lh target/release/kova` shows ≤ 30 MB.
- **AC2:** Given macOS Intel build (no RAG/ort), then binary is ≤ 15 MB.
- **AC3:** Given Android APK, then APK is ≤ 20 MB and AAB ≤ 7 MB.

---

### P5 — Federal Contractor / Government Procurement

**P5-01** As a federal contractor, I want `kova govdocs sbom` to print a machine-readable SBOM listing every Rust dependency with version and license so that I can satisfy EO 14028 supply chain requirements.
- **AC1:** Given `kova govdocs sbom`, when it runs, then output contains all direct and transitive dependencies from `Cargo.lock` with their version and license.
- **AC2:** Given `govdocs/SBOM.md` exists in the binary (baked at build), then the content is current as of the build date and includes the BLAKE3 hash of the Cargo.lock used.
- **AC3:** Given the output is piped to a file, then it parses as valid markdown with a machine-readable table.

**P5-02** As a federal contractor, I want `kova govdocs cmmc` to print the CMMC Level 2 practice coverage matrix so that I can determine eligibility for DoD contracts.
- **AC1:** Given `kova govdocs cmmc`, then `govdocs/CMMC.md` is printed to stdout.
- **AC2:** Given the document, then it lists at least 10 CMMC Level 2 practices with coverage status (Implemented / Partial / N/A).
- **AC3:** Given `kova govdocs list`, then `cmmc` appears in the list.

**P5-03** As a federal contractor, I want `kova govdocs itar`, when printed, to document that the codebase contains no ITAR-controlled technology so that I can confirm legal export status.
- **AC1:** Given `kova govdocs itar`, then `govdocs/ITAR_EAR.md` is printed.
- **AC2:** Given the document, then it states the software classification (EAR99 or equivalent) and confirms no ECCN-controlled cryptographic export issues.
- **AC3:** Given the Unlicense (public domain), then the document confirms that public domain software is generally EAR99.

**P5-04** As a federal contractor, I want `kova govdocs fedramp` to print FedRAMP notes so that I can begin the ATO process.
- **AC1:** Given `kova govdocs fedramp`, then `govdocs/FedRAMP_NOTES.md` is printed.
- **AC2:** Given the document, then it addresses the FedRAMP Moderate baseline at minimum (or explains why a lower baseline applies).
- **AC3:** Given the air-gap capability, then the document explicitly notes that kova can operate without cloud services (supporting FedRAMP IL4+ environments).

**P5-05** As a federal contractor, I want CAGE code `1CQ66` and UEI `W7X3HAQL9CF9` to appear in `kova --help` output or the README footer so that I can verify supplier identity without a separate lookup.
- **AC1:** Given the README footer, then it contains `CAGE 1CQ66` and `UEI W7X3HAQL9CF9`.
- **AC2:** Given `kova govdocs` any document, then the footer includes the CAGE code.
- **AC3:** Given the binary is built from the repo, then a `kova version` or `kova --version` output includes the CAGE code.

**P5-06** As a federal contractor, I want `kova govdocs ssdf` to print NIST 800-218 SSDF compliance evidence so that I can verify secure development lifecycle practices.
- **AC1:** Given `kova govdocs ssdf`, then `govdocs/SSDF.md` is printed with coverage of SSDF practices: PW (Prepare), PS (Protect), PV (Produce), RV (Respond).
- **AC2:** Given the document, then it references `cargo audit`, the threat model, and the BLAKE3 signing practices as SSDF PW.4 / PW.7 evidence.
- **AC3:** Given the document, then it references the TRIPLE SIMS gate as SSDF PV.6 (testing) evidence.

**P5-07** As a federal contractor, I want the ASSUMED_BREACH_THREAT_MODEL.md to be accessible without building from source so that my security team can review it.
- **AC1:** Given the public GitHub repository, then `ASSUMED_BREACH_THREAT_MODEL.md` is present in the repo root and accessible without authentication.
- **AC2:** Given `kova govdocs security`, then `govdocs/SECURITY.md` prints the kova-specific threat surface and mitigations table.
- **AC3:** Given the threat model, then it documents the 8 applicable threat scenarios for kova (binary compromise, pipeline compromise, C2 pivot, etc.).

**P5-08** As a federal contractor, I want to verify that kova produces no outbound network traffic except when explicitly invoked by the user so that I can use it in restricted network environments.
- **AC1:** Given `KOVA_INFERENCE=local`, when kova runs any command, then `strace -e network` shows zero outbound network calls (no telemetry, no update checks).
- **AC2:** Given `KOVA_INFERENCE=remote`, then network calls are limited to `api.anthropic.com` on port 443.
- **AC3:** Given `kova c2 inspect`, then network calls are only to configured SSH nodes (no third-party DNS resolution for unexpected hosts).

**P5-09** As a federal contractor, I want `cargo audit --json` on the locked dependencies to return zero high/critical findings so that I can provide supply chain assurance.
- **AC1:** Given `audit.toml` with justified exemptions, when `cargo audit` runs against `Cargo.lock`, then no unexempted high or critical advisories are present.
- **AC2:** Given a new advisory is published for a kova dependency, then `cargo audit` in the `kova test` gate catches it within 24 h of advisory publication.
- **AC3:** Given an exemption in `audit.toml`, then it includes a justification comment explaining why the exemption is safe.

**P5-10** As a federal contractor, I want the SDVOSB certification status to be verifiable via SAM.gov (CAGE 1CQ66, UEI W7X3HAQL9CF9) so that set-aside contract eligibility is confirmed.
- **AC1:** Given the CAGE and UEI in the README, when I query SAM.gov for CAGE `1CQ66`, then The Cochran Block LLC appears as an active SDVOSB.
- **AC2:** Given the govdocs, then the SBOM lists the EIN `41-3835237` for cross-reference with SAM.gov.
- **AC3:** Given the binary, then `kova govdocs sbom` prints the CAGE/UEI/EIN together so a procurement officer can verify all three in one step.

**P5-11** As a federal contractor, I want to confirm that kova is Unlicense (public domain) so that there are no IP licensing restrictions for government use or redistribution.
- **AC1:** Given the LICENSE file, then its first line is `This is free and unencumbered software released into the public domain.`
- **AC2:** Given `kova govdocs`, then every document footer includes `PUBLIC DOMAIN | UNLICENSE`.
- **AC3:** Given the Cargo.toml, then `license = "Unlicense"` and no proprietary deps appear (all deps are OSS).

**P5-12** As a federal contractor, I want `kova govdocs accessibility` to print Section 508 conformance documentation so that I can satisfy federal accessibility requirements.
- **AC1:** Given `kova govdocs accessibility`, then `govdocs/ACCESSIBILITY.md` is printed.
- **AC2:** Given the document, then it addresses Section 508 1194.22 (web) for the WASM client and 1194.21 (software) for the TUI/GUI surfaces.
- **AC3:** Given the TUI, then it is navigable via keyboard alone (no mouse required) satisfying the most basic 508 mobility accommodation.

**P5-13** As a federal contractor, I want `kova govdocs privacy` to confirm that kova collects no PII by default so that Privacy Act (5 U.S.C. § 552a) compliance is addressed.
- **AC1:** Given `kova govdocs privacy`, then `govdocs/PRIVACY.md` is printed.
- **AC2:** Given the document, then it states that kova stores only user-provided content (prompts, file paths) in local redb and never transmits it to third parties except when `KOVA_INFERENCE=remote`.
- **AC3:** Given `KOVA_INFERENCE=local`, then zero bytes of prompt content leave the device.

**P5-14** As a federal contractor, I want `kova govdocs fips` to document the cryptographic primitives used so that I can assess FIPS 140-2 posture.
- **AC1:** Given `kova govdocs fips`, then `govdocs/FIPS.md` is printed.
- **AC2:** Given the document, then it lists BLAKE3 (for NanoSign), AES-256-GCM (if used), HKDF, and Argon2id with their FIPS status and rationale.
- **AC3:** Given BLAKE3 is not FIPS-validated, then the document acknowledges this and provides a migration path or N/A justification.

**P5-15** As a federal contractor, I want the TIMELINE_OF_INVENTION.md to provide dated, commit-level prior-art documentation so that IP ownership is unambiguous for government contract purposes.
- **AC1:** Given `TIMELINE_OF_INVENTION.md`, then entries are dated and reference specific git commit hashes.
- **AC2:** Given a commit referenced in the timeline, when I run `git show <hash>`, then the commit exists and matches the described change.
- **AC3:** Given the timeline, then it covers features from at least 12 months of development history.

---
