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
7. **Verification ritual** before commit (use `verification-before-completion` skill): `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → no-stub check via `rg -n 'todo!|unimplemented!|TODO|FIXME' <changed-files>` (there is **no** `vox stub-check` command; the repo gate is the heavy `vox ci toestub-budget`, reserved for final verification) → `cargo fmt -p <crate>`, paste output. Self-review with `requesting-code-review` skill.
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
| `crates/vox-config/src/policy/registry.rs` | add `GuiDesignRule` domain variant | Modify (C1a) |
| `crates/vox-cli/src/commands/ci/policy_registry.rs` | `gui_design_rule_entries()` + extend `build_registry()` + parity | Modify (C1b) |
| `contracts/policy/policy-registry.v1.yaml` | regenerated SSOT (via gate) | Regenerate (C1b) |
| `crates/vox-codegen/src/web_ir/validate_palette.rs:233-248` | read contrast thresholds from config | Modify (C2) |
| `contracts/gui/component-registry.v1.json` | shadcn-shaped component catalog | Create (C3) |
| `crates/vox-codegen/.../component_registry_sync test` | registry⇄`primitives::resolve` parity | Create (C3) |
| `crates/vox-codegen-ts/src/token_export.rs` | TS token union + DTCG import/export | Create (C4) |
| `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs` + `dispatch.rs` | `vox_gui_components`/`_tokens` (validate_vuv deferred) | Create/Modify (C5) |
| `docs/.../where-things-live.md`, handoff | register + GUI-surfacing note | Modify (C6) |

**Pre-flight (paste output; NOT code). These correct three load-bearing facts the original draft got wrong — read carefully:**
- `rg -n "enum PolicyDomain|enum PolicySeverity|struct PolicySource|enum PolicySourceKind|pub origin|pub severity" crates/vox-config/src/policy/registry.rs | head -40` — confirm: `PolicyDomain` variants; `severity: Option<PolicySeverity>` (NOT a bare `Severity`); `origin: String` (NOT an `Origin` enum — set it to `"builtin".into()`); the `PolicySource`/`PolicySourceKind` shape.
- `rg -n "fn build_registry|_entries\(\)|fn crl_gate_entries|DomainExpectation" crates/vox-cli/src/commands/ci/policy_registry.rs | head -40` — **builtin policy entries are declared HERE in vox-cli, NOT in vox-config's registry.rs** (which only *loads* YAML). This is where C1b adds `gui_design_rule_entries()`.
- `rg -n "validate_web_ir_with_registry|fn validate_palette|3.0|4.5" crates/vox-codegen/src/web_ir/validate.rs crates/vox-codegen/src/web_ir/validate_palette.rs | head` — confirm validator entry + the two literal thresholds (~:235/:244).
- `rg -n '"vox_check" =>|=> handle_|match name|fn dispatch' crates/vox-orchestrator-mcp/src/dispatch.rs | head -40` — **MCP tools are dispatched via a `match name { ... }` arm in `dispatch.rs`, NOT a `.register()` call.** Note an existing arm to mirror, and find the separate tool-schema/advertise list (`rg -n "vox_check_workspace" crates/vox-orchestrator-mcp/src/`).
- `rg -n "match tag|html_tag|fn resolve" crates/vox-codegen/src/web_ir/primitives/mod.rs | head` — primitive `resolve` for C3.
- `cargo run -p vox-arch-check` — baseline.

---

## Task C1a `[SEQUENTIAL]`: Add the `GuiDesignRule` domain variant (enum-only, trivially green)

Add ONLY the new `PolicyDomain` variant and fix every match arm the compiler flags. This is its own atomic task so a quota cutoff can't leave a half-added enum.

**Files:** Modify `crates/vox-config/src/policy/registry.rs` (the `PolicyDomain` enum + its `Display`/serde/match sites).

- [ ] **Step 1 (verify-before-use):** `rg -n "enum PolicyDomain" -A 12 crates/vox-config/src/policy/registry.rs` and `rg -n "PolicyDomain::" crates/vox-config/src crates/vox-cli/src | head -40` — list every match site that will need a new arm.

- [ ] **Step 2: Implement.** Add `GuiDesignRule` to `enum PolicyDomain`. Compile (`cargo build -p vox-config`) and fix every non-exhaustive-match error the compiler lists (Display impl, any `match domain {}`), mirroring the existing `ArchRule`/`CrlGate` arms.

- [ ] **Step 3: Run → green.** `cargo build -p vox-config` then `cargo test -p vox-config`. No new behavior yet — this commit just adds the variant.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-config/src/policy/registry.rs
git commit -m "feat(policy): add GuiDesignRule PolicyDomain variant"
```

