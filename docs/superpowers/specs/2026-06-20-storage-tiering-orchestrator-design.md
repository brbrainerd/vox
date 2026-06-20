---
title: "Storage Tiering Orchestrator — Design"
description: "Adaptive per-category storage tiering for a 3-tier Windows workstation (D: fast NVMe / C: warm NVMe / X: cold HDD), prioritizing Rust build I/O, using junctions, idle-gating, and Defender/Search tuning."
category: "architecture"
status: "research"
---

# Storage Tiering Orchestrator — Design

> Host-tooling design for the Vox developer workstation. Goal: keep the highest-I/O
> work on the fastest drive automatically, safely, and reversibly — without ever
> moving data that is currently in use, and without relocating the OS.

## Context & hardware

| Tier | Drive | Measured | Role |
|---|---|---|---|
| **HOT (D:)** | SN850X 2 TB, PCIe Gen 4 ×4, **CPU-direct (M.2_1)** | ~NVMe (≈ C:) | Active Rust build set (vox = first-class) + shared build caches + transient hot promotions |
| **WARM (C:)** | 990 PRO 4 TB, Gen 4, chipset (DMI-shared) | SeqW ~1 GB/s, 2,744 files/s | OS, apps, Docker, LLM models, recently-active repos, working media |
| **COLD (X:)** | 2× 16 TB RAID0 HDD | SeqW ~113 MB/s, 398 files/s | Dormant repos, games, backups, finished media, downloads |

C: and D: are the **same NVMe speed class**; the only physical edge D: has is the
CPU-direct lane (lower contention under concurrent load). The real performance lever is
therefore **(a) keep hot data off the HDD** and **(b) isolate the build's I/O on its own
drive** so it doesn't contend with OS/Docker/everything-else. The HDD is ~7–100× slower
on small-file/random I/O — gaming tolerates it (user accepts slow game loads), builds do not.

## Governing constraint

**Data that is "hot" is in use (open handles), and in-use data cannot be moved safely.**
So tiering is observe → wait for idle → migrate → fast next time. There is no safe
"move it while it's being hammered." Every move is idle-gated.

## Section 1 — Drive roles & per-category policy

| Category | Audited examples (size) | Mode | Promote→ | Demote→ |
|---|---|---|---|---|
| **Rust active build** | `vox`(646 GB), vox-* worktrees, `jj-fork`, `edit-mind-rust` | **AUTO** | D: | C:→X: when dormant |
| **Shared build caches** | `.cargo`, `.rustup`, `sccache` | **PINNED** D: | D: | never |
| **Other dev repos** | `govSim`, `Ovi`, `fableforge`, `NullCascade` | **AUTO** (size+recency) | C:/D: | X: |
| **LLM models** | `.lmstudio`(89 GB), `.ollama` | **ADVISORY** | D: (opt) | C: |
| **Docker / VHDX** | `AppData\Local\Docker`(290 GB) | **MANUAL/PINNED** C: (isolated from build) | C: | — |
| **Games** | Steam (406 GB + 2.7 TB) | **MANUAL** (Steam GUI / whole-library junction); never auto-promote | — | X: |
| **Media / archive** | `brVideo`, `CrossDevice`, `Downloads`, `VRT` | **AUTO** demote when cold | C: | X: |
| **OS + installed apps** | Windows, Program Files | **LOCKED** never move | — | — |

Cross-cutting: **size floor** (default ≥ 5 GB; smaller dirs never move) and **idle-gating**
(zero open handles required). Worktree optimization: all `vox` worktrees share one
`target`/`sccache` on D: to kill multi-TB `target/` duplication.

## Section 2 — Architecture (6 units)

1. **Catalog** — SQLite source of truth: per managed dir → {path, category, current tier,
   size, last-access, junction status, pinned/locked}.
2. **Monitor** — emits a **heat score** per dir from access recency/frequency (FS
   timestamps + watcher), live process I/O (perf counters → best-effort handle→dir map,
   advisory), and per-drive free space. Senses only.
3. **Policy Engine** — applies category rules to Catalog + heat: **promote/demote/hold**,
   gated by size floor, mode, and target free-space headroom. AUTO→queue move;
   ADVISORY→recommendation; MANUAL/LOCKED→never.
4. **Mover** — the *only* unit with FS write authority. idle-gate → robocopy → verify →
   junction-swap → journal. (Detail in §3.)
5. **System I/O Reduction (tuner)** — keeps D: a trusted Dev Drive; syncs **Defender
   exclusions** as dirs promote/demote; scopes **Windows Search** away from cold/bulk
   tiers. Never moves data, never touches the OS.
