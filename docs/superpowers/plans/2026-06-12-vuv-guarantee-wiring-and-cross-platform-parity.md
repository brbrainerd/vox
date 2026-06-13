# VUV Guarantee Wiring & Cross-Platform Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make VUV's two headline guarantees — *bad contrast is impossible* and *occlusion/tier-inversion is impossible* — actually fire on real `.vox` compiles, extend them to the React Native/mobile target, and lock them in with a compile-fail corpus, so LLM-generated UIs are verified-good on web, Tauri desktop, and phone.

**Architecture:** Almost everything needed already exists as code — WCAG calculators, a token registry with fg/bg surface pairs, a closed 7-tier `LayerTier` enum, tier-inversion/mark checks, portal emitters — but is orphaned, advisory, quote-broken, or web-only. This plan is therefore dominated by *wiring and enforcement*, not new design: fix the broken seams (Phase A), run the same validators on the mobile emit path and close the RN primitive/style gap (Phase B), then build the verification spine that asserts the forbidden things stay forbidden (Phase C). New-vocabulary work (responsive kwargs, marks syntax, stdlib) is deliberately deferred to follow-up plans (Phase D/E/F triggers).

**Tech Stack:** Rust workspace crates (`vox-compiler`, `vox-codegen`, `vox-cli`), JSON contracts under `contracts/`, `.vox` golden corpus under `examples/golden/`, React Native/Expo emit under `crates/vox-codegen/src/codegen_ts/rn/`.

---

## Audit basis (2026-06-12)

This plan implements the verified findings of a six-dimension audit (6 auditors + adversarial verification of every critical/high gap; 4 dimensions agent-audited, 2 hand-audited after rate-limiting). Summary of the state of the world:

**Real and working today:** VUV-1..9 authoring surface (18 primitives in `lowering_shared/primitive_tags.rs:15-41`, keyword-only view-calls, ~60 universal style kwargs, snake_case event + ~40 aria kwargs, `@form` with web+RN emitters, nested `routes {}`, rename registry + `vox migrate names`); namespaced diagnostics with did-you-mean; Tauri desktop = web emit + wrapper (ADR accepted); RN/Expo emit gated by tsc-compile e2e in CI; Playwright GUI suite in CI (tiered post-merge).

**Verified gaps this plan closes** (audit gap IDs in brackets):

| # | Gap | Where |
|---|---|---|
| 1 | `color`/`bg` kwargs format into Tailwind classes with **zero** validation — `color: gray.300` on white compiles silently [CONTRAST-1] | `web_ir/primitives/mod.rs:694-695` |
| 2 | Surface-pair contrast pipeline dead end-to-end: lowering stores `data-vox-surface` **with embedded quotes** (`"\"primary\""`), validators look up unquoted → every lookup misses [CONTRAST-2] | `web_ir/lower.rs:280` vs `validate.rs:804`, `validate_a11y.rs:274` |
| 3 | `insufficient_contrast` (the <3:1 hard WCAG failure) swept into the advisory blanket — never blocks a build [CONTRAST-3] | `web_ir/validate.rs:875` |
| 4 | `@tokens` contrast check compares a token's light variant against its own dark variant — a pair never rendered together [CONTRAST-4] | `typeck/contrast.rs:35-76` |
| 5 | Token registry opt-in (cwd-relative `vox.tokens.json`); goldens validate with no registry [CONTRAST-6] | `tokens/mod.rs:68` |
| 6 | `check_tier_inversions` / `check_duplicate_marks` / `check_dangling_marks` have **zero production callers** — `vox/layer/tier-inversion` can never fire [layout G1, authoring G2] | `typeck/layer.rs:44,87,145`; only `typeck/mod.rs:179` wired |
| 7 | Portal infrastructure (`emit_layer_stylesheet`, portal roots/resolver) never invoked; modal/toast/drawer emit inline `position:fixed` divs — the exact stacking-context bug the memo claims VUV prevents [layout G3] | `web_ir/layer_emit.rs:45-105`; `primitives/mod.rs:384-399` |
| 8 | Wide-open occlusion escape hatches: `position="absolute"`, `top`/`left`/`inset`, arbitrary `z="999"`, verbatim `raw_class` accepted everywhere [layout G4] | `primitives/mod.rs:782-812` |
| 9 | Two divergent tier enums with two incompatible z ladders (`LayerTier` vs `ZTier`; ×100 vs 0..50,100) [layout G5] | `hir/nodes/layer.rs` vs `web_ir/mod.rs:55`, `layer_emit.rs:32` vs `primitives/mod.rs:536,796` |
| 10 | `@layer` attaches to fns, not components; `default_for_primitive` names (PascalCase `Tooltip`/`Dialog`) don't match the lowercase primitive set [layout G6] | `hir/nodes/decl.rs:469` vs `:577`; `layer.rs:91-100` |
| 11 | Mobile build (`BuildTarget::Mobile`) calls `generate_rn` straight from HIR — **no** web_ir validators run; RN diagnostics are eprintln warnings [XP-4, CONTRAST-5] | `vox-cli/src/commands/build.rs:117-132` |
| 12 | 9 web primitives don't emit on RN (incl. the whole overlay family — modals silently flatten to `<View>`) [XP-2] | `codegen_ts/rn/component.rs:364-510` |
| 13 | RN drops ~60 style kwargs to a hardcoded 12-key StyleSheet with literal hex colors [XP-3] | `rn/component.rs:92-126, 1043-1077` |
| 14 | Unknown style kwargs leak through as raw HTML attributes (no rejection, no did-you-mean) [authoring G4] | view-call lowering |
| 15 | No compile-fail corpus: nothing asserts that the forbidden things actually fail to compile (hand-audited) | `crates/vox-compiler/tests/` |
| 16 | No `vox test view` snapshot command (VUV-12 zero code) [authoring G1] | — |
| 17 | Only 11 of 69 goldens are UI; no `@form`/layer/tokens goldens (training corpus thin) [authoring G7] | `examples/golden/` |
| 18 | Stale Tauri-mobile CLI target + CI workflow contradicts accepted desktop-only ADR [XP-5] | `.github/workflows/`, vox-cli |
| 19 | Doc drift: syntax SSOT says 14 primitives / VUV-7 partial; layer memo implies Rules 1–5 exist [authoring G6] | `docs/src/architecture/` |

**Out of scope (follow-up plans, see Phase D/E/F):** responsive/adaptive vocabulary (VUV-11, XP-1), marks + typed subordination syntax (GA-26 Rules 4–5, layout G2), `chat_panel` stdlib / `@session` / cache decorators (VUV-13/14/15). These need design decisions, not wiring; each gets its own plan when this one lands.

## Pre-flight (read before Task A1)

1. **PR #239 collision check.** The handoff branch for PR #239 closed the `web_ir` ignored-test cohort (75 tests), which includes the three `#[ignore]`d lower→validate tests at `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:2009,2041,2072` referenced by Task A1, and possibly the mass-ignored dashboard goldens (`golden_dashboard_{chrome,composites,surfaces}_test.rs`, sunset 2026-08-01). Run `gh pr view 239 --json state,mergedAt` first. If merged, rebase this worktree and re-check which `#[ignore]`s remain; skip the un-ignore steps that are already done.
2. **Branch discipline.** Work on this worktree's branch (`claude/pensive-wiles-ff9ac7`) or a fresh `spec/vuv-guarantee-wiring` branch — never directly on `main`.
3. **Formatting:** never `cargo fmt --all` (Windows arg-limit, os error 206). Use `cargo fmt -p <crate>` or `vox run scripts/fmt.vox`.
4. **Architecture gate:** after tasks that add files, run `cargo run -p vox-arch-check`. New concepts get a row in `docs/src/architecture/where-things-live.md` (Task C4 batches this).
5. **Known design conflict to carry through Phase A:** `LayerTier::allows_child` is implemented as *child ≤ parent* (`hir/nodes/layer.rs:83-85` — Modal-inside-Tooltip fires because Modal(4) > Popover(3)). GA-26 acceptance criterion 2 (Modal-inside-**Toast** refuses) does NOT follow from that rule (4 ≤ 5 passes). Task A9 resolves this with a `may_parent_surfaces()` leaf-surface rule (Toast and Popover tiers may not parent any surface) and Task C4 amends the SSOT to match.

## File structure

```
contracts/tokens/
  tailwind-palette.v1.json            (new, A3)  vendored palette name→hex; SSOT for color vocabulary
  vox.tokens.default.json             (new, A6)  embedded default registry (surfaces, fg/bg pairs)
crates/vox-codegen/src/web_ir/
  lower.rs                            (mod, A1)  store surface name unquoted in mirror attr
  validate.rs                         (mod, A2/A3/A9)  advisory carve-out; wire palette + layer passes
  validate_a11y.rs                    (mod, A1/A4)  unquote lookups; pub contrast_ratio
  validate_palette.rs                 (new, A3/A4)  color/bg vocabulary + pairwise WCAG checks
  validate_layer.rs                   (new, A9/A10)  surface-tree tier inversion + escape-hatch checks
  layer_emit.rs                       (mod, A7/A11)  single ladder; called from scaffold
  primitives/mod.rs                   (mod, A3/A5/A7/A11)  mirror attrs; unknown-kwarg mirror; ladder; portal routing
  mod.rs                              (mod, A7)  ZTier delegates to LayerTier ladder
crates/vox-codegen/src/codegen_ts/
  scaffold.rs                         (mod, A11)  globals.css gains layer stylesheet; portal roots in app shell
  rn/component.rs                     (mod, B2/B3)  card/list/wrap mappings; kwarg lowering; tier hard error
  rn/overlay_host.rs                  (new, B4)  tier-ordered RN overlay host emit
crates/vox-compiler/src/
  typeck/contrast.rs                  (mod, A12)  fg/bg pair check (on: metadata), not light-vs-dark
  hir/nodes/layer.rs                  (mod, A8/A9)  lowercase default_for_primitive; may_parent_surfaces
  hir/nodes/decl.rs                   (mod, A8)  HirReactiveComponent.layer
  parser/descent/decl/head.rs         (mod, A8)  @layer on component decls
  tokens/mod.rs                       (mod, A6)  embedded default registry fallback
crates/vox-cli/src/commands/
  build.rs                            (mod, B1)  mobile branch runs web_ir validators, gates build
  test_view.rs                        (new, C2)  `vox test view <name> --props k=v` snapshot
crates/vox-compiler/tests/
  forbidden_corpus_test.rs            (new, C1)  every examples/forbidden/*.vox must fail with its exact code
examples/forbidden/                   (new, C1)  one .vox per structurally-forbidden bug class
examples/golden/                      (mod, C3)  form_basic.vox, layered_overlay.vox, tokens_theme.vox
```

Each task below is one red→green→commit cycle (some have two cycles when a pair of checks shares a file). Run commands from the repo root.

