# Plan C — Graphify Run Lifecycle + Autonomous Rerun (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).
> **DEPENDS ON Plan A** (`RebuildMeta`, freshness-correct manifest). Land A first.

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A signature change fixes all callers in the SAME task. Crash between tasks → compiling, tested tree.
2. **Verify-before-use.** First step of each task is an `rg`/read confirming exact symbols. Differs → STOP, do not invent.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Fails twice → STOP + handoff note (what failed, last good SHA). No looping.
5. **Parallel dispatch.** Honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all` (`-p <crate>`); automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs.
7. **Verification ritual before commit** (skill `verification-before-completion`), paste output: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`.
8. **Rollback on broken tree:** `git reset --hard HEAD`; re-attempt the single task.
9. **Skills:** `brainstorming` / `dispatching-parallel-agents` / `using-git-worktrees`.
10. **Determinism + no `.unwrap()` on I/O in lib code.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** Close the freshness loop (`lexical_lag` actually round-trips) and add an autonomous `vox graphify refresh --auto` that decides — per a deterministic cost/value policy — whether each stale corpus needs a native rebuild, a Turso re-ingest, or nothing.

**Architecture:** (C1) add `write_manifest` + `set_lexical_ingest_sha256` to `vox-config` (no writer exists today). (C2) add a shared `graph_digest` (BLAKE3) helper and make `vox graphify ingest` stamp `lexical_ingest_sha256` so `lexical_lag` clears after ingest and re-fires after a later rebuild. (C3) a pure `refresh_action(stale_reasons)` policy + a `Refresh` subcommand that, with `--auto`, executes the chosen action natively.

**Tech Stack:** Rust; `serde_json`; `blake3` (via `vox-graphify-reader`); `chrono` (CLI); `clap`.

> **Scope note (deferred, not placeholder):** the *usage-driven* value score (rebuild only when a corpus has recent search-log hits) needs a metadata-filtered DB query that does **not** exist (`vox-db` has only `query_knowledge_nodes(query, limit)`). That belongs to Plan D's learning loop. Plan C's gate is a deterministic stale-reason policy — fully airtight, no new DB method.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-config/src/graphify.rs` | manifest writer + lexical stamp | Modify (C1) |
| `crates/vox-graphify-reader/src/lib.rs` | `graph_digest` helper | Modify (C2) |
| `crates/vox-cli/src/commands/graphify/mod.rs` | ingest stamp; `Refresh` cmd + policy | Modify (C2, C3) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "pub struct GraphifyManifest|fn read_manifest|enum GraphifyError|MANIFEST_BASENAME" crates/vox-config/src/graphify.rs` — confirm `GraphifyManifest` derives `Default`, `read_manifest(path) -> Option<GraphifyManifest>` is private (same module), and `GraphifyError::{Io,Parse}` variants exist.
- `rg -n "GraphifyCmd::Ingest|fn upsert_projected_nodes|fn load_projected_nodes|fn resolve_head_sha|use chrono::Utc" crates/vox-cli/src/commands/graphify/mod.rs` — confirm the ingest arm + helpers.
- `rg -n "blake3" crates/vox-graphify-reader/Cargo.toml` — confirm `blake3` is a dep of the reader.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task C1 `[SEQUENTIAL]`: Manifest writer + lexical stamp

**Files:**
- Modify: `crates/vox-config/src/graphify.rs`
- Test: inline `#[cfg(test)]` in `crates/vox-config/src/graphify.rs`

- [ ] **Step 1 (verify-before-use):** Run the first Pre-flight line. Confirm `#[derive(... Default)] pub struct GraphifyManifest`, private `fn read_manifest`, and `GraphifyError::Parse { path, detail }` / `GraphifyError::Io { path, source }`. Differs → STOP.

- [ ] **Step 2: Write the failing test.** In `graphify.rs` `mod tests`:

