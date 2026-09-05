# P6 — Payload & Data SSOT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Steps use `- [ ]` checkboxes.
>
> **Read [`2026-09-05-00-INDEX.md`](2026-09-05-00-INDEX.md) first** for file-ownership rules and global constraints.

**Goal:** users can bound and relocate everything Vox writes to disk, and nothing large is downloaded without an explicit act.

**Spec:** [`../specs/2026-09-04-distribution-and-plugin-architecture.md`](../specs/2026-09-04-distribution-and-plugin-architecture.md) §5

**You own:** `crates/vox-config/src/paths.rs`, `crates/vox-config/src/graphify.rs`, `contracts/retrieval/`

## Global constraints

See the index. Non-negotiable everywhere: assert on the artifact never the exit code (`cmd > /tmp/x.log 2>&1; echo $?`); `cargo test -p X` needs `--all-targets` or it can report "0 passed" when tests live in a bin target; guards must run on macOS (no `grep -oP`); never execute a downloaded binary or set `com.apple.quarantine`.

## Measured footprint

| Path | Measured | Override | Gap |
|---|---|---|---|
| `~/.vox/bin` | **136 MB** | none | `VOX_HOME` is honored **nowhere** in the tree |
| `<repo>/.vox/cache/graphify` | **138 MB** | none | no disable, no relocate; only editable in a contracts YAML |
| `~/.vox/cache` (model catalog) | 492 KB, refreshed every 6h | none | background network on by default |
| `~/Library/Application Support/vox` | 6.6 MB | `VOX_DATA_DIR` | ok |
| HF model weights | 0 here | `HF_HOME` (hf-hub's own) | the default Qwen3-8B is ~16 GB and that number appears nowhere |
| Speech models | — | `VOX_ORATIO_SHERPA_MODEL_DIR` | **downloads automatically on first use** |

---

## Task 1: Honour `VOX_HOME`
- [ ] `dot_vox_user_dir()` hardcodes `~/.vox`. Make `VOX_HOME` the root, defaulting to today's path.
- [ ] Test with `VOX_HOME` pointed at a temp dir: every subdirectory follows.

## Task 2: Bound the graphify cache
- [ ] Add `VOX_GRAPHIFY_CACHE_DIR` and a disable switch. Today the only relocation lever is editing `contracts/retrieval/vox-graph-corpora.v1.yaml`.
- [ ] Note the live migration: `repo_graphify_cache_dir` and the YAML say `.vox/cache/graphify`, while `primary_cache_dir` and newly-registered corpora use `.vox/cache/vox-graph`. Finish or document it; do not leave both.
- [ ] Investigate why `crate-map`, `repo-code-graph` and `config-audit` produce byte-identical 12,890,417-byte graphs despite three distinct declared `extraction_mode`s. Either they are genuinely the same content written three times — a 90 MB waste — or the mode is not being applied. Do not assume which.

## Task 3: Make large downloads explicit
- [ ] Speech models download as a side effect of **path resolution** (`resolve_sherpa_model_paths`). Make it an explicit act.
- [ ] State the size before downloading. `DEFAULT_MODEL_ID` is a Qwen3-8B (~16 GB in bf16) and no size appears in code or docs.

## Task 4: A doctor check for footprint
- [ ] New check module (you own the file). Report per-directory sizes and which are unbounded.
- [ ] Register with a **one-line append** to `checks_standard/mod.rs` — P3 also appends; see the index's shared-file protocol.

## Verification
- [ ] With `VOX_HOME` and `VOX_DATA_DIR` set to temp dirs, assert nothing is written outside them.
- [ ] `cargo test -p vox-config --all-targets` with real counts.
