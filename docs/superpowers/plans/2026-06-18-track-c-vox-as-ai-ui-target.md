# Track C — Vox as an AI-UI-Generation Target (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `test-driven-development.skill.md`. Steps use checkbox (`- [ ]`).

> **🤖 EXECUTION TARGET — READ FIRST.** Run end-to-end by **Gemini 3.5 Flash in Google Antigravity**. Same reliability constraints as Tracks A/B (≈48% completion; no mid-task checkpoint; quota hard-cutoff; API hallucination; weak long-context recall). Obey the Operating Rules. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Research: [`../../src/architecture/ai-ui-generators-and-vox-as-target-research-2026-06-18.md`](../../src/architecture/ai-ui-generators-and-vox-as-target-research-2026-06-18.md). Design: [`../../src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md`](../../src/architecture/automatic-gui-and-debugging-vox-design-2026-06-18.md) §3b.

## Operating Rules (apply to EVERY task)
1. **Atomic + green + committed** — a crash between tasks leaves a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use** — confirm every symbol/path with the task's `rg`/read step before coding; never invent APIs.
3. **Self-contained** — don't rely on remembering earlier tasks.
4. **Two-strike circuit breaker** — fail twice → STOP + handoff note; don't loop.
5. **Parallel dispatch** — honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file (handoff §3 / `dispatching-parallel-agents` skill).
6. **Vox house rules** — no `cargo fmt --all`; automation is `.vox`; `docs/src/` `.md` needs frontmatter; **generated files are regenerated, never hand-edited** (e.g. `policy-registry.v1.yaml` is written by `vox ci policy-registry --write`).
7. **Verification ritual** before commit (use `verification-before-completion` skill): `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`, paste output. Self-review with `requesting-code-review` skill.
8. **Rollback on broken tree** — `git reset --hard HEAD` to the last green commit and re-attempt the one task. Never build forward on a broken tree.
9. **Skill references** — native under `crates/vox-skills/skills/superpowers/`.
10. **Rust implementation constraints** — obey design §5b: additive struct/enum fields (`GuiDesignRule`, `execution_context`) break match arms/constructors → fix each; **no `.unwrap()` in MCP handlers** (return `Result`, surface errors as diagnostics); deterministic output (sort keys; `serde_json::Map` is BTreeMap when `preserve_order` is off); prefer runtime contract reads over deep `include_str!` paths; `cargo run -p vox-arch-check` must pass.

**Goal:** Make Vox/VUV a first-class target external AI UI generators (v0.dev, Claude Design, Cursor, Claude Code) emit into — so their output inherits Vox's compile-time contrast/occlusion/a11y guarantees — by (1) making the GUI rule set **modular and SSOT-driven** (registered in the existing policy-registry, surfaced in the GUI automatically), (2) publishing a **shadcn-compatible component registry** and a **DTCG-interop typed token catalog**, and (3) exposing components/tokens/**validation** as **MCP tools**.

**Architecture:** Reuse Vox's proven patterns — `policy-registry` (SSOT + parity gate + GUI surface), `web_ir::validate_*` (already registry-threaded via `validate_web_ir_with_registry`), the MCP `register()` tool pattern, and `contracts/tokens/tokens.v1.json`. Net new is small and additive.

**Scope:** high-leverage subset (modular rules + component registry + token/DTCG export + MCP tools). **Deferred:** EBNF grammar (gap 4), escape-hatch (`raw_class`) policy matrix (gap 5), full bespoke GUI design panel (rules surface via existing `policy.rs` for now).