```rust
#[test]
fn lexical_stamp_clears_and_refires_lag() {
    let tmp = tempfile::tempdir().unwrap();
    let mpath = tmp.path().join(".graphify_manifest.v1.json");
    let m = GraphifyManifest { graph_json_sha256: Some("x".into()), ..Default::default() };
    write_manifest(&mpath, &m).unwrap();

    set_lexical_ingest_sha256(&mpath, "x").unwrap();
    let after = read_manifest(&mpath).unwrap();
    assert!(lexical_lag_stale_reason(&after).is_none(), "matched sha → no lag");

    set_lexical_ingest_sha256(&mpath, "y").unwrap();
    let after2 = read_manifest(&mpath).unwrap();
    assert_eq!(lexical_lag_stale_reason(&after2).as_deref(), Some("lexical_lag"));
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-config lexical_stamp_clears_and_refires_lag` → FAIL (`write_manifest`/`set_lexical_ingest_sha256` missing).

- [ ] **Step 4: Implement** after `read_manifest` in `graphify.rs`:

```rust
/// Write a manifest to disk (pretty JSON).
pub fn write_manifest(path: &Path, manifest: &GraphifyManifest) -> Result<(), GraphifyError> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| GraphifyError::Parse {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    fs::write(path, json).map_err(|source| GraphifyError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Read-modify-write the manifest's `lexical_ingest_sha256` (creates a minimal manifest if absent).
pub fn set_lexical_ingest_sha256(manifest_path: &Path, sha: &str) -> Result<(), GraphifyError> {
    let mut manifest = read_manifest(manifest_path).unwrap_or_default();
    manifest.lexical_ingest_sha256 = Some(sha.to_string());
    write_manifest(manifest_path, &manifest)
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-config lexical_stamp_clears_and_refires_lag` → PASS.

- [ ] **Step 6: Verify (Rule 7) + commit.**

```bash
git add crates/vox-config/src/graphify.rs
git commit -m "feat(graphify): manifest writer + lexical_ingest_sha256 stamp"
```

---

## Task C2 `[SEQUENTIAL]`: Ingest stamps the lexical sha (closes the lag loop)

