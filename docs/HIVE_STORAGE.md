# Hive Storage Architecture

**Purpose:** Document storage decisions for the hive mesh (`/mnt/hive`): NFS vs GlusterFS, tuning, and optional sync-to-local build pattern.

**Date:** 2026-03-11

---

## 1. Architecture

| Component | Role |
|-----------|------|
| **st** | NFS server — exports `/mnt/hive` to workers |
| **Workers** | lf, gd, bt, st — mount `/mnt/hive` from st |
| **c2-core** | Mac mini — `~/hive-vault` maps to `/mnt/hive` on workers |
| **Sync** | `kova c2 sync` — rsync workspace from c2-core to workers |

Workers run `cargo build` directly on the shared NFS mount.

---

## 2. Decision: NFS over GlusterFS

**Question:** Is GlusterFS fast enough for `/mnt/hive` across 10Gb?

**Answer:** GlusterFS over 10Gb has sufficient throughput (~200–400 MB/s), but **NFS is better suited** for cargo/workspace workloads.

| Factor | NFS | GlusterFS |
|--------|-----|-----------|
| Cargo metadata | Lower overhead | Higher metadata latency |
| Locking | Simpler, NFSv4 | More complex, distributed |
| Setup | Single server (st) | Multi-node cluster |
| Use case fit | Shared build tree | Replication, HA failover |

**Recommendation:** Keep NFS. Do not migrate to GlusterFS for cargo/workspace.

---

## 3. NFS Tuning

If st is the NFS server:

1. **NFSv4** — Use NFSv4 (not v3) for better locking and state.
2. **Mount options** — On workers, add `rsize=65536,wsize=65536` for 10Gb throughput.
3. **Disk** — Ensure st has SSD/NVMe for `/mnt/hive` if possible.
4. **Monitoring** — `kova c2 inspect --recommend` warns when st disk is low (< 50 GB).

Example `/etc/fstab` on workers:

```
st:/mnt/hive /mnt/hive nfs rsize=65536,wsize=65536,vers=4,_netdev 0 0
```

---

## 4. Optional: Sync-to-Local Build Pattern

**Problem:** Building directly on NFS can be slow due to metadata round-trips.

**Pattern:** Sync workspace to a local path on each worker (e.g. `/tmp/hive-build`), then run cargo there. Builds are faster; artifacts are ephemeral.

**Usage:**

```bash
# Sync to local path on workers (instead of /mnt/hive)
kova c2 sync --local --all   # incremental rsync to all workers
kova c2 sync --local --all --full   # full tar-stream (when workers have no content)

# Build on local path (faster)
kova c2 run f20 --broadcast --local
```

**Behavior:**

- `sync --local` rsyncs to `target:/tmp/hive-build/projects/workspace/` (and projects).
- `run --broadcast --local` runs cargo in `/tmp/hive-build` instead of `/mnt/hive`.
- Artifacts (`target/`) stay on worker; no sync-back unless needed.

**When to use:** If NFS builds prove slow, use `--local` for release builds. Sync once, then broadcast.

---

## 5. One-Command Build (Preferred)

**Prefer `kova c2 build --broadcast`** over `sync` + `run`. One command does sync (unless `--no-sync`) + parallel broadcast.

```bash
kova c2 build --broadcast --local --release   # sync + build on all workers, parallel
kova c2 build --broadcast --no-sync          # skip sync; assume already synced
```

Sync and build run in parallel across all workers. Output is streamed with `[node]` prefix.

---

## 6. SSH ControlMaster (Connection Reuse)

Reduce SSH handshake overhead by reusing connections. Add to `~/.ssh/config`:

```
Host lf gd bt st
  ControlMaster auto
  ControlPath ~/.kova/ssh-%r@%h:%p
  ControlPersist 10m
```

First SSH opens a master; subsequent SSHs reuse it. Zero handshake for sync→build.

---

## 7. Related Docs

- [HIVE_BLAZING.md](HIVE_BLAZING.md) — Parallel design, benchmarks
- [HIVE_MESH_USAGE_GAP.md](HIVE_MESH_USAGE_GAP.md) — Usage gaps, sync vs broadcast paths
- [PLAN_EXECUTION_PATH.md](PLAN_EXECUTION_PATH.md) — C2 broadcast flow
