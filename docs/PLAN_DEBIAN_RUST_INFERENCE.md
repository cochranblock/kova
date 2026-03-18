<!-- Unlicense — cochranblock.org -->
<!-- Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3 -->

# PLAN: Debian Nodes — Rust-Only Inference (Replace Ollama)

**Purpose:** Migrate IRONHIVE Debian nodes (lf, gd, bt, st) from ollama-hosted models to kova serve + Kalosm (pure Rust).

**Date:** 2026-03-16

---

## 1. Current vs Target

| Aspect | Current (Ollama) | Target (Rust) |
|--------|------------------|---------------|
| **Server** | ollama (Go, port 11434) | kova serve (Rust, port 3002) |
| **Inference** | llama.cpp via ollama | Kalosm + candle (GGUF) |
| **API** | ollama /api/generate, /api/tags | OpenAI-compat /v1/chat/completions, /v1/models |
| **Provider** | Provider::Ollama | Provider::OpenAiCompat |
| **Model format** | ollama Modelfile + GGUF | Raw GGUF only |

---

## 2. Architecture Overview

```
                    ┌─────────────────────────────────────────┐
                    │  Mac mini (c2-core)                      │
                    │  kova cluster dispatch                   │
                    │  Provider::OpenAiCompat → http://n:3002  │
                    └──────────────────┬──────────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         │                             │                             │
         ▼                             ▼                             ▼
┌─────────────────┐           ┌─────────────────┐           ┌─────────────────┐
│ lf (n0)         │           │ gd (n1)         │           │ bt (n2)         │
│ kova serve :3002│           │ kova serve :3002│           │ kova serve :3002│
│ Kalosm + 14b   │           │ Kalosm + 14b   │           │ Kalosm + 32b   │
│ GGUF           │           │ GGUF           │           │ GGUF           │
└─────────────────┘           └─────────────────┘           └─────────────────┘
         │                             │                             │
         └─────────────────────────────┼─────────────────────────────┘
                                       │
                              ┌─────────────────┐
                              │ st (n3)         │
                              │ kova serve :3002│
                              │ Kalosm + 14b   │
                              └─────────────────┘
```

---

## 3. Per-Node Model Mapping

| Node | Role | Tier | Model (cluster) | GGUF path (node) |
|------|------|------|-----------------|------------------|
| n0 (lf) | PrimaryGen | Mid | qwen2.5-coder:14b | ~/.kova/models/ or /mnt/hive/models/ |
| n1 (gd) | Reviewer | Mid | qwen2.5-coder:14b | same |
| n2 (bt) | SecondaryGen | Heavy | qwen2.5-coder:32b | same |
| n3 (st) | Batch | Mid | qwen2.5-coder:14b | same |
| c2 (local) | Coordinator | Light | qwen2.5-coder:7b | ~/.kova/models/ |

Each node configures `KOVA_INFERENCE_MODEL` or `~/.kova/config.toml` `[models] coder` to point at its GGUF.

---

## 4. Model Storage Options

### 4a. Per-node local (~/.kova/models/)

- **Pros:** No NFS dependency for inference; each node loads from local SSD.
- **Cons:** Must sync GGUF files to each node; ~14GB+ per 14b model, ~20GB+ for 32b.

### 4b. Shared NFS (/mnt/hive/models/)

- **Pros:** Single copy; `kova c2 sync` or rsync already used for workspace.
- **Cons:** NFS read latency for model load; first load slower.

### 4c. Hybrid (recommended)

- Store GGUF on NFS at `/mnt/hive/models/`.
- Optionally symlink or copy to `~/.kova/models/` on each node for faster cold start.
- `kova model install` or manual `rsync` to populate.

---

## 5. Deployment Steps

### Phase 1: Prepare kova serve binary

1. Build kova with inference + serve:
   ```sh
   cargo build --release -p kova --features "serve,inference,rag"
   ```
2. Deploy to nodes via `kova c2 build --broadcast --release` or manual `scp`.

### Phase 2: Model setup per node