**Tech Stack:** Rust; `vox-config` (policy registry), `vox-codegen` (`web_ir` validators/primitives), `vox-codegen-ts` (token export), `vox-orchestrator-mcp` (MCP tools); `contracts/`.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-config/src/policy/registry.rs` | add `GuiDesignRule` domain + register 3 GUI rules | Modify (C1) |
| `contracts/policy/policy-registry.v1.yaml` | regenerated SSOT (via gate) | Regenerate (C1) |
| `crates/vox-codegen/src/web_ir/validate_palette.rs:233-248` | read contrast thresholds from config | Modify (C2) |
| `contracts/gui/component-registry.v1.json` | shadcn-shaped component catalog | Create (C3) |
| `crates/vox-codegen/.../component_registry_sync test` | registry⇄`primitives::resolve` parity | Create (C3) |
| `crates/vox-codegen-ts/src/token_export.rs` | TS token union + DTCG import/export | Create (C4) |
| `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs` | `vox_gui_components`/`_tokens`/`vox_validate_vuv` | Create (C5) |
| `docs/.../where-things-live.md`, handoff | register + GUI-surfacing note | Modify (C6) |

**Pre-flight (paste output; NOT code):**
- `rg -n "enum PolicyDomain|pub domain|fn builtin|policies\(\)|fn all" crates/vox-config/src/policy/registry.rs | head` — confirm `PolicyDomain` variants + how builtin entries are declared.
- `rg -n "validate_web_ir_with_registry|fn validate_palette|wcag21_contrast_ratio|3.0|4.5" crates/vox-codegen/src/web_ir/validate.rs crates/vox-codegen/src/web_ir/validate_palette.rs | head` — confirm validator entry + hardcoded thresholds.
- `rg -n "pub fn register|ToolDefinition|fn resolve\(" crates/vox-orchestrator-mcp/src/mcp_client.rs crates/vox-codegen/src/web_ir/primitives/mod.rs | head` — confirm MCP `register()` + primitive `resolve`.
- `cargo run -p vox-arch-check` — baseline.

---

## Task C1 `[SEQUENTIAL]`: Register GUI design rules in the policy-registry (modular SSOT)

Make the GUI rule *set* data-driven: add a `GuiDesignRule` domain and register the three existing validators (contrast, layer/occlusion, a11y) as builtin policy entries with `severity`, `default_enabled`, and config params. The existing `vox-gui/src/commands/policy.rs` then surfaces them in the GUI **automatically**.

**Files:** Modify `crates/vox-config/src/policy/registry.rs`; regenerate `contracts/policy/policy-registry.v1.yaml`.

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight policy `rg`. Confirm `enum PolicyDomain { CiGate, AuditCheck, CodeAuditRule, ArchRule, .. }` and find the function that returns the builtin `PolicyEntry` list (the generator source). Note the `PolicyEntry` field set (id, domain, title, group, description, severity, blocking, runs_on, source, docs, default_enabled, protected, origin).

- [ ] **Step 2: Failing test.** Add a test asserting the three GUI rules are registered:

```rust
#[test]
fn gui_design_rules_registered() {
    let ids: Vec<&str> = builtin_policies().iter().map(|p| p.id.as_str()).collect();
    for id in ["gui-design-rule/contrast", "gui-design-rule/layer-occlusion", "gui-design-rule/a11y"] {
        assert!(ids.contains(&id), "missing {id}");
    }
    assert!(builtin_policies().iter().any(|p| p.domain == PolicyDomain::GuiDesignRule));
}
```

> Replace `builtin_policies()` with the actual builtin-list function name from Step 1.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-config gui_design_rules_registered`.

