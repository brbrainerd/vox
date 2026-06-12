---
title: Crate Consolidation & Dependency Currency Audit (2026-06-12)
description: Verified audit of LoC deletable by using crates (in-tree or ecosystem), under-use of existing workspace dependencies, and dependency upgrades worth taking. 10 codebase lenses + web version research, with adversarial per-finding verification (each verifier read the code and checked crate health). Includes executed quick wins, the prioritized backlog, and the verified-rejected list with reasons.
category: "Architecture SSOTs"
---

# Crate Consolidation & Dependency Currency Audit (2026-06-12)

**Method:** 10 parallel codebase-discovery lenses + 3 web version-research agents over the 107-crate workspace; 66 raw findings deduped to 50; top findings each adversarially verified by an agent required to (a) read the cited code and (b) web-check the candidate crate's health, plus a completeness critic. Past audits here ran ~50% false-positive; the verifiers killed or corrected several findings accordingly (see §4).

**Headline:** the dominant pattern is **not** "missing crates" — it is **under-use of crates already in `workspace.dependencies`** (walkdir, comfy-table, chrono, tempfile, which, base64, urlencoding, dirs, reqwest) and **duplicate hand-rolled helpers across crates**. Roughly ~1,400 LoC of verified deletions, most needing zero new dependencies and therefore zero build-time cost.

## 1. Executed (this branch, committed)

**Wave 1 — quick wins:**

| Change | LoC | Commit notes |
|---|---|---|
| Delete orphan `vox-actor-runtime/src/rate_limit.rs` (never compiled; codegen emits `governor` directly) | −65 | verified zero refs |
| Delete dead `vox-cli/src/render.rs` (`#![allow(dead_code)]`, zero call sites, contained a real multibyte-slice panic at line ~295) | −396 | critic find, hand-verified |
| cr-p1/cr-p2: hand-rolled TcpStream HTTP/1.1 clients → `reqwest::blocking` (already a dep of vox-audit) | −130 | fixes https-URL + IPv6 latent bugs; `Policy::none()` preserves 3xx=not-live |
| `vox_http_client::parse_retry_after` SSOT + `Utf8LineBuffer` reuse in vox-gamify transport (7 call sites) | −80 | verifier rejected tokio-util; in-tree SSOT was better |

**Wave 2 — verified backlog items 1–10 (subset), each adversarially re-verified by a code-reading agent before applying:**

| Change | LoC | Commit notes |
|---|---|---|
| chrono over hand-rolled date math: `vox-orchestrator/memory/time.rs` (−141, dropped the no-dep calendar engine + vestigial `unix_secs_to_ymd`) + `vox-ml-cli/populi_attest.rs` ISO-8601 formatter | −133 | checkpoint `now_utc()` sites left alone (would change on-disk format) |
| strsim over 6 hand-rolled Levenshtein DP loops (vox-cli ×2, vox-compiler, vox-db, vox-search, vox-populi) | −132 | vox-speech generic word-level WER `levenshtein<T>` correctly skipped; strsim has zero transitive deps |
| `which::which` for PATH probe (debug.rs) + `tempfile::persist` atomic write (fmt.rs) | −24 | load-bearing custom-semantics sites (cli-tests `.cmd`-first, binary_ssot collect-all, vox-config env-first dirs) intentionally skipped |

Total executed: **~1,000 LoC deleted** across 8 commits; all touched crates compile, 488 vox-cli lib tests pass (1 pre-existing environmental failure: `policy_registry::...parity_passes_for_default_domains` fails on clean tree too — sandbox stdin-redirect limitation), clippy green.

