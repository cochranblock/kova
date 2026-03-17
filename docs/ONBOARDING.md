# Onboarding Guide for Kova

Welcome to Kova! This guide will help you set up your local environment, understand the project structure, and get productive quickly.

## Quick Start (5 minutes)

### 1. Clone the Repository
```bash
git clone https://github.com/cochranblock/kova.git
cd kova
```

### 2. Install Rust (if needed)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 3. Build & Test
```bash
cargo build
cargo test
```

### 4. Install Shell Aliases
Copy `.kova-aliases` to your shell profile:
```bash
echo 'source ~/kova/.kova-aliases' >> ~/.zshrc
source ~/.zshrc
```

Now you can use short commands: `kx0` (build), `kt` (test), `kg` (GUI).

---

## Understanding Kova

### Project Structure
```
kova/                          # Rust workspace root
├── Cargo.toml                 # Workspace manifest
├── src/
│   ├── main.rs               # CLI entrypoint (504 lines)
│   ├── repl.rs               # Interactive REPL (161 lines)
│   ├── agent_loop.rs         # Agentic tool loop (117 lines)
│   ├── tools.rs              # 7 tools: read/write/edit/bash/glob/grep/memory (574 lines)
│   ├── node_cmd.rs           # Tokenized SSH commands (c1-c9, ci) (557 lines)
│   ├── cargo_cmd.rs          # Tokenized cargo wrapper (x0-x9) (515 lines)
│   ├── git_cmd.rs            # Tokenized git commands (g0-g9)
│   ├── c2.rs                 # Swarm orchestration (675 lines)
│   ├── serve.rs              # Axum HTTP + WebSocket (782 lines)
│   ├── gui.rs                # egui desktop GUI (834 lines)
│   ├── inference.rs          # Local LLM via Kalosm (156 lines)
│   ├── config.rs             # Config & paths (641 lines)
│   ├── router.rs             # Intent classification (262 lines)
│   ├── context_loader.rs     # Project context loading (226 lines)
│   └── ...
├── kova-web/                 # WASM thin client
├── kova-core/                # Core library (WASM-safe)
├── tests/                    # Integration tests
├── docs/                     # Documentation
├── .kova-aliases             # 97 shell aliases
└── README.md                 # Architecture overview

Total: ~8,247 lines of Rust + shell
```

### Tech Stack
- **Language:** Rust 1.70+
- **Async Runtime:** Tokio
- **Web Framework:** Axum (HTTP), egui (GUI)
- **Storage:** sled (embedded k/v store)
- **Local LLM:** Kalosm (Qwen2.5-Coder GGUF)
- **Serialization:** bincode + zstd
- **Error Handling:** thiserror
- **Testing:** Built-in (no external framework)

### Binaries
- **kova** — All-inclusive binary (CLI + GUI + HTTP + LLM + swarm)
- **kova-test** — Quality gate binary (compiles, tests, clippy, smoke tests)

---

## Tokenization: The Unique Part

Kova uses **compressed identifiers** to reduce token consumption when feeding code to LLMs. This is critical to understand:

### Why Tokenization?
When you submit code to Claude/GPT-5, token count matters. Instead of:
```rust
fn process_user_request(user_id: u64) -> Result<Response, Error> { ... }
```

Kova writes:
```rust
fn f14(s5: u64) -> t0<t1> { ... }
```

**Benefit:** Saves tokens → more code in same context window → better reasoning → faster iteration.

### The Map
See `docs/compression_map.md` for the canonical reference. Excerpt:
```
f0 = main
f14 = process_user_request
f79 = router_intent
s5 = user_id
t0 = Response
t1 = Error
c1 = node status (nstat)
x0 = cargo build
p0 = cd kova
n0 = kova-legion-forge
```

### Important Rules
1. **Before coding:** Check if your function/type/field is already in the map
2. **After coding:** Add new identifiers to the map immediately
3. **When reviewing:** Verify all new IDs are documented

### Example: Adding a New Function
```rust
// NEW: Message router with caching
fn f81(ctx: &t47) -> t48 {
    // ...
}

// Update docs/compression_map.md:
f81 = route_message_cached
t47 = RouterContext
t48 = RouteResult
```

---

## Shell Aliases Cheat Sheet

### Kova CLI
```bash
k       # kova (CLI)
kc      # kova chat (REPL)
ks      # kova serve + open browser
kg      # kova gui
kt      # kova test
kb      # kova bootstrap
kx      # kova x (cargo tokens)
```

### Cargo Tokens (x0-x9)
```bash
kx0  # cargo build
kx1  # cargo check
kx2  # cargo test
kx3  # cargo clippy
kx4  # cargo run
kx5  # cargo build --release
kx6  # cargo clean
kx7  # cargo doc
kx8  # cargo fmt --check
kx9  # cargo bench
```

### Node Commands (c1-c9, ci)
```bash
kn1   # node status (nstat)
kn2   # node specs (nspec)
kn3   # node services (nsvc)
kn4   # rustup status (nrust)
kn5   # sync to all (nsync)
kn6   # build (nbuild)
kn7   # logs (nlog)
kn8   # kill (nkill)
kn9   # deploy (ndeploy)
knci  # compact inspect
```

### Git Tokens (g0-g9)
```bash
kg0   # git status
kg1   # git diff
kg2   # git log
kg3   # git push
kg4   # git pull
kg5   # git commit
kg6   # git branch
kg7   # git stash
kg8   # git add
kg9   # git staged
```

