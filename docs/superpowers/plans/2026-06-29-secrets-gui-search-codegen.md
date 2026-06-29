# Secrets GUI Search Codegen — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate a GUI settings search index for managed secrets from the Clavis spec registry, so every `SecretId` is findable in settings search automatically and drift-gated by CI.

**Architecture:** Mirror the existing `generatedSettingsIndex.ts` codegen. Add a render function in `config_gui_codegen.rs` that iterates `vox_secrets::spec::all_specs()` (skipping config-only) and emits `SettingEntry` rows to `generatedSecretsIndex.ts`; emit + drift-check it inside the existing `run()`; spread it into `settingsIndex.ts`. No backend, panel, or gate changes — all already shipped.

**Tech Stack:** Rust (`vox-cli` codegen), TypeScript/React (`vox-gui` settings index), Vitest (pnpm), `vox ci config-gui-codegen` CI drift gate.

**Verification context:** Everything else in the original spec is already in `main` (gate union, non-VOX detection, required-CI-sweep, panel grouping/health/backend-strip). This plan is the one genuine gap. See `docs/superpowers/specs/2026-06-29-clavis-ssot-secrets-gui-design.md`.

**House rules:** Repo is pnpm-managed (`npm` fails). Never `cargo fmt --all` on Windows — use `cargo fmt -p <crate>`. `vox-gui` breaks `clippy --all-targets` — exclude it.

---

### Task 1: Render function for the secrets search index

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` (add render fn + const + unit test)

Reference for shape: the existing `render_generated_index_ts` (same file, ~line 61) and the spec API:
- `vox_secrets::spec::all_specs() -> Vec<&'static SecretSpec>`
- `SecretSpec { id, canonical_env: &str, aliases: &[&str], scope_description: &str, .. }`
- `spec.id.metadata().taxonomy_class` is a `TaxonomyClass` with const `slug()` and `is_config_only()`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `config_gui_codegen.rs`:

```rust
    #[test]
    fn secrets_index_has_header_and_known_provider() {
        let ts = render_generated_secrets_index_ts(&vox_secrets::spec::all_specs());
        assert!(ts.contains("@generated"));
        assert!(ts.contains("GENERATED_SECRETS_INDEX"));
        // A known LLM provider key surfaces, in the secrets section, tagged llm.
        assert!(ts.contains("GEMINI_API_KEY"), "Gemini key missing from secrets index");
        assert!(ts.contains("section: \"secrets\""));
        assert!(ts.contains("\"llm\""), "taxonomy slug keyword missing");
        // Config-only specs must be excluded (OperatorTuning slug is "config").
        assert!(!ts.contains("section: \"config\""));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli secrets_index_has_header_and_known_provider`
Expected: FAIL — `render_generated_secrets_index_ts` not found.

- [ ] **Step 3: Write the render function**

Add near `render_generated_index_ts` in `config_gui_codegen.rs`:

```rust
/// Output path for the generated secrets search-index TS module.
const SECRETS_OUT_REL_PATH: &str = "crates/vox-gui/ui/src/config/generatedSecretsIndex.ts";

/// Render the GUI search-index module for managed secrets, generated from the
/// Clavis spec registry (the SSOT). Config-only specs are excluded — they are
/// operator tuning, not credentials. Sorted by (slug, canonical_env).
pub fn render_generated_secrets_index_ts(specs: &[&'static vox_secrets::spec::SecretSpec]) -> String {
    let mut rows: Vec<(String, String, String, Vec<String>)> = specs
        .iter()
        .filter(|s| !s.id.metadata().taxonomy_class.is_config_only())
        .map(|s| {
            let slug = s.id.metadata().taxonomy_class.slug();
            let id = format!("secret-{}", s.canonical_env.to_lowercase().replace('_', "-"));
            let hint = if s.scope_description.is_empty() {
                s.canonical_env.to_string()
            } else {
                s.scope_description.to_string()
            };
            // keywords: slug + env tokens + aliases + literals, deduped, sorted.
            let mut kw: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            kw.insert(slug.to_string());
            for tok in s.canonical_env.to_lowercase().split('_').filter(|t| !t.is_empty()) {
                kw.insert(tok.to_string());
            }
            for a in s.aliases {
                kw.insert(a.to_lowercase());
            }
            for lit in ["api key", "token", "secret", "clavis"] {
                kw.insert(lit.to_string());
            }
            (id, slug.to_string(), hint, kw.into_iter().collect())
        })
        .collect();
    rows.sort_by(|a, b| (&a.1, &a.0).cmp(&(&b.1, &b.0)));

    let mut body = String::new();
    for (id, _slug, hint, kw) in rows {
        // label = canonical env (panel labels rows the same way). Recover it from id.
        let label = id.trim_start_matches("secret-").to_uppercase().replace('-', "_");
        let kw_ts = kw.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(", ");
        body.push_str(&format!(
            "  {{ id: {id:?}, section: \"secrets\", label: {label:?}, hint: {hint:?}, keywords: [{kw_ts}] }},\n"
        ));
    }
    format!(
        "// @generated by `vox ci config-gui-codegen` from vox_secrets spec — DO NOT EDIT.\n\
         import type {{ SettingEntry }} from '../components/surfaces/Settings/settingsIndex';\n\
         export const GENERATED_SECRETS_INDEX: SettingEntry[] = [\n{body}];\n"
    )
}
```

Add `vox-secrets` to `crates/vox-cli/Cargo.toml` `[dependencies]` only if not already present (check first — `vox secrets` CLI lives in vox-cli, so it almost certainly is).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli secrets_index_has_header_and_known_provider`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_gui_codegen.rs
git commit -m "feat(cli): render secrets GUI search index from Clavis spec

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Emit + drift-check the secrets index in `run()`

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` (the `run()` fn, ~line 200)

