# Kova Compression Map

Tokenization for traceability. Source: workspace tokenization rules.

## Functions (fN)

| Token | Human name | Module | Tests |
|-------|------------|--------|-------|
| f14 | plan | plan | integration: full_pipeline_*, tunnel_update_*, setup_roguerepo_* |
| f15 | execute | compute | integration: full_pipeline_* |
| f20 | full_pipeline | intent | intent::f62_full_pipeline, integration |
| f21 | tunnel_update | intent | intent::f62_tunnel_update, integration |
| f22 | setup_roguerepo | intent | intent::f62_setup_roguerepo, integration |
| f25 | load_backlog | backlog | backlog::f25_roundtrip, integration::load_backlog_roundtrip |
| f62 | parse_intent | intent | intent::f62_* |
| f73 | store_message | context | (via storage roundtrip) |
| f74 | load_messages | context | (via storage roundtrip) |
| f76 | stream | inference | — |
| f78 | model_path_for_role | config | config::model_role_default_filename |
| f79 | classify | router | router::parse_*, router::clarification_* |
| f80 | chat_complete | inference | — |
| f60 | triple_sims_run | exopack | triple_sims::sim1*, sim2*, sim3*, report* |
| f61 | run_cargo_test_n | exopack | — |
| f170 | sim1_user_story | exopack | triple_sims::sim1_runs_without_panic |
| f171 | sim2_feature_gap | exopack | triple_sims::sim2_runs_without_panic |
| f172 | sim3_impl_deep_dive | exopack | triple_sims::sim3_modules_exist |
| f81 | run_code_gen_pipeline | pipeline | pipeline::cargo_check_clippy_test_* |
| f82 | load_project_context | context_loader | context_loader::f82_* |
| f86 | recent_changes_snapshot | recent_changes | recent_changes::f86_* |
| f87 | format_recent_changes | recent_changes | recent_changes::f87_* |
| f91 | cargo_check | pipeline | — |
| f92 | cargo_clippy | pipeline | — |
| f93 | cargo_test | pipeline | — |
| f90 | kova_test | bin/kova-test | clippy, TRIPLE SIMS (f61), release build, bootstrap smoke, serve smoke (exopack http_client) |
| f83 | target_file_hint | context_loader | context_loader::f83_target_hint |
| f84 | format_diff | output | output::f84_diff |
| f85 | resolve_target_path | output | output::f85_resolve_target |
| f39 | open | storage | storage::store_* |
| f40 | put_compressed | storage | storage::store_put_get_compressed_roundtrip |
| f41 | get_compressed | storage | storage::store_* |
| f42 | put_raw | storage | storage::store_put_raw_get_raw |
| f43 | get_raw | storage | storage::store_put_raw_get_raw |
| f77 | model_install | model | — |
| f94 | default_project | config | — |
| f95 | discover_projects | config | — |
| f96 | projects_root | config | (internal) |
| f97 | home | config | — |
| f98 | kova_dir | config | — |
| f99 | prompts_dir | config | — |
| f100 | sled_path | config | — |
| f101 | models_dir | config | — |
| f102 | inference_model_path | config | — |
| f103 | backlog_path | config | — |
| f104 | workspace_root | config | — |
| f105 | load_build_preset | config | — |
| f106 | all_build_presets | config | — |
| f107 | infer_preset_name | config | — |
| f108 | bind_addr | config | — |
| f109 | bootstrap | config | — |
| f110 | load_prompt | config | — |
| f111 | load_cursor_prompts | cursor_prompts | cursor_prompts::* |
| f112 | format_context | context_loader | context_loader::* |
| f113 | gui_run | gui | — |
| f114 | serve_run | serve | — |
| f115 | explain_trace | academy | — |
| f116 | fix_and_retry | pipeline | — |
| f117 | run_cargo | pipeline/compilation | — |
| f118 | categorize | pipeline/error_kind | — |
| f119 | kova_c2_run | c2 | CLI orchestration (kova c2 run) |
| f120 | kova_c2_broadcast | c2 | SSH broadcast to workers |
| f121 | autopilot_run | autopilot | Type prompt into Cursor composer |
| f122 | nstat | node_cmd | Hostname, uptime, load (c1) |
| f123 | nspec | node_cmd | CPU, RAM, disk, rust version (c2) |
| f124 | nsvc | node_cmd | Running services (c3) |
| f125 | nrust | node_cmd | Check/install Rust toolchain (c4) |
| f126 | nsync | node_cmd | Rsync project to nodes (c5) |
| f127 | nbuild | node_cmd | Remote cargo build (c6) |
| f128 | nlog | node_cmd | Tail journalctl (c7) |
| f129 | nkill | node_cmd | Kill process by name (c8) |
| f130 | ndeploy | node_cmd | Sync + build + restart (c9) |
| f131 | nci | node_cmd | Compact inspect, one-line-per-node (ci) |
| f132 | node_cmd_dispatch | node_cmd | Central dispatcher for c1-c9/ci |
| f133 | cargo_exec | cargo_cmd | Execute single cargo command, compressed output |
| f134 | cargo_exec_multi | cargo_cmd | Run on multiple projects in parallel |
| f135 | cargo_exec_chain | cargo_cmd | Sequential commands, stop on error |
| f136 | cargo_cmd_dispatch | cargo_cmd | Central dispatcher for x0-x9 |
| f137 | repl_run | repl | Interactive REPL entry point |
| f138 | repl_stream_print | repl | Stream tokens to stdout |
| f139 | repl_build_system_prompt | repl | Assemble system prompt from all sources |
| f140 | parse_tool_calls | tools | Extract tool calls from LLM output |
| f141 | dispatch_tool | tools | Execute tool call, return result |
| f142 | tool_read_file | tools | Read file with line numbers |
| f143 | tool_write_file | tools | Write file, create dirs |
| f144 | tool_edit_file | tools | String replacement edit |
| f145 | tool_bash | tools | Execute shell command |
| f146 | tool_glob | tools | Glob file search |
| f147 | agent_turn | agent_loop | Single agent turn: inference → tools |
| f148 | agent_loop | agent_loop | Outer loop until done or max iterations |
| f149 | format_tool_prompt | tools | Format tool defs for system prompt |
| f150 | tool_grep | tools | Search file contents |
| f155 | tool_memory_write | tools | Append to persistent memory |
| f156 | git_exec | git_cmd | Execute git command, return compressed |
| f157 | compress_status | git_cmd | M/A/D/? + short path |
| f158 | compress_diff | git_cmd | +N/-N summary, file:line +/-content |
| f159 | compress_log | git_cmd | Truncate hashes to 7 chars |
| f160 | git_cmd_dispatch | git_cmd | Central dispatcher for g0-g9 |

## Types (tN)

| Token | Human name |
|-------|------------|
| t0 | Intent |
| t1 | IntentKind |
| t3 | Plan |
| t5 | ActionKind |
| t6 | Executor |
| t12 | Store |
| t86 | RecentChange |
| t88 | BuildPreset |
| t89 | ModelRole |
| t90 | ProjectContext |
| t91 | Message |
| t92 | AppState |
| t93 | LastTrace |
| t94 | RouterResult |
| t95 | ErrorKind |
| t96 | NodeCmd |
| t97 | NodeResult |
| t99 | CargoCmd |
| t100 | CargoResult |
| t101 | ToolDef |
| t102 | ToolParam |
| t103 | ToolCall |
| t104 | ToolResult |
| t106 | AgentAction |
| t107 | GitCmd |
| t108 | GitResult |

## Struct fields (sN) — plan t3

| Token | Human name | Type |
|-------|------------|------|
| s4 | project | PathBuf |
| s5 | approuter_dir | Option<PathBuf> |
| s7 | project_hint | Option<String> |
| s14 | node_token | String (n0..n3) |
| s15 | success | bool |
| s16 | output_fields | Vec<(&str, String)> |

## Node Command Tokens

### Nodes (nN)

| Token | Hostname | Host |
|-------|----------|------|
| n0 | kova-legion-forge | lf |
| n1 | kova-tunnel-god | gd |
| n2 | kova-thick-beast | bt |
| n3 | kova-elite-support | st |