6. **Control surface** — daemon + CLI + config. Encodes hybrid autonomy (per-category auto
   vs advisory), dry-run, and a one-click approval queue.

Data flow: `Monitor → heat → Policy → {Mover, Tuner} → Catalog`. Monitor senses, Policy
decides, Mover acts, Tuner quiets — all write-to-FS logic isolated in the Mover.

## Section 3 — Move mechanism

**Link type:** **directory junctions** (`mklink /J`) are the default — no admin,
app-transparent, cross-local-volume, followed correctly by IDEs, cargo, and Steam.
*Symlinks* (`mklink /D`) only as a per-category override when a tool needs true symlink
semantics (requires admin / Developer Mode; more app-compat caveats). *Hard links* are
same-volume-only → **not usable** for cross-drive tiering.

**Copy→verify→swap sequence (per move):**
1. **Idle-gate** — confirm no open handles under the dir (handle enumeration or exclusive
   rename probe). Builds, running games, live containers block their dirs.
2. **robocopy** source → target on the new drive (mirrored, retried).
3. **Verify** — file count + total bytes match (+ optional sampled hash).
4. **Swap** — rename original `dir`→`dir.bak` (atomic), create junction `dir`→target.
5. **Validate** — junction resolves; sanity read succeeds.
6. **Finalize** — delete `dir.bak` after success (or retain until next pass for safety).
On any failure → **roll back** (remove junction, restore `dir.bak`). Every step journaled
so a crash mid-move is replay-recoverable on restart.

**Reversal (promote↔demote):** the inverse — remove junction, relocate data to the new
tier, recreate junction (or restore a real dir if returning to its home drive).

**Steam/games:** no reliable per-game move API exists. Supported paths: (a) **manual**
per-game via Steam's "Move install folder" GUI; (b) **whole-library junction** to relocate
an entire `SteamLibrary` wholesale (Steam follows it). Orchestrator **never auto-moves
individual games** (anti-cheat / validation risk); it may *recommend* and perform
whole-library junctions only.

## Section 4 — Safety model & failure modes

- **Allowlist + locked/pinned** — only cataloged, allowlisted dirs are ever touched; OS,
  Program Files, Docker data = never auto-moved.
- **Size floor** — dirs under threshold are skipped.
- **Free-space guard** — never promote without target headroom (+margin); never strand a
  tier below a reserve.
- **Idle-gating** — never move a dir with open handles.
- **Copy-verify-swap + journal** — every move atomic-ish and reversible; crash-safe via
  journal replay (`.bak` + junction recoverable).
- **Junction hygiene** — validate targets exist; refuse to nest junctions; detect
  already-junctioned paths.
- **Tuner bounds** — only adds/removes exclusions for *managed* dirs; never disables
  Defender globally; all changes logged and reversible.

| Failure | Handling |
|---|---|
| Power loss mid-move | Journal replay on restart restores consistent state |
| Source locked / busy | Skip, retry next idle window |
| Target full | Abort before any write; no partial state |
| Junction creation fails | Roll back (restore `.bak`) |
| App can't follow junction | Per-category symlink override or allowlist exclusion |

## Section 5 — Implementation substrate & phasing

**Phase 1 — PowerShell advisory/manual tool (prove the mechanics).** Catalog (SQLite or
JSON), Policy for the AUTO *repo* category + *media demote* category, junction-based Mover
with copy-verify-swap + journal, and the System I/O Reduction tuner (Defender exclusions +
Search scoping). Driven by a Scheduled Task on an **idle trigger**. Dry-run default;
advisory queue surfaced to the user. Lives **outside the Vox repo** (host tooling;
AGENTS.md bans new in-repo `.ps1` glue).

**Phase 2 — Rust daemon (`vox-tier`, sibling to / part of `vox-build-broker`).** Adds live
process-I/O monitoring (Windows perf counters / ETW), the full hybrid auto/advisory engine,
and robust handle-based idle-gating. Junctions via `std::os::windows` / winapi. This is the
"first-class" form once the policy is proven safe in Phase 1.

Phasing mirrors the **hybrid autonomy**: start advisory/manual (safe), graduate categories
to AUTO as trust accrues.

## Open questions (for the plan)

1. Catalog store: SQLite vs flat JSON for Phase 1.
2. Heat-score weighting (recency vs frequency vs live I/O) and the cold→demote threshold
   (default 14 days?).
3. Whether Phase 2 lands inside `vox-build-broker` or as a standalone `vox-tier` crate.
4. Idle-trigger source (Task Scheduler idle vs a lightweight always-on watcher).