---

## Phase A — Make the web guarantees real

### Task A1: Fix the surface quote-encoding bug (unblocks the whole contrast pipeline)

The lowerer stores `data-vox-surface` values as TS-expression strings with embedded quotes (`"\"primary\""` — see the convention comment at `web_ir/lower.rs:269-272`); both lookup sites compare unquoted, so every registry lookup misses: legit surfaces fire false `unknown_surface` errors and the ancestor contrast walker silently never runs.

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/validate.rs:804` (`validate_surface_refs`)
- Modify: `crates/vox-codegen/src/web_ir/validate_a11y.rs:274` (`walk_contrast` surface lookup)
- Test: module tests in `crates/vox-codegen/src/web_ir/validate.rs`

- [ ] **Step 1: Write the failing test** (in `validate.rs` `#[cfg(test)] mod tests`, following the existing `make_route` test style):

```rust
#[test]
fn surface_lookup_tolerates_quoted_attr_values() {
    use crate::web_ir::{DomNode, DomNodeId, WebIrModule};
    let registry = vox_compiler::tokens::TokenRegistry::load_from_str(
        r#"{
            "surfaces": {
                "primary": { "fg": "color.text.primary", "bg": "color.surface.primary" }
            },
            "tokens": {
                "color.text.primary":    { "light": "#111111", "dark": "#eeeeee" },
                "color.surface.primary": { "light": "#fafafa", "dark": "#1a1a1a" }
            }
        }"#,
    )
    .expect("registry json");
    let mut m = WebIrModule::default();
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0),
        tag: "section".to_string(),
        // Exactly what lower.rs:280 produces: JSON-encoded value with embedded quotes.
        attrs: vec![("data-vox-surface".to_string(), "\"primary\"".to_string())],
        children: vec![],
        span: None,
    });
    m.view_roots.push(("Page".to_string(), DomNodeId(0)));

    let diags = validate_web_ir_with_registry(&m, &registry);
    assert!(
        !diags.iter().any(|d| d.code == "web_ir_validate.surface.unknown_surface"),
        "quoted surface value must resolve against the registry, got: {diags:?}"
    );
}
```

> If `TokenRegistry::load_from_str` rejects this shape, mirror the JSON shape used by the existing registry tests at `crates/vox-compiler/src/tokens/mod.rs:275-290` — the shape of the fixture is not the point of the test; the quoted attr value is.

- [ ] **Step 2: Run it, watch it fail**

Run: `cargo test -p vox-codegen surface_lookup_tolerates_quoted_attr_values`
Expected: FAIL — `unknown_surface` diagnostic present (lookup missed because of the quotes).

- [ ] **Step 3: Normalize at both lookup sites**

In `validate.rs` (`validate_surface_refs`):

```rust
// Attr values are TS-expression strings (see lower.rs convention comment) —
// a plain string literal arrives JSON-encoded. Strip one layer of quotes
// before registry lookup.
let surface_name = v.trim_matches('"');
if registry.lookup_surface(surface_name).is_none() {
```

(and use `surface_name` in the diagnostic message). In `validate_a11y.rs:270-274`, apply the same `.map(|(_, v)| v.as_str().trim_matches('"'))` before `lookup_surface`.

- [ ] **Step 4: Run the test + crate suites**

Run: `cargo test -p vox-codegen && cargo test -p vox-compiler web_ir`
Expected: new test PASS; pre-existing suites green.

- [ ] **Step 5: Un-ignore the three lower→validate e2e tests** at `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:2009,2041,2072` (skip if PR #239 already did — see Pre-flight 1). Run them; if they expose further mismatches, fix at the lookup site, not by re-quoting tests.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-codegen/src/web_ir/validate.rs crates/vox-codegen/src/web_ir/validate_a11y.rs crates/vox-compiler/tests/web_ir_lower_emit_test.rs
git commit -m "fix(web_ir): strip JSON quotes at surface lookups - revives surface contrast pipeline"
```

### Task A2: Make `insufficient_contrast` blocking (carve it out of the advisory blanket)

`is_advisory_diagnostic` ends with `|| d.code.starts_with("web_ir_validate.a11y.")` (`validate.rs:875`), sweeping the <3:1 hard-failure code into warning land. The advisory rationale ("don't break emit for an accessible-name gap") doesn't apply: a contrast violation can fail the build while still emitting JSX.

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/validate.rs:861-876`
- Test: module tests in the same file

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn insufficient_contrast_is_not_advisory() {
    let d = WebIrDiagnostic {
        code: "web_ir_validate.a11y.insufficient_contrast".to_string(),
        message: String::new(),
        span: None,
        category: Some("a11y".to_string()),
    };
    assert!(!is_advisory_diagnostic(&d), "hard WCAG failure must block the build");

    // The rest of the a11y family stays advisory (emit must still produce JSX).
    let d2 = WebIrDiagnostic {
        code: "web_ir_validate.a11y.low_contrast".to_string(),
        message: String::new(),
        span: None,
        category: Some("a11y".to_string()),
    };
    assert!(is_advisory_diagnostic(&d2));
}
```

- [ ] **Step 2: Run it, watch it fail**

Run: `cargo test -p vox-codegen insufficient_contrast_is_not_advisory`
Expected: FAIL on the first assert (blanket `starts_with` returns true).

- [ ] **Step 3: Implement** — replace the blanket suffix clause with an explicit carve-out:

```rust
    ) || d.code.ends_with("_warning")
        || (d.code.starts_with("web_ir_validate.a11y.")
            && d.code != "web_ir_validate.a11y.insufficient_contrast")
```

Check `WebIrDiagnostic::severity()` (`web_ir/mod.rs:511-519`) stays consistent: `insufficient_contrast` must NOT be in its warning list (it isn't today — only `low_contrast` is; leave that).

- [ ] **Step 4: Run the full crate suite** — `cargo test -p vox-codegen`. The emitter gate (`codegen_ts/emitter.rs:602-604`) and golden filter (`golden_vox_examples_test.rs:61`) consume `is_advisory_diagnostic`; if any golden now fails on a real contrast violation, that golden has a real bug — fix the golden's colors, do not re-blanket the code.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/web_ir/validate.rs
git commit -m "feat(web_ir): insufficient_contrast now blocks builds (advisory carve-out)"
```

### Task A3: Color vocabulary — reject unknown `color`/`bg` values with did-you-mean

`resolve_universal_kwarg` formats any string into a class (`"color" => vec![format!("text-{}", v_dashed)]`, `primitives/mod.rs:694-695`). First half of the gray-on-white fix: every `color`/`bg`/`border_color` value must resolve to a known palette entry or registry token — `color: zink.400` and `color: #aaa` become compile errors.

**Files:**
- Create: `contracts/tokens/tailwind-palette.v1.json`
- Create: `crates/vox-codegen/src/web_ir/validate_palette.rs`
- Modify: `crates/vox-codegen/src/web_ir/primitives/mod.rs` (mirror attrs in `resolve`/`apply_universal_kwargs`)
- Modify: `crates/vox-codegen/src/web_ir/mod.rs` (`pub mod validate_palette;`), `validate.rs:899` (wire into `validate_web_ir_full`)

- [ ] **Step 1: Vendor the palette contract.** Generate from the GUI workspace's existing Tailwind install (or copy the default-palette table from tailwindcss docs if offline):

```bash
cd crates/vox-gui/ui && node -e "const c=require('tailwindcss/colors'); const flat={}; for (const [hue,v] of Object.entries(c)) { if (typeof v==='string') flat[hue]=v; else if (v && typeof v==='object') for (const [shade,hex] of Object.entries(v)) flat[hue+'.'+shade]=hex; } console.log(JSON.stringify({version:1,colors:flat},null,2))" > ../../../contracts/tokens/tailwind-palette.v1.json
```

Shape (deprecated aliases like `lightBlue` must be deleted from the generated file):

```json
{
  "version": 1,
  "colors": {
    "white": "#ffffff",
    "black": "#000000",
    "zinc.50": "#fafafa",
    "zinc.400": "#a1a1aa",
    "gray.300": "#d1d5db",
    "blue.600": "#2563eb"
  }
}
```

- [ ] **Step 2: Write the failing tests** in new `crates/vox-codegen/src/web_ir/validate_palette.rs`:

```rust
//! Color-vocabulary + pairwise-contrast validation for VUV style kwargs.
//!
//! The lowerer mirrors `color`/`bg`/`border_color` kwarg raw values into
//! `data-vox-color` / `data-vox-bg` / `data-vox-border-color` attrs (JSON-quoted,
//! same convention as `data-vox-surface`). This pass checks the values against
//! the vendored Tailwind palette (contracts/tokens/tailwind-palette.v1.json)
//! plus the project token registry. Codes: `web_ir_validate.style.unknown_color`
//! (error), and — Task A4 — `web_ir_validate.a11y.insufficient_contrast` /
//! `low_contrast` for resolvable fg/bg pairs.

use super::{DomNode, WebIrDiagnostic, WebIrModule};

pub const PALETTE_JSON: &str = include_str!("../../../../contracts/tokens/tailwind-palette.v1.json");

/// Resolve a kwarg color value (`zinc.400`, `white`) to its hex via palette then registry.
pub fn resolve_color(value: &str, registry: Option<&vox_compiler::tokens::TokenRegistry>) -> Option<String> {
    // implemented in Step 4
    unimplemented!()
}

pub fn validate_palette(
    module: &WebIrModule,
    registry: Option<&vox_compiler::tokens::TokenRegistry>,
    out: &mut Vec<WebIrDiagnostic>,
) {
    // implemented in Step 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_ir::{DomNode, DomNodeId, WebIrModule};

    fn module_with_attrs(attrs: Vec<(&str, &str)>) -> WebIrModule {
        let mut m = WebIrModule::default();
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "span".to_string(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.to_string(), format!("\"{v}\"")))
                .collect(),
            children: vec![],
            span: None,
        });
        m.view_roots.push(("V".to_string(), DomNodeId(0)));
        m
    }

    #[test]
    fn known_palette_color_passes() {
        let m = module_with_attrs(vec![("data-vox-color", "zinc.400")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn unknown_color_is_rejected_with_suggestion() {
        let m = module_with_attrs(vec![("data-vox-color", "zink.400")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        let d = out
            .iter()
            .find(|d| d.code == "web_ir_validate.style.unknown_color")
            .expect("unknown color must be rejected");
        assert!(d.message.contains("zinc.400"), "did-you-mean expected: {}", d.message);
    }

    #[test]
    fn raw_hex_is_rejected() {
        let m = module_with_attrs(vec![("data-vox-bg", "#aaaaaa")]);
        let mut out = vec![];
        validate_palette(&m, None, &mut out);
        assert!(out.iter().any(|d| d.code == "web_ir_validate.style.unknown_color"));
    }
}
```

