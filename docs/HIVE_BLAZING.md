# Hive Blazing Speed

**Purpose:** Document the parallel sync + broadcast design for `kova c2 build`.

**Date:** 2026-03-11

---

## Design

| Phase | Behavior |
|-------|----------|
| Sync | **Dynamic:** tar-stream when full sync (dir missing), rsync when incremental. Parallel to all nodes. |
| Build | One thread per node. Each runs `cargo build --release`. Output streamed with `[node]` prefix. |

**Before:** Sequential — sync to lf, then gd, then bt, then st. Build on lf, then gd, etc.

**After:** Parallel — sync to all nodes concurrently. Build on all nodes concurrently.

### Sync Strategy

| Context | Method | Why |
|---------|--------|-----|
| `kova c2 build` (preflight failed) | Tar-stream | Full sync — dir missing on workers |
| `kova c2 sync` (default) | Rsync | Incremental — workers likely have content |
| `kova c2 sync --full` | Tar-stream | User requests full refresh |

---

## Commands

```bash
# One command: sync + build (parallel)
kova c2 build --broadcast --local --release

# Skip sync when already synced
kova c2 build --broadcast --no-sync --release

# Restrict nodes
kova c2 build --broadcast --nodes lf,gd --release
```

---

## Success Criteria

- Sync: ~4x faster with 4 nodes (parallel vs sequential).
- Build: wall time = slowest node (parallel).
- One command. No separate sync.

---

## Tar-Stream Sync (Implemented)

Tar-stream: create tar once (symlinks + tar -ch), write to temp file, then `cat file | ssh node "..."` in parallel threads. One disk read, N network writes. Used when `full_sync=true` (build preflight failed, or `kova c2 sync --full`).
