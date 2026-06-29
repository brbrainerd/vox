# Secrets Categorization + GUI Search Codegen — Implementation Plan (rev 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the secrets classification SSOT so every key is correctly categorized (fixing the existing GUI panel grouping), then surface all secrets in the unified, drift-gated GUI settings search index — all generated from the Clavis spec registry.

**Architecture:** Phase A fixes `taxonomy_class` derivation (reuse `capabilities_for_secret` as the one classification source). Phase B emits secret rows into the *existing* `generatedSettingsIndex.ts` (the `SettingEntry.section` field is the discriminator — no second file, no new drift branch). Phase C wires/test the TS. Phase D is a separable hygiene-regex hardening.

**Tech Stack:** Rust (`vox-secrets` classification, `vox-cli` codegen), TypeScript/React (`vox-gui` settings index), Vitest (pnpm), `vox ci config-gui-codegen` CI drift gate.

**Why rev 2:** A 4-reviewer adversarial audit found (1) `metadata().taxonomy_class` is degenerate — ~everything is `AuxTooling`, so panel categories are broken; (2) the original two-file plan hit a dead `--check` early-return; (3) blanket `token`/`secret` keywords would flood the uncapped federated omnisearch; (4) a lossy id→label round-trip; (5) brittle string-contains tests. All folded in. See spec `docs/superpowers/specs/2026-06-29-clavis-ssot-secrets-gui-design.md`.

**House rules:** pnpm only (`npm` fails). Never `cargo fmt --all` on Windows — `cargo fmt -p <crate>`. `vox-gui` breaks `clippy --all-targets` — scope to the crate. Generated `.ts` is `eol=lf` (`.gitattributes:18`).

---

### Task 1: Repair the taxonomy classification SSOT (Phase A)

**Files:**
- Modify: `crates/vox-secrets/src/spec/types.rs` (add `TaxonomyClass::from_capability`)
- Modify: `crates/vox-secrets/src/spec/mod.rs` (add `taxonomy_class_for`)
- Modify: `crates/vox-secrets/src/lib.rs` (repoint `list_secret_status` to it, ~lines 491 & 499)
- Test: `crates/vox-secrets/src/spec/mod.rs` (`#[cfg(test)]`)

Context: `capabilities_for_secret(id) -> &'static [Capability]` (`mod.rs:156-225`) already classifies every secret by capability. `metadata().taxonomy_class` (`ids.rs:589-703`) ignores it and returns `AuxTooling` almost always. We add a regular (non-const) derivation fn and repoint the grouping consumers; we do NOT perform const-fn surgery on `metadata()`.

- [ ] **Step 1: Write the failing distribution test**

Add to a `#[cfg(test)] mod taxonomy_tests` in `crates/vox-secrets/src/spec/mod.rs`:

```rust
#[cfg(test)]
mod taxonomy_tests {
    use super::*;

    #[test]
    fn taxonomy_is_not_degenerate() {
        let specs = all_specs();
        // A known LLM provider must classify as llm, GPU as gpu, social as social.
        assert_eq!(taxonomy_class_for(SecretId::GeminiApiKey).slug(), "llm");
        assert_eq!(taxonomy_class_for(SecretId::VoxRunpodApiKey).slug(), "gpu");
        assert_eq!(taxonomy_class_for(SecretId::VoxSocialRedditClientId).slug(), "social");
        // Tuning knobs are operator config, not credentials.
        assert!(taxonomy_class_for(SecretId::GeminiTuningTemperature).is_config_only());

        // No single slug may dominate (>70%) — guards against the degenerate
        // all-"aux" regression this test exists to prevent.
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for s in &specs {
            *counts.entry(taxonomy_class_for(s.id).slug()).or_default() += 1;
        }
        let total = specs.len();
        let max = counts.values().copied().max().unwrap_or(0);
        assert!(counts.len() >= 6, "expected >=6 distinct taxonomy slugs, got {}", counts.len());
        assert!(max * 100 / total <= 70, "one slug holds {}% of specs (degenerate)", max * 100 / total);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-secrets taxonomy_is_not_degenerate`
Expected: FAIL — `taxonomy_class_for` not found.