### Project Navigation (p0-p9)
```bash
p0    # cd kova
p1    # cd approuter
p2    # cd cochranblock
p3    # cd oakilydokily
p4    # cd rogue-repo
p5    # cd ronin-sites
p6    # cd kova-core
p7    # cd exopack
p8    # cd whyyoulying
p9    # cd wowasticker
```

### Compound Aliases
```bash
p0b   # cd kova && cargo build
kc2b  # kova c2 ncmd broadcast-build (all nodes)
kc2s  # kova c2 ncmd sync-all
```

---

## Worker Nodes (Optional)

If you have access to the cluster:

| Token | Host | Hostname | Role |
|-------|------|----------|------|
| n0 | lf | kova-legion-forge | Primary build |
| n1 | gd | kova-tunnel-god | Tunnel/relay + deploy target |
| n2 | bt | kova-thick-beast | Heavy compute |
| n3 | st | kova-elite-support | Support/backup |

### SSH Setup
```bash
# Initialize SSH CA
kova ssh-ca init

# Sign your local key
kova ssh-ca sign ~/.ssh/id_rsa.pub

# Deploy to all nodes
kova c2 ncmd ndeploy
```

### Remote Commands
```bash
# Check status of all nodes
kova c2 ncmd nstat

# Get specs for specific nodes
kova c2 ncmd nspec --nodes n1,n2

# Broadcast a build
kova c2 ncmd nbuild
```

---

## Local LLM (Inference)

Kova includes a local LLM using Kalosm with Qwen2.5-Coder (500M params):

```bash
# Start the LLM server
kova serve

# In another terminal, chat with the LLM
kova chat

# Or use GUI
kg
```

**Default model:** Qwen2.5-Coder-0.5B (~1.5 GB, good for CPU inference)

See `src/inference.rs` for details.

---

## Development Workflow

### 1. Check Tokenization Map
Before you code, check if your function/type/field is already mapped:
```bash
grep -i "my_function" docs/compression_map.md
```

### 2. Create a Branch
```bash
git checkout -b feature/add-webhook-support
```

### 3. Write Code & Tests
```bash
cargo test --lib             # Unit tests
cargo test --test '*'        # Integration tests
cargo clippy                 # Linting
cargo fmt                    # Format
```

### 4. Update Compression Map
If you added new functions/types, add them to `docs/compression_map.md`:
```
f160 = add_webhook_support
t101 = WebhookConfig
```

### 5. Commit with Copilot Trailer
```bash
git commit -m "feat: Add webhook support

- Implement WebhookHandler trait
- Add config option for webhooks
- Include unit and integration tests

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

### 6. Push & Open PR
```bash
git push origin feature/add-webhook-support
```

---

## Running Kova

### CLI (REPL)
```bash
kova
# Type: read src/main.rs
# Type: explain this code
# Type: exit
```

### GUI (Desktop App)
```bash
kg
# Opens egui window with agent loop visualization
```

### HTTP Server + Web Client
```bash
ks
# Starts server on http://localhost:8080
# Opens browser automatically
```

### Local LLM + Agent Loop
```bash
kova chat
# Starts REPL with local inference
# Try: "Build the kova project"
```

---

## Testing

### Run All Tests
```bash
cargo test
```

### Run Specific Test
```bash
cargo test agent_loop
```

### Run Integration Tests
```bash
cargo test --test '*'
```

### Watch for Changes (requires cargo-watch)
```bash
cargo install cargo-watch
cargo watch -x test
```

### Quality Gate (Full CI)
```bash
cargo run -p kova --bin kova-test --features tests
```

Runs: compile → clippy → unit tests → integration tests → HTTP tests → exit code.

---

## Troubleshooting

### "command not found: kova"
Make sure aliases are sourced:
```bash
source ~/.kova-aliases
```

### Build fails with "error: failed to fetch"
Clean and retry:
```bash
cargo clean
cargo build
```

### Local LLM is slow
Check RAM and CPU. The default model needs ~2GB RAM. For faster inference:
1. Use a smaller model (e.g., tinyllama)
2. Run on n2 (heavy compute node)
3. Enable GPU support if available

### Tests hang
Increase timeout or run with single thread:
```bash
cargo test -- --test-threads=1
```

### Node commands don't work
Ensure SSH keys are set up and you have access to the cluster:
```bash
ssh lf hostname  # Test connection
kova ssh-ca init  # Initialize SSH CA
```

---

## Documentation

- **README.md** — Project overview, architecture, tokenization map
- **CONTRIBUTING.md** — PR process, commit conventions, code style
- **docs/ARCHITECTURE.md** — Deep dive into architecture
- **docs/compression_map.md** — Tokenization reference (MUST READ)
- **docs/RUNBOOKS.md** — Common tasks & incident response
- **docs/TOURNAMENT_RESULTS.md** — LLM performance benchmarks

---

## Getting Help

- **Slack:** Post in #team-kova or #dev
- **GitHub Discussions:** https://github.com/cochranblock/kova/discussions
- **GitHub Issues:** https://github.com/cochranblock/kova/issues
- **Code of Conduct:** See CONTRIBUTING.md

---

## What's Next?

1. ✅ Set up local environment
2. ✅ Read `docs/compression_map.md`
3. ✅ Run `cargo test` to ensure everything works
4. ✅ Try `kg` to see the GUI
5. ✅ Try `kova chat "Build the project"` to see the agent loop
6. ⬜ Check out a simple issue and open a PR
7. ⬜ Get added to the on-call rotation (after 2-3 PRs)

Welcome to the team! 🚀
