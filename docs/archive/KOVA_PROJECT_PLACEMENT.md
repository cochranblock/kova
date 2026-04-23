# Project Placement — Kova Bare-Metal Swarm

**Purpose:** Where each project should live and run, based on system survey and practical use of connected bare metal.

**Survey date:** 2026-03-09

---

## 1. System Survey

### c2-core (Mac mini)
| Spec | Value |
|------|-------|
| CPU | 10 cores (Apple Silicon) |
| RAM | 16 GB |
| Arch | arm64 |
| Hive access | `~/hive-vault` = NFS mount of `st:/mnt/hive` |

### Workers (x86_64, headless Debian)

| Node | Cores | RAM | Notes |
|------|-------|-----|-------|
| lf | 20 | 15 GB | |
| gd | 20 | 31 GB | |
| bt | 12 | **46 GB** | Most RAM |
| st | 14 | 30 GB | NFS exports hive to c2-core |

**Total worker compute:** 66 cores, 122 GB RAM.

---

## 2. Project Inventory & Roles

| Project | Role | Runtime | Notes |
|---------|------|---------|-------|
| **approuter** | Reverse proxy + Cloudflare tunnel registration | Server | Gateway for all apps. Must run where tunnel terminates. |
| **cochranblock** | Portfolio site (cochranblock.org) | Server | Registers with approuter. Axum. |
| **oakilydokily** | Product site | Server | Registers with approuter. |
| **rogue-repo** | HTTP API (roguerepo.io) | Server | Registers with approuter. |
| **ronin-sites** | Multi-tenant tattoo shop SaaS | Server | Approuter or standalone. |
| **kova** | GUI AI assistant (egui + Kalosm) | Desktop | Needs display. Local LLM. |
| **kova-core** | Shared types | Library | Dependency only. |
| **kova-web** | WASM thin client for kova | Static | Build → deploy anywhere. |
| **kova-daemon** | Swarm execution daemon | Daemon | Runs on workers. |
| **exopack** | Test augmentation (screenshot, mocks, triple_sims) | Library | Build-time / test dep. |
| **whyyoulying** | Labor fraud detection | CLI | Portable. |
| **wowasticker** | Dioxus mobile app | App | Cross-platform build. |
| **rogue-runner** | Endless runner game | App | Native + WASM. |
| **vendor** | xml5ever patch | Build dep | Workspace patch. |
| **hive-vault** | NFS mount of st:/mnt/hive | Mount | Not a project. |
| **data** | Cloudflared config, registry | Config | Top-level config. |

---

## 3. Recommended Placement

### A. Source of truth: `/mnt/hive/projects/`

**Put the full workspace in the hive.** c2-core already mounts hive at `~/hive-vault`. Workers see `/mnt/hive` natively.

```
/mnt/hive/projects/
├── workspace/           # Root Cargo workspace (or symlink structure)
│   ├── cochranblock/
│   ├── approuter/
│   ├── kova/
│   ├── kova-core/
│   ├── kova-web/
│   ├── rogue-repo/
│   ├── oakilydokily/
│   ├── exopack/
│   ├── whyyoulying/
│   ├── wowasticker/
│   ├── vendor/
│   └── Cargo.toml
├── ronin-sites/         # Separate workspace
└── kova-daemon/         # Standalone (different deployment)
```

**Benefits:**
- Single source. Edit on c2-core via `~/hive-vault/projects/`; workers build from `/mnt/hive/projects/`.
- kova-node `cargoBuild` works: `workingDir=/mnt/hive/projects`, `projectPath=workspace/rogue-repo`.
- No sync step. GlusterFS replicates.

**Migration:** rsync from `~/` to `/mnt/hive/projects/` once, then develop from hive-vault. Or symlink `~/cochranblock` → `~/hive-vault/projects/workspace/cochranblock` during transition.

---

### B. Where services run

| Service | Recommended host | Reason |
|---------|------------------|--------|
| **approuter** | c2-core | Tunnel endpoint. Mac is always on, has network. |
| **cochranblock** | c2-core | Small. Run alongside approuter. |
| **oakilydokily** | c2-core | Same. |
| **rogue-repo** | c2-core or bt | bt has 46 GB RAM if you scale. Start on c2-core. |
| **ronin-sites** | c2-core or bt | Same. |
| **kova** | c2-core only | GUI needs display. |
| **kova-node** | lf, gd, bt, st | One daemon per worker. |

---

### C. Build farm usage

**Workers = 66 cores.** Use for:

1. **Parallel `cargo build`** — kova-node `cargoBuild` on `/mnt/hive/projects/workspace`. Split by crate or target.
2. **CI-style pipelines** — `cargo check`, `cargo test`, `cargo clippy` across workspace.
3. **Cross-compilation** — Workers are x86. Build Linux binaries there; c2-core builds arm64/Mac.

**Suggested split:**
- lf, gd: 20 cores each — primary build nodes.
- bt: 46 GB RAM — heavy builds, or run extra services.
- st: 14 cores — builds + NFS export for hive.

---

### D. What stays on c2-core only

| Item | Location | Reason |
|------|----------|--------|
| `~/.kova/` | c2-core | kova config, prompts, backlog. |
| `~/data/` | c2-core | Cloudflared, registry. Config. |
| `~/scripts/` | c2-core or hive | Dev scripts. Could live in hive/projects/scripts. |
| `~/docs/` | c2-core or hive | Personal docs. |
| `~/Desktop`, `~/Downloads`, etc. | c2-core | Standard home dirs. |

---

## 4. Action Summary

| Action | Priority |
|--------|----------|
| Create `/mnt/hive/projects/` on hive | High |
| Rsync workspace to `/mnt/hive/projects/workspace/` | High |
| Point c2-core dev at `~/hive-vault/projects/workspace/` (or symlink) | High |
| Deploy kova-node to workers | High (when Phase 2 ready) |
| Run approuter + apps on c2-core | Medium (if not already) |
| Use workers for `cargo build` via kova-node | Medium (Phase 2+) |

---

## 5. Path Map (After Migration)

| Path on c2-core | Path on workers | Purpose |
|-----------------|-----------------|---------|
| `~/hive-vault/projects/workspace/` | `/mnt/hive/projects/workspace/` | Main Rust workspace |
| `~/hive-vault/projects/kova-daemon/` | `/mnt/hive/projects/kova-daemon/` | Swarm daemon |
| `~/hive-vault/projects/ronin-sites/` | `/mnt/hive/projects/ronin-sites/` | Ronin (separate) |
| `~/.kova/` | — | kova config (c2 only) |
| `~/data/` | — | Tunnel/config (c2 only) |