- [ ] **Step 3: Add `from_capability` to `TaxonomyClass`**

In `crates/vox-secrets/src/spec/types.rs`, inside `impl TaxonomyClass`:

```rust
    /// Map a secret's primary capability to its taxonomy class. Single SSOT for
    /// classification — `capabilities_for_secret` already groups every SecretId.
    pub const fn from_capability(cap: Capability) -> Self {
        match cap {
            Capability::ChatCloudPrimary | Capability::ChatCloudAlt => Self::LlmProviderKey,
            Capability::GpuCloud => Self::CloudGpuInfra,
            Capability::Mesh => Self::MeshTransport,
            Capability::ScholarlyPublication => Self::ScholarlyPublication,
            Capability::ScientiaSyndication => Self::SocialSyndication,
            Capability::DbRemote => Self::TelemetrySearch,
            Capability::RuntimeIngress | Capability::PublishReview | Capability::Orchestration => Self::PlatformIdentity,
            Capability::AuxTools | Capability::AutonomousResearch => Self::AuxTooling,
        }
    }
```

(Confirm the `Capability` variant list against `types.rs` — it is: PlatformIdentity? No — `Capability` variants are at types.rs ~196-210: ChatCloudPrimary, ChatCloudAlt, GpuCloud, PublishReview, DbRemote, Mesh, RuntimeIngress, AuxTools, Orchestration, ScientiaSyndication, ScholarlyPublication, AutonomousResearch. Match must be exhaustive over exactly those.)

- [ ] **Step 4: Add `taxonomy_class_for` to `spec/mod.rs`**

```rust
/// The canonical taxonomy class for a secret. Derives from the secret's primary
/// capability (the classification SSOT), with tuning knobs forced to config.
#[must_use]
pub fn taxonomy_class_for(id: SecretId) -> TaxonomyClass {
    // Numeric tuning knobs live in the secret registry but are operator config,
    // not credentials — keep them out of the secrets surfaces.
    if id.spec().canonical_env.contains("_TUNING_") || id.spec().canonical_env.ends_with("_TUNING") {
        return TaxonomyClass::OperatorTuning;
    }
    match capabilities_for_secret(id).first() {
        Some(cap) => TaxonomyClass::from_capability(*cap),
        None => TaxonomyClass::AuxTooling,
    }
}
```

Verify the tuning env names actually contain `_TUNING_` (e.g. `GEMINI_TUNING_TEMPERATURE`) by reading `registry/llm.rs` / wherever the `*Tuning*` specs live; if the naming differs, switch the guard to an explicit `matches!(id, SecretId::GeminiTuningTemperature | ...)` list of the 11 tuning ids found in `ids.rs:632-642`.

- [ ] **Step 5: Repoint the panel grouping consumer**

In `crates/vox-secrets/src/lib.rs`, in `list_secret_status` (~488-510): replace `spec.id.metadata().taxonomy_class.is_config_only()` (line ~491) with `taxonomy_class_for(spec.id).is_config_only()` and `spec.id.metadata().taxonomy_class.slug()` (line ~499) with `taxonomy_class_for(spec.id).slug()`. This fixes the live panel grouping with no GUI edit.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p vox-secrets taxonomy_is_not_degenerate`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-secrets/src/spec/types.rs crates/vox-secrets/src/spec/mod.rs crates/vox-secrets/src/lib.rs
git commit -m "fix(secrets): derive taxonomy from capability SSOT (was degenerate AuxTooling)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Emit secret rows into the unified settings index (Phase B)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` (extract row renderer, add secret rows, compose in `run()`, tests)

Design: keep ONE generated file (`generatedSettingsIndex.ts`, export `GENERATED_SETTINGS_INDEX`). The config rows and the new secret rows share that array. No second file, no second `--check` branch (so the line-219 early-return is never an issue).

- [ ] **Step 1: Write the failing structural test**

Add to the `#[cfg(test)] mod tests` in `config_gui_codegen.rs`:

```rust
    #[test]
    fn secret_rows_cover_non_config_specs_cleanly() {
        let specs = vox_secrets::spec::all_specs();
        let rows = render_secret_rows(&specs);

        // Invariant 1: one row per non-config spec, exactly.
        let expected = specs.iter()
            .filter(|s| !vox_secrets::spec::taxonomy_class_for(s.id).is_config_only())
            .count();
        let emitted = rows.matches("  { id: \"secret-").count();
        assert_eq!(emitted, expected, "emitted {emitted} secret rows, expected {expected}");

        // Invariant 2: every label is the exact canonical_env (no lossy id round-trip).
        for s in specs.iter().filter(|s| !vox_secrets::spec::taxonomy_class_for(s.id).is_config_only()) {
            assert!(rows.contains(&format!("label: {:?}", s.canonical_env)),
                "label for {} is not its canonical_env", s.canonical_env);
        }

        // Invariant 3: a known provider carries its real taxonomy slug keyword.
        assert!(rows.contains("\"llm\""), "expected an llm-classified secret row");

        // Invariant 4: NO blanket flooding keywords (they pollute the uncapped
        // federated omnisearch). "clavis" is the only allowed generic keyword.
        assert!(!rows.contains("\"token\""), "generic 'token' keyword floods omnisearch");
        assert!(!rows.contains("\"secret\""), "generic 'secret' keyword floods omnisearch");

        // Invariant 5: ids are unique.
        let ids: Vec<&str> = rows.lines()
            .filter_map(|l| l.split("id: \"").nth(1))
            .filter_map(|s| s.split('"').next())
            .collect();
        let mut uniq = ids.clone(); uniq.sort_unstable(); uniq.dedup();
        assert_eq!(ids.len(), uniq.len(), "duplicate secret ids");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-cli secret_rows_cover_non_config_specs_cleanly`
Expected: FAIL — `render_secret_rows` not found.

- [ ] **Step 3: Add `render_secret_rows`**

In `config_gui_codegen.rs`, add (returns ONLY row lines, to be embedded in the existing array):

```rust
/// Render `SettingEntry` rows for managed secrets, generated from the Clavis
/// spec registry. Config-only specs (operator tuning) are excluded. Returns the
/// row lines only — embedded into the shared GENERATED_SETTINGS_INDEX array.
pub fn render_secret_rows(specs: &[&'static vox_secrets::spec::SecretSpec]) -> String {
    let mut rows: Vec<(String, String, String, Vec<String>)> = specs
        .iter()
        .filter(|s| !vox_secrets::spec::taxonomy_class_for(s.id).is_config_only())
        .map(|s| {
            let slug = vox_secrets::spec::taxonomy_class_for(s.id).slug();
            let id = format!("secret-{}", s.canonical_env.to_lowercase().replace('_', "-"));
            let label = s.canonical_env.to_string(); // carried, never reconstructed
            let hint = if s.scope_description.is_empty() { s.canonical_env.to_string() }
                       else { s.scope_description.to_string() };
            // keywords: slug + env tokens + alias tokens (incl. deprecated) + "clavis".
            // No "token"/"secret"/"api key" literals — they flood omnisearch.
            let mut kw: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            kw.insert(slug.to_string());
            kw.insert("clavis".to_string());
            let mut add_tokens = |name: &str| {
                for t in name.to_lowercase().split('_').filter(|t| !t.is_empty()) {
                    kw.insert(t.to_string());
                }
            };
            add_tokens(s.canonical_env);
            for a in s.aliases { add_tokens(a); }
            for a in s.deprecated_aliases { add_tokens(a); }
            (id, label, hint, kw.into_iter().collect())
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0)); // by id == by canonical_env, deterministic

    let mut body = String::new();
    for (id, label, hint, kw) in rows {
        let kw_ts = kw.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(", ");
        body.push_str(&format!(
            "  {{ id: {id:?}, section: \"secrets\", label: {label:?}, hint: {hint:?}, keywords: [{kw_ts}] }},\n"
        ));
    }
    body
}
```

Note `deprecated_aliases` is a `SecretSpec` field (`types.rs:229`).

- [ ] **Step 4: Compose both row sets into the single file in `run()`**