- [ ] **Step 1: Extend `run()` to also write/compare the secrets file**

In `run()`, after the existing `OUT_REL_PATH` block, add a parallel block for the secrets index. In the `check` branch, compare on-disk vs rendered and `bail!` on drift; in the write branch, write it. Insert before the final `Ok(())`:

```rust
    // Secrets search index — generated from the Clavis spec registry SSOT.
    let secrets_rendered = render_generated_secrets_index_ts(&vox_secrets::spec::all_specs());
    let secrets_path = root.join(SECRETS_OUT_REL_PATH);
    if check {
        let on_disk = std::fs::read_to_string(&secrets_path).map_err(|e| {
            anyhow::anyhow!(
                "config-gui-codegen --check: cannot read {SECRETS_OUT_REL_PATH}: {e}. \
                 Run `vox ci config-gui-codegen` to generate it."
            )
        })?;
        if on_disk != secrets_rendered {
            anyhow::bail!(
                "config-gui-codegen drift: {SECRETS_OUT_REL_PATH} is stale. \
                 Regenerate with `vox ci config-gui-codegen` and commit."
            );
        }
        println!("config-gui-codegen OK: {SECRETS_OUT_REL_PATH} matches Clavis spec.");
    } else {
        if let Some(parent) = secrets_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&secrets_path, &secrets_rendered)?;
        let n = secrets_rendered.matches("  { id:").count();
        println!("config-gui-codegen: wrote {n} secret(s) to {SECRETS_OUT_REL_PATH}");
    }
```

Note: the `check` branch currently `return`s early after the first file. Move the secrets-check block ABOVE that early `return` (or remove the early return and fall through), so both files are validated in one `--check` run. Verify by reading the current `run()` control flow before editing.

- [ ] **Step 2: Generate the file**

Run: `cargo run -p vox-cli -- ci config-gui-codegen`
Expected: prints `wrote N secret(s) to crates/vox-gui/ui/src/config/generatedSecretsIndex.ts`; the file now exists.

- [ ] **Step 3: Verify the drift gate is clean**

Run: `cargo run -p vox-cli -- ci config-gui-codegen --check`
Expected: prints both `OK` lines, exit 0.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_gui_codegen.rs crates/vox-gui/ui/src/config/generatedSecretsIndex.ts
git commit -m "feat(cli): emit + drift-gate generatedSecretsIndex.ts

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Wire the generated index into settings search

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.test.ts` (new)

- [ ] **Step 1: Write the failing test**

Create `settingsIndex.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { searchSettings } from './settingsIndex';

describe('settings search includes managed secrets', () => {
  it('finds a provider key by name', () => {
    const hits = searchSettings('gemini');
    expect(hits.some(h => h.section === 'secrets')).toBe(true);
  });
  it('finds keys by the clavis keyword', () => {
    const hits = searchSettings('clavis');
    expect(hits.length).toBeGreaterThan(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --dir crates/vox-gui/ui vitest run settingsIndex.test.ts`
Expected: FAIL — only the single hardcoded `secrets-keys` entry exists; `searchSettings('gemini')` returns nothing in the `secrets` section.

- [ ] **Step 3: Spread the generated secrets index**

In `settingsIndex.ts`: add the import at the top (next to the existing `GENERATED_SETTINGS_INDEX` import):

```ts
import { GENERATED_SECRETS_INDEX } from '../../../config/generatedSecretsIndex';
```

Replace the hardcoded line (currently line 27):

```ts
  { id: 'secrets-keys', section: 'secrets', label: 'Keys & secrets', hint: 'Provider API keys (OpenRouter, Gemini, …)', keywords: ['api key', 'openrouter', 'anthropic', 'token', 'clavis'] },
```

with the spread (place it next to `...GENERATED_SETTINGS_INDEX`):

```ts
  ...GENERATED_SECRETS_INDEX,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --dir crates/vox-gui/ui vitest run settingsIndex.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.test.ts
git commit -m "feat(gui): surface every managed secret in settings search

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust tests + drift gate**

Run: `cargo test -p vox-cli config_gui_codegen` then `cargo run -p vox-cli -- ci config-gui-codegen --check`
Expected: tests pass; check prints both `OK` lines.

- [ ] **Step 2: GUI test suite for the settings surface**

Run: `pnpm --dir crates/vox-gui/ui vitest run settingsIndex`
Expected: PASS.

- [ ] **Step 3: Confirm no clippy regression in vox-cli**

Run: `cargo clippy -p vox-cli -- -D warnings`
Expected: clean (do NOT run `--all-targets` across the workspace; `vox-gui` build script breaks it).

- [ ] **Step 4: Final commit if anything regenerated**

```bash
git status --short
# if generatedSecretsIndex.ts changed, regenerate and commit:
# cargo run -p vox-cli -- ci config-gui-codegen && git add -A && git commit -m "chore: regenerate secrets index"
```

---

## Self-review notes

- **Spec coverage:** the spec's single goal (secrets in GUI search, drift-gated) maps to Tasks 1-3; Task 4 is verification. No other spec requirement remains (all else shipped).
- **Type consistency:** `render_generated_secrets_index_ts` is the name used in Task 1 test, Task 1 impl, and Task 2 `run()`. `GENERATED_SECRETS_INDEX` / `SECRETS_OUT_REL_PATH` consistent across tasks. Emitted rows match the `SettingEntry` interface (`id`, `section`, `label`, `hint`, `keywords`).
- **Open verification during impl:** Task 2 Step 1 flags the `run()` early-`return` in the `check` branch — the implementer must read current control flow and ensure both files are checked.