- [ ] **Step 3: Run, watch them fail** — `cargo test -p vox-codegen validate_palette`. Expected: panic at `unimplemented!()`.

- [ ] **Step 4: Implement.** `resolve_color`: lazy-parse `PALETTE_JSON` (`once_cell`/`std::sync::OnceLock` — match whichever the crate already uses; grep `OnceLock` in vox-codegen) into a `HashMap<String,String>`; lookup order: palette → `registry.lookup(value)`. `validate_palette`: walk `module.dom_nodes`, for each Element scan attrs for `data-vox-color`/`data-vox-bg`/`data-vox-border-color`, `trim_matches('"')`, and if `resolve_color` returns `None`, push:

```rust
out.push(WebIrDiagnostic {
    code: "web_ir_validate.style.unknown_color".to_string(),
    message: format!(
        "Unknown color '{v}' for `{kwarg}` — not in the Tailwind palette or the project token registry.{did_you_mean}"
    ),
    span: None,
    category: Some("style".to_string()),
});
```

Did-you-mean: reuse the levenshtein suggestion helper that `suggest_tokens` uses (`validate.rs:640-644`) — lift it to a shared `fn suggest<'a>(input: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String>` rather than duplicating. Raw hex (`#…`) falls out naturally (not in palette) — keep one explicit message branch telling the author to use a palette name or token.

- [ ] **Step 5: Mirror the kwarg values during lowering.** In `primitives/mod.rs`, where `resolve`/`apply_universal_kwargs` consumes static kwarg pairs, add (same JSON-quoted convention as `data-vox-surface` at `lower.rs:276-280`):

```rust
// Mirror color-bearing kwargs for the palette/contrast validators (A3/A4).
for (k, v) in &static_pairs {
    let mirror = match k.as_str() {
        "color" => Some("data-vox-color"),
        "bg" => Some("data-vox-bg"),
        "border_color" => Some("data-vox-border-color"),
        _ => None,
    };
    if let Some(mk) = mirror {
        attrs.push((mk.to_string(), format!("\"{}\"", v.trim_matches('"'))));
    }
}
```

Wire the pass: in `validate_web_ir_full` (`validate.rs:880-902`) add `super::validate_palette::validate_palette(module, registry, &mut out);` after the overlay call. Add `pub mod validate_palette;` to `web_ir/mod.rs`. `unknown_color` must NOT be added to `is_advisory_diagnostic`.

- [ ] **Step 6: Run the workspace UI suites** — `cargo test -p vox-codegen && cargo test -p vox-compiler`. Any golden using an off-palette color now fails: fix the golden (that's the feature working). Expected churn: a handful of dashboard fixtures.

- [ ] **Step 7: Commit**

```bash
git add contracts/tokens/tailwind-palette.v1.json crates/vox-codegen/src/web_ir/validate_palette.rs crates/vox-codegen/src/web_ir/mod.rs crates/vox-codegen/src/web_ir/validate.rs crates/vox-codegen/src/web_ir/primitives/mod.rs
git commit -m "feat(web_ir): color/bg kwargs validate against palette+registry (unknown_color)"
```

### Task A4: Pairwise WCAG contrast on kwarg colors — gray-on-white refuses compile

Second half of the headline fix: when a node's resolved foreground and background are both known (same node, or nearest ancestor `data-vox-bg`/surface), compute the WCAG ratio: error <3:1 (`insufficient_contrast`, blocking after A2), warn <4.5:1 (`low_contrast`).

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/validate_palette.rs`
- Modify: `crates/vox-codegen/src/web_ir/validate_a11y.rs` (make its contrast-ratio fn `pub`, the canonical one)

- [ ] **Step 1: Write the failing tests** (in `validate_palette.rs` tests; helper builds a parent/child arena):

```rust
fn parent_child(parent_attrs: Vec<(&str, &str)>, child_attrs: Vec<(&str, &str)>) -> WebIrModule {
    let mut m = WebIrModule::default();
    let enc = |a: Vec<(&str, &str)>| {
        a.into_iter().map(|(k, v)| (k.to_string(), format!("\"{v}\""))).collect::<Vec<_>>()
    };
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(0), tag: "div".to_string(), attrs: enc(parent_attrs),
        children: vec![DomNodeId(1)], span: None,
    });
    m.dom_nodes.push(DomNode::Element {
        id: DomNodeId(1), tag: "span".to_string(), attrs: enc(child_attrs),
        children: vec![], span: None,
    });
    m.view_roots.push(("V".to_string(), DomNodeId(0)));
    m
}

#[test]
fn gray_text_on_white_panel_is_a_hard_error() {
    let m = parent_child(vec![("data-vox-bg", "white")], vec![("data-vox-color", "gray.300")]);
    let mut out = vec![];
    validate_palette(&m, None, &mut out);
    assert!(
        out.iter().any(|d| d.code == "web_ir_validate.a11y.insufficient_contrast"),
        "gray.300 on white is ~1.46:1 and must hard-fail, got: {out:?}"
    );
}

#[test]
fn marginal_contrast_warns_not_errors() {
    // gray.500 (#6b7280) on white ≈ 4.83:1 → passes AA; gray.400 (#9ca3af) ≈ 2.96 → error;
    // pick a pair in the 3:1..4.5:1 band: gray.450 doesn't exist, use zinc.500 (#71717a) ≈ 4.6 passes…
    // slate.400 (#94a3b8) on white ≈ 2.4 errors. Use blue.400 (#60a5fa) on white ≈ 2.8 → error.
    // Verified in-band pair: gray.500 on gray.100 (#f3f4f6) ≈ 4.2:1.
    let m = parent_child(vec![("data-vox-bg", "gray.100")], vec![("data-vox-color", "gray.500")]);
    let mut out = vec![];
    validate_palette(&m, None, &mut out);
    assert!(out.iter().any(|d| d.code == "web_ir_validate.a11y.low_contrast"));
    assert!(!out.iter().any(|d| d.code == "web_ir_validate.a11y.insufficient_contrast"));
}

#[test]
fn no_known_background_means_no_check() {
    let m = parent_child(vec![], vec![("data-vox-color", "gray.300")]);
    let mut out = vec![];
    validate_palette(&m, None, &mut out);
    assert!(out.iter().all(|d| !d.code.contains("contrast")), "{out:?}");
}
```

> Recompute the in-band ratios with the canonical function when implementing; if a chosen pair lands outside the intended band, swap the palette entries in the *test*, not the thresholds.

- [ ] **Step 2: Run, watch them fail** — `cargo test -p vox-codegen validate_palette`. Expected: no contrast codes emitted yet.

- [ ] **Step 3: Implement.** (a) In `validate_a11y.rs`, make the existing WCAG relative-luminance/ratio function `pub fn contrast_ratio(fg_hex: &str, bg_hex: &str) -> Option<f64>` — this becomes the *single canonical implementation*; delete/redirect the other two divergent copies the audit found (grep `fn relative_luminance|fn contrast` across `crates/` — one uses an outdated sRGB threshold; CONTRAST-10). (b) In `validate_palette`, do a parent-pointer walk over the arena (build `child→parent` map from each Element's `children`); for every node carrying `data-vox-color`, find effective bg = own `data-vox-bg`, else nearest ancestor `data-vox-bg`, else nearest ancestor surface's bg token resolved via registry (`lookup_surface(...).bg_key` → `registry.lookup`). When both hexes resolve:

```rust
if let Some(ratio) = super::validate_a11y::contrast_ratio(&fg_hex, &bg_hex) {
    if ratio < 3.0 {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.a11y.insufficient_contrast".to_string(),
            message: format!(
                "Text color '{fg}' on background '{bg}' has contrast {ratio:.2}:1 — below the 3:1 hard floor. Pick a darker/lighter pair (WCAG AA needs 4.5:1 for body text)."
            ),
            span: None,
            category: Some("a11y".to_string()),
        });
    } else if ratio < 4.5 {
        out.push(WebIrDiagnostic {
            code: "web_ir_validate.a11y.low_contrast".to_string(),
            message: format!("Text color '{fg}' on '{bg}' is {ratio:.2}:1 — below WCAG AA 4.5:1."),
            span: None,
            category: Some("a11y".to_string()),
        });
    }
}
```

No known background → no check (honest scope; the forbidden-corpus entry in C1 documents this boundary).

- [ ] **Step 4: Run suites** — `cargo test -p vox-codegen && cargo test -p vox-compiler`. Fix any golden that now hard-fails (it has a real contrast bug).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/web_ir/validate_palette.rs crates/vox-codegen/src/web_ir/validate_a11y.rs
git commit -m "feat(web_ir): pairwise WCAG contrast on color/bg kwargs - gray-on-white refuses compile"
```

### Task A5: Reject unknown style kwargs (no silent leak-through as HTML attrs)

Today `text("hi", colr: zinc.400)` silently emits a `colr` HTML attribute. For LLM self-repair loops this is the worst possible behavior: a typo'd kwarg must come back as a diagnostic with a suggestion, not vanish into the DOM.

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/primitives/mod.rs` (in `resolve`: classify leftover kwargs)
- Modify: `crates/vox-codegen/src/web_ir/validate_palette.rs` (or a small `validate_kwargs` fn in `validate.rs` — keep it with the suggestion helper from A3)

- [ ] **Step 1: Write the failing test** (DomNode-level, same `module_with_attrs` helper; the mirror attr is `data-vox-unknown-kwarg`):

```rust
#[test]
fn unknown_kwarg_is_rejected_with_suggestion() {
    let m = module_with_attrs(vec![("data-vox-unknown-kwarg", "colr")]);
    let mut out = vec![];
    validate_palette(&m, None, &mut out);
    let d = out
        .iter()
        .find(|d| d.code == "web_ir_validate.style.unknown_kwarg")
        .expect("unknown kwarg must produce a diagnostic");
    assert!(d.message.contains("color"), "did-you-mean 'color' expected: {}", d.message);
}
```

Plus an emit-level test asserting the mirror attr is produced: in `primitives/mod.rs` tests, call `resolve("text", &[("colr".to_string(), "\"zinc.400\"".to_string())])` (match the existing `resolve` test call shape at `primitives/mod.rs:~1040`) and assert the emission's attrs contain `("data-vox-unknown-kwarg", "\"colr\"")` and do NOT contain a bare `colr` attr.

- [ ] **Step 2: Run, watch both fail.** `cargo test -p vox-codegen unknown_kwarg`

- [ ] **Step 3: Implement.** In `resolve`, the leftover branch that currently passes unrecognized kwargs through as attrs: keep passthrough ONLY for `attr_`-prefixed (documented escape, strip prefix), `data_*`, event kwargs (`on_*` — handled upstream by `map_jsx_attr_name`), and aria kwargs; everything else lowers to the `data-vox-unknown-kwarg` mirror instead of a real attr. In the validator, suggestion candidates = `UNIVERSAL_STYLE_KWARGS` ∪ the primitive's typed props ∪ `on_*`/`aria_*` families; message: `` "Unknown kwarg `colr` on `text` — not a style axis, event, or aria kwarg. Did you mean `color`? (escape hatch: attr_colr)" ``. The code goes in the non-advisory set (it's an error).

- [ ] **Step 4: Run suites; fix fallout.** `cargo test -p vox-codegen && cargo test -p vox-compiler`. Any fixture relying on bare-attr leak-through must move to `attr_*`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/web_ir/primitives/mod.rs crates/vox-codegen/src/web_ir/validate_palette.rs
git commit -m "feat(web_ir): unknown style kwargs are errors with did-you-mean (no HTML-attr leak)"
```