Read the current `run()` (`config_gui_codegen.rs:200-229`). It calls `render_generated_index_ts(CONFIG_KEYS)` → full file string. Refactor so the file body includes secret rows. Minimal change: build the rendered file as config-file-minus-closing + secret rows + closing. Cleanest concrete edit — replace the `let rendered = render_generated_index_ts(CONFIG_KEYS);` line with:

```rust
    let secret_rows = render_secret_rows(&vox_secrets::spec::all_specs());
    // Splice secret rows into the generated index before its closing "];\n".
    let base = render_generated_index_ts(CONFIG_KEYS);
    let rendered = base.replacen("];\n", &format!("{secret_rows}];\n"), 1);
```

This keeps `render_generated_index_ts` and its existing tests untouched, and the existing `--check` branch (comparing on-disk vs `rendered`) now validates the secrets too — no new branch, no early-return hazard.

- [ ] **Step 5: Run the structural test**

Run: `cargo test -p vox-cli secret_rows_cover_non_config_specs_cleanly`
Expected: PASS.

- [ ] **Step 6: Regenerate the file and verify drift is clean**

Run: `cargo run -p vox-cli -- ci config-gui-codegen` then `cargo run -p vox-cli -- ci config-gui-codegen --check`
Expected: first writes the file (now containing `secret-*` rows); `--check` prints OK, exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_gui_codegen.rs crates/vox-gui/ui/src/config/generatedSettingsIndex.ts
git commit -m "feat(cli): emit categorized secret rows into unified settings index

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: GUI search wiring + flooding/uniqueness tests (Phase C)

**Files:**
- Test: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.test.ts` (new)
- (No edit to `settingsIndex.ts` — the `secrets-keys` landing entry at line 27 stays, and secret rows arrive via the existing `...GENERATED_SETTINGS_INDEX` spread at line 34.)

- [ ] **Step 1: Write the tests (they encode three guarantees)**

Create `settingsIndex.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { searchSettings, SETTINGS_INDEX } from './settingsIndex';