Also verified: **CVE-2026-44471** (gix-fs symlink worktree escape, critical) — fixed in gix-fs 0.21.1; our Cargo.lock resolves **0.21.2 → already patched** ([GHSA-f89h-2fjh-2r9q](https://github.com/GitoxideLabs/gitoxide/security/advisories/GHSA-f89h-2fjh-2r9q)).

Side-finding (tooling): the arch-check orphan gate misses undeclared-module `.rs` files inside a crate's `src/` — both deletions above were invisible to it.

## 2. Verified adopt backlog (prioritized; all "already in tree" unless noted)

Each row was confirmed by a verifier that read the code. Net LoC are glue-corrected.

> **Status:** items 2 (chrono), 8 (which), 9 (tempfile fmt.rs subset), strsim (item 11), and item 1 (walkdir) are **DONE** (§1 Waves 2–3). Remaining below are still open.
>
> **Wave 3 (walkdir):** 11 of 14 hand-rolled `fs::read_dir` walkers across vox-arch-check (1, +dep), vox-audit (1), vox-cli (9) migrated to `WalkDir`, ~−100 LoC, per-site error semantics preserved, `walk_prune_skips_target_directory` green. **3 sites deliberately deferred** (retired_symbol_check docs stack-loop + stub_check shell = brace-delicate mid-function scaffold swaps; `ars/discover` `max_depth+1` mapping subtlety) — marginal LoC, higher risk; flagged for a focused pass.
>
> **PRUNED — windows-sys 0.59→0.61 dedup is illusory:** `cargo tree -d` shows four versions (0.48/0.52/0.59/0.61). **rustix 0.38.44 (deep transitive dep) pins 0.59** and 0.61 is already in-tree, so bumping vox-cli's direct pin does NOT remove the 0.59 compile. The claimed build-time win does not materialize. (The §3 row should be read with this correction.)
>
> **Wave 4 (serde_yaml → serde_yaml_ng):** the archived `serde_yaml` 0.9 (final `0.9.34+deprecated`) replaced workspace-wide with the maintained `serde_yaml_ng` 0.10 fork via a **single Cargo package-rename line** (`serde_yaml = { package = "serde_yaml_ng", version = "0.10" }`) — zero source changes across all 28 consumer crates. Used API surface is core-only (`from_str`/`Value`/`to_string`/`Error`/`from_slice`), all present in the fork. Verified: `cargo check --workspace --exclude vox-gui` clean (vox-gui fails pre-existing on absent `ui/dist` pnpm frontend, unrelated); vox-config (53) + ecosystem-support parity (9, parse real contract YAML) green. This is the §3 "do-now" serde_yaml item, **DONE**.

1. **walkdir consolidation, ~12 hand-rolled `fs::read_dir` recursions** (~150 net LoC) — **DONE (Wave 3)** except the 3 deferred sites noted above. — vox-cli `commands/ci/{attention_ledger_parity, canonical_docs, command_compliance/validators (×2), db_schema_coverage, retired_symbol_check (×3 + 1 iterative), row_serde_lint, string_id_lint}`, `diagnostics/stub_check`, `migrate`, `extras/ars/discover` (use `max_depth`), vox-audit `regression_budget`, vox-arch-check `walk_repo_files`. vox-cli/vox-audit *already depend on walkdir and use it elsewhere in the same directory tree*. Preserve each site's swallow-vs-propagate error semantics; keep arch-check's `walk_prune_skips_target_directory` test green. Two cited sites (safety_inventory, code-audit scanner) already use WalkDir — excluded. Critic notes ~48 files use raw `read_dir` across vox-cli+vox-orchestrator, so the full population is larger than this verified set.
2. **chrono for hand-rolled date/time math** (~150 net LoC) — `vox-orchestrator/src/memory/time.rs` (whole file; vox-orchestrator already depends on chrono), `vox-ml-cli/src/commands/populi_attest.rs:147-206` (~58), plus the three identical `checkpoint_state.rs` copies in vox-populi / plugin-mens-candle-cuda / -metal.
3. **comfy-table for hand-drawn tables** (~140 net LoC) — `vox-ml-cli/commands/mens/status.rs` box panel (real ANSI-width alignment bug: `{:<40}` pads after owo-colors styling) and `vox-cli/commands/db_research/reliability.rs`; "maybe": a shared table helper for vox-cli search/list commands (~50).
4. **gray_matter or one shared frontmatter splitter** (~75 net LoC, 5 copies) — vox-corpus `extract_docs`, vox-search `memory_hybrid`, vox-plugin-host `skill_parser`, vox-cli `agentskills_compliance` + `stub_check/fix_pipeline`. Must target the serde_yaml **replacement** (§3), not serde_yaml.
5. **dirs for home/data-dir resolution** (~65 net LoC) — `vox-config/src/paths.rs` + duplicates.
6. **`opener` (or `webbrowser`) for open-in-browser/file-manager** (~54 LoC; *new dep, tiny*) — `vox-cli/src/fs_utils.rs` + verbatim copy in `vox-cli-core/src/fs_utils.rs` + vox-gui `search.rs`.
7. **base64 URL_SAFE_NO_PAD + urlencoding in vox-publisher ORCID OAuth** (~55 net LoC; dep lines added to vox-publisher only) — `scholarly/orcid_oauth.rs` hand-rolls base64url and percent-encoding (correctness-sensitive). (Two audit rows double-counted this file; merged here.)
8. **which for PATH probing** (~55 net LoC) — vox-cli-tests, vox-cli `debug.rs`, doctor `binary_ssot.rs`.
9. **tempfile atomic-write pattern** (~45 net LoC verified for fmt.rs + 3 siblings; the full cluster across populi/plugins/audit/publisher from discovery is ~14 sites) — best as one shared `write_atomic` helper in a low-layer crate built on `NamedTempFile::persist`.
10. **copy_dir_recursive dedup onto walkdir** (~29 net LoC) — vox-cli `fs_utils/run/bundle` 3 duplicates + vox-repository `workspace_path_migration`.
11. **strsim for 7 hand-rolled Levenshtein copies** (~150–200 LoC; *unverified — verifier died twice on infra*; critic calls it the largest unflagged cluster) — vox-cli diagnostics, vox-db error_enrichment, vox-compiler tokens, vox-search symbol_proximity, vox-populi estimator, vox-speech eval, vox-cli speech_runtime_suite. Verify before executing.
12. **Duration parsing/formatting consolidation** (critic find, unverified) — 5 `parse_duration` copies + 2 `format_duration` copies → `humantime` 2 or the existing `vox-workflow-runtime::duration_literal` SSOT.
13. **Glob matching** (~40 LoC, critic find) — hand-rolled `glob_match` in vox-gui `search.rs` and vox-cli `docs_reality_audit.rs` despite `glob` AND `globset` in workspace deps.

Unverified tail (smaller, plausible; spot-check before doing): lru for `SchemaCache` in vox-orchestrator-mcp; `dunce` for the two Windows `\\?\`-stripping canonicalize helpers; hex::encode for ~15 manual `format!("{:02x}")` loops; `json!` macro for manual `serde_json::Map` building (~8 sites); globset for scope-list matching in orchestrator-mcp `submission.rs`; serde-derive for the ~45 manual `Value` accesses in `mens/status.rs` and ~30 in `orch_daemon`.

## 3. Dependency currency (web-verified 2026-06-12)

**Do now (cheap or security):**
- **serde_yaml 0.9 — archived/deprecated** (final release `0.9.34+deprecated`). Replace with **serde_yaml_ng** or **serde-norway** (API-compatible swap). Do **not** pick `serde_yml` (RUSTSEC-2025-0068, unsound).
- **windows-sys 0.59 → 0.61** — ecosystem moved; the direct 0.59 pin forces a duplicate ~334k-LoC bindings compile on every Windows build. Straight build-time win; verify with `cargo tree -d`.
- **tiktoken-rs 0.5 → 0.12** — token counting for o200k/gpt-4o/o-series/gpt-5 is wrong or missing today; small Result-returning API change.
- **reqwest 0.12 → 0.13 wave** (with reqwest-middleware 0.5, reqwest-retry 0.9) — rustls now default → collapses duplicate TLS stacks; enable new `query`/`form` features where used.
- **jsonwebtoken 9 → 10** — pick the `rust_crypto` backend feature (avoids aws-lc-rs dragging cmake/NASM into Windows builds).
- **notify 6 → 8** — fixes unaligned `FILE_NOTIFY_INFORMATION` access on Windows; 7.0 dropped always-on crossbeam-channel.
- **turso 0.4 → 0.6.1** — data-correctness fixes for VoxDB's engine; land 0.6.1, don't chase 0.7-pre.
- **tokenizers 0.21 → 0.23** — newer model tokenizer.json formats; pairs with hf-hub later (1.0 RC in flight, wait for final).
- **quick-xml 0.31 → 0.40** — DoS-relevant malformed-input panic fixes; mechanical but touches every parse site.
- **scraper 0.20 → 0.27 + html2text 0.12 → 0.17** in one PR (shared html5ever tree; html2text's panicking API became Result).
- **tokio-tungstenite 0.24 → 0.29** — Bytes/Utf8Bytes zero-copy payloads.
- **peft-rs 1.0.3** patch bump; re-validate the qlora-rs local patch after.

**Plan as dedicated waves:**
- **rand dual-pin (0.8 + 0.9 → eventually 0.10):** wave 1 = finish 0.8→0.9 (10 crates; gen→random, thread_rng→rng), retire the `rand09` alias; wave 2 = 0.9→0.10 later. Unblocks rand_distr 0.4→0.6.
- **toml 0.8 → 1.x + toml_edit 0.22 → 0.25** same PR (shared internals). Watch the silent order-preservation default change.
- **tantivy 0.22 → 0.26** own PR with reindex handling (0.24 reads 0.22 indices; later may not). Keep the feature gate.
- **Audio stack: symphonia 0.6 + rubato 3.0 + cpal 0.18** one migration PR (large API overhauls; cpal streams no longer auto-start — silent-failure trap). Real Windows wins (WASAPI auto-reroute).
- **RustCrypto 0.11 wave** (aes-gcm + chacha20poly1305 + argon2 0.6) — wait for finals, bump together; vox-crypto publishes, so semver-relevant.
- **schemars08 dual-pin retirement** — blocked on typify; sole consumer is vox-scientia-jsonschema-codegen.
- **candle 0.9 → 0.10** — stays deferred (0.10 does not retire the kernels patch).
- **keyring v3 → keyring-core split** — maintainer warns against blind v4 upgrade; fold into the planned vox-secrets A-9 L0/L3 split.

**Skip (verified deliberate):** bincode 3 (orphan final release of an archived project; stay 2.x, watch bincode-next), abi_stable 0.11 (dormant but ABI-stable is the point; RustSec watch only), hound (WAV is frozen), deadpool 0.13 (wiremock pins ^0.12 — bumping splits the dep), criterion/similar/lru/libloading/dialoguer (fine, opportunistic; libloading 0.9 only with a plugin-loader test pass).

## 4. Verified-rejected findings (do NOT do; reasons matter)

- **heck for case conversions** — current naive `to_snake_case` output is **persisted** in user DB table names (`"HTTPRoute" → "h_t_t_p_route"`, pinned by test) and generated TS export identifiers; heck silently renames both = data-orphaning migration event. The `@json rename_all` impl deliberately mirrors serde's naive rules (serde doesn't use heck either). Only safe site: clap-facing `kebab_to_pascal` (12 LoC, not worth it). The two `slugify_heading` copies are worth deduping in-tree, not via the `slug` crate (wrong semantics).
- **failsafe for the vox-db circuit breaker** — failsafe exposes no state/failure-count introspection; preserving the vox-db-types `CircuitState` public API means shadow-tracking state = rebuilding what you deleted. The HalfOpen double-probe race is benign; a ~5-line CAS fix beats the dep (crate also 2 years stale with rand ^0.8 pin).
- **backon for retry loops** — repo has **twice** documented evaluating and rejecting backon (resilient_http.rs:3-6, social_retry.rs:3-5) in favor of vox_foundation primitives; real fix for candle_whisper is deduping the two ~800-line near-identical backend files, not the 28-LoC retry loop.
- **console::truncate_str for truncation helpers** — the six helpers implement three different contracts (display chars / persisted byte caps / grapheme platform limits); display-width truncation would regress two of them. One-line bug worth fixing inline: `summarize_text` compares bytes but takes chars.
- **termimad/dialoguer for render.rs** — module was dead; deleted instead (§1).
- **tokio-util LinesCodec** — in-tree `Utf8LineBuffer` superior; done (§1).

## 5. Build-time guardrails observed throughout

New-dep adds in the backlog are limited to tiny crates (`opener`/`dunce`/`strsim`/`gray_matter`); everything else rides existing deps. The biggest build-time levers found are dedup-shaped: windows-sys 0.59 pin, rand dual-pin, schemars dual-pin, reqwest-0.13 TLS-stack collapse, deadpool non-bump. Keep tantivy/wasmtime behind their existing feature gates.
