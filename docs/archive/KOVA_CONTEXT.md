# Kova Ecosystem — Context for AI Agents

**Purpose:** Restart context. Read this before continuing Kova-related work.

**Last updated:** 2026-03-09

**When presenting to user:** Always use the release binary: `./target/release/kova` or `cargo run -p kova --release`. Never use debug (`./target/debug/kova`).

---

## 1. Two Kova Products (Same Brand, Different Roles)

| Product | Path | Role |
|---------|------|------|
| **kova** | `~/kova/` | GUI-first local AI assistant. Intent parsing, LLM orchestration (Kalosm), code gen pipeline. Replaces Cursor-style workflows locally. |
| **kova-daemon** | `~/kova-daemon/` | Distributed computing swarm daemon. Zero-copy Cap'n Proto protocol. `kova-node` runs on worker nodes; replaces SSH bash aliases with typed binary execution. |

**Relationship:** Independent today. Future: kova (GUI) could dispatch commands to the swarm via kova-node.

---

## 1b. Hybrid UI (Option C)

Kova uses **two clients, one API**:

| Client | Tech | Use case |
|--------|------|----------|
| **Native** | egui/eframe | Desktop, offline, direct inference. `kova gui` |
| **Web** | HTML/JS | Browser, remote access, PWA. `kova serve` → http://localhost |

Both talk to the same HTTP/WebSocket API. Native runs inference locally; web connects to `kova serve` which runs inference on the server.

```bash
kova gui      # Native egui (default when no args)
kova serve    # HTTP API + web client at /
kova serve --open   # Serve and open browser
```

**Web parity:** Project selector, prompts view, backlog list, Copy/Apply/diff, Explain. Same features as native where applicable.

---

## 2. Deployment Topology

- **c2-core:** Mac mini (this machine). Hostname: `kova-c2-core`. Command center.
- **Worker nodes:** Headless Debian, x86 (Desktop, Dell XPS, HP EliteBook). Examples: `kova-thick-beast`, `kova-legion-forge`, `kova-tunnel-god`, `kova-elite-support`.
- **SSH:** `~/.ssh/kova-commander` for swarm access. Aliases: `lf`, `gd`, `bt`, `st` (from `sshall` in `.zshrc`).
- **Storage:** GlusterFS at `/mnt/hive` (on workers only). Projects under `/mnt/hive/projects`. **c2-core (Mac) does not have `/mnt/hive`** — it is not mounted locally.

---

## 3. Workspace Layout (Combined)

```
~/ (or $HOME)
├── Cargo.toml              # Root workspace: kova, kova-core, kova-web, kova-daemon/kova-node, exopack, etc.
├── kova/                   # GUI, serve, kova-c2, kova-test
├── kova-core/              # Shared types (Intent, Backlog)
├── kova-web/               # WASM thin client
├── kova-daemon/
│   ├── schema/kova_protocol.capnp
│   └── kova-node/         # Worker daemon (workspace member)
├── KOVA_CONTEXT.md         # This file
└── ...
```

**Note:** All kova-named code is in one workspace. `kova-test` runs exopack (triple_sims + interface) for validation.

---

## 4. Path Conventions

- **c2-core (macOS):** `$HOME` = `/Users/mcochran`
- **Workers (Debian):** `$HOME` = `/home/mcochran`, `/mnt/hive` for shared storage (GlusterFS)
- **c2-core:** `/mnt/hive` does not exist. Use `$HOME` for local projects. To test kova-node against hive, SSH to a worker or mount hive locally.
- **Docs:** Prefer `$HOME` or `~` in examples. Avoid hardcoding `/home/mcochran` when targeting macOS.

---

## 5. kova-daemon Status (Phase 1)

- Schema: `Command` (cargoBuild, fileSync), `Telemetry` (CPU thermal, memory, result)
- Build: `capnp` compiler required (`brew install capnp` on macOS)
- Network listener: **not yet implemented**

---

## 6. /mnt/hive Layout (Workers Only)

Expected on each worker node:

```
/mnt/hive/
├── projects/    # Cargo builds, workspaces (target for cargoBuild commands)
└── ...          # Other shared data (fileSync source/destination)
```

c2-core does not mount this. To inspect hive contents, SSH to a worker: `ssh lf ls -la /mnt/hive`.

---

## 7. kova c2 (Orchestration — Same Binary)

Tokenized commands. Runs on c2-core. Replaces `sshall` + manual intent.

```bash
# Run locally (default)
kova c2 run f20 --project ~/rogue-repo

# Broadcast to workers (lf gd bt st)
kova c2 run f20 --project ~/hive-vault/projects/workspace/rogue-repo --broadcast

# Local-only commands (f21/f22/f23 use approuter)
kova c2 run f21
kova c2 run f22

# List nodes
kova c2 nodes
```

**Tokens:** f18=compile, f19=test, f20=full-pipeline, f21=tunnel-update, f22=setup-roguerepo, f23=cloudflare-purge.

---

## 8. Key Files

| File | Purpose |
|------|---------|
| `~/.zshrc` | Kova Swarm config, `sshall` broadcast to lf/gd/bt/st |
| `~/.ssh/config` | kova-commander for lf gd bt st core c2 *.kova.inside |
| `kova/docs/PLAN_BUILD_KOVA.md` | kova (GUI) build plan |
| `kova-daemon/README.md` | kova-daemon (swarm) overview |
| `kova c2` | Orchestration subcommand (tokenized commands) |
