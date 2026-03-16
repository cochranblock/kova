<p align="center">
  <img src="https://raw.githubusercontent.com/cochranblock/kova/main/assets/logo.png" alt="Kova" width="120">
</p>

# Kova

Augment engine. Local LLM agentic tool loop, swarm orchestration, tokenized everything.

## Proof of Artifacts

*Wire diagrams for quick review.*

### Wire / Architecture

```mermaid
flowchart TB
    User[User] --> GUI[GUI / egui]
    User --> Serve[HTTP / serve]
    GUI --> AgentLoop[Agent loop]
    Serve --> AgentLoop
    AgentLoop --> Router[Router]
    Router --> Infer[Inference]
    Router --> Cargo[cargo_cmd]
    Router --> Node[node_cmd]
    AgentLoop --> C2[c2 swarm]
    C2 --> Nodes[lf gd bt st]
```

---

## Artifacts

| Artifact | What | Lines |
|----------|------|-------|
| `src/main.rs` | CLI entrypoint. 15 subcommands incl. tokenized `s`/`g` short forms | 504 |
| `src/repl.rs` | Interactive REPL. Claude Code replacement with local LLM | 161 |
| `src/agent_loop.rs` | Agentic tool loop: inference → parse → execute → feed back | 117 |
| `src/tools.rs` | 7 tools: read, write, edit, bash, glob, grep, memory_write | 574 |
| `src/node_cmd.rs` | Tokenized SSH node commands (c1-c9, ci). Parallel execution | 557 |
| `src/cargo_cmd.rs` | Tokenized cargo wrapper (x0-x9). Compressed output for AI context | 515 |
| `src/c2.rs` | Swarm orchestration: sync, build, broadcast to 4 worker nodes | 675 |
| `src/serve.rs` | Axum HTTP API + WebSocket streaming + embedded web client | 782 |
| `src/gui.rs` | Native egui desktop GUI | 834 |
| `src/config.rs` | Config, paths, feature detection, model resolution | 641 |
| `src/inference.rs` | Local LLM inference via Kalosm (Qwen2.5-Coder GGUF) | 156 |
| `src/router.rs` | Intent classification and routing | 262 |
| `src/inspect.rs` | Resource inspection: CPU, RAM, disk, GPU across all nodes | 282 |
| `src/cursor_prompts.rs` | Load .cursorrules + .cursor/rules/*.mdc as system prompt | 241 |
| `src/context_loader.rs` | Project context: Cargo.toml, recent changes, git state | 226 |
| `src/ssh_ca.rs` | SSH host CA: init, sign, setup. No host key churn | 163 |
| `src/web.rs` | Embedded kova-web (egui WASM thin client) | 72 |
| `src/storage.rs` | sled k/v store with zstd + bincode | 119 |
| `.kova-aliases` | 97 shell aliases for macOS + Debian. Deployed to all nodes | 160 |
| **Total** | **8,087 lines Rust + 160 lines shell** | **8,247** |

## Binaries

| Binary | Features | Purpose |
|--------|----------|---------|
| `kova` | gui, serve, inference | All-inclusive: REPL, GUI, HTTP, LLM, swarm, tools |
| `kova-test` | tests (exopack) | Quality gate: clippy, TRIPLE SIMS 3x, release build, smoke |

## Tokenized Command Map

### Shell Aliases (`.kova-aliases`)

```
k     = kova              ks    = serve + open browser
kc    = chat (REPL)       kg    = gui
kt    = test              kb    = bootstrap
kx0-9 = cargo tokens      kn1-9 = node tokens
kc2b  = broadcast build   kc2s  = sync all
p0-p9 = cd to project     p0b   = cd + build
```

### Cargo Tokens (`kova x`)

| Token | Command | Token | Command |
|-------|---------|-------|---------|
| x0 | build | x5 | build --release |
| x1 | check | x6 | clean |
| x2 | test | x7 | doc |
| x3 | clippy | x8 | fmt --check |
| x4 | run | x9 | bench |

### Node Tokens (`kova c2 ncmd`)

| Token | Command | Token | Command |
|-------|---------|-------|---------|
| c1 | nstat (status) | c6 | nbuild |
| c2 | nspec (specs) | c7 | nlog |
| c3 | nsvc (services) | c8 | nkill |
| c4 | nrust (rustup) | c9 | ndeploy |
| c5 | nsync | ci | compact inspect |

### Project Tokens

| Token | Project | Token | Project |
|-------|---------|-------|---------|
| p0 | kova | p5 | ronin-sites |
| p1 | approuter | p6 | kova-core |
| p2 | cochranblock | p7 | exopack |
| p3 | oakilydokily | p8 | whyyoulying |
| p4 | rogue-repo | p9 | wowasticker |

## Micro Olympics

Local LLM tournament across the cluster. 42 competitors, 6 events, 45 challenges.

**Champion: qwen2.5-coder:0.5b** (500M params, 91% accuracy, 6/7 gold medals)

Full results: [`docs/TOURNAMENT_RESULTS.md`](docs/TOURNAMENT_RESULTS.md)

Run: `kova micro tournament`

## Worker Nodes

| Token | Host | Role |
|-------|------|------|
| n0/lf | kova-legion-forge | Primary build |
| n1/gd | kova-tunnel-god | Tunnel/relay |
| n2/bt | kova-thick-beast | Heavy compute |
| n3/st | kova-elite-support | Support/backup |

## Features

```toml
default  = ["serve", "inference", "rag"]
serve    = axum + tower + tracing (+ kova-web WASM, built automatically)
gui      = eframe + egui (native desktop)
inference = kalosm + reqwest + lru
autopilot = enigo (type into Cursor)
daemon   = capnp (worker node)
tests    = exopack (quality gate)
```

## Build

```sh
cargo build                          # default (serve + inference + rag)
cargo build --release --features serve --target aarch64-apple-darwin
cargo run --features tests --bin kova-test   # quality gate
```

**Serve (web GUI):** Builds kova-web (egui→WASM) automatically. Requires:
`rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli`.

If using a workspace, remove `kova/kova-web` from `members` — kova-web is built by kova's build script.

## Setup

```sh
# Install aliases (macOS)
echo '[ -f "$HOME/.kova-aliases" ] && . "$HOME/.kova-aliases"' >> ~/.zshrc

# Deploy to worker nodes
for n in lf gd bt st; do scp .kova-aliases "$n":~/; done

# Bootstrap kova config
kova bootstrap

# Install local LLM
kova model install
```

---

Unlicense — cochranblock.org
