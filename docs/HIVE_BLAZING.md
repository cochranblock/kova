# Hive Blazing Speed

**Purpose:** Document the parallel sync + broadcast design for `kova c2 build`.

**Date:** 2026-03-11

---

## Design

| Phase | Behavior |
|-------|----------|
| Sync | One thread per node. Each rsyncs workspace to that node. Parallel. |
| Build | One thread per node. Each runs `cargo build --release`. Output streamed with `[node]` prefix. |

**Before:** Sequential — sync to lf, then gd, then bt, then st. Build on lf, then gd, etc.

**After:** Parallel — sync to all nodes concurrently. Build on all nodes concurrently.

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

## Future: Tar-Stream Sync

Current sync uses parallel rsync (one per node). Tar-stream would: create tar once, stream to each node via `cat file | ssh node "..."`. One disk read, N network writes. Possible future optimization.
