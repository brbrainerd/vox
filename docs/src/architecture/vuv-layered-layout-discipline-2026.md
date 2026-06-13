---
title: "VUV Layered Layout Discipline — making Z-fighting and tier inversion structurally unrepresentable (2026)"
description: "Design memo motivating GA-26: typed Z-tiers, partitioning containers, and Mark<T> typed jump targets. Adopts wlr-layer-shell's four-tier model and i3/Sway's tree-of-partitioning-containers discipline as the structural foundation for VUV view trees."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-12"
training_eligible: true
training_rationale: "Names a class of UI bugs (Z-fighting, accidental occlusion, tier inversion) that appear in every web app and proposes a compile-time prevention strategy grounded in Wayland-compositor practice. Useful as the canonical answer to 'why doesn't VUV expose z-index?'"
---

# VUV Layered Layout Discipline — Design Memo

> **Companion graft:** [GA-26 in the boilerplate-reduction gap analysis](boilerplate-reduction-gap-analysis-2026.md#ga-26--layered-layout-discipline-typed-z-tiers--partitioning-containers--markt).

## §1 — Problem statement

Three classes of UI rendering bug recur across every web/native app:

1. **Z-fighting.** Two visible elements occupy the same rectangle at the same `z-index`. Render order is undefined; the surviving element flickers on layout shift, hover, or React re-render.
2. **Accidental occlusion.** An element is fully hidden behind another with no developer intent — typically caused by a sticky positioned ancestor, a CSS-grid auto-placement collision, or a portal landing inside a stacking context.
3. **Tier inversion.** A semantically-subordinate surface (a `Tooltip`) renders *above* a semantically-dominant one (a `Modal`), or worse, the dominant surface is interaction-blocked by a stale subordinate. (Ask any designer how often a tooltip survives the modal that opened it.)

CSS today provides no structural protection against any of the three. `z-index` is a global integer space; portals can land anywhere; stacking contexts are accidentally created by `transform`, `opacity`, `filter`, `will-change`, `isolation`, and a long tail of "wait, *that* triggers it?" properties. Designers paper over this with house conventions ("999 = modal, 1000 = toast"), but the conventions are unenforced and rot under refactor.

The desktop window-manager community has already solved this — twice. **`wlr-layer-shell`** (Wayland) and **i3/Sway**'s tree-of-containers expose the lessons. This memo extracts what to copy.

## §2 — What Sway / i3 / `wlr-layer-shell` get right

### 2.1 Tree partitioning instead of free positioning

Sway represents the screen as a tree: **root → outputs → workspaces → containers → leaves**. Every visible region of the screen is owned by *exactly one* leaf. Geometry is not expressed in pixels; it falls out of the tree because each non-leaf has a *layout* (`splith`, `splitv`, `tabbed`, `stacked`) and children divide their parent's rectangle along that axis. ([i3 §Tree](https://i3wm.org/docs/userguide.html#_tree))

The structural consequence: **two siblings inside a `splith` or `splitv` cannot overlap**. They literally cannot — the parent's rect is partitioned. Z-fighting between siblings is unrepresentable.

### 2.2 Tabbed / stacked: occlusion is total and intentional

When you *do* want stacking, Sway exposes `tabbed` and `stacked` layouts. At most one child is visible at a time; the rest are hidden behind a tab strip or title list. Occlusion is total (not partial), intentional (the user picks the visible child), and reversible. This is the right model for `<Tabs>` and `<Accordion>` — both are structural occlusion, not z-stack overlap.

### 2.3 Floating as the explicit, narrow escape hatch

Tiling is the default. Floating exists as an opt-out keyed off the surface's *typed role* — `dialog`, `tooltip`, `notification`, `popup_menu` — never the content. The i3 manual's framing: floating "violates the tiling paradigm but can be useful for some corner cases like 'Save as' dialog windows." ([i3 §Floating](https://i3wm.org/docs/userguide.html#floating))

The lesson: **a `Float` parent must be a typed primitive whose role is named in advance**. There is no `<div style="position: absolute">` in VUV; if you need overlap, you declare a `Float<Tooltip>` and the type system threads the tier.

### 2.4 `wlr-layer-shell`: Z-tiers as a closed enum

Every wlroots compositor exposes `zwlr_layer_shell_v1`, which defines exactly four named layers with fixed z-order: **`background` (0) → `bottom` (1) → `top` (2) → `overlay` (3)**. Regular shell surfaces sit between `bottom` and `top`. ([protocol spec](https://wayland.app/protocols/wlr-layer-shell-unstable-v1))

A surface chooses its tier *at construction*. The compositor enforces ordering between tiers. **Ordering within a tier is deliberately undefined** — designs that depend on it are explicitly broken, which is the right incentive: "if you care about ordering, declare a sub-tier."

### 2.5 Marks: typed cross-tree jump targets

i3/Sway marks are vim-style named labels on containers. `mark "primary-action"` declares; `[con_mark="primary-action"] focus` jumps. Marks are unique within a session (with `--replace` opt-in for re-use) and survive tree mutations. ([i3 §marks](https://i3wm.org/docs/userguide.html#vim_like_marks))

This is exactly the affordance VUV needs to **eliminate prop-drilling** for cross-tree references: tooltip-target, scroll-anchor, focus-on-mount, modal-return-focus. A `Mark<"checkout-button">` is a unique, compile-checked handle — the compiler can verify uniqueness, dangling-target, and tier compatibility without runtime inspection of the DOM.

## §3 — What Sway gets wrong (and we shouldn't copy)

- **Auto-synthesized parent containers are confusing.** Splitting then closing leaves single-child split containers that change geometry on the *next* split in non-obvious ways. AeroSpace (an i3-clone for macOS) explicitly rewrote its tree from mutable doubly-linked to immutable persistent because of stability bugs. *Lesson:* tree mutation in VUV must be transactional and validated; partial states are the bug surface.
- **Mode-mixing is leaky.** Sway issue [#7591](https://github.com/swaywm/sway/issues/7591) shows mouse-tiling silently dropping a `workspace_layout tabbed` invariant. *Lesson:* invariants expressed only in config (and not in the type system) get bypassed by alternate input paths.
- **Floating is an unstructured pile.** Once you opt into floating, you lose the tree's guarantees entirely. *Lesson:* VUV's `Float` must itself be a typed primitive that retains tier and mark guarantees, not a free-for-all.
- **Drop-in i3 compat constrains evolution.** Sway is a near-superset of i3 by design and cannot fix i3's schema mistakes without breaking config compatibility. *Lesson:* VUV is greenfield; we don't owe back-compat to a CSS feature whose semantics are broken.

## §3.5 — Enforcement status (2026-06-12)

As of 2026-06-12 the five rules are wired into the compiler — previously they existed only as unit-tested functions with no production callers. Current state:

| Rule | Status | Where |
|---|---|---|
| 1. Partitioning containers; no `position:absolute` inside them | **Enforced** | [`web_ir/validate_layer.rs`](../../../crates/vox-codegen/src/web_ir/validate_layer.rs) — `vox/layer/absolute-in-partition`, `raw-z-index`, `raw-class-occlusion` |
| 2. Z-tiers a closed enum; one canonical ladder | **Enforced** | `ZTier::z_value()` = tier × 100 (single SSOT); `vox/layer/tier-inversion` over the surface tree |
| 3. Overlap requires a typed surface parent | **Enforced** | `overlay` is the transparent portal host; `modal`/`toast`/`drawer` carry `data-vox-layer`; everything else is a partition |
| 4. Subordination as a typed edge | **Partial** | leaf-surface rule (`may_parent_surfaces`: Toast/Popover may not parent surfaces) → `vox/layer/leaf-surface`. Full `Tooltip::for(Mark)` lifetime edges are backlog. |
| 5. `Mark<T>` typed jump targets | **Backlog** | `HirMark`/`check_duplicate_marks`/`check_dangling_marks` exist but have no surface syntax yet. |

Semantics note: GA-26 acceptance criterion 2 (Modal-inside-Toast refuses) holds via the **leaf-surface** rule, not tier ordering alone — `allows_child` (child ≤ parent) would permit Modal(4) under Toast(5), so `may_parent_surfaces()` is the additional guard. Emit still uses inline `position:fixed` for overlays (portal emission is backlog); the *structural compile-time* guarantees above are independent of that.

## §4 — The five rules for VUV

If VUV wants Z-fighting, accidental occlusion, and tier inversion to be **compile-time errors**, these are the rules:

### Rule 1. Layout containers are typed and partition their region.

`Row`, `Col`, `Tabs`, `Stack` divide their parent rectangle. Siblings within them are provably non-overlapping. **No `position: absolute` inside a partitioning layout.** The only way to overlap is to enter a `Float<role>` parent.

### Rule 2. Z-tiers are a closed enum, not a number.

VUV adopts a seven-tier ladder, extending `wlr-layer-shell`'s four to cover web/native app concerns:

```
Background       — wallpapers, backdrops
Content          — the main view tree (default for component renders)
Chrome           — app shell: nav rail, status bar, tab strip
Popover          — non-modal overlays anchored to a target (Tooltip, Menu, ComboboxList)
Modal            — interaction-blocking dialogs (Dialog, AlertDialog, ConfirmDialog)
Toast            — transient self-dismissing notifications (Snackbar, Banner)
SystemOverlay    — debug overlay, accessibility cursor, focus ring (escape-hatch tier; reserved)
```

A surface declares its tier at construction. The type system rejects placing a `Tooltip` (Popover tier) at `Modal` tier or vice versa. **Within-tier ordering is deliberately unspecified** — designs that depend on it are explicitly broken.

### Rule 3. Overlap requires an explicit `Float` / `Overlay` parent.

The way i3 requires `floating enable`. Tiled is the default; the escape hatch is named, narrow, and keyed off the surface's *role* (`Dialog`, `Tooltip`, `Toast`) — never its content. There is no `position: absolute` in VUV source; the codegen emits absolute positioning, but only as a consequence of a typed `Float<role>` parent.

### Rule 4. Subordination is a typed edge, not a render-order coincidence.

A `Tooltip` is constructed as `Tooltip::for(target: Mark<T>)`; the compiler enforces that:
- its tier is strictly above `target`'s tier (Popover > Content);
- it is dismissed when `target` unmounts;
- it is auto-positioned relative to `target`'s rect.

This is what i3 marks would look like if they carried lifetimes.

### Rule 5. Marks (typed jump targets) replace ad-hoc IDs.

Borrow `mark` / `[con_mark="…"] focus` directly:
- `Mark<"checkout-button">` is a unique, compile-checked handle for cross-tree focus, scroll, anchoring, and tooltip targeting.
- Uniqueness is enforced per view-tree scope (`vox/layer/duplicate-mark`).
- Dangling references are rejected at compile time (`vox/layer/dangling-mark`).
- This eliminates prop-drilling without re-introducing global string IDs.

## §5 — Comparison with status quo

| Problem | Status quo (CSS + React) | VUV with GA-26 |
|---|---|---|
| Two siblings render at same `z-index` | Undefined order; depends on DOM order | Structurally impossible; siblings inside a partitioning layout don't overlap |
| Tooltip renders above modal | `z-index: 1001` for tooltip wins; designer file-system-greps to fix | `vox/layer/tier-inversion` at compile time |
| Modal portal lands inside ancestor's stacking context (transform/filter) | Visible bug; debugger required | Modal lives in the `Modal`-tier portal root; ancestor stacking contexts cannot affect it |
| Cross-tree focus management ("focus the search input from header") | Refs prop-drilled, or global query selector | `Mark<"search-input">`; cross-tree handle |
| Tooltip survives unmount of its target | Common React leak | Tooltip's `Mark<T>` lifetime ends when target unmounts; tooltip auto-dismisses |
| `position: absolute` collisions in deeply-nested trees | Visual debugging only | Banned in source; only `Float<role>` parents emit absolute positioning |

## §6 — Out of scope

- **Native (iOS/Android) layer-shell binding.** Defer with [GA-09b](boilerplate-reduction-gap-analysis-2026.md#ga-09b--native-deep-link-emit-for-iosandroid). The seven-tier ladder maps cleanly to UIWindowLevel / WindowManager.LayoutParams, but this graft is web-first.
- **Compositing-layer hints.** `will-change`, `contain: layout`, `isolation` — these are performance follow-ups, not structural correctness.
- **Floating-window discipline at runtime.** A user dragging a "floating" window across tiles is a separate concern from compile-time prevention of tier inversion. The initial graft is structural-only.
- **Per-app workspace navigation.** i3-style workspaces map naturally to Vox `routes`, which already exist. No re-derivation needed.

## §7 — Cross-references

- [GA-26 in the gap analysis](boilerplate-reduction-gap-analysis-2026.md#ga-26--layered-layout-discipline-typed-z-tiers--partitioning-containers--markt) — the implementation graft.
- [GA-19](boilerplate-reduction-gap-analysis-2026.md#ga-19--semantic-ui-primitives-proposed-cc-25) — semantic UI primitives (dialog, tooltip, menu) bind to specific tiers; lands cleanest after GA-26.
- [GA-20](boilerplate-reduction-gap-analysis-2026.md#ga-20--design-tokens-as-types-cc-23) — design tokens host elevation shadows that visually convey tier; tiers are the structural sister of those visual tokens.
- [`crates/vox-codegen/src/web_ir/validate_overlay.rs`](../../../crates/vox-codegen/src/web_ir/validate_overlay.rs) — existing validation seam to strengthen.

## §8 — Sources

- [i3 User's Guide §Tree](https://i3wm.org/docs/userguide.html#_tree) — tree-of-containers model and split / tabbed / stacked layout invariants.
- [i3 User's Guide §Floating](https://i3wm.org/docs/userguide.html#floating) — typed escape hatch for non-tiled surfaces.
- [i3 User's Guide §VIM-like marks](https://i3wm.org/docs/userguide.html#vim_like_marks) — typed cross-tree jump targets.
- [`zwlr_layer_shell_v1` protocol spec](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) — four-tier layer shell with deliberate within-tier ordering opacity.
- [Sway issue #7591](https://github.com/swaywm/sway/issues/7591) — mode-mixing leak motivating type-system enforcement over config-only invariants.
- [AeroSpace](https://github.com/nikitabobko/AeroSpace) — persistent-tree rewrite for stability, motivating transactional view-tree mutation.
