---
title: "Graphify duplicate-corpus bytes findings (2026-09)"
description: "Investigates the claimed three byte-identical 12,890,417-byte graphify corpora: verifies which are actually identical, why, and what the fix would be."
category: "Architecture SSOTs"
---

# Graphify duplicate-corpus bytes — findings (2026-09)

## Claim under investigation

`vox ci artifact-audit` and the disk-footprint plan (P6-2) claim three corpora in
`contracts/retrieval/vox-graph-corpora.v1.yaml` — `config-audit`, `crate-map`, and
`repo-code-graph` — produce byte-identical 12,890,417-byte `graph.json` files despite
three distinct declared `extraction_mode`s (`audit`, `crate-map`, `structural`).

## What is actually true

**Two are byte-identical, not three.** Measured directly against
`/Users/brbrainerd/dev/vox/.vox/cache/graphify/` on 2026-09-04:

| Corpus | `extraction_mode` | Size (bytes) | sha256 | `git_sha` (manifest) |
| --- | --- | --- | --- | --- |
| `config-audit` | `audit` | 12,890,417 | `04afa254e68f54436d4b3d5b069b9bbef713e51f90e059b82f5f20c9e5f00e17` | `d9fb993402a96d5f0f3172420e221ba4a4ef81df` |
| `crate-map` | `crate-map` | 12,890,417 | `04afa254e68f54436d4b3d5b069b9bbef713e51f90e059b82f5f20c9e5f00e17` | `d9fb993402a96d5f0f3172420e221ba4a4ef81df` |
| `repo-code-graph` | `structural` | 12,918,863 | `6a56f379185e3fdd3e9751c273bdf35572eb19a5186824784b52c2494c86e6a9` | `47cea7587da1bfab5a76279cfbf68c0aaf1ec643` |

`config-audit` and `crate-map` are byte-identical (confirmed by `shasum -a 256` on both
files, matching the plan's stated size and hash). `repo-code-graph` is a different size
with a different hash and a later mtime — it is **not** part of the duplication. So
"three identical corpora" is one file short: it's two.

The reason `repo-code-graph` differs is not a different `extraction_mode` branch — it's
a different build. Its manifest (`git_sha: 47cea758...`) was built at a later commit
than `config-audit`/`crate-map` (`git_sha: d9fb9934...`), and the repo tree grew between
those two commits: `repo-code-graph`'s manifest reports `node_count: 30610,
edge_count: 31550` vs. `config-audit`/`crate-map`'s `node_count: 30538, edge_count:
31481`. Rebuilding `repo-code-graph` at the same git sha as the other two would very
likely reproduce the same byte-identical output as them, for the reason below — the
three corpora were simply not all rebuilt from the same tree state on this measurement
pass.

## Root cause

`crates/vox-graph-reader/src/rebuild.rs::rebuild_graph` branches on
`meta.extraction_mode` at exactly two places:

- `crates/vox-graph-reader/src/rebuild.rs:205` — `gui_wiring = extraction_mode ==
  Some("gui-wiring")`, gating whether the CLI-catalog/transport-wrapper join logic
  runs at all.
- `crates/vox-graph-reader/src/rebuild.rs:418` — `extraction_mode == Some("modules")`
  selects `super::lens::collapse_to_modules(&structural_graph)` instead of emitting
  the raw structural graph.

`contracts/retrieval/vox-graph-corpora.v1.yaml` declares four non-virtual
`extraction_mode` values across its corpora: `structural` (`repo-code-graph`,
`vox-config-graph`), `gui-wiring` (`vox-gui-surface`), `audit` (`config-audit`), and
`crate-map` (`crate-map`). Neither `"audit"` nor `"crate-map"` — nor, for that matter,
`"structural"` — matches either branch condition above (`"gui-wiring"` or
`"modules"`). All three fall through to the same unmodified `structural_graph` value
with no mode-specific extraction step applied.

So for any two corpora that (a) share a `scope_path` and (b) are built from the same
git tree state, and whose `extraction_mode` is one of `structural`/`audit`/`crate-map`
(i.e. not `gui-wiring` and not `modules`), the rebuild pipeline is *guaranteed* to
produce byte-identical `graph.json` output — the declared mode is recorded in the
manifest (`GraphifyManifest.extraction_mode`) but never changes what
`rebuild_graph` actually does. `config-audit`, `crate-map`, and `repo-code-graph` all
use `scope_path: "."` (whole repo), so this applies to all three: the only reason
`repo-code-graph`'s bytes differ today is that it was rebuilt at a different commit,
not that its `extraction_mode` did anything.

**This is a real defect in the declared extraction modes, not a caching bug.** The
`audit` and `crate-map` modes are documented, registered corpora with distinct
`default_for_intents` (`config_audit` / `build_time`, `crate_arrangement`) implying
distinct extraction semantics, but no such semantics exist in the implementation.

## Wasted bytes (measured, not estimated)

`config-audit/graph.json` and `crate-map/graph.json` are both 12,890,417 bytes on disk,
byte-for-byte identical. One of the two is redundant storage: **12,890,417 bytes
(~12.3 MiB) wasted**, out of the ~138–150 MB total measured graphify cache footprint
cited in the P6 plan. This is a lower bound: if `repo-code-graph` were rebuilt at the
same commit as the other two, it would very likely also become byte-identical to them
(same reasoning above), which would put the wasted total closer to 2× that figure
(~24.6 MiB across three copies of one graph instead of two).

## What the fix would be, and who owns it

The fix is **not** to delete a cache directory — a cache concluded to be redundant is
exactly the one whose regeneration cost has not been measured, and deleting either
`config-audit` or `crate-map` would just cause the next `vox graphify status`/rebuild
to silently regenerate an identical copy at the same disk cost. The actual fix has two
independent parts, both outside this task's scope (`crates/vox-config/`) and outside
files this task may write:

1. **Implement `audit` and `crate-map` as real extraction modes** in
   `crates/vox-graph-reader/src/rebuild.rs::rebuild_graph` (add branches alongside the
   existing `gui-wiring`/`modules` ones), so each corpus's output actually reflects its
   declared purpose instead of falling through to the generic structural graph. Owned
   by whichever stream owns `vox-graph-reader` (this task may not edit that crate).
2. **Alternatively (or additionally), de-duplicate storage** for corpora that share a
   `scope_path` and, until (1) lands, an identical extraction result — e.g. content-hash
   the graph and have the registry point a second corpus id at the same on-disk file
   (or a symlink) instead of a full second copy — a decision for whoever owns
   `contracts/retrieval/vox-graph-corpora.v1.yaml`'s corpus model and
   `crates/vox-cli/src/commands/graphify/mod.rs`'s rebuild orchestration.

Neither change was made as part of this investigation: this document is the report
requested by task P6-2c, not the fix.