- [ ] **Step 4: Implement.** Add `GuiDesignRule` to `PolicyDomain` (and any `Display`/`serde rename`/match arms the compiler flags — fix each it lists). Append three `PolicyEntry` builtins (mirror an existing `arch-rule/*` entry's construction exactly), e.g.:

```rust
PolicyEntry {
    id: "gui-design-rule/contrast".into(),
    domain: PolicyDomain::GuiDesignRule,
    title: "WCAG contrast".into(),
    group: "GUI Design".into(),
    description: "Text/background contrast must meet WCAG AA (configurable thresholds).".into(),
    severity: Severity::Error,           // match the existing Severity type/import
    blocking: true,
    runs_on: vec!["ci".into(), "pre-push".into()],
    source: /* PolicySource mirroring an arch-rule entry; ref the validator path */,
    docs: Some("docs/src/architecture/ai-ui-generators-and-vox-as-target-research-2026-06-18.md".into()),
    default_enabled: true,
    protected: false,
    origin: Origin::Builtin,             // match existing enum/variant
}
```

(+ `gui-design-rule/layer-occlusion` → `validate_layer.rs`, `gui-design-rule/a11y` → `validate_a11y.rs`.) Use the exact field types/enum variants found in Step 1.

- [ ] **Step 5: Run → PASS, then regenerate the SSOT.** `cargo test -p vox-config gui_design_rules_registered`; then `vox ci policy-registry --write` (regenerates the YAML — do NOT hand-edit it); then `vox ci policy-registry` (parity gate must pass).

- [ ] **Step 6: Rule 7 + commit.**

```bash
git add crates/vox-config/src/policy/registry.rs contracts/policy/policy-registry.v1.yaml
git commit -m "feat(policy): register GUI design rules (contrast/occlusion/a11y) as GuiDesignRule domain"
```

---

## Task C2 `[SEQUENTIAL]`: Make contrast thresholds data-driven (prove modularity)

Replace the hardcoded `3.0`/`4.5` in `validate_palette.rs` with values resolved from the rule config, so a rule's parameters are tunable from the SSOT, not the source.

**Files:** Modify `crates/vox-codegen/src/web_ir/validate_palette.rs` (~lines 233-248); a small config carrier.

- [ ] **Step 1 (verify-before-use):** `rg -n "fn validate_palette|registry|3.0|4.5|with_registry" crates/vox-codegen/src/web_ir/validate_palette.rs crates/vox-codegen/src/web_ir/validate.rs`. Confirm `validate_palette` already receives a `registry` arg (it does — `validate_web_ir_with_registry`) and the two literal thresholds.

- [ ] **Step 2: Failing test.** Add a test that drives `validate_palette` with a config specifying a *stricter* floor (e.g. min 7.0 AAA) and asserts a pair that passes at 4.5 now fails — proving the threshold is read, not hardcoded. (Construct the smallest WebIrModule the existing palette tests use — mirror `parent_child(...)` helper already in that file's tests.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-codegen <test_name>`.

- [ ] **Step 4: Implement.** Introduce a tiny `ContrastThresholds { hard_floor: f64, aa_body: f64 }` with defaults `{3.0, 4.5}`, sourced from the rule config when present (read the `gui-design-rule/contrast` params; if config plumbing is heavy, accept a `ContrastThresholds` param with the defaults and thread it from `validate_web_ir_with_registry`). Replace the literals at 235/244 with the struct fields. Keep defaults identical so existing tests stay green.

- [ ] **Step 5: Run → PASS + regression.** `cargo test -p vox-codegen`.

- [ ] **Step 6: Rule 7 + commit.**

```bash
git add crates/vox-codegen/src/web_ir/validate_palette.rs
git commit -m "feat(web_ir): contrast thresholds read from rule config (data-driven, default 3.0/4.5)"
```

---

## Task C3 `[PARALLEL-SAFE]`: shadcn-compatible component registry + parity test

Publish a machine-readable component catalog external generators can read; pin it to reality with a sync test.

**Files:** Create `contracts/gui/component-registry.v1.json`; add a parity test in `crates/vox-codegen`.

- [ ] **Step 1 (verify-before-use):** `rg -n "match tag|\"row\"|\"button\"|\"panel\"|html_tag" crates/vox-codegen/src/web_ir/primitives/mod.rs | head -40`. List the exact primitive names `resolve` handles.

- [ ] **Step 2: Create the registry** `contracts/gui/component-registry.v1.json` (shadcn-registry-shaped) listing each primitive with `name`, `html_tag`, key props/variants, and `accessibility` constraints. Example entry:

```json
{
  "$schema": "https://voxlang.org/schemas/component-registry.v1.json",
  "version": "1.0",
  "components": [
    { "name": "button", "html_tag": "button",
      "props": [{ "name": "variant", "type": "enum", "values": ["default","outline","ghost"], "default": "default" }],
      "accessibility": ["must_have_accessible_name", "must_have_keyboard_handler"] }
  ]
}
```

- [ ] **Step 3: Failing parity test.** Add `crates/vox-codegen/tests/component_registry_sync.rs`: parse the JSON, and for every `name` assert `web_ir::primitives::resolve(name, &[]).is_some()`, and that no `resolve`-handled primitive is missing from the registry (compare against the name list from Step 1, inlined as a const in the test).

- [ ] **Step 4: Run → FAIL then PASS.** `cargo test -p vox-codegen component_registry_sync` (fails until the JSON matches the real primitive set; fix the JSON, not the test).

- [ ] **Step 5: Rule 7 + commit.**

```bash
git add contracts/gui/component-registry.v1.json crates/vox-codegen/tests/component_registry_sync.rs
git commit -m "feat(gui): shadcn-compatible component registry + primitives parity test"
```

---

## Task C4 `[PARALLEL-SAFE]`: typed token export + DTCG interop

Pure functions (no I/O in tests): emit a TS discriminated-union token catalog, and convert between Vox tokens and W3C DTCG.

**Files:** Create `crates/vox-codegen-ts/src/token_export.rs` + module decl.

- [ ] **Step 1 (verify-before-use):** `rg -n "color|spacing|font|\"value\"|\"on\"" contracts/tokens/tokens.v1.json | head`. Confirm the token JSON shape (color groups → shades; optional `{value, on}`).

- [ ] **Step 2: Failing tests.** Add to `token_export.rs`:

```rust
#[test]
fn emits_ts_color_union() {
    let json = r#"{"version":"1.0","color":{"zinc":{"50":"#fafafa","400":"#a1a1aa"}}}"#;
    let ts = emit_token_types(json);
    assert!(ts.contains("export type ColorToken ="), "{ts}");
    assert!(ts.contains("\"zinc.50\""), "{ts}");
    assert!(ts.contains("\"zinc.400\""), "{ts}");
}
#[test]
fn roundtrips_dtcg() {
    let json = r#"{"version":"1.0","color":{"zinc":{"50":"#fafafa"}}}"#;
    let dtcg = to_dtcg(json);            // W3C: { "zinc": { "50": { "$value":"#fafafa", "$type":"color" } } }
    assert!(dtcg.contains("\"$value\": \"#fafafa\""), "{dtcg}");
    assert!(dtcg.contains("\"$type\": \"color\""), "{dtcg}");
    let back = from_dtcg(&dtcg);
    assert!(back.contains("#fafafa"), "{back}");
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-codegen-ts token_export`.

- [ ] **Step 4: Implement** `emit_token_types`, `to_dtcg`, `from_dtcg` using `serde_json::Value` to walk the color map. `to_dtcg` wraps each leaf as `{ "$value": hex, "$type": "color" }` (W3C DTCG); `from_dtcg` unwraps. Add `pub mod token_export;` to the crate's module file. **Determinism (design §5b.4):** `serde_json::Map` is a `BTreeMap` here (`preserve_order` is off), so iteration is key-sorted — but still collect token names into a `BTreeSet`/sorted `Vec` before emitting the union so output is stable regardless of feature flags. Return `Result`/`String`; no `.unwrap()` on parse — surface a parse error.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-codegen-ts token_export`, then Rule 7.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-codegen-ts/src/token_export.rs crates/vox-codegen-ts/src/lib.rs
git commit -m "feat(codegen-ts): typed token TS export + W3C DTCG import/export"
```

---

## Task C5 `[SEQUENTIAL]`: MCP tools — components, tokens, validate_vuv

Expose the registry + tokens + the validation pass over MCP so external generators read and self-check.

**Files:** Create `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs` + register it where other tool modules register.

- [ ] **Step 1 (verify-before-use):** `rg -n "pub fn register|\.register\(|mod compiler_tools|mod code_validator|register_all|fn register_tools" crates/vox-orchestrator-mcp/src/*.rs | head`. Find the exact registration entry point and mirror how `compiler_tools`/`code_validator` register a tool (name + async handler). Also `rg -n "validate_web_ir|run_frontend|web_ir" crates/vox-orchestrator-mcp/src/ crates/vox-codegen/src/web_ir/validate.rs | head` to find how to lower+validate VUV from a string (reuse the existing frontend→web_ir→`validate_web_ir_with_registry` path).

- [ ] **Step 2: Failing test.** Add a test that calls the `vox_validate_vuv` handler with a known-bad snippet (e.g. `gray.300` text on white — the palette tests prove this fails ~1.5:1) and asserts the returned JSON contains a contrast diagnostic; and that `vox_gui_components` returns the registry JSON containing `"button"`.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator-mcp gui_registry`.

- [ ] **Step 4: Implement** `gui_registry_tools.rs`. Handlers return `Result` and surface errors as a diagnostic payload — **no `.unwrap()`** (design §5b.3):
  - `vox_gui_components` → read the registry at runtime from a workspace-relative path resolved via `env!("CARGO_MANIFEST_DIR")` joined to `../../contracts/gui/component-registry.v1.json` (robust; avoids brittle `include_str!("../../../…")` depth — design §5b.6). Verify the relative depth from `crates/vox-orchestrator-mcp` to repo root with `rg --files contracts/gui/component-registry.v1.json` first. If embedding is preferred, `include_str!` only after confirming the exact `../` count from this source file.
  - `vox_gui_tokens` → return the token catalog (+ optional `format=dtcg` via `token_export::to_dtcg`); on parse error return an error result, don't panic.
  - `vox_validate_vuv` → lower the submitted VUV via the existing frontend, run `validate_web_ir_with_registry`, return diagnostics as JSON; map any lowering error to a diagnostic, never `unwrap`.
  Register all three at the entry point from Step 1, mirroring an existing module's registration exactly.

- [ ] **Step 5: Run → PASS + regression.** `cargo test -p vox-orchestrator-mcp`, then Rule 7.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator-mcp/src/gui_registry_tools.rs crates/vox-orchestrator-mcp/src/<entrypoint>.rs
git commit -m "feat(mcp): vox_gui_components/_tokens/validate_vuv tools for AI UI generators"
```

---

## Task C6 `[PARALLEL-SAFE]` (docs): register + GUI-surfacing note

**Files:** `docs/src/architecture/where-things-live.md`; handoff doc.

- [ ] **Step 1:** Add where-things-live rows for the component registry, token export, and MCP GUI tools (match the table columns). Add one line noting GUI design rules surface via the existing `crates/vox-gui/src/commands/policy.rs` (policy-registry consumer) — no new panel needed for v1.

- [ ] **Step 2: Verify GUI surfacing (no code):** `rg -n "domain|GuiDesignRule|policy" crates/vox-gui/src/commands/policy.rs` — confirm the GUI policy view lists all domains (so `GuiDesignRule` entries appear). If it filters by domain, note the follow-up to include `GuiDesignRule`.

- [ ] **Step 3: Commit.**

```bash
git add docs/src/architecture/where-things-live.md docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md
git commit -m "docs: register Track C surfaces; note GUI design-rule surfacing via policy.rs"
```

---

## Parallelization summary (for the Antigravity orchestrator)

- **C1→C2 SEQUENTIAL** (C2 reads the rule config C1 registers; both relate to the rule engine).
- **C3, C4 PARALLEL-SAFE** (disjoint files: `contracts/gui/` + new test vs new `token_export.rs`) — run together, and beside C2.
- **C5 SEQUENTIAL after C3 + C4** (uses the component registry JSON and `token_export`; also needs the validator path).
- **C6 PARALLEL-SAFE** (docs) — any time after C5.
- **Waves:** W1 = C1→C2; W2 = {C3, C4} parallel; W3 = C5; W4 = C6. Never two agents on the same crate's shared file.

## Self-Review

- **Spec coverage:** modular SSOT rule registry (C1, extends policy-registry — no new parallel SSOT) + data-driven proof (C2); shadcn component registry (C3); typed token export + DTCG interop (C4); MCP components/tokens/validate (C5); GUI surfacing reused via `policy.rs` + docs (C6). Matches design §3b items 1–4; item 5 (bespoke GUI panel) intentionally reuses `policy.rs` and is otherwise deferred.
- **Deferred (YAGNI):** EBNF grammar; escape-hatch policy matrix + `@unsafe`; bespoke GUI design-system panel; publishing the registry to a hosted URL for "Open in v0" (needs hosting infra).
- **Placeholder scan:** C1/C2/C5 use verify-then-mirror steps with exact `rg` (must match live policy/validator/MCP code); C3/C4 are concrete. No fabricated APIs.
- **Type consistency:** `GuiDesignRule` domain id prefix `gui-design-rule/*` consistent C1/C6; `emit_token_types`/`to_dtcg`/`from_dtcg` consistent C4/C5; component-registry path consistent C3/C5.
- **Accuracy note:** validators confirmed to exist at `crates/vox-codegen/src/web_ir/validate_{palette,layer,a11y,overlay}.rs`; `validate_web_ir_with_registry` already threads a registry; policy YAML is generated (regenerate, never hand-edit).

## Execution Handoff

Track C only; depends conceptually on VUV/GUI existing (it does) but is independent of Tracks A/B at the code level. Recommended order overall: Track A (lowest risk) → Track C (this) → Track B. Missing skills: see [handoff §4](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).
