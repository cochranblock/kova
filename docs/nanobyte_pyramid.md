# Nanobyte Pyramid — 1,000 Trained Reflexes

Kova-engine isn't a generalist. It's 1,000 tiny muscle-memory models, each trained on one specific task until the response is automatic. Input → pattern match → the reflex that's done this 10,000 times fires → output.

Training data: 87K LOC, 968 commits, 1,200+ tests, deploy logs, session transcripts across 15 Rust projects.

---

## LAYER 1: CODE REFLEXES (muscle memory for writing code)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 1 | add-route | "add /endpoint" | All .route() additions across 15 projects |
| 2 | add-test | "test this" | 1,200+ test functions paired with code they test |
| 3 | fix-compile | cargo error output | Error→fix pairs from 968 commits |
| 4 | fix-clippy | clippy warning | Clippy warning→fix pairs (89 "fix" commits) |
| 5 | add-feature-gate | "make it optional" | All #[cfg(feature)] patterns in codebase |
| 6 | add-struct | "need a type for X" | All struct definitions + their derive macros |
| 7 | add-cli-flag | "add --flag" | All clap derive patterns across projects |
| 8 | write-handler | "handle POST /x" | All axum handler functions |
| 9 | add-serde | "serialize this" | All Serialize/Deserialize impls |
| 10 | add-error | "error type for X" | All thiserror enum variants |
| 11 | tokio-spawn | "run this async" | All tokio::spawn patterns |
| 12 | reqwest-call | "fetch from URL" | All reqwest client builds + calls |
| 13 | sled-store | "persist this" | All sled insert/get patterns |
| 14 | zstd-compress | "compress before store" | All zstd encode/decode patterns |
| 15 | regex-match | "extract pattern" | All regex usage |
| 16 | ssh-cmd | "run on node" | All SSH command executions from c2.rs |

## LAYER 2: FILE REFLEXES (muscle memory for repo operations)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 17 | create-claude-md | new project | 15 CLAUDE.md files — structure, build commands, module maps |
| 18 | create-backlog | audit needed | 15 BACKLOG.md files — 20 prioritized items each |
| 19 | create-toi | ship milestone | 15 TIMELINE_OF_INVENTION.md files |
| 20 | create-poa | prove it works | 15 PROOF_OF_ARTIFACTS.md files |
| 21 | add-gitignore | new repo | target/, .env, .claude/ patterns |
| 22 | add-unlicense | new repo | Unlicense + header-writer pattern |
| 23 | add-contributors | new file | Header comment with Foundational Founders |
| 24 | add-govdocs | compliance needed | 12 federal compliance doc templates |
| 25 | add-readme-backlink | repo created | cochranblock.org backlink in README |
| 26 | add-compression-map | P13 pass | docs/compression_map.md with f/t/s tables |

## LAYER 3: BUILD REFLEXES (muscle memory for cargo)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 27 | cargo-check | code changed | Every cargo check invocation |
| 28 | cargo-clippy | pre-commit | Every clippy run + fix pattern |
| 29 | cargo-test | verify | Every test gate invocation |
| 30 | cargo-build-release | deploy prep | Release profile per project (opt-level, lto, strip) |
| 31 | bump-edition | "update edition" | 12 projects bumped from 2021→2024 |
| 32 | bump-msrv | edition mismatch | rust-version alignment with edition |
| 33 | add-dep | "need crate X" | All Cargo.toml dependency additions |
| 34 | remove-dep | "don't need X" | All dependency removals |
| 35 | feature-gate-dep | "make optional" | optional = true + feature gate patterns |
| 36 | fix-vendor | vendor broken | .cargo/config.toml vendor source patterns |
| 37 | decouple-dep | path→git | All path dep→git dep conversions (this session!) |

## LAYER 4: GIT REFLEXES (muscle memory for version control)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 38 | commit-with-trailers | changes staged | 968 commits with Foundational Founders trailers |
| 39 | commit-message-style | describe change | Verb-first, lowercase, specific |
| 40 | push-always | commit done | P21 — always push, never ask |
| 41 | git-lfs-setup | large files | pixel-forge LFS migration pattern |
| 42 | git-filter-repo | history bloat | Scrub safetensors from history |
| 43 | create-github-repo | new project | gh repo create + push |

## LAYER 5: DEPLOY REFLEXES (muscle memory for shipping)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 44 | rsync-to-bt | build needed | rsync source to build server |
| 45 | build-on-bt | source synced | ssh bt cargo build --release |
| 46 | scp-via-mac | bt→gd blocked | Tunnel through Mac Mini |
| 47 | mv-binary | binary copied | Atomic rename into /home/mcochran/bin/ |
| 48 | start-new-binary | binary replaced | nohup + PID relay (Gemini Man) |
| 49 | verify-running | binary started | ps aux grep + health check |
| 50 | source-cargo-env | bt no PATH | source ~/.cargo/env before cargo on nodes |
| 51 | kill-root-cargo-toml | workspace conflict | Remove/exclude ~/Cargo.toml on nodes |
| 52 | remove-vendor-config | vendor missing | Delete .cargo/config.toml, fetch from crates.io |

