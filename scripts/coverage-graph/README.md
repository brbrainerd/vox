# Semantic Test-Coverage Toolchain

Deterministic + LLM tools that build a **searchable map of what the test suite actually
proves** — overlaid on the graphify code graph. The point is the gap between three
strengths of coverage:

| Strength   | Meaning                                           | Source                    |
|------------|---------------------------------------------------|---------------------------|
| `reached`  | symbol's code executed during *some* test         | llvm-cov (`ingest_reaches`) |
| `targeted` | a test references the symbol                       | `overlay_tests` (static)  |
| `proven`   | a test **asserts** on the symbol's behavior        | `overlay_tests` + LLM behaviors |

`reached − proven` is the keystone: code that runs in a test but proves nothing.

> **Language-policy note (VoxScript-first deferral).** These tools are Python, which
> AGENTS.md §VoxScript-First Glue Code bans for new project automation; no exemption
> mechanism exists. The Vox rewrite is **deferred, not waived**: the scripts are
> deterministic, stdlib-only analysis tooling (~2.3K lines, 43 pytest tests) and a
> faithful rewrite is out of scope for Phase 1. Tracked as a Phase 1.5 follow-up;
> do not extend these scripts — add new functionality in Vox.

## Pipeline

1. **Build the base code graph (115 crates, deterministic)**
   `python rebuild_full_graph.py . graphify-out/graph.full.json`

2. **Phase 1 — static targeted/proven overlay (deterministic)**
   `python overlay_tests.py --graph graphify-out/graph.full.json --repo-root . \
       --out graphify-out/graph.coverage.json --report graphify-out/COVERAGE_MAP.md`

3. **Phase 2 — LLM behavior extraction (per crate)**
   Run the throttled extraction Workflow `phase2_extract_v2.js` over the crate list
   (args = JSON array of crate names), then synthesize **deterministically** from the
   run journal — never via an LLM synth step (it fails on large crates and loses
   extraction):
   `python recover_and_synth.py --journal <run>/journal.jsonl --out-dir graphify-out`

   > **MUST throttle (large workspace).** Do NOT fan out one agent per crate for all
   > ~109 crates in a single 16-wide `parallel()` burst — it trips transient
   > server-side rate limiting (`Server is temporarily limiting requests`) after the
   > first wave and ~80% of crates fail. `phase2_extract_v2.js` processes the list in
   > **sequential chunks of 8** (parallel within a chunk, awaited between) to bound the
   > burst. Do not `.catch`-mask agent failures into `n:0` results — that hides them
   > from Workflow resume. If a run partially fails: run `recover_and_synth.py` on the
   > partial journal to lock in the crates that succeeded (synth is deterministic and
   > composes partial journals), then re-run `phase2_extract_v2.js` over only the
   > still-empty crate set. See memory `feedback-graphify-large-extraction-throttle`.

   Produces `COVERAGE_BEHAVIORS_<crate>.md` + the `COVERAGE_BEHAVIORS_INDEX.md` overview.

4. **Make it queryable**
   `python merge_behaviors_to_graph.py --journals-list graphify-out/_our_journals.txt \
       --graph graphify-out/graph.coverage.json --out graphify-out/graph.semantic.json`
   Install as canonical (`cp graph.semantic.json <repo>/graphify-out/graph.json`) →
   `graphify query "which behaviors are proven about X"`.

5. **Phase 0 — reached layer from CI coverage**
   CI (`.github/workflows/ci.yml`) already publishes the **`llvm-cov`** artifact, which
   includes `target/llvm-cov-lcov.info` (per-function `FNDA` execution counts). Download
   that artifact from a `main` CI run, then:
   `python ingest_reaches.py --lcov target/llvm-cov-lcov.info --graph graphify-out/graph.json \
       --out graphify-out/graph.json --report graphify-out/REACHED_VS_PROVEN.md`
   `REACHED_VS_PROVEN.md` ranks crates by **reached-but-unproven** symbol count.

   > Why not run Phase 0 in CI directly? The graph (~118 MB) is regenerated, not
   > committed, so it isn't present in the CI checkout. Phase 0 is therefore a local/
   > periodic step against the published lcov artifact. Local `cargo llvm-cov export` is
   > blocked on Windows (the `-object` list overflows the command-line limit, `os error
   > 206`); use the Linux CI artifact.

## Outputs (regenerable; large graphs are gitignored)
- `COVERAGE_MAP.md` — per-crate proven/targeted/unproven (Phase 1)
- `COVERAGE_BEHAVIORS_*.md` + `_INDEX.md` — per-crate proven behaviors + ranked gaps (Phase 2)
- `REACHED_VS_PROVEN.md` — reached-but-unproven keystone set (Phase 0)
- `DUPLICATION_AND_WIRING.md`, `OS_COMPATIBILITY.md` — companion structural audits