describe('settings search: secrets are surfaced, categorized, and not flooding', () => {
  it('finds a provider key via a generated secret- row (not just the landing entry)', () => {
    const hits = searchSettings('gemini');
    expect(hits.some(h => h.id.startsWith('secret-'))).toBe(true);
  });

  it('merged index has no duplicate ids', () => {
    const ids = SETTINGS_INDEX.map(s => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('generic "token" query does not dump every secret (omnisearch flooding guard)', () => {
    const secretCount = SETTINGS_INDEX.filter(s => s.section === 'secrets').length;
    expect(secretCount).toBeGreaterThan(10); // sanity: secrets are present
    const hits = searchSettings('token').filter(h => h.section === 'secrets');
    expect(hits.length).toBeLessThan(secretCount / 2);
  });
});
```

- [ ] **Step 2: Run to verify the first test fails before regeneration is wired**

Run: `pnpm --dir crates/vox-gui/ui vitest run settingsIndex.test.ts`
Expected: the `secret-` test FAILS if the generated file from Task 2 isn't present/regenerated; the flooding test PASSES (we dropped the `token` keyword). If all pass, Task 2's regeneration already landed — acceptable.

- [ ] **Step 3: Ensure the generated file is committed**

The file was regenerated in Task 2 Step 6. Confirm `git status` shows `generatedSettingsIndex.ts` committed with `secret-*` rows.

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm --dir crates/vox-gui/ui vitest run settingsIndex.test.ts`
Expected: PASS (all three).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.test.ts
git commit -m "test(gui): secrets surfaced in search, categorized, no omnisearch flooding

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Contract honesty (one-liner)

**Files:**
- Modify: `contracts/gui/omnisearch-index.v1.yaml` (~lines 16-24)

- [ ] **Step 1: Update the `setting` lane description**

Read lines 16-24; the `setting` kind is documented as `SETTINGS_INDEX + config-gui-codegen (generatedSettingsIndex.ts)`. Append to that description: `; also includes managed secrets (secret-* entries) generated from the Clavis spec`. No schema/gate change — keeps the contract truthful.

- [ ] **Step 2: Commit**

```bash
git add contracts/gui/omnisearch-index.v1.yaml
git commit -m "docs(contract): note secrets now in the setting search lane

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Hygiene-regex hardening (Phase D — SEPARABLE)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_hygiene.rs` (regex at ~417 and ~595)

Independent of Tasks 1-4. The parity gate's wrapper-helper allowlist omits `env_usize` and `env_duration_from_ms`; `env_usize` has a live production read (`resilient_http.rs:58`, `VOX_HTTP_RETRY_MAX_ATTEMPTS`) invisible to the gate.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `config_hygiene.rs`:

```rust
    #[test]
    fn env_usize_wrapper_is_detected() {
        let registered = std::collections::HashSet::new(); // empty → must flag
        let src = r#"let n = env_usize("VOX_HTTP_RETRY_MAX_ATTEMPTS", 3);"#;
        let hits = check_env_reads_registered(src, "x.rs", &registered);
        assert_eq!(hits.len(), 1, "env_usize read not detected");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli env_usize_wrapper_is_detected`
Expected: FAIL — current regex omits `env_usize`.

- [ ] **Step 3: Add the two helpers to both regex literals**

In the two regex strings (`:417` and `:595`), extend the helper alternation from
`env::var(?:_os)?|env_var|env_flag|env_u32|env_i64|env_u64|env_duration|env_truthy`
to add `|env_usize|env_duration_from_ms`. Place `env_duration_from_ms` BEFORE `env_duration` is NOT needed (regex alternation tries left-to-right but both require `\s*\(` after; `env_duration_from_ms(` only matches the longer token), so simply append `|env_usize|env_duration_from_ms`.

- [ ] **Step 4: Run test + re-baseline**

Run: `cargo test -p vox-cli env_usize_wrapper_is_detected` (expect PASS), then `cargo run -p vox-cli -- ci config-hygiene --update-baseline` to grandfather any newly-surfaced existing reads, then `cargo run -p vox-cli -- ci config-hygiene` (expect OK).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_hygiene.rs contracts/config/config-hygiene-baseline.txt
git commit -m "fix(ci): detect env_usize + env_duration_from_ms in hygiene gate

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Full verification

- [ ] **Step 1: Rust** — `cargo test -p vox-secrets` and `cargo test -p vox-cli config_gui_codegen` and `cargo test -p vox-cli config_hygiene`. Expected: all pass.
- [ ] **Step 2: Drift gate** — `cargo run -p vox-cli -- ci config-gui-codegen --check`. Expected: OK.
- [ ] **Step 3: GUI** — `pnpm --dir crates/vox-gui/ui vitest run settingsIndex`. Expected: pass.
- [ ] **Step 4: Clippy** — `cargo clippy -p vox-secrets -p vox-cli -- -D warnings` (NOT `--all-targets`; `vox-gui` build script breaks it). Expected: clean.
- [ ] **Step 5: Confirm regenerated artifact committed** — `git status --short`; if `generatedSettingsIndex.ts` is dirty, regenerate and commit.

---

## Self-review notes

- **Spec coverage:** Phase A→Task 1; Phase B→Task 2; Phase C→Task 3; contract→Task 4; Phase D→Task 5; verify→Task 6. Every spec section maps to a task.
- **Type consistency:** `taxonomy_class_for` (Task 1 def; Tasks 1-3 uses), `from_capability` (Task 1), `render_secret_rows` (Task 2 def + test), `GENERATED_SETTINGS_INDEX` (existing, extended in Task 2), `SettingEntry` 5-field shape (Task 2 emits exactly those). Emitted `id` = `secret-<kebab>`; tests filter on `secret-` prefix consistently.
- **Audit findings closed:** degenerate taxonomy (Task 1), dead `--check` (avoided by single-file splice, Task 2 Step 4), omnisearch flooding (no `token`/`secret` keywords + flooding test, Tasks 2-3), lossy label (label carried, Task 2 Step 3 + Invariant 2), brittle tests (structural invariants, Tasks 1-2), landing entry preserved (Task 3), deprecated aliases included (Task 2 Step 3), contract honesty (Task 4), regex blind spot (Task 5).
- **Open verification during impl:** Task 1 Step 4 — confirm tuning env-name pattern (`_TUNING_`) or fall back to an explicit id list; Task 1 Step 3 — `Capability` match must be exhaustive over the real variant set.