---

## Task C1b `[SEQUENTIAL]`: Register the three GUI rules as builtin entries (in vox-cli, where builtins live)

Builtin `PolicyEntry`s are NOT declared in `vox-config` (that file only loads YAML). They are declared in `crates/vox-cli/src/commands/ci/policy_registry.rs` as `*_entries()` functions composed by `build_registry()`. Add a `gui_design_rule_entries()` mirroring `crl_gate_entries()`.

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`; regenerate `contracts/policy/policy-registry.v1.yaml`.

- [ ] **Step 1 (verify-before-use):** `rg -n "fn crl_gate_entries|fn build_registry|DomainExpectation|policies.extend" crates/vox-cli/src/commands/ci/policy_registry.rs` — read `crl_gate_entries()` (the simplest template), the `build_registry()` `.extend(...)` chain, and the `DomainExpectation` parity list. Note the EXACT `PolicyEntry` field types: `severity: Option<PolicySeverity>`, `origin: String`, `source: PolicySource { kind: PolicySourceKind, .. }`. Run `rg -n "enum PolicySourceKind" crates/vox-config/src/policy/registry.rs`.

- [ ] **Step 2: Failing test.** In the `#[cfg(test)]` module of `policy_registry.rs`, add:

```rust
#[test]
fn gui_design_rules_registered() {
    let ids: Vec<&str> = gui_design_rule_entries().iter().map(|p| p.id.as_str()).collect();
    for id in ["gui-design-rule/contrast", "gui-design-rule/layer-occlusion", "gui-design-rule/a11y"] {
        assert!(ids.contains(&id), "missing {id}");
    }
    assert!(gui_design_rule_entries().iter().all(|p| p.domain == PolicyDomain::GuiDesignRule));
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-cli gui_design_rules_registered`.

- [ ] **Step 4: Implement.** Add `fn gui_design_rule_entries() -> Vec<PolicyEntry>` mirroring `crl_gate_entries()` exactly, with three entries. Use the REAL field types (confirmed in Step 1) — do NOT use `Origin::Builtin` or a bare `Severity` (neither exists):

```rust
PolicyEntry {
    id: "gui-design-rule/contrast".into(),
    domain: PolicyDomain::GuiDesignRule,
    title: "WCAG contrast".into(),
    group: "GUI Design".into(),
    description: "Text/background contrast must meet WCAG AA (configurable thresholds).".into(),
    severity: Some(PolicySeverity::Error),     // Option<PolicySeverity>
    blocking: true,
    runs_on: vec!["ci".into(), "pre-push".into()],
    source: PolicySource { /* mirror crl_gate_entries' PolicySource exactly; point kind/path at validate_palette.rs */ },
    docs: Some("docs/src/architecture/ai-ui-generators-and-vox-as-target-research-2026-06-18.md".into()),
    default_enabled: true,
    protected: false,
    origin: "builtin".into(),                   // String, not an enum
}
```

(+ `gui-design-rule/layer-occlusion` → `validate_layer.rs`, `gui-design-rule/a11y` → `validate_a11y.rs`.) Then: (a) add `policies.extend(gui_design_rule_entries());` to `build_registry()` after the last existing `.extend`; (b) add a matching `DomainExpectation` for `GuiDesignRule` to the parity list (mirror the `code_audit_entries` expectation) so the gate covers the new domain.

- [ ] **Step 5: Run → PASS, then regenerate + parity.** `cargo test -p vox-cli gui_design_rules_registered`; then `vox ci policy-registry --write` (regenerates the YAML — never hand-edit); then `vox ci policy-registry` (parity gate must pass).