## LAYER 6: FLEET REFLEXES (muscle memory for tmux orchestration)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 53 | tf0-status | "how's fleet" | Fleet status check |
| 54 | tf1-dispatch | targeted task | Single pane dispatch with context |
| 55 | tf2-broadcast | portfolio-wide | Edition bumps, header-writer, dep updates |
| 56 | tf3-sponge | rate-limit-prone | Long builds, test suites |
| 57 | tf4-peek | pane stuck | Check pane output |
| 58 | tf5-unblock | session start | Start auto-approve daemon |
| 59 | tf6-qa | before deploy | QA sweep all panes |
| 60 | tfp-push | queue work | Push to backlog stack |
| 61 | tfdr-drain | queue loaded | Auto-dispatch until empty |
| 62 | effort-tier | pane assigned | Set /effort per tier (max/high/medium/low) |
| 63 | esc-before-dispatch | pane busy | Send Escape before new task |
| 64 | self-contain-prompt | dispatching | Every push includes full context, no assumptions |

## LAYER 7: NODE REFLEXES (muscle memory for C2 ops)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 65 | ci-inspect | "check nodes" | kova c2 ncmd ci — compact one-liner per node |
| 66 | c1-status | node health | hostname, uptime, load |
| 67 | c2-specs | capacity check | CPU, RAM, disk, Rust version |
| 68 | c5-sync | code needs deploying | rsync project to nodes |
| 69 | c9-full-deploy | ship it | sync + build + restart pipeline |
| 70 | wol-wake | node sleeping | Wake-on-LAN by MAC address |
| 71 | sshallp-broadcast | all nodes | Parallel SSH to all nodes |

## LAYER 8: CONTRACT REFLEXES (muscle memory for bidding)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 72 | scout-run | "find contracts" | whobelooking scout — 8 APIs |
| 73 | score-opportunity | opp found | NAICS match + agency + keyword + set-aside scoring |
| 74 | scrape-opp | need details | Headless Chrome on sam.gov/opp/{id}/view |
| 75 | draft-email | opportunity matched | Capability statement + past performance template |
| 76 | draft-white-paper | BAA found | Flywheel framing + named techniques |
| 77 | update-cap-statement | new capability | cochranblock.org/govdocs edits |
| 78 | check-sam-status | registration | SAM.gov Active, CAGE, UEI verification |
| 79 | perf-benchmark | site updated | whobelooking perf — FPS, CLS, TTFB |

## LAYER 9: AI TRAINING REFLEXES (muscle memory for model ops)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 80 | check-training-log | "how's training" | ssh lf, tail train log |
| 81 | check-loss-trend | log read | loss going up = lr too high |
| 82 | kill-bad-training | loss diverging | Kill process, resume from last good checkpoint |
| 83 | pick-lr | resuming training | lr history → loss curves (5e-5 was too high, 2e-5 better) |
| 84 | nanosign-model | checkpoint saved | BLAKE3 sign on .safetensors |
| 85 | verify-nanosign | model loaded | Check signature before inference |
| 86 | augment-training-data | quality low | brightness, h-flip, rotation, palette swap |

## LAYER 10: QUALITY REFLEXES (muscle memory for standards)

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 87 | triple-sims | test pass | Run 3x, all must match |
| 88 | truth-audit | deploy prep | Verify every factual claim against real data |
| 89 | slop-filter | text output | Ban utilize/leverage/optimize/comprehensive etc |
| 90 | zero-warnings | build done | No warnings in any project, ever |
| 91 | header-check | new file | Unlicense + Contributors in every .rs file |
| 92 | p23-triple-lens | arch decision | Optimist/pessimist/paranoia/synthesis |
| 93 | exopack-gate | pre-ship | Full test binary with triple sims |

## LAYER 11: SECRETS / INFRA REFLEXES

| # | Reflex | Trigger | Trained On |
|---|--------|---------|-----------|
| 94 | secrets-write | new key | ~/.secrets/lock.sh write |
| 95 | secrets-sync | key changed | scp to nodes |
| 96 | symlink-env | new project | ln -sf ~/.secrets/.env .env |
| 97 | no-systemctl | deploy | Binary self-manages via PID relay, never systemctl |
| 98 | no-root-cargo | workspace conflict | Each project owns its own workspace |
| 99 | fish-tank-css | animation needed | Static mask + bg-position loop, not oversized transform |
| 100 | mobile-viewport | mobile fix | 100dvh not 100vh, disable GPU-heavy effects |

---

## SUMMARY

100 reflexes mapped. Each is a specific action pattern extracted from real git history and session behavior. Training data exists for every one — commit diffs, before/after code, input→output pairs.

**Next 900:** Mine tmux session transcripts, Claude Code conversation logs, deploy logs, error→fix pairs at function level. Each repeated action becomes a reflex. Target: 1,000 reflexes by end of Q2 2026.

**Architecture:** MoE router (~20K params) at top. 1,000 nanobyte reflex models (~500 params each) below. Total: ~520K params. Under 2MB. Runs on a phone.

---

*Part of the kova-engine nanobyte pyramid. Training data: 15 Rust projects, 87K LOC, 968 commits, 1,200+ tests.*