**Files:**
- Modify: `crates/vox-graphify-reader/src/lib.rs` (digest helper)
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (Ingest arm)
- Test: `crates/vox-graphify-reader/tests/rebuild_tests.rs` (digest determinism)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "GraphifyCmd::Ingest|fn corpus_by_id|load_all_corpora|load_graphify_corpora" crates/vox-cli/src/commands/graphify/mod.rs`. Confirm the Ingest arm resolves `corpus_id` and that a `corpus_by_id` helper exists. Note whether Plan B switched loads to `load_all_corpora` (use whichever load fn the file currently uses).

- [ ] **Step 2: Write the failing digest test.** Append to `crates/vox-graphify-reader/tests/rebuild_tests.rs`:

```rust
#[test]
fn graph_digest_is_stable_and_distinct() {
    let a = vox_graphify_reader::graph_digest(b"{\"nodes\":[]}");
    let b = vox_graphify_reader::graph_digest(b"{\"nodes\":[]}");
    let c = vox_graphify_reader::graph_digest(b"{\"nodes\":[{}]}");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.len() >= 32);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test rebuild_tests graph_digest_is_stable_and_distinct` → FAIL (`graph_digest` missing).

- [ ] **Step 4: Add the helper** in `crates/vox-graphify-reader/src/lib.rs` (top-level, after the module decls):

```rust
/// BLAKE3 hex digest of graph bytes. The single source of truth for `graph_json_sha256`
/// (rebuild) and `lexical_ingest_sha256` (ingest) so `lexical_lag` comparisons are valid.
pub fn graph_digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
```

- [ ] **Step 5: Stamp the manifest after ingest.** In `crates/vox-cli/src/commands/graphify/mod.rs`, in the `GraphifyCmd::Ingest` arm, after the line that prints `graphify ingest: corpus=... upserted=...`, add:

```rust
            // Stamp lexical_ingest_sha256 = digest of the graph just projected, so lexical_lag
            // clears now and re-fires after a later rebuild changes graph_json_sha256.
            let corpus = corpus_by_id(&reg, &corpus_id)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let graph_bytes = std::fs::read(repo_root.join(&corpus.graph_path))
                .with_context(|| format!("read graph for digest: {}", corpus.graph_path))?;
            let digest = vox_graphify_reader::graph_digest(&graph_bytes);
            vox_config::graphify::set_lexical_ingest_sha256(
                &repo_root.join(&corpus.manifest_path),
                &digest,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
```

(`reg` is the registry already loaded in this arm; `corpus_by_id` already exists. If the arm shadowed `reg`, reuse the in-scope binding.)

- [ ] **Step 6: Run + build.** `cargo test -p vox-graphify-reader --test rebuild_tests graph_digest_is_stable_and_distinct` → PASS. `cargo build -p vox-cli` → clean.

- [ ] **Step 7: Smoke (manual, paste output).** With a built `vox`: `vox graphify rebuild --corpus repo-code-graph` then `vox graphify ingest --corpus repo-code-graph` then `vox graphify status --corpus repo-code-graph` → must NOT list `lexical_lag`. (If the DB is unavailable, ingest fails before the stamp — note it; the unit-tested pieces still hold.)

- [ ] **Step 8: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/lib.rs crates/vox-cli/src/commands/graphify/mod.rs crates/vox-graphify-reader/tests/rebuild_tests.rs
git commit -m "feat(graphify): ingest stamps lexical_ingest_sha256 — closes the lexical_lag loop"
```

---

## Task C3 `[SEQUENTIAL]`: Autonomous `vox graphify refresh --auto` + policy gate

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs`
- Test: inline `#[cfg(test)]` in `crates/vox-cli/src/commands/graphify/mod.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "enum GraphifyCmd|fn assess_all|fn resolve_source_dir|fn upsert_projected_nodes|fn load_projected_nodes" crates/vox-cli/src/commands/graphify/mod.rs`. Confirm the enum + helpers. (`resolve_source_dir` exists only if Plan B landed; if not, use `repo_root.join(&corpus.scope_path)`.)

- [ ] **Step 2: Write the failing policy test.** In `mod.rs` `mod tests`:

```rust
#[test]
fn refresh_action_maps_reasons() {
    use super::{refresh_action, RefreshAction};
    assert_eq!(refresh_action(&["graph_missing".into()]), RefreshAction::Rebuild);
    assert_eq!(refresh_action(&["git_drift".into()]), RefreshAction::Rebuild);
    assert_eq!(refresh_action(&["ttl_expired".into()]), RefreshAction::Rebuild);
    assert_eq!(refresh_action(&["lexical_lag".into()]), RefreshAction::Ingest);
    assert_eq!(refresh_action(&[]), RefreshAction::Skip);
    // rebuild dominates a co-occurring lexical_lag
    assert_eq!(refresh_action(&["git_drift".into(), "lexical_lag".into()]), RefreshAction::Rebuild);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-cli refresh_action_maps_reasons` → FAIL (missing).

- [ ] **Step 4: Implement the pure policy** near the top of `mod.rs`:

```rust
/// What an autonomous refresh should do for a corpus, given its stale reasons.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RefreshAction {
    Rebuild,
    Ingest,
    Skip,
}

/// Deterministic cost/value gate: a structural change (missing/corrupt/drift/ttl) needs a
/// native rebuild; a lexical-only lag needs a cheap re-ingest; otherwise do nothing.
pub(crate) fn refresh_action(stale_reasons: &[String]) -> RefreshAction {
    let has = |r: &str| stale_reasons.iter().any(|s| s == r);
    if has("graph_missing") || has("graph_corrupt") || has("git_drift") || has("ttl_expired") {
        RefreshAction::Rebuild
    } else if has("lexical_lag") {
        RefreshAction::Ingest
    } else {
        RefreshAction::Skip
    }
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-cli refresh_action_maps_reasons` → PASS.

- [ ] **Step 6: Add the `Refresh` subcommand.** Add to `GraphifyCmd` after `Rebuild` (or after `Index` if Plan B landed):

```rust
    /// Assess all corpora and (with --auto) rebuild/ingest each stale one per policy.
    Refresh {
        /// Corpus id (default: all corpora).
        #[arg(long)]
        corpus: Option<String>,
        /// Execute the chosen action; without it, only print what would happen.
        #[arg(long)]
        auto: bool,
    },
```

Add the arm in `run()` (reuses `assess_all`, `refresh_action`, the rebuild meta build from the `Rebuild` arm, and the ingest path from the `Ingest` arm):

```rust
        GraphifyCmd::Refresh { corpus, auto } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let head = resolve_head_sha()?;
            let statuses = assess_all(repo_root, &reg, &corpus, head.as_deref())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            for s in &statuses {
                if s.is_fresh {
                    println!("fresh   {}", s.corpus_id);
                    continue;
                }
                let action = refresh_action(&s.stale_reasons);
                println!("{:?}  {} (stale: {})", action, s.corpus_id, s.stale_reasons.join(","));
                if !auto {
                    continue;
                }
                let c = corpus_by_id(&reg, &s.corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                match action {
                    RefreshAction::Rebuild => {
                        let source_dir = resolve_source_dir(repo_root, c);
                        let output_file = repo_root.join(&c.graph_path);
                        let cache_dir = output_file
                            .parent().ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                            .join("file_cache");
                        let meta = vox_graphify_reader::rebuild::RebuildMeta {
                            corpus_id: c.id.clone(),
                            git_sha: head.clone(),
                            scope_path: c.scope_path.clone(),
                            extraction_mode: c.extraction_mode.clone(),
                            built_at_rfc3339: Utc::now().to_rfc3339(),
                        };
                        vox_graphify_reader::rebuild::rebuild_graph(repo_root, &source_dir, &output_file, &cache_dir, &meta)
                            .map_err(|e| anyhow::anyhow!("refresh rebuild {}: {e}", c.id))?;
                        println!("  rebuilt {}", c.id);
                    }
                    RefreshAction::Ingest => {
                        let nodes = load_projected_nodes(repo_root, &reg, &c.id)?;
                        let upserted = tokio::runtime::Builder::new_current_thread()
                            .enable_all().build().context("tokio runtime for refresh ingest")?
                            .block_on(upsert_projected_nodes(&nodes))?;
                        let graph_bytes = std::fs::read(repo_root.join(&c.graph_path))
                            .with_context(|| format!("read graph for digest: {}", c.graph_path))?;
                        vox_config::graphify::set_lexical_ingest_sha256(
                            &repo_root.join(&c.manifest_path),
                            &vox_graphify_reader::graph_digest(&graph_bytes),
                        ).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        println!("  ingested {} ({} nodes)", c.id, upserted);
                    }
                    RefreshAction::Skip => {}
                }
            }
        }
```

> If Plan B did NOT land, replace `load_all_corpora` with `load_graphify_corpora` and `resolve_source_dir(repo_root, c)` with `repo_root.join(&c.scope_path)`.

- [ ] **Step 7: Build + smoke.** `cargo build -p vox-cli` → clean. `cargo run -p vox-cli -- graphify refresh` (prints actions, no execution); `cargo run -p vox-cli -- graphify refresh --auto` on a repo with a missing graph → rebuilds it; re-run `refresh` → `fresh`.

- [ ] **Step 8: Verify (Rule 7) + arch-check + commit.**

```bash
git add crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): autonomous refresh --auto with deterministic cost/value gate"
```

---

## Parallelization summary
- **C1 → C2 → C3 strict SEQUENTIAL** (C2 uses C1's `set_lexical_ingest_sha256`; C3 reuses C2's stamp + rebuild/ingest paths; C2/C3 share `mod.rs`).

## Self-Review
- **Spec coverage:** "know when they expire" (C1 closes lexical_lag), "automate rerunning" + "decide if we want to" (C3 `refresh --auto` + `refresh_action` gate).
- **Placeholder scan:** none. Usage-driven scoring is explicitly DEFERRED to Plan D (needs a non-existent DB query) — scoped down, not stubbed.
- **Type consistency:** `set_lexical_ingest_sha256`/`write_manifest` (C1) used verbatim in C2/C3; `graph_digest` identical in C2/C3; `RefreshAction`/`refresh_action` identical across test + arm.
- **Antigravity fit:** atomic+green+commit; verify-before-use first; the DB-dependent ingest path is build+smoke-verified while the pure policy + manifest logic is unit-tested (a fast model can't silently break the tested core).