- [ ] **Step 6: Rule 7 + commit.**

```bash
git add crates/vox-cli/src/commands/ci/policy_registry.rs contracts/policy/policy-registry.v1.yaml
git commit -m "feat(policy): register GUI design rules (contrast/occlusion/a11y) as GuiDesignRule entries"
```

---

## Task C2 `[SEQUENTIAL]`: Make contrast thresholds data-driven (prove modularity)

Replace the hardcoded `3.0`/`4.5` in `validate_palette.rs` with values resolved from the rule config, so a rule's parameters are tunable from the SSOT, not the source.

**Files:** Modify `crates/vox-codegen/src/web_ir/validate_palette.rs` (~lines 233-248); a small config carrier.

- [ ] **Step 1 (verify-before-use):** `rg -n "fn validate_palette|registry|3.0|4.5|with_registry" crates/vox-codegen/src/web_ir/validate_palette.rs crates/vox-codegen/src/web_ir/validate.rs`. Confirm `validate_palette` already receives a `registry` arg (it does — `validate_web_ir_with_registry`) and the two literal thresholds.

- [ ] **Step 2: Failing test.** Add a test that drives `validate_palette` with a config specifying a *stricter* floor (e.g. min 7.0 AAA) and asserts a pair that passes at 4.5 now fails — proving the threshold is read, not hardcoded. (Construct the smallest WebIrModule the existing palette tests use — mirror `parent_child(...)` helper already in that file's tests.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-codegen <test_name>`.

- [ ] **Step 4: Implement.** Introduce a tiny `ContrastThresholds { hard_floor: f64, aa_body: f64 }` with `Default` = `{3.0, 4.5}`. Thread it as an explicit parameter from `validate_web_ir_with_registry` → `validate_palette` → `check_contrast`/`walk_contrast`, and replace the literals at ~:235/:244 with `thresholds.hard_floor` / `thresholds.aa_body`. Keep the default identical so existing tests stay green. (Do NOT attempt to source values from the policy-registry YAML — `validate_palette` receives a `TokenRegistry`, not a `PolicyRegistry`; there is no policy-param→validator wire today. This task proves modularity via a defaulted, explicitly-threaded struct, which is the realistic seam; config-sourcing is a documented follow-up.)

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

## Task C5 `[SEQUENTIAL]`: MCP tools — components + tokens (two pure file-read tools)

Expose the component registry + token catalog over MCP so external generators read them. **`vox_validate_vuv` is DESCOPED from this task** (see C5-deferred note) because there is no string→web_ir convenience entry point — it needs the full parse→typecheck→HIR→lower pipeline, which is unverified and high-risk for a weak model. Ship the two safe, pure file-read tools first.

**Files:** Create `crates/vox-orchestrator-mcp/src/gui_registry_tools.rs`; modify `crates/vox-orchestrator-mcp/src/dispatch.rs` (add `match` arms + `use`), and the tool-schema/advertise list.

- [ ] **Step 1 (verify-before-use):** `rg -n '"vox_check" =>|=> handle|match name' crates/vox-orchestrator-mcp/src/dispatch.rs | head` — find an existing dispatch arm to mirror (tools are dispatched by a `match name { "vox_..." => ... }` arm; there is **no** `.register()` API for local tools). Then `rg -n "vox_check_workspace" crates/vox-orchestrator-mcp/src/` to locate EVERY place a tool name is enumerated — there is a tool-definition/advertise list separate from `dispatch.rs`; both must list the new tools or they dispatch but are never advertised.

- [ ] **Step 2: Failing test.** Add a test asserting `vox_gui_components` returns the registry JSON containing `"button"`, and `vox_gui_tokens` returns the token catalog (and `format=dtcg` yields `"$value"`).

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator-mcp gui_registry`.

- [ ] **Step 4: Implement** `gui_registry_tools.rs` with two handler functions returning `Result` (no `.unwrap()`):
  - `vox_gui_components` → read `contracts/gui/component-registry.v1.json`. **Prefer `include_str!`** with the exact `../` depth (confirm with `rg --files contracts/gui/component-registry.v1.json`, then count directories from `crates/vox-orchestrator-mcp/src/` to repo root) so a shipped binary doesn't depend on a repo checkout; fall back to a `CARGO_MANIFEST_DIR`-relative runtime read only if embedding is impractical.
  - `vox_gui_tokens` → return the token catalog (+ optional `format=dtcg` via `token_export::to_dtcg`); on parse error return an error result, don't panic.
  Then wire BOTH into MCP: (a) add `use crate::gui_registry_tools;` to `dispatch.rs`; (b) add two `match name` arms (`"vox_gui_components" => ...`, `"vox_gui_tokens" => ...`) mirroring the existing arm from Step 1; (c) add both tool names to the tool-definition/advertise list found in Step 1.

- [ ] **Step 5: Run → PASS + regression.** `cargo test -p vox-orchestrator-mcp`, then Rule 7. Then `cargo run -p vox-arch-check` (this crate now depends on `vox-codegen-ts::token_export` — confirm no layer inversion).

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator-mcp/src/gui_registry_tools.rs crates/vox-orchestrator-mcp/src/dispatch.rs
git commit -m "feat(mcp): vox_gui_components + vox_gui_tokens tools for AI UI generators"
```

> **C5-deferred — `vox_validate_vuv`:** requires a string→web_ir path. Before implementing, a follow-up task must `rg -n "fn .*-> .*HirModule|fn parse|fn compile" crates/vox-compiler/src crates/vox-codegen/src` to find (or add) a clean `&str → HirModule → lower_hir_to_web_ir_with_summary → validate_web_ir_with_registry` entry point. If none exists, that follow-up adds it first. Do NOT attempt `vox_validate_vuv` in C5.

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

- **C1a→C1b→C2 SEQUENTIAL** (C1a adds the enum variant; C1b adds the entries in vox-cli; C2 threads a defaulted threshold struct in the validator).
- **C3, C4 PARALLEL-SAFE** (disjoint files: `contracts/gui/` + new test vs new `token_export.rs`) — run together, and beside C2.
- **C5 SEQUENTIAL after C3 + C4** (uses the component registry JSON and `token_export`).
- **C6 PARALLEL-SAFE** (docs) — any time after C5.
- **Waves:** W1 = C1a→C1b→C2; W2 = {C3, C4} parallel; W3 = C5; W4 = C6. Never two agents on the same crate's shared file.

## Self-Review

- **Spec coverage:** modular SSOT rule registry (C1, extends policy-registry — no new parallel SSOT) + data-driven proof (C2); shadcn component registry (C3); typed token export + DTCG interop (C4); MCP components/tokens/validate (C5); GUI surfacing reused via `policy.rs` + docs (C6). Matches design §3b items 1–4; item 5 (bespoke GUI panel) intentionally reuses `policy.rs` and is otherwise deferred.
- **Deferred (YAGNI):** EBNF grammar; escape-hatch policy matrix + `@unsafe`; bespoke GUI design-system panel; publishing the registry to a hosted URL for "Open in v0" (needs hosting infra).
- **Placeholder scan:** C1/C2/C5 use verify-then-mirror steps with exact `rg` (must match live policy/validator/MCP code); C3/C4 are concrete. No fabricated APIs.
- **Type consistency:** `GuiDesignRule` domain id prefix `gui-design-rule/*` consistent C1/C6; `emit_token_types`/`to_dtcg`/`from_dtcg` consistent C4/C5; component-registry path consistent C3/C5.
- **Accuracy note:** validators confirmed to exist at `crates/vox-codegen/src/web_ir/validate_{palette,layer,a11y,overlay}.rs`; `validate_web_ir_with_registry` already threads a registry; policy YAML is generated (regenerate, never hand-edit).

## Execution Handoff

Track C only; depends conceptually on VUV/GUI existing (it does) but is independent of Tracks A/B at the code level. Recommended order overall: Track A (lowest risk) → Track C (this) → Track B. Missing skills: see [handoff §4](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).