### Task A6: Default token registry — surfaces work without a project `vox.tokens.json`

`TokenRegistry::load_from_project_dir` (`tokens/mod.rs:68`) returns `None` without a cwd file, so all registry-dependent checks silently skip for every project that didn't opt in (i.e., all of them — audit found zero production users).

**Files:**
- Create: `contracts/tokens/vox.tokens.default.json` (a small, tasteful default: `primary`/`muted`/`inverse` surfaces + their fg/bg tokens with `light:`/`dark:` values, every pair ≥4.5:1 — compute with `contrast_ratio` while authoring)
- Modify: `crates/vox-compiler/src/tokens/mod.rs`

- [ ] **Step 1: Write the failing test** (in `tokens/mod.rs` tests):

```rust
#[test]
fn project_dir_without_tokens_file_falls_back_to_embedded_default() {
    let dir = tempfile::tempdir().unwrap();
    let reg = TokenRegistry::load_from_project_dir(dir.path())
        .expect("must fall back to the embedded default registry");
    assert!(reg.lookup_surface("primary").is_some());
}
```

(If `tempfile` isn't already a dev-dependency of vox-compiler, check `Cargo.toml`; it's used widely in the workspace — add `tempfile.workspace = true` to `[dev-dependencies]` if missing.)

- [ ] **Step 2: Run, watch it fail** — returns `None` today.

- [ ] **Step 3: Implement**

```rust
pub const DEFAULT_TOKENS_JSON: &str =
    include_str!("../../../../contracts/tokens/vox.tokens.default.json");

pub fn load_from_project_dir(project_dir: &std::path::Path) -> Option<Self> {
    let path = project_dir.join("vox.tokens.json");
    if let Ok(s) = std::fs::read_to_string(&path) {
        return Self::load_from_str(&s).ok();
    }
    // No project registry: fall back to the embedded default so surface/contrast
    // checks always have ground truth. A project file fully replaces the default.
    Self::load_from_str(DEFAULT_TOKENS_JSON).ok()
}
```

Audit call sites of `load_from_project_dir(...)` (grep) — any `if let Some(reg)` guard now always takes the Some branch; confirm none of them treated `None` as "validation disabled on purpose" (if one did, that was the opt-in bug this task removes — note it in the commit message).

- [ ] **Step 4: Run** — `cargo test -p vox-compiler tokens && cargo test -p vox-codegen`.

- [ ] **Step 5: Commit**

```bash
git add contracts/tokens/vox.tokens.default.json crates/vox-compiler/src/tokens/mod.rs
git commit -m "feat(tokens): embedded default registry - surface/contrast checks are default-on"
```

### Task A7: One canonical z-ladder (collapse the two tier enums' ladders)