1. Create models dir: `~/.kova/models/` or `/mnt/hive/models/`.
2. Place GGUF per node:
   - n0, n1, n3: Qwen2.5-Coder-7B or 14B (e.g. `Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf`)
   - n2: Qwen2.5-Coder-32B or equivalent
   - c2: 7B or 3B for coordinator
3. Per-node config `~/.kova/config.toml`:
   ```toml
   [models]
   coder = "/mnt/hive/models/Qwen2.5-Coder-14B-Instruct-Q4_K_M.gguf"  # or ~/.kova/models/...
   ```

### Phase 3: systemd service (Debian)

Create `/etc/systemd/system/kova-serve.service` on each node:

```ini
[Unit]
Description=Kova inference server
After=network.target

[Service]
Type=simple
User=mcochran
WorkingDirectory=/home/mcochran
ExecStart=/home/mcochran/bin/kova serve --bind 0.0.0.0:3002
Restart=on-failure
RestartSec=10
Environment="KOVA_INFERENCE_MODEL=/mnt/hive/models/Qwen2.5-Coder-14B-Instruct-Q4_K_M.gguf"

[Install]
WantedBy=multi-user.target
```

Enable: `sudo systemctl enable kova-serve && sudo systemctl start kova-serve`.

### Phase 4: Stop ollama, switch cluster

1. On each node: `sudo systemctl stop ollama` (or `ollama serve` if manual).
2. Cluster already uses `Provider::OpenAiCompat` and port 3002 (see `cluster.rs`).
3. Verify: `curl http://lf:3002/v1/models` returns model list.

### Phase 5: Update aliases and tooling

1. **.kova-aliases** — change tunnel targets from 11434 → 3002:
   - `_at 3002` instead of `_at 11434`
   - Or new aliases: `ktgd`, `ktbt`, etc. tunnel to 3002
2. **ktuns** — check `/v1/models` instead of `/api/tags`:
   ```sh
   _tp() { curl -s --max-time 3 "http://localhost:$1/v1/models" 2>/dev/null | grep -q '"data"' && echo "$1($2): OK" || echo "$1($2): DOWN"; }
   ```

---

## 6. Code Changes (Already Done / Minor)

| Component | Status |
|-----------|--------|
| `cluster.rs` | Uses `Provider::OpenAiCompat`, port 3002 |
| `serve.rs` | `/v1/chat/completions`, `/v1/models` |
| `providers.rs` | OpenAiCompat, Local (Kalosm), Ollama (fallback) |
| `inference.rs` | Kalosm + GGUF via `f80`, `f76` |

**Optional enhancement:** serve could map request `model` to path (e.g. `qwen2.5-coder:14b` → config or models dir). Currently it uses `inference_model_path()` only; cluster sends model name but node uses its configured path. This is acceptable if each node has one primary model.

---

## 7. Rollback

If issues arise:

1. Restart ollama on nodes: `sudo systemctl start ollama`.
2. Revert cluster to `Provider::Ollama` and port 11434 (if that code path still exists).
3. Or run both temporarily: ollama on 11434, kova serve on 3002; switch cluster config.

---

## 8. Checklist

- [ ] Build kova with serve + inference
- [ ] Deploy kova binary to lf, gd, bt, st
- [ ] Create /mnt/hive/models or ~/.kova/models
- [ ] Download/place GGUF per tier (14b, 32b, 7b)
- [ ] Per-node config.toml or KOVA_INFERENCE_MODEL
- [ ] systemd unit for kova serve
- [ ] Stop ollama
- [ ] Update .kova-aliases (tunnels, ktuns)
- [ ] Smoke test: `kova cluster status`, `kova cluster gen`
- [ ] Remove ollama from nodes (apt remove) when stable

---

## 9. Related Docs

- [HIVE_STORAGE.md](HIVE_STORAGE.md) — NFS, /mnt/hive
- [HIVE_BLAZING.md](HIVE_BLAZING.md) — Parallel broadcast
- [hosting-schematic.mdc](~/.cursor/rules/hosting-schematic.mdc) — approuter, ports