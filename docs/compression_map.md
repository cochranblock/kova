<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

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
| f170 | estimate_tokens | context_mgr | context_mgr::estimate_tokens_* |
| f171 | trim_conversation | context_mgr | context_mgr::trim_conversation_* |
| f172 | trim_tool_output | context_mgr | context_mgr::trim_tool_output_* |
| f173 | summarize_old_turns | context_mgr | context_mgr::summarize_* |
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
| f161 | log_llm | trace | Log LLM call to sled |
| f162 | recent_llm_traces | trace | Query recent LLM traces |
| f163 | llm_stats | trace | Aggregate LLM stats |
| f164 | print_llm_stats | trace | Print formatted LLM stats |
| f165 | print_recent_traces | trace | Print recent traces table |
| f166 | tool_rag_search | tools | RAG search tool for agent loop |
| f167 | needs_reindex | rag | Check if dir has .rs files modified since last index |
| f168 | mark_indexed | rag | Record dir indexed timestamp in sled |
| f169 | auto_reindex | rag | Check + reindex if needed, return chunk count |
| f189 | generate_image | imagegen | Dispatch to provider |
| f190 | generate_sd | imagegen | Stable Diffusion WebUI txt2img |
| f191 | generate_dalle | imagegen | OpenAI DALL-E generations |
| f192 | save_image | imagegen | Write image bytes to file |
| f193 | list_sd_models | imagegen | List SD models |
| f174 | mcp_tools_list | mcp | Return kova tools in MCP format |
| f175 | mcp_handle_request | mcp | Parse JSON-RPC request, dispatch to tool, return response |
| f176 | mcp_stdio_loop | mcp | Stdin/stdout MCP server loop |
| f177 | ci_check | ci | Run cargo check, optionally clippy and tests |
| f178 | ci_watch | ci | Poll loop: detect file changes, run ci_check |
| f179 | ci_once | ci | Single CI run with default config |
| f180 | print_ci_result | ci | Formatted CI output with pass/fail status |
| f181 | export_from_traces | training_data | Read LLM traces from sled, export as training data |
| f182 | export_jsonl | training_data | Write training examples as JSONL |
| f183 | export_csv | training_data | Write training examples as CSV |
| f184 | export_dpo_pairs | training_data | Build DPO chosen/rejected pairs from scored examples |
| f194 | record_failure | feedback | Store challenge failure in sled |
| f195 | recent_failures | feedback | Query recent failure records |
| f196 | generate_challenge_from_failure | feedback | LLM-generate harder variant from failure |
| f197 | export_generated_challenges | feedback | Format generated challenges as tce() Rust code |
| f198 | feedback_stats | feedback | Count failures by model, event type, challenge type |
| f185 | review_diff | review | Send diff to LLM for code review |
| f186 | review_staged | review | Review currently staged changes |
| f187 | review_branch | review | Review diff between current branch and base |
| f188 | format_review | review | Human-readable formatted review output |
| f199 | provider_generate | providers | Generate text from any provider |
| f200 | provider_list | providers | Load configured providers from config |
| f201 | extract_symbols | syntax | Extract all symbols from Rust source |
| f202 | extract_functions | syntax | Extract only function symbols |
| f203 | extract_structs | syntax | Extract struct and enum symbols |
| f204 | extract_impls | syntax | Extract impl blocks |
| f205 | format_outline | syntax | Format symbols as code outline |
| f206 | outline_file | syntax | Parse file and return outline |
| f207 | orchestration_router_resident | config | Router stays loaded |
| f208 | orchestration_specialist_idle_unload_secs | config | Specialist idle timeout |
| f209 | model_cache_size | config | Max models in memory |
| f210 | code_gen_structured | config | Grammar-constrained Coder output |
| f211 | router_structured | config | Grammar-constrained Router output |
| f212 | model_idle_unload_secs | config | Model idle eviction secs |
| f213 | orchestration_max_fix_retries | config | DDI fix loop cap |
| f214 | orchestration_run_clippy | config | Run clippy in fix loop |
| f215 | cursor_prompts_enabled | config | Inject Cursor rules/skills |
| f216 | ollama_url | config | Ollama base URL |
| f217 | default_model | config | Default model for review/feedback |
| f218 | hive_local_base | config | Hive sync-to-local path |
| f219 | hive_shared_base | config | Hive NFS shared path |
| f220 | fast_localhost | config | Skip TLS on loopback |
| f296 | error_block_with_context | pipeline/error_kind | Build error block with categorized context |
| f297 | run_factory | factory | Run factory pipeline |
| f298 | run_init | ssh_ca | Create CA key and known_hosts entry |
| f299 | run_sign | ssh_ca | Sign host key with CA |
| f300 | run_setup | ssh_ca | Full CA setup for node |
| f301 | run_academy | academy | Run academy: plan, execute, verify, commit |
| f302 | format_question | elicitor | Format question with optional choices |
| f303 | parse_reply | elicitor | Parse user input into ElicitorReply |
| f304 | build_restatement | elicitor | Build "I'll do X in Y. Proceed?" |
| f305 | run_gauntlet | gauntlet | Run gauntlet challenges |
| f306 | run_cargo | cargo | Run cargo with args, return (success, stderr) |
| f307 | extract_error_key | cargo | Extract core error identifier for loop detection |
| f308 | truncate | cargo | Truncate string to max chars |
| f309 | extract_rust_block | cargo | Extract first ```rust block from text |
| f310 | prompt_wants_binary | cargo | Detect if prompt asks for binary |
| f311 | build_system_prompt | cargo | Build system prompt for code gen |
| f312 | write_temp_project | cargo/sandbox | Write temp Cargo project |
| f313 | write_validation_project | cargo/sandbox | Write validation project |
| f314 | write_temp_crate | cargo/sandbox | Write simple temp lib crate |
| f315 | run_test_suite | lib | Deploy quality gate |
| f316 | from_str_loose | training_data | Parse format string loosely |
| f317 | extension | training_data | File extension for format |
| f318 | default_output_dir | training_data | Default output directory |
| f320 | apply | theme | Apply full theme to egui context |
| f321 | message_frame | theme | Message frame style |
| f322 | input_frame | theme | Input frame style |
| f323 | panel_frame | theme | Panel frame style |
| f325 | intent_name | intent | Intent name for display |
| f326 | now_ms | trace | Current timestamp in millis |
| f327 | extract_rust_block | codegen/helpers | Delegate to cargo f309 |
| f328 | build_system_prompt | codegen/helpers | Delegate to cargo f311 |
| f329 | prompt_wants_binary | codegen/helpers | Delegate to cargo f310 |
| f330 | truncate | codegen/helpers | Delegate to cargo f308 |
| f331 | fix_and_retry_cluster | codegen/fix_loop | Fix via cluster dispatch |
| f333 | default_provider | providers | Default provider selection |
| f334 | provider_health | providers | Check provider health |
| f335 | provider_version | providers | Get provider version |
| f336 | provider_list_models | providers | List models on provider |
| f337 | provider_generate_stream | providers | Stream generation from provider |
| f338 | quick_gen | cluster | Quick code generation dispatch |
| f339 | quick_review | cluster | Quick code review dispatch |
| f340 | quick_fix | cluster | Quick fix dispatch |
| f341 | run_moe | moe | Run MoE pipeline |
| f342 | embed_texts | rag | Generate embeddings for texts |
| f343 | embed_query | rag | Generate query embedding |
| f344 | chunk_rust_file | rag | Chunk a Rust file for indexing |
| f345 | index_directory | rag | Index all .rs files in directory |
| f346 | format_context | rag | Format search results as LLM context |
| f347 | to_intent | c2 | Convert c2 token to intent |
| f348 | name | c2 | Token name string |
| f349 | is_local_only | c2 | Check if token is local-only |
| f350 | default_nodes | c2 | Default node list |
| f351 | node_mac | c2 | MAC address for Wake-on-LAN |
| f352 | wake_node | c2 | Send WoL magic packet |
| f353 | resolve_project | c2 | Resolve project path for node |
| f354 | run_command | c2 | Run c2 command on nodes |
| f355 | run_nodes | c2 | Run command across nodes |
| f356 | run_build | c2 | Sync + build on nodes |
| f357 | sync_parallel | c2 | Parallel sync to nodes |
| f358 | run_sync | c2 | Run sync operation |
| f359 | run_inspect | inspect | Run c2 inspect |
| f360 | print_table | inspect | Print resource table |
| f361 | print_recommend | inspect | Print recommendations |
| f362 | print_json | inspect | Print JSON output |
| f363 | clarification_question | router | Get clarification question |
| f364 | clarification_choices | router | Get clarification choices |
| f365 | use_coder | router | Check if coder model needed |

## Types (TN)

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
| T90 | ProjectContext |
| t91 | Message |
| T92 | AppState |
| T93 | LastTrace |
| T94 | RouterResult |
| T95 | ErrorKind |
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
| T109 | LlmTrace |
| T110 | LlmStats |
| t111 | ContextBudget |
| T112 | McpRequest |
| T113 | McpResponse |
| t114 | CiConfig |
| t115 | CiResult |
| T116 | TrainingExample |
| T117 | ExportFormat |
| T118 | ReviewRequest |
| T119 | ReviewResult |
| T120 | ReviewIssue |
| T121 | Severity |
| T122 | ImageProvider |
| T123 | ImageRequest |
| T124 | ImageResult |
| T125 | ImageFormat |
| T126 | FailureRecord |
| T127 | GeneratedChallenge |
| T128 | FeedbackStats |
| T129 | Provider |
| T130 | ProviderConfig |
| T131 | ProviderResponse |
| T132 | Symbol |
| T133 | SymbolKind |
| T175 | DpoPair |
| T176 | KovaError |
| T177 | ElicitorReply |
| T178 | FactoryResult |
| T179 | StageResult |
| T180 | FactoryConfig |
| T181 | Factory |
| T182 | Step |
| T183 | StepAction |
| T184 | StepStatus |
| T185 | AcademyConfig |
| T186 | AcademyResult |
| T187 | GauntletReport |
| T188 | ModelInfo |
| T189 | ModelTier |
| T190 | NodeRole |
| T191 | TaskKind |
| T192 | InferNode |
| T193 | Cluster |
| T194 | ExpertVariant |
| T195 | MoeResult |
| T196 | MoeConfig |
| T197 | Chunk |
| T198 | SearchResult |
| T199 | RagStats |
| T200 | VectorStore |
| T201 | Mode |
| T202 | Verdict |
| T203 | VisualQc |
| T204 | MsgKind |
| T205 | HostInfo |
| T206 | KovaStream |
| T207 | KovaCommand |
| T208 | KovaKernel |
| T209 | KernelConfig |
| T210 | McpError |
| T211 | StrategyConfig |
| T212 | Token (c2) |
| T213 | SpriteQc |

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
| context_mgr | 11 | estimate_tokens, trim_conversation, trim_tool_output, budget_remaining, summarize |
| mcp | 7 | tools_list, initialize, tools/list, tools/call dispatch, unknown method, parse error, tool error |
| ci | 4 | valid project passes, broken code fails, result formatting, ci_once returns result |
| review | 6 | format_review readable, severity ordering, empty issues, parse response, score clamp, fallback summary |
| training_data | 4 | csv_escape, export_jsonl, export_csv, export_dpo_pairs |
| feedback | 5 | record/retrieve roundtrip, stats counts, category mapping, export tce calls, parse challenge |
| imagegen | 6 | request defaults, format extensions, base64 roundtrip, dalle size, save image temp, save creates dirs |
| providers | 3 | default ollama, config serde roundtrip, response fields |
| syntax | 8 | basic fn, struct+impl, enum, trait, async fn, outline lines, empty source, kind short |
| **Total** | **153** | |

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