`layer_emit.rs:32-37` assigns `tier*100`; `primitives/mod.rs:536-542,796-802` assigns `0/10/20/30/40/50/100` (twice). When A11 wires the portal stylesheet these two ladders would fight. Canonical = `tier * 100` (per layer_emit's GA-26 header).

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/mod.rs` (ZTier gains `z_value()` + a `From<ZTier> for LayerTier` parity bridge)
- Modify: `crates/vox-codegen/src/web_ir/layer_emit.rs`, `crates/vox-codegen/src/web_ir/primitives/mod.rs` (both consume `z_value()`)
- Test: module tests in `web_ir/mod.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn ztier_and_layertier_agree_on_names_and_order() {
    use vox_compiler::hir::nodes::layer::LayerTier;
    for z in [ZTier::Background, ZTier::Content, ZTier::Chrome, ZTier::Popover,
              ZTier::Modal, ZTier::Toast, ZTier::SystemOverlay] {
        let lt = LayerTier::from_str(z.to_str()).expect("every ZTier name parses as LayerTier");
        assert_eq!(z.z_value(), (lt as i32) * 100, "single ladder: {}", z.to_str());
    }
}
```

And in `primitives/mod.rs` tests: assert the `z: modal` kwarg now resolves to the class `z-[400]` (Modal = index 4 × 100), not `z-40`.

- [ ] **Step 2: Run, watch fail** — `z_value` doesn't exist; ladder mismatch.

- [ ] **Step 3: Implement.** `impl ZTier { pub fn z_value(self) -> i32 { (self as i32) * 100 } }` (the derive order in `mod.rs:55-63` already matches LayerTier's). Replace the hand-rolled ladders: `layer_emit.rs:32-37` and BOTH tables in `primitives/mod.rs` (`:536-542` and `:796-802`) call `ZTier::from_str(...).map(|t| t.z_value())`. Add a `// SSOT: z ladder = ZTier::z_value(); do not hand-roll` comment at each former table site.

- [ ] **Step 4: Run + golden churn.** `cargo test -p vox-codegen && cargo test -p vox-compiler`. Snapshot/golden tests asserting `z-10`…`z-50` classes need updating to `z-[100]`…`z-[500]` — mechanical; update goldens, eyeball the diff is *only* z classes.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/web_ir/mod.rs crates/vox-codegen/src/web_ir/layer_emit.rs crates/vox-codegen/src/web_ir/primitives/mod.rs
git commit -m "refactor(web_ir): single canonical z ladder (ZTier::z_value, tier*100)"
```

### Task A8: `@layer(tier:)` on components + primitive-name table alignment

`@layer` parses but only lands on the endpoint-fn HIR struct (`hir/nodes/decl.rs:469-471`); `HirReactiveComponent` (`decl.rs:577-586`) has no layer field, and `default_for_primitive` (`hir/nodes/layer.rs:91-100`) speaks PascalCase names (`Tooltip`, `Dialog`) that don't exist in the lowercase primitive set (`modal`, `toast`, `drawer`, `overlay`).

**Files:**
- Modify: `crates/vox-compiler/src/hir/nodes/layer.rs` (lowercase rows in `default_for_primitive`)
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` (`HirReactiveComponent.layer: Option<HirLayerDecl>`)
- Modify: `crates/vox-compiler/src/parser/descent/decl/head.rs` (accept `@layer` before `component`; reuse the existing `@layer` arg parser at `head.rs:1964`)
- Modify: `crates/vox-codegen/src/web_ir/lower.rs` (component root gains `data-vox-layer` attr when set)
- Test: `crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs` (extend; the fn-decl test at `:333-337` shows the existing pattern)

- [ ] **Step 1: Write the failing tests**

```rust
// in hir/nodes/layer.rs tests
#[test]
fn default_tier_covers_the_lowercase_primitive_vocabulary() {
    assert_eq!(LayerTier::default_for_primitive("modal"), LayerTier::Modal);
    assert_eq!(LayerTier::default_for_primitive("toast"), LayerTier::Toast);
    assert_eq!(LayerTier::default_for_primitive("drawer"), LayerTier::Modal);
    assert_eq!(LayerTier::default_for_primitive("overlay"), LayerTier::Popover);
    assert_eq!(LayerTier::default_for_primitive("row"), LayerTier::Content);
}
```

```rust
// in ga_boilerplate_grafts_test.rs
#[test]
fn layer_decorator_attaches_to_component_decl() {
    let src = r#"
@layer(tier: chrome)
component NavRail() {
    view: column() { text("nav") }
}
"#;
    let hir = lower_to_hir(src); // use the same helper the fn-decl @layer test at :333 uses
    let comp = hir.components.iter().find(|c| c.name == "NavRail").expect("component");
    assert_eq!(comp.layer.as_ref().expect("layer decl").tier, LayerTier::Chrome);
}
```

- [ ] **Step 2: Run, watch fail** — `cargo test -p vox-compiler layer_decorator_attaches default_tier_covers`. The HIR test fails to compile (`no field layer`) — that counts as the red step for a struct change; the name-table test fails on `drawer`→Content.

- [ ] **Step 3: Implement.** (a) Extend `default_for_primitive` with lowercase rows (keep the PascalCase rows — they serve future semantic primitives): `"modal" | "drawer" => Modal`, `"toast" => Toast`, `"overlay" => Popover`. (b) Add `pub layer: Option<HirLayerDecl>` to `HirReactiveComponent` (+ `..Default` sites — compiler errors guide you). (c) In the component-decl parser, accept the `@layer` decorator using the same arg parsing as the fn path, store on the component. The reserved-tier check (`typeck/mod.rs:179`) must also run over component layer decls — extend its input collection. (d) In `web_ir/lower.rs`, when lowering a component whose `layer` is set, push `("data-vox-layer", format!("\"{}\"", tier.as_str()))` on the root element (same quoting convention).

- [ ] **Step 4: Run** — `cargo test -p vox-compiler && cargo test -p vox-codegen`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/hir/nodes/layer.rs crates/vox-compiler/src/hir/nodes/decl.rs crates/vox-compiler/src/parser/descent/decl/head.rs crates/vox-codegen/src/web_ir/lower.rs crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs
git commit -m "feat(vuv): @layer(tier:) on component decls; tier defaults for lowercase primitives"
```

### Task A9: Wire tier-inversion into the validate pipeline (surface tree over the DOM arena)

`check_tier_inversions` has zero production callers. Wire it via a new `validate_layer` pass that builds the *surface tree* — only tier-introducing nodes (overlay-family primitives + explicit `data-vox-layer`) with nesting preserved; ordinary nodes are transparent so `modal { text }` stays legal. Two semantics decisions locked here (Pre-flight 5): (1) root-level surfaces under content flow are always allowed (they hoist to portals in A11); (2) Toast and Popover are *leaf surfaces* — they may not parent any other surface (`may_parent_surfaces`), which is what makes GA-26 criterion 2 (modal-inside-toast refuses) true.

**Files:**
- Modify: `crates/vox-compiler/src/hir/nodes/layer.rs` (`may_parent_surfaces`)
- Create: `crates/vox-codegen/src/web_ir/validate_layer.rs`
- Modify: `crates/vox-codegen/src/web_ir/mod.rs`, `validate.rs` (wire pass)

- [ ] **Step 1: Write the failing tests** in `validate_layer.rs`:

```rust
//! GA-26 wiring: build the surface tree from the DOM arena and run the
//! compiler's tier checks (`vox/layer/tier-inversion`, `vox/layer/leaf-surface`).
//! Surface-introducing nodes: overlay/toast/drawer/modal primitives and any
//! element carrying `data-vox-layer`. Ordinary elements are transparent.

use super::{DomNode, WebIrDiagnostic, WebIrModule};
use vox_compiler::hir::nodes::layer::LayerTier;

pub fn validate_layer(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    // Step 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_ir::{DomNode, DomNodeId, WebIrModule};

    /// nested(tags) builds tags[0] > tags[1] > … as a single spine.
    fn nested(tags: &[&str]) -> WebIrModule {
        let mut m = WebIrModule::default();
        for (i, tag) in tags.iter().enumerate() {
            let children = if i + 1 < tags.len() { vec![DomNodeId(i as u32 + 1)] } else { vec![] };
            m.dom_nodes.push(DomNode::Element {
                id: DomNodeId(i as u32), tag: tag.to_string(), attrs: vec![], children, span: None,
            });
        }
        m.view_roots.push(("V".to_string(), DomNodeId(0)));
        m
    }

    #[test]
    fn modal_inside_popover_overlay_is_tier_inversion() {
        let mut out = vec![];
        validate_layer(&nested(&["column", "overlay", "modal"]), &mut out);
        assert!(out.iter().any(|d| d.code == "vox/layer/tier-inversion"), "{out:?}");
    }

    #[test]
    fn modal_inside_toast_is_rejected_as_leaf_surface_violation() {
        let mut out = vec![];
        validate_layer(&nested(&["column", "toast", "modal"]), &mut out);
        assert!(out.iter().any(|d| d.code == "vox/layer/leaf-surface"), "{out:?}");
    }

    #[test]
    fn modal_with_ordinary_content_at_root_is_fine() {
        let mut out = vec![];
        validate_layer(&nested(&["column", "modal", "row", "text"]), &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn toast_inside_modal_is_leaf_ok_but_only_one_level() {
        // toast(5) under modal(4): allowed by child<=parent? No — 5>4 → inversion.
        let mut out = vec![];
        validate_layer(&nested(&["modal", "toast"]), &mut out);
        assert!(out.iter().any(|d| d.code == "vox/layer/tier-inversion"), "{out:?}");
    }
}
```

- [ ] **Step 2: Run, watch fail** — `cargo test -p vox-codegen validate_layer` (empty impl → no diagnostics).

- [ ] **Step 3: Implement.** (a) `LayerTier::may_parent_surfaces(self) -> bool { !matches!(self, LayerTier::Toast | LayerTier::Popover) }` in vox-compiler with a unit test. (b) `validate_layer`: recursive walk from each view root carrying `nearest_surface: Option<LayerTier>`; node's tier = explicit `data-vox-layer` (trim quotes, `LayerTier::from_str`) else `LayerTier::default_for_primitive(tag)` *if* tag ∈ {overlay, toast, drawer, modal} else transparent (inherit `nearest_surface`, recurse). On a surface node with `Some(parent_tier)`: if `!parent_tier.may_parent_surfaces()` → push `vox/layer/leaf-surface` ("a {parent} is a leaf surface and cannot contain a {child}; declare them as siblings"); else if `!parent_tier.allows_child(child_tier)` → push `vox/layer/tier-inversion` with the same message shape as `typeck/layer.rs:55-80`. Use `category: Some("layer".to_string())`. Wire into `validate_web_ir_full` after the palette pass. Neither code is advisory. (You are intentionally *not* converting through `LayerCheckNode` — the DomNode walk needs surface-transparency, which `check_tier_inversions`'s every-node walk doesn't model; leave the typeck fns for the HIR-level path and note the duplication for the C4 doc sweep.)

- [ ] **Step 4: Run suites** — `cargo test -p vox-codegen && cargo test -p vox-compiler`. Golden fallout = real tier bugs in fixtures; restructure those fixtures.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/hir/nodes/layer.rs crates/vox-codegen/src/web_ir/validate_layer.rs crates/vox-codegen/src/web_ir/mod.rs crates/vox-codegen/src/web_ir/validate.rs
git commit -m "feat(web_ir): tier-inversion + leaf-surface checks wired into validation (GA-26 rule 2/4)"
```

### Task A10: Close the occlusion escape hatches

`position="absolute"`, `top/left/inset/bottom/right`, arbitrary `z="999"`, and `raw_class="absolute z-[9999]"` all pass through unchecked inside partitioning containers (`primitives/mod.rs:782-812`) — they reintroduce exactly the bugs A9 forbids structurally.

**Files:**
- Modify: `crates/vox-codegen/src/web_ir/validate_layer.rs` (these are layer-discipline checks; same walk)
- Modify: `crates/vox-codegen/src/web_ir/primitives/mod.rs` (mirror `position`/inset/`z`/`raw_class` raw values into `data-vox-pos-raw`, `data-vox-z-raw`, `data-vox-raw-class` attrs, same pattern as A3 Step 5)

- [ ] **Step 1: Write the failing tests** (extend `validate_layer.rs` tests; helper from A9 plus an attrs variant):

```rust
#[test]
fn absolute_position_inside_partitioning_parent_is_rejected() {
    let mut m = nested(&["row", "panel"]);
    if let DomNode::Element { attrs, .. } = &mut m.dom_nodes[1] {
        attrs.push(("data-vox-pos-raw".to_string(), "\"absolute\"".to_string()));
    }
    let mut out = vec![];
    validate_layer(&m, &mut out);
    assert!(out.iter().any(|d| d.code == "vox/layer/absolute-in-partition"), "{out:?}");
}

#[test]
fn raw_z_index_outside_overlay_context_is_rejected() {
    let mut m = nested(&["column", "panel"]);
    if let DomNode::Element { attrs, .. } = &mut m.dom_nodes[1] {
        attrs.push(("data-vox-z-raw".to_string(), "\"999\"".to_string()));
    }
    let mut out = vec![];
    validate_layer(&m, &mut out);
    assert!(out.iter().any(|d| d.code == "vox/layer/raw-z-index"), "{out:?}");
}

#[test]
fn raw_class_smuggling_absolute_or_z_is_rejected() {
    let mut m = nested(&["row", "panel"]);
    if let DomNode::Element { attrs, .. } = &mut m.dom_nodes[1] {
        attrs.push(("data-vox-raw-class".to_string(), "\"shrink-0 absolute z-[9999]\"".to_string()));
    }
    let mut out = vec![];
    validate_layer(&m, &mut out);
    assert!(out.iter().any(|d| d.code == "vox/layer/raw-class-occlusion"), "{out:?}");
}

#[test]
fn absolute_inside_a_surface_node_is_allowed() {
    // Inside modal/overlay subtrees absolute positioning is the surface's own business.
    let mut m = nested(&["modal", "panel"]);
    if let DomNode::Element { attrs, .. } = &mut m.dom_nodes[1] {
        attrs.push(("data-vox-pos-raw".to_string(), "\"absolute\"".to_string()));
    }
    let mut out = vec![];
    validate_layer(&m, &mut out);
    assert!(out.iter().all(|d| d.code != "vox/layer/absolute-in-partition"), "{out:?}");
}
```

- [ ] **Step 2: Run, watch all four fail** (first three: no diagnostic; fourth passes trivially until the check exists — assert it stays green after Step 3).

- [ ] **Step 3: Implement.** Mirror attrs in `primitives/mod.rs`: when resolving kwargs `position`, `top`, `bottom`, `left`, `right`, `inset` push `data-vox-pos-raw` (value = the position value, or `"inset"` for the inset family); kwarg `z` with a value that is NOT one of the seven tier names pushes `data-vox-z-raw`; `raw_class` additionally mirrors verbatim to `data-vox-raw-class` (keep its existing class output — A10 only *flags*). In `validate_layer`'s walk, track `inside_surface: bool` (true once any surface node is an ancestor): outside a surface, `data-vox-pos-raw` ∈ {absolute, fixed, sticky, inset} → `vox/layer/absolute-in-partition` ("overlap requires a surface parent: wrap this in overlay/modal/toast/drawer or give the component @layer"); `data-vox-z-raw` → `vox/layer/raw-z-index` ("z is a closed tier enum: background…system_overlay"); `data-vox-raw-class` whose whitespace-split tokens include `absolute`/`fixed`/`sticky` or start with `z-` or `-m` → `vox/layer/raw-class-occlusion` naming the offending token. All errors, non-advisory.

- [ ] **Step 4: Corpus migration.** `cargo test -p vox-codegen && cargo test -p vox-compiler` — the dashboard chrome fixtures use `raw_class` heavily (`golden_dashboard_chrome_test.rs:43-81`); migrate offenders to sanctioned forms (surface parents, tier kwargs). Budget the largest churn of Phase A here. If a fixture has no sanctioned expression yet (genuinely needs absolute art), use `attr_class` + a tracked TODO referencing Phase E.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/web_ir/validate_layer.rs crates/vox-codegen/src/web_ir/primitives/mod.rs examples/ crates/vox-compiler/tests/
git commit -m "feat(web_ir): occlusion escape hatches closed (absolute-in-partition, raw-z-index, raw_class scan)"
```

### Task A11: Emit the portal infrastructure (stylesheet, roots, portal routing)

`emit_layer_stylesheet`/`emit_layer_portal_roots`/`emit_layer_portal_resolver` (`layer_emit.rs:45-105`) are never called; modal/toast/drawer emit inline `position:fixed` divs (`primitives/mod.rs:384-399`) that die inside any transformed ancestor. Generated apps must get the `[data-vox-layer]` CSS ladder + portal roots, and the overlay family must `createPortal` into them.

**Files:**
- Modify: `crates/vox-codegen/src/codegen_ts/scaffold.rs` (append `emit_layer_stylesheet()` output to the generated `app/globals.css`; emit `vox-layer-resolver.ts` + portal-roots component; mount roots in the app shell next to where `globals.css` is referenced — see `scaffold.rs:20,31`)
- Modify: `crates/vox-codegen/src/web_ir/primitives/mod.rs:384-399` (modal/toast/drawer emit a portal wrapper instead of inline fixed divs)
- Test: emit-snapshot tests in `scaffold.rs` / `primitives/mod.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// scaffold.rs tests
#[test]
fn generated_globals_css_contains_the_seven_tier_ladder() {
    let out = /* call the existing scaffold-generation fn used by neighboring tests */;
    let css = out.files.iter().find(|(p, _)| p == "app/globals.css").map(|(_, c)| c).expect("globals.css");
    for tier in ["background", "content", "chrome", "popover", "modal", "toast", "system_overlay"] {
        assert!(css.contains(&format!("[data-vox-layer=\"{tier}\"]")), "missing tier {tier}");
    }
}
```

```rust
// primitives/mod.rs tests
#[test]
fn modal_emits_portal_not_inline_fixed() {
    let emission = resolve("modal", &[]).expect("modal resolves");
    let all = format!("{emission:?}");
    assert!(!all.contains("fixed"), "modal must not inline position:fixed; got {all}");
    assert!(all.contains("data-vox-layer"), "modal must target its tier root");
}
```

(Adapt the exact `resolve` return-shape assertions to the existing modal test right next to `primitives/mod.rs:384` — assert on the same fields it asserts on.)

- [ ] **Step 2: Run, watch both fail.**

- [ ] **Step 3: Implement.** (a) Scaffold: append `layer_emit::emit_layer_stylesheet()` to the `globals.css` content; add generated files `app/vox-layer-resolver.ts` (= `emit_layer_portal_resolver()`) and the portal-roots JSX (= `emit_layer_portal_roots()`) mounted once in the emitted app shell. These are per-build generated files, NOT in the scaffold-once list. (b) Primitives: modal/toast/drawer resolve to an element tagged with `data-vox-layer="<tier>"` and lose their inline `position:fixed` styles; the emit path (reactive/emitter) wraps any `data-vox-layer`-tagged root in `createPortal(child, resolveLayerRoot("<tier>"))` using the resolver import. This also un-orphans `semantic_ui_emit.rs:19`'s dangling `./vox-layer-resolver` import — add an emit test asserting the generated TS for a dialog now typechecks via the existing tsc-compile e2e harness if one covers web (`crates/vox-cli-tests/tests/build_e2e.rs`).

- [ ] **Step 4: Run** — `cargo test -p vox-codegen && cargo test -p vox-cli-tests build_e2e`. Visual sanity: build one fixture app (`cargo run -p vox-cli -- build crates/vox-cli-tests/tests/fixtures/full_app -o /tmp/vuv-a11`) and grep the output for `data-vox-layer` roots.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/scaffold.rs crates/vox-codegen/src/web_ir/primitives/mod.rs crates/vox-codegen/src/web_ir/layer_emit.rs
git commit -m "feat(codegen): seven-tier portal ladder emitted; modal/toast/drawer route through portals"
```

### Task A12: Fix `@tokens` contrast check to validate fg/bg pairs (not light-vs-dark of one token)

`check_tokens` (`typeck/contrast.rs:35-76`) compares a token's own light/dark variants — never rendered together; meaningless. The registry JSON already models pairing (`on` + `text_role`, `tokens/mod.rs:190-201`); the `@tokens` declaration grammar needs the same.

**Files:**
- Modify: `crates/vox-compiler/src/typeck/contrast.rs`
- Modify: the `@tokens` grammar parser (find via the grammar test at `crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs:10-13`) to accept `on: <token-name>`
- Test: `ga_boilerplate_grafts_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn tokens_contrast_checks_fg_on_bg_per_variant() {
    // gray-300 text on white bg, declared as a pair: must refuse in light mode.
    let src = r#"
@tokens {
    color surface_page light: #ffffff dark: #1a1a1a
    color text_muted   light: #d1d5db dark: #6b7280  on: surface_page
}
"#;
    let diags = typecheck_diags(src); // same helper the existing @tokens grammar test uses
    assert!(
        diags.iter().any(|d| d.code.as_deref() == Some("vox/tokens/insufficient-contrast")),
        "gray-300-on-white pair must refuse compile, got {diags:?}"
    );
}

#[test]
fn tokens_without_on_pairing_no_longer_checks_light_vs_dark() {
    let src = r#"
@tokens {
    color brand light: #ffffff dark: #f8f8f8
}
"#;
    // Near-identical light/dark variants are FINE when the token isn't a paired fg.
    let diags = typecheck_diags(src);
    assert!(diags.iter().all(|d| d.code.as_deref() != Some("vox/tokens/insufficient-contrast")), "{diags:?}");
}
```

- [ ] **Step 2: Run, watch fail** — first test: no such code fires (parse error on `on:` or silent pass); second currently FAILS the old check (white vs near-white <4.5:1) proving the wrong axis.

- [ ] **Step 3: Implement.** Grammar: optional `on: <ident>` after the variant list, stored as `pair_bg: Option<String>` on the HIR token decl. `check_tokens`: delete the light-vs-dark comparison; for every token with `pair_bg`, resolve the bg token, compute contrast per variant (light fg hex vs light bg hex; dark vs dark) using the canonical `contrast_ratio` (A4 — vox-compiler can't depend on vox-codegen, so the canonical fn lives in vox-compiler; A4's step already placed it where both can reach: if it ended up in vox-codegen, move it to `vox_compiler::tokens::contrast` now and re-export). Code `vox/tokens/insufficient-contrast` (<4.5:1 error — declared pairs are body-text claims, hold them to AA).

- [ ] **Step 4: Run** — `cargo test -p vox-compiler contrast && cargo test -p vox-compiler ga_boilerplate`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-compiler/src/typeck/contrast.rs crates/vox-compiler/src/parser crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs
git commit -m "fix(typeck): @tokens contrast validates declared fg/bg pairs per variant"
```

---

## Phase B — Mobile parity (the guarantees follow the tree, not the target)

### Task B1: Mobile builds run the web_ir validators and gate on them

`BuildTarget::Mobile` (`build.rs:117-177`) goes HIR→RN directly; no validator runs; RN diagnostics are eprintln warnings. The validators are semantic checks over the lowered view tree — run them as an analysis pass (lower to web_ir, validate, discard the IR).

**Files:**
- Modify: `crates/vox-cli/src/commands/build.rs:117-132`
- Test: `crates/vox-cli-tests/tests/build_e2e.rs` (fixture-driven)

- [ ] **Step 1: Write the failing test.** Add fixture `crates/vox-cli-tests/tests/fixtures/mobile_bad_contrast/main.vox`:

```vox
component Home() {
    view: panel(bg: white) {
        text("barely there", color: gray.300)
    }
}
```

(plus the minimal `Vox.toml` copied from the neighboring `mobile_form` fixture, `target = "mobile"`). Test, following the existing e2e pattern in `build_e2e.rs`:

```rust
#[test]
fn mobile_build_fails_on_contrast_violation() {
    let out = run_vox_build_fixture("mobile_bad_contrast"); // same helper as neighboring tests
    assert!(!out.status.success(), "mobile build must gate on validators");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("insufficient_contrast"), "{stderr}");
}
```

- [ ] **Step 2: Run, watch fail** — `cargo test -p vox-cli-tests mobile_build_fails_on_contrast` (build currently succeeds).

- [ ] **Step 3: Implement.** At the top of the Mobile branch in `build.rs`:

```rust
// The guarantee validators are target-agnostic semantic checks over the view
// tree. Run them as an analysis pass; the web IR itself is discarded.
let web_ir = vox_codegen::web_ir::lower::lower_hir_to_web_ir(&hir);
let registry = vox_compiler::tokens::TokenRegistry::load_from_project_dir(&project_dir);
let diags = match &registry {
    Some(reg) => vox_codegen::web_ir::validate::validate_web_ir_with_registry(&web_ir, reg),
    None => vox_codegen::web_ir::validate::validate_web_ir(&web_ir),
};
let (errors, warnings): (Vec<_>, Vec<_>) = diags
    .iter()
    .partition(|d| !vox_codegen::web_ir::validate::is_advisory_diagnostic(d));
for d in &warnings {
    eprintln!("warning[{}]: {}", d.code, d.message);
}
if !errors.is_empty() {
    for d in &errors {
        eprintln!("error[{}]: {}", d.code, d.message);
    }
    anyhow::bail!("mobile build failed: {} validator error(s)", errors.len());
}
```

(Match the exact public validate fn names — grep `pub fn validate_web_ir` in `validate.rs`; A6 made the registry always-Some, so the None arm is belt-and-braces.) Note `--target=mobile` resolution of `project_dir` — reuse however the web branch obtains it.

- [ ] **Step 4: Run** — `cargo test -p vox-cli-tests`. The existing mobile fixtures (`mobile_form`, `full_app`) must still build: if they newly fail, they have real violations — fix the fixtures.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/build.rs crates/vox-cli-tests/tests/
git commit -m "feat(build): mobile target runs web_ir validators as a blocking analysis pass"
```

### Task B2: RN primitive parity — structural set + hard error for silently-flattened surfaces

`jsx_to_rn` (`rn/component.rs:364-510`) supports ~10 primitives; `wrap`, `card`, `list`, `list_item`, `route_outlet`, and the whole overlay family degrade to bare `<View>` via the `rn-unsupported-tag` fallback (`:493-509`). A silently-flattened modal is a shipped bug.

**Files:**
- Modify: `crates/vox-codegen/src/codegen_ts/rn/component.rs`
- Test: module tests beside the existing `jsx_to_rn` tests in the same file

- [ ] **Step 1: Write the failing tests** (match the call shape of the existing rn component tests in that file):

```rust
#[test]
fn structural_primitives_emit_on_rn() {
    for (tag, expected) in [
        ("wrap", "flexWrap"),
        ("card", "<View"),
        ("list", "<View"),       // semantic list container; FlatList reserved for data-driven lists
        ("list_item", "<View"),
    ] {
        let tsx = emit_single_primitive_for_test(tag); // mirror the existing per-tag test helper
        assert!(tsx.contains(expected), "{tag} must emit, got: {tsx}");
    }
}

#[test]
fn tier_primitives_are_hard_errors_on_rn_until_b4() {
    for tag in ["modal", "toast", "drawer", "overlay"] {
        let result = emit_single_primitive_expect_diags(tag);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "rn-unsupported-tier-primitive"),
            "{tag} must hard-error, not flatten to <View>"
        );
    }
}
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement.** Add match arms: `wrap` → `<View style={{flexDirection:'row',flexWrap:'wrap'}}>`; `card` → `<View>` with the card default style (border-radius + padding from the B3 table; until B3 lands use the existing style-table mechanism); `list`/`list_item` → `<View>` with column/row roles + `accessibilityRole="list"`/`"listitem"` where RN supports it. For `modal|toast|drawer|overlay`: emit diagnostic `rn-unsupported-tier-primitive` ("the overlay family has no RN representation yet — track Phase B4") and make `generate_rn` treat it as an error (B1's gate then fails the build — verify the severity plumbing reaches `rn_output.diagnostics`). `route_outlet` stays unsupported-with-existing-code (routes emit separately via `rn/routes.rs`).

- [ ] **Step 4: Run** — `cargo test -p vox-codegen rn && cargo test -p vox-cli-tests`. The tsc-compile e2e (CI) is the real gate — run the mobile fixture build locally.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/rn/component.rs
git commit -m "feat(rn): wrap/card/list/list_item emit; tier primitives hard-error instead of flattening"
```

### Task B3: RN style kwargs — token-driven lowering, no silent drops

RN reverse-engineers ~7 Tailwind class combos into a hardcoded 12-key StyleSheet with literal hex (`rn/component.rs:92-126,1043-1077`); everything else — `pad_x: 4`, `bg: blue.600`, `gap: 8` — is silently discarded on mobile.

**Files:**
- Create: kwarg→RN-style table as `fn kwarg_to_rn_style(kwarg: &str, value: &str) -> Option<(String, String)>` in `rn/component.rs` (or a new `rn/style_map.rs` if component.rs is over its LoC budget — check `docs/src/architecture/layers.toml`)
- Modify: `rn/component.rs` to consume HIR attrs directly through it; colors resolve via `validate_palette::resolve_color` (A3) so web and RN share one color SSOT
- Test: module tests

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn universal_kwargs_lower_to_rn_styles() {
    assert_eq!(kwarg_to_rn_style("pad_x", "4"), Some(("paddingHorizontal".into(), "16".into()))); // 4 × 4px scale
    assert_eq!(kwarg_to_rn_style("gap", "8"), Some(("gap".into(), "32".into())));
    assert_eq!(kwarg_to_rn_style("bg", "blue.600"), Some(("backgroundColor".into(), "\"#2563eb\"".into())));
    assert_eq!(kwarg_to_rn_style("radius", "xl"), Some(("borderRadius".into(), "12".into())));
}

#[test]
fn unmappable_kwarg_produces_dropped_kwarg_diagnostic() {
    // e.g. `leading` (line-height scale) until mapped
    let result = emit_component_with_kwarg_for_test("text", "tracking", "widest");
    assert!(result.diagnostics.iter().any(|d| d.code == "rn-dropped-kwarg"), "silent drops are forbidden");
}
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement.** Build the table by walking `UNIVERSAL_STYLE_KWARGS` (`primitives/mod.rs:58-123`) category by category: spacing kwargs (`pad*`, `gap*`, `m*`, `w`, `h`, …) multiply the Tailwind scale (×4px); colors via `resolve_color` (hex from the A3 palette — kills the hardcoded `#0a7ea4` table); `radius`/`shadow`/`opacity`/`align`/`justify` map to their RN equivalents (`borderRadius`, `elevation`+`shadowColor`, `opacity`, `alignItems`, `justifyContent`). Anything genuinely unmappable (web-only axes like `tracking`, `leading_*` until mapped, `safe_area` handled by existing RN SafeArea wiring) emits `rn-dropped-kwarg` *warning* naming the kwarg. Replace the `class_string_to_style_key` heuristic path for HIR-sourced components with direct kwarg consumption; delete the 12-key hex table once nothing references it.

- [ ] **Step 4: Run** — `cargo test -p vox-codegen rn && cargo test -p vox-cli-tests`. Inspect one emitted fixture's StyleSheet by eye for sanity.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/rn/
git commit -m "feat(rn): universal style kwargs lower to StyleSheet via shared palette (no silent drops)"
```

### Task B4: RN overlay host — modal/toast/drawer get a real mobile representation

Un-error the B2 tier primitives by giving them an RN story: a generated tier-ordered host component using RN `<Modal>` for the Modal tier and absolutely-positioned tier Views for toast/popover, mirroring the web ladder semantics.

**Files:**
- Create: `crates/vox-codegen/src/codegen_ts/rn/overlay_host.rs` — emits `VoxLayerHost.tsx` (one host, children slotted by tier, render order = `ZTier::z_value` order)
- Modify: `rn/component.rs` (modal→`<Modal visible={…}>`; toast/drawer→tier slots), `rn/scaffold.rs` (mount host in `App.tsx`)
- Test: module tests + the mobile tsc-compile e2e

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn layer_host_renders_tiers_in_ladder_order() {
    let tsx = emit_layer_host();
    let modal_pos = tsx.find("tier-modal").expect("modal slot");
    let toast_pos = tsx.find("tier-toast").expect("toast slot");
    assert!(modal_pos < toast_pos, "toast renders above modal");
}

#[test]
fn rn_modal_emits_native_modal_component() {
    let tsx = emit_single_primitive_for_test("modal");
    assert!(tsx.contains("<Modal"), "RN modal must use react-native Modal, got: {tsx}");
}
```

- [ ] **Step 2: Run, watch fail** (`emit_layer_host` doesn't exist; `modal` still hard-errors from B2).

- [ ] **Step 3: Implement.** `overlay_host.rs` emits a TSX component with one slot per tier in `z_value` order (context-based slot registration — `VoxLayerContext.register(tier, node)`); modal arm replaces the B2 error with RN `<Modal transparent visible={...}>`; toast/drawer render into their slots. Scaffold mounts `<VoxLayerHost>` at the App root. Remove `rn-unsupported-tier-primitive` for the now-supported tags (keep it for `overlay` if generic overlay stays web-only — decide by what the fixtures use; if kept, say so in the diagnostic message).

- [ ] **Step 4: Run** — `cargo test -p vox-codegen rn && cargo test -p vox-cli-tests` (tsc e2e compiles the generated host).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-codegen/src/codegen_ts/rn/
git commit -m "feat(rn): tier-ordered VoxLayerHost; modal/toast/drawer render natively on mobile"
```

### Task B5: Retire the stale Tauri-mobile path (ADR says desktop-only)

The accepted ADR (`docs/src/architecture/adr-NNN-scope-tauri-desktop-only.md`) scopes Tauri to desktop, but a Tauri-mobile CLI target and CI workflow remain — contradicting docs and burning CI.

**Files:**
- Modify: the CLI target enum/parse arm offering tauri-mobile (grep `tauri.*mobile|mobile.*tauri` in `crates/vox-cli/ crates/vox-config/`), the workflow (grep the same in `.github/workflows/`)

- [ ] **Step 1: Write the failing test** — in the CLI arg-parse tests (find the target-parse test module via `grep -rn "BuildTarget" crates/vox-config/src --include=*.rs`):

```rust
#[test]
fn tauri_mobile_target_is_rejected_with_adr_pointer() {
    let err = parse_build_target("tauri-mobile").unwrap_err();
    assert!(err.to_string().contains("desktop-only"), "must cite the ADR: {err}");
}
```

- [ ] **Step 2: Run, watch fail** (target currently parses).

- [ ] **Step 3: Implement** — remove the variant/arm; add the explicit rejection message ("Tauri is scoped desktop-only (see adr-NNN-scope-tauri-desktop-only.md); use --target=mobile for RN/Expo"). Delete the stale CI job. Memory note: Rust `Command` spawns must keep `CREATE_NO_WINDOW` if any spawn code is touched.

- [ ] **Step 4: Run** — `cargo test -p vox-cli -p vox-config && cargo run -p vox-arch-check`.

- [ ] **Step 5: Commit**

```bash
git add crates/ .github/workflows/
git commit -m "chore(tauri): retire tauri-mobile target+CI per desktop-only ADR"
```

---

## Phase C — Verification spine: the forbidden stays forbidden

### Task C1: Forbidden corpus — compile-fail harness over `examples/forbidden/`

Nothing today asserts that the structurally-impossible things actually fail. One `.vox` per forbidden bug class, each annotated with the exact diagnostic code it must die with; a harness test drives all of them. This is the regression net for every guarantee in Phases A/B — and the spec for what "VUV makes X impossible" *means*.

**Files:**
- Create: `examples/forbidden/` — initial set (first line of each file: `// expect-error: <code>`):
  - `contrast_gray_on_white.vox` → `web_ir_validate.a11y.insufficient_contrast`
  - `unknown_color.vox` → `web_ir_validate.style.unknown_color`
  - `raw_hex_color.vox` → `web_ir_validate.style.unknown_color`
  - `unknown_kwarg_typo.vox` → `web_ir_validate.style.unknown_kwarg`
  - `tier_inversion_modal_in_overlay.vox` → `vox/layer/tier-inversion`
  - `modal_inside_toast.vox` → `vox/layer/leaf-surface`
  - `absolute_in_row.vox` → `vox/layer/absolute-in-partition`
  - `raw_z_index.vox` → `vox/layer/raw-z-index`
  - `raw_class_absolute.vox` → `vox/layer/raw-class-occlusion`
  - `tokens_low_contrast_pair.vox` → `vox/tokens/insufficient-contrast`
  - `system_overlay_user_tier.vox` → `vox/layer/reserved-tier`
- Create: `crates/vox-compiler/tests/forbidden_corpus_test.rs`

- [ ] **Step 1: Write the harness + the first fixture, watch it fail.** Fixture `contrast_gray_on_white.vox`:

```vox
// expect-error: web_ir_validate.a11y.insufficient_contrast
component Bad() {
    view: panel(bg: white) {
        text("unreadable", color: gray.300)
    }
}
```

Harness:

```rust
//! Every file in examples/forbidden/ must FAIL compilation with exactly the
//! diagnostic code named in its `// expect-error:` header. This suite is the
//! contract for "VUV makes this bug unrepresentable" — a file that starts
//! compiling cleanly is a regression, not a victory.
use std::path::PathBuf;

fn forbidden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/forbidden")
}

#[test]
fn every_forbidden_example_fails_with_its_declared_code() {
    let mut checked = 0;
    for entry in std::fs::read_dir(forbidden_dir()).expect("examples/forbidden exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let expected = src
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("// expect-error: "))
            .unwrap_or_else(|| panic!("{path:?} missing `// expect-error:` header"))
            .trim()
            .to_string();
        // Full pipeline: parse → typecheck → lower → web_ir validate (registry default-on).
        // Reuse the compile-and-collect-diagnostics helper from golden_vox_examples_test.rs.
        let diags = compile_collect_all_diagnostics(&src);
        assert!(
            diags.iter().any(|d| d.code == expected),
            "{path:?}: expected `{expected}`, got {:?}",
            diags.iter().map(|d| &d.code).collect::<Vec<_>>()
        );
        checked += 1;
    }
    assert!(checked >= 11, "forbidden corpus shrank: {checked} files");
}
```

`compile_collect_all_diagnostics` = extract the parse→lower→validate plumbing already used by `golden_vox_examples_test.rs` into a shared helper (it currently filters advisory diagnostics at `:61` — this harness wants them unfiltered).

Run: `cargo test -p vox-compiler forbidden_corpus` — Expected: fails on the count assert (one file so far) — that's the red step for the harness itself.

- [ ] **Step 2: Add the remaining fixtures one at a time.** Each: write the `.vox`, run the harness, confirm it fails-with-the-right-code *because the Phase A/B check fires* (if it doesn't, you found a wiring bug — fix the check, not the fixture). Keep each fixture minimal (≤15 lines).

- [ ] **Step 3: Wire into CI visibility.** The test runs with `cargo test -p vox-compiler` so it's already in the Rust CI job — confirm with `grep -n "vox-compiler" .github/workflows/ci.yml` (no new job needed).

- [ ] **Step 4: Commit**

```bash
git add examples/forbidden/ crates/vox-compiler/tests/forbidden_corpus_test.rs crates/vox-compiler/tests/golden_vox_examples_test.rs
git commit -m "test(vuv): forbidden corpus - every impossible bug class must fail with its exact code"
```

### Task C2: `vox test view` — deterministic view snapshots (VUV-12 first slice)

The VUV-12 promise (render a view to a deterministic Web-IR snapshot with `--props`) has zero code. First slice: snapshot the *validated Web IR subtree* of one named view, not a browser render — deterministic by construction, no JS runtime needed.

**Files:**
- Create: `crates/vox-cli/src/commands/test_view.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs` + the clap derive (mirror exactly how `migrate` registered itself — see `commands/migrate/mod.rs:66` and its registration)
- Test: `crates/vox-cli-tests/tests/test_view_e2e.rs`

- [ ] **Step 1: Write the failing e2e test**

```rust
#[test]
fn test_view_snapshot_is_deterministic_and_prop_sensitive() {
    let fixture = fixture_dir("form_basic"); // existing fixture with a component
    let run = |props: &str| {
        run_vox(&["test", "view", "Home", "--props", props, "--project", fixture.to_str().unwrap()])
    };
    let a = run("title=Hello");
    let b = run("title=Hello");
    let c = run("title=Other");
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert_eq!(a.stdout, b.stdout, "same props must snapshot byte-identically");
    assert_ne!(a.stdout, c.stdout, "different props must change the snapshot");
}
```

(Adapt `run_vox`/`fixture_dir` to the helpers `build_e2e.rs` already uses; pick a component name actually present in the fixture.)

- [ ] **Step 2: Run, watch fail** — `error: unrecognized subcommand 'test'` (or the existing `test` subcommand lacks `view` — check `commands/mod.rs` first and slot accordingly).

- [ ] **Step 3: Implement.** Pipeline: load project → HIR → find component by name (error listing known components if absent) → substitute `--props k=v` as the component's props (string/number/bool literals; reject others with a clear message) → `lower_hir_to_web_ir` → run full validation (fail with diagnostics on error — a snapshot of an invalid view is worthless) → print the component's DomNode subtree as **canonical JSON** (serde_json with sorted keys / `to_string_pretty` over a BTreeMap projection) so byte-equality is meaningful.

- [ ] **Step 4: Run** — `cargo test -p vox-cli-tests test_view && cargo test -p vox-cli`.

- [ ] **Step 5: Register the new command** in the catalog if the command-sync gate requires it: run `cargo run -p vox-cli -- ci ssot-drift` (memory: CLI command additions cascade through `catalog.v1.yaml` — use the official sync commands, never hand-edit).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ crates/vox-cli-tests/tests/test_view_e2e.rs contracts/
git commit -m "feat(cli): vox test view - deterministic validated Web-IR snapshots (VUV-12 slice 1)"
```

### Task C3: Golden corpus expansion — the patterns LLMs must learn, as compiling goldens

Only 11 of 69 goldens are UI; none exercise `@form`, layers, or tokens (authoring G7) — so MENS retraining (VUV-7, still pending) would learn none of the guarantee-bearing patterns.

**Files:**
- Create: `examples/golden/form_basic_ui.vox`, `examples/golden/layered_overlay.vox`, `examples/golden/tokens_theme.vox`
- Modify: whatever golden-list test enumerates `examples/golden/` (grep `examples/golden` in `crates/vox-compiler/tests/`) picks them up automatically — confirm, don't assume

- [ ] **Step 1: Write `layered_overlay.vox`** (the canonical "how to do overlap right" sample — exercises A8/A9/A11):

```vox
@layer(tier: chrome)
component TopBar() {
    view: row(justify: between, pad: 4, bg: zinc.900) {
        text("Vox App", color: zinc.50, weight: bold)
        button(on_click: open_settings) { text("Settings") }
    }
}

component App() {
    let show_confirm = state(false)
    view: column(gap: 4) {
        TopBar()
        panel(surface: primary, pad: 6) {
            text("Body content", color: zinc.100)
            button(on_click: fn() { show_confirm.set(true) }) { text("Delete") }
        }
        modal(open: show_confirm) {
            text("Really delete?", weight: bold)
            row(gap: 2) {
                button(on_click: fn() { show_confirm.set(false) }) { text("Cancel") }
            }
        }
        toast(open: false) { text("Deleted") }
    }
}
```

> Before committing, compile it: `cargo run -p vox-cli -- check examples/golden/layered_overlay.vox`. Adjust to the *actual* reactive-state and modal-open kwarg shapes used by the dashboard fixtures (`crates/vox-cli-tests/tests/fixtures/full_app/main.vox` is ground truth) — the golden must compile clean with zero warnings; that's its test.

- [ ] **Step 2: Write `form_basic_ui.vox`** — an `@form` with text + bool fields (mirror `fixtures/form_basic/main.vox` but golden-corpus-shaped) and `tokens_theme.vox` — an `@tokens` block with `on:` pairs (A12 syntax) + components consuming `surface:`.

- [ ] **Step 3: Run the golden suite** — `cargo test -p vox-compiler golden`. All three must compile with zero validator errors AND zero advisory warnings (these are training-eligible exemplars; warnings in exemplars teach the model warnings are normal).

- [ ] **Step 4: Commit**

```bash
git add examples/golden/
git commit -m "docs(golden): form/@layer/@tokens exemplar goldens for the training corpus"
```

### Task C4: Doc truth sweep — SSOTs say what the code does

Drift documented by the audit: syntax SSOT says 14 primitives (real: 18) and stale VUV-7 status; the layer memo + GA-26 imply Rules 1–5 exist (post-Phase-A: Rules 1/2/3 enforced, 4/5 still future); GA-26 criterion 2 semantics changed (leaf-surface rule); `@resource` is claimed by MCP so VUV-15's cache decorator needs a new name.

**Files:**
- Modify: `docs/src/architecture/gui-authoring-syntax-2026.md` (status table: primitive count, VUV-7 reality, VUV-9 ✅, add a VUV-10..15 "not started" row block, link this plan)
- Modify: `docs/src/architecture/vuv-layered-layout-discipline-2026.md` (status: research → partially-enforced; per-rule enforcement table: Rule 1 ✅ A10, Rule 2 ✅ A7/A9, Rule 3 ✅ A11 — `Float<role>` realized as surface primitives + `@layer`, Rules 4/5 ⏳ Phase E; document `allows_child` + `may_parent_surfaces` semantics replacing criterion 2's original wording)
- Modify: `docs/src/architecture/boilerplate-reduction-gap-analysis-2026.md` (GA-26 status + criterion 2 wording; GA-20 status row)
- Modify: `docs/src/architecture/where-things-live.md` (new rows: `validate_palette.rs`, `validate_layer.rs`, `overlay_host.rs`, `test_view.rs`, forbidden corpus, palette contract)
- Note: VUV-15 cache decorator renamed `@singleton` (record in the roadmap doc; `@resource` = MCP, see `parser/descent/mod.rs:515`)

- [ ] **Step 1: Make the edits.** Keep frontmatter intact (these files have it; you are editing, not creating). Never touch `SUMMARY.md`/`architecture-index.md`/`feed.xml` by hand — regenerate.

- [ ] **Step 2: Regenerate + verify** — `cargo run -p vox-doc-pipeline` then `cargo run -p vox-cli -- ci docs-quality`. Expected: both green.

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs(vuv): SSOTs reflect enforced reality (rule status, primitive count, @singleton rename)"
```

---

## Phase D/E/F — Follow-up plans (triggered, not detailed here)

Per the roadmap discipline (each phase gets its own plan when its predecessor lands):

- **Phase D — Responsive & layout vocabulary (VUV-11 + XP-1).** Trigger: Phase B merged. Scope: breakpoint registry in `tokens.v1.json`; `_sm/_md/_lg` kwarg variants lowering to Tailwind variants on web AND a `useWindowDimensions`-driven style switch on RN (the cross-target semantics are the design work — do NOT ship web-only); `grid()`/`cluster()` primitives in both emitters; goldens + forbidden entries for breakpoint misuse. This is the highest-leverage *authoring* gap for "one UI, desktop and phone."
- **Phase E — Marks & typed subordination (GA-26 Rules 4–5, layout G2).** Trigger: Phase C merged and the leaf-surface model proven in practice. Scope: `mark:` declaration kwarg + `Mark<"…">` references through parser→HIR→`check_duplicate_marks`/`check_dangling_marks` (already written and unit-tested — this phase finally makes them reachable); tooltip-target/focus/scroll-anchor consumers; the `Float<role>` story for genuinely-absolute art (the A10 TODO escape valve).
- **Phase F — Stdlib & state (VUV-13/14/15).** Trigger: independent of D/E; after Phase C. Scope: `crates/vox-ui-stdlib` `chat_panel` (< 30-line reference chat app gate), `@session` per the roadmap's fixed design, `@singleton` (renamed from `@memo`/`@resource` pair per the C4 collision — `@memo` keeps its name, the resource-singleton becomes `@singleton`).

---

## Self-review notes (written against the audit, fresh-eyes pass)

1. **Coverage:** all 19 numbered audit gaps map to tasks: 1→A3/A4, 2→A1, 3→A2, 4→A12, 5→A6, 6→A9, 7→A11, 8→A10, 9→A7, 10→A8, 11→B1, 12→B2/B4, 13→B3, 14→A5, 15→C1, 16→C2, 17→C3, 18→B5, 19→C4. Deferred-by-design: XP-1/VUV-11 (Phase D), layout G2 marks (Phase E), VUV-13/14/15 (Phase F), XP-6 dual web emit + XP-7 iOS artifact pipeline (host-blocked; tracked in the mobile Phase-2 scoping doc), XP-8 heading levels (fold into B3 if trivial, else Phase D).
2. **Ordering constraints:** A1→A4 (contrast walker needs unquoted lookups); A2 before A4's error expectations; A3 before A4/B3 (shared `resolve_color`); A7 before A9/A11/B4 (ladder); A8 before A9 (`data-vox-layer` population); B2 before B4 (error → native swap). Within-phase order as listed is safe.
3. **Type consistency check:** `WebIrDiagnostic { code, message, span, category }` used uniformly; `resolve_color` signature consistent between A3 (def) and B3 (use); `contrast_ratio` lives in vox-compiler per A12's dependency-direction note — A4 implementers: place it there from the start.
4. **Honest limits:** A4 checks only resolvable fg/bg pairs (no inherited-from-CSS-default checking) — the forbidden corpus documents the boundary; A9's surface-transparency model intentionally diverges from `check_tier_inversions`'s every-node walk; C2 snapshots IR, not pixels (browser/pixel verification stays with the existing Playwright tier).