### Commands (cN)

| Token | Function | Purpose |
|-------|----------|---------|
| c1 | f122 | nstat: hostname, uptime, load |
| c2 | f123 | nspec: cpu, ram, disk, rust |
| c3 | f124 | nsvc: running services |
| c4 | f125 | nrust: check/install rust |
| c5 | f126 | nsync: rsync project |
| c6 | f127 | nbuild: remote cargo build |
| c7 | f128 | nlog: tail journalctl |
| c8 | f129 | nkill: kill process |
| c9 | f130 | ndeploy: sync+build+restart |
| ci | f131 | compact inspect |

### Output Fields (oN)

| Token | Field |
|-------|-------|
| o0 | node (nN token) |
| o1 | hostname |
| o2 | uptime |
| o3 | load |
| o4 | cpu cores |
| o5 | ram |
| o6 | disk free |
| o8 | rust version |
| o9 | services |
| o10 | status |
| o11 | message |

## Git Command Tokens

| Token | Function | Purpose |
|-------|----------|---------|
| g0 | f157 | status: M/A/D/? + path |
| g1 | f158 | diff: +N/-N, file:line changes |
| g2 | f159 | log --oneline, 7-char hashes |
| g3 | — | push (silent on success) |
| g4 | — | pull ("current" or "+N files") |
| g5 | — | commit -m |
| g6 | — | branch list, * = current |
| g7 | — | stash |
| g8 | — | add (files or -A) |
| g9 | f158 | diff --staged |

## Test Coverage by Module

| Module | Tests | What they hit |
|--------|-------|---------------|
| cargo_cmd | 13 | JSON parse (E/W/artifact/cap/path), text fallback, project resolve, token parse, supports_json |
| serve | 9 | /api/status, /project, /projects, /prompts, /build/presets, /api/file 404, / HTML, /context/recent, safe_hint |
| node_cmd | 8 | resolve_node, to_token, headers_for, expand_header, print_oneline, node_map |
| git_cmd | 5 | status compress, diff compress, log compress, path strip, status clean |
| tools | 5 | parse JSON block, bare JSON, no tool calls, read file, edit file |
| storage | 3 | compressed roundtrip, raw roundtrip, missing key |
| router | 3 | parse clarification (with/without question), canned |
| pipeline | 2 | check+clippy+test on valid lib, fix loop syntax error |
| recent_changes | 3 | empty, includes recent, format |
| cursor_prompts | 1 | load includes baked |
| gui | 1 | build system prompt includes baked |
| **Total** | **88** | |

## Test traceability

Each test has `/// fN=human_name` doc comment linking to the function under test.
Run `rg '/// f[0-9]+=' kova/src` to list coverage.

### Macros

- **kova_test!(fN, test_name, { body })** — Adds `#[test]` and `/// fN=traceability` doc. Use for unit tests.
- **assert_matches!** (dev-dep) — Pattern assertion; cleaner than `match { X => ..., _ => panic!() }`.

### recent_changes (f86/f87)

**Stay on task.** Polls project for files modified in last N minutes. Tokenized output for LLM context.

- **CLI:** `kova recent [--project DIR] [--minutes 30]`
- **HTTP:** `GET /context/recent?minutes=30` (serve mode)
- **Pipeline:** f82_with_recent injects recent changes into code-gen prompt (30 min default)

### exopack (critical)

**kova-test** binary (f90) depends on exopack for TRIPLE SIMS via f61. Main binary uses plain `cargo test` (f93); no exopack in main.

### Robustness

- **f62_table_driven** — Single table of (input, expected); add rows to extend coverage.
- **f62_precedence_*** — Explicit precedence tests (full vs test, cloudflare vs cache).
- **extract_rust_block** — Edge cases: multiple blocks, empty, non-rust, unclosed.
- **context_loader** — src vs root, nonexistent file, underscores in filename.
- **Integration** — tunnel/setup use temp dirs (no ~/approuter). full_pipeline_on_rogue_repo is `#[ignore]`.
