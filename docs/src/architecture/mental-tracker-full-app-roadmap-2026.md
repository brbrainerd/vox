---
title: "Vox Mental-Health Tracker — Full-App Implementation Roadmap (2026)"
description: "Staged roadmap for the Vox-generated React Native mental-health tracker: calendar via React interop, trauma-informed append-only schema, slider/bottom-sheet/in-app-toast VUV primitives, and the codegen changes to deliver them."
category: "Architecture SSOTs"
last_updated: "2026-05-30"
training_eligible: false

schema_type: "TechArticle"
---

# Vox Mental-Health Tracker — Full-App Implementation Roadmap (2026-05-30)

> Status: **design / awaiting greenlight**. Produced by a 4-agent research workflow
> (calendar library · Vox↔React interop · PTSD/CPTSD clinical schema · fly-out/toast
> UX + codegen) and verified against the live `mobile/rn-expo-foundation` worktree.
>
> Invariants preserved throughout: **one append-only `HealthEventLog`**; **single HIR →
> dual target (web + RN)**; on-device durable persistence (already built & proven live).

---

## 0. Already done (verified live on Android emulator)

- `html.length is not a function` (Export render crash) — fixed in `builtin_registry.rs`
  (`lookup_method` now resolves unambiguous `Property` lowerings with no type hint).
- **Cross-relaunch durability** — on-device journal writes append-only NDJSON via
  `expo-file-system`; tapped Mood ×3 → force-killed app → relaunched → "Events stored: 3"
  survived. Applied to both the real adapter backend and the Expo Go shim.

---

## 1. Library + interop decisions

| Concern | Decision |
|---|---|
| **Calendar** | `react-native-big-calendar` v4.19.0 (MIT, pure-JS, month+week+day, no native module → works in Expo Go). Fallback `@howljs/calendar-kit` v2.5.6 only if timeline density becomes a hard requirement. |
| **Calendar delivery** | **React interop** (`import react`), NOT a new VUV primitive. A full calendar is too large to re-emit as RnNodes; it's the one widget that justifies the escape hatch. |
| **Slider** | New VUV primitive → `@react-native-community/slider` (native module; verify against pinned Expo/RN). |
| **Bottom sheet** | New `RnNode` arm reusing the existing `drawer` primitive → core RN `<Modal>` styled bottom-anchored. No `@gorhom/bottom-sheet` (avoids reanimated/gesture-handler weight). |
| **Toast** | Generated in-app `vox-toast.tsx` (ToastProvider + `useToast`, `Animated` fade + auto-dismiss). NOT `mobile.notify` (system seam), NOT a third-party lib. Carries undo/edit/delete actions with the just-returned `eid`. |
| **Edit/Delete** | Event-sourced. `record_correction` (exists) for edit; new `record_deletion` tombstone for delete. **No `db.Delete`** (throws `UnsupportedOnPlatform` on device). |

Interop is the critical-path enabler **and** the riskiest piece. Slider/sheet/toast are
pure-codegen primitives with web parity; calendar requires wiring `es_module_specifier`
into the RN emitter, which is currently **web-only**.

---

## 2. `@table` schema (trauma-informed, PTSD/CPTSD-aware)

Keep the single append-only log. **Do not add per-category tables.**

**A. Reuse `HealthEventLog` as-is** — already has `event_kind`, `payload_json`,
`correction_of`, tz fields, `schema_version`. The 13 clinical domains become `event_kind`
discriminants over `payload_json`; no change to the log itself.

**13 diagnostically-useful domains** (non-diagnostic labels in UI copy):
mood, sleep, intrusion/flashback, dissociation, hyperarousal/startle, avoidance,
nightmare, emotional-regulation, self-perception/shame (CPTSD), relationship/trust
(CPTSD), grounding/coping-used, substance/medication, body/somatic.

**B. Add a static `CategoryDef` registry** (data-driven Home grid + last-value defaulting):

```vox
@table type CategoryDef {
    category_id: str        // stable kind key, e.g. "mood", "sleep", "intrusion"
    display_label: str      // non-diagnostic, plain-language
    event_kind: str         // maps to HealthEventLog.event_kind
    input_mode: str         // "slider" | "toggle" | "counter" | "detailed"
    slider_min: int
    slider_max: int
    default_to_last: bool    // DECISION 2 (resolved): default slider to last value,
                             //   per-category opt-out (set false on anchoring-prone kinds)
    sort_order: int
    icon_key: str
}
```

**C. New tombstone mutation** (delete), append-only, collapses via existing
`_is_superseded_in`:

```vox
// vox:skip — excerpt; calls the existing `record_health_event` in main.vox.
// Not standalone by design: inlining that dependency here would duplicate the
// real mutation (split-brain) — so this block documents the addition in place.
@mutation fn record_deletion(of_event_id: str, tz_iana: str, tz_offset_minutes: int) to Result[str] {
    // sentinel kind; correction_of → target; never itself superseded → row hidden
    return record_health_event("_deleted", "{}", "", "app", "", of_event_id, tz_iana, tz_offset_minutes)
}
```

**D. New `@query last_value_for(category_id)`** — fold over `replayTable` for slider
default-at-last-value.

**Trauma-informed guardrails (copy, not schema):** non-diagnostic labels; last-value
default **opt-in per category** (anchoring → over-reporting risk); single-sleep-event
double-count guarded at materializer; append-only supersede, never erase.

**Open risk:** confirm a `_deleted` tombstone is excluded from `timeline_events_json`
**and** the CSV/clinical export (export must not leak deletes).

---

## 3. Codegen / VUV / runtime changes

| # | Change | Files | Scope |
|---|---|---|---|
| **3a** | **Slider primitive**: `RnNode::Slider` + `jsx_to_rn` arm for `"slider"` + emit/collect/clone arms; emit `@react-native-community/slider` import; web arm `<input type=range>`. | component.rs; web_ir/primitives/mod.rs; primitive_tags.rs; scaffold.rs | Bounded |
| **3b** | **Bottom sheet**: `RnNode::Sheet` arm for existing `"drawer"` tag → controlled `<Modal transparent animationType="slide">`; add `Modal` to RN imports; `sheet`/`sheet_backdrop` styles. | component.rs | Bounded |
| **3c** | **In-app toast**: generate `vox-toast.tsx` (Provider+hook); lower `toast(...)` view-call → `useToast().show({...})`; wrap router root in `<ToastProvider>` (both `App.tsx` flat **and** `app/_layout.tsx` router paths). Web mirror via existing Toast portal. | rn/mod.rs; scaffold.rs; routes layout; web_ir/layer_emit.rs | Significant |
| **3d** | **Calendar via React interop**: wire `es_module_specifier` into RN emitter — after sibling/endpoint import block in `emit_rn_component`, iterate `hir.imports`, emit `import {item} from "{spec}";` for each PascalCase tag used in this component's view. Thread `hir.imports` into `emit_rn_component`. Verify callback props lower as arrows, not IIFEs. | component.rs; rn/mod.rs; shared filter factored from reactive.rs | Significant |
| **3e** | **Edit/Delete affordances**: `record_deletion` mutation + UI wiring calling `record_correction`/`record_deletion` with returned `eid`. | main.vox | Bounded |

**Cross-cutting CI gate (3a/3b/3c):** vox-cli-tests asserts **both** targets `tsc`-pass for
one source. Every RN arm lands with its matching web arm in the same PR.

**Named-vs-default import gap (3d):** `es_module_specifier` carries no named/default
distinction; current emit is default-only. If `react-native-big-calendar` needs a named
import (`import { Calendar } from ...`), 3d expands to an IR/syntax extension
(`import react { Calendar } from "..."`). **Single biggest scope risk — resolve before 3d.**

---

## 4. Staged build order

Each slice is independently shippable and demonstrably improves the running app.

- **Slice 0 — Schema + delete** (smallest, no codegen): `record_deletion` + `CategoryDef`
  + `last_value_for` + tombstone-excluded-from-export test. Pure `.vox` + tests.
  Unblocks 3e + slider-defaulting.
- **Slice 1 — Toast (3c):** highest-leverage misclick recovery; unblocks undo/edit/delete.
  Bind the returned `eid` (currently discarded) so toast can offer undo.
- **Slice 2 — Edit/Delete (3e):** wire toast actions + row/sheet buttons. Depends 0 + 1.
- **Slice 3 — Slider (3a):** replace fixed Mood buttons with slider defaulting to last
  value (needs `last_value_for`).
- **Slice 4 — Bottom sheet (3b):** "add detailed" flow + edit host; fixes the silent
  `drawer → <View>` degradation; web parity free.
- **Slice 5 — Calendar via interop (3d):** capstone. Embed `react-native-big-calendar`,
  feed it a `@query` event list, wire `onEdit`/`onDelete` to Slice-2 affordances. Land
  with a vox-cli-tests fixture asserting the emitted TSX imports + renders an external
  component, `tsc --noEmit` clean.

```
Slice 0 (schema/delete) ──┬─→ Slice 2 (edit/delete UI) ──→ Slice 5 (calendar)
                          ├─→ Slice 3 (slider default)        ↑
Slice 1 (toast) ──────────┘─→ Slice 2 ─────────────────────────┘
                                Slice 4 (sheet) ──→ Slice 5 (edit host)
```

---

## 5. `.vox` source vs codegen split

| App `.vox` source (`apps/vox-mental-tracker/src/main.vox`) | Codegen (`crates/vox-*`) |
|---|---|
| `CategoryDef` table + seed data | `RnNode::Slider`/`Sheet` variants + arms |
| 13 `event_kind` payload shapes | `es_module_specifier` → RN import wiring (3d) |
| `record_deletion`, `last_value_for`, `record_correction` reuse | Generated `vox-toast.tsx` |
| `quick_add`/`add_detailed` binding `eid` | `slider`/`switch` in `PRIMITIVE_TAGS`; web parity |
| Home `CategoryGrid` + sheet/slider/calendar tags | `@react-native-community/slider` in `package.json` |
| `import react Calendar from "react-native-big-calendar"` | ToastProvider router-root injection (both paths) |

Rule: **anything one app needs → `.vox`; anything every Vox RN app should get → codegen.**
No raw TSX in the app except via `import react`.

---

## 6. Open decisions

### Resolved (2026-05-30)

- **Decision 2 — Last-value defaulting:** ✅ **Last value, per-category opt-out.** Slider
  defaults to last selected; `CategoryDef.default_to_last = false` disables it on
  anchoring-prone kinds. Wired via the new `default_to_last` field above.
- **Decision 3 — Runtime target:** ✅ **Dev Client** (one-time custom build), not Expo Go.
  Native modules (`@react-native-community/slider` now, on-device Whisper STT later) work
  without per-module Expo Go gymnastics. **Consequence:** the Slice 3 (slider) ordering is
  no longer gated by Expo Go compatibility — it can ship as soon as its codegen lands.
  Adds a one-time Dev Client build step before Slice 3.

### Still open (need a call before the affected slice)

1. **Named-export import syntax (blocks 3d/Slice 5).** Default or named import for the
   calendar? If named, land `import react { X } from "..."` IR extension now, or pin to a
   default-export calendar for v1? *Recommend: confirm export shape first; build the
   named-import extension only if forced.*
4. **Tombstone export semantics.** Confirm deleted rows excluded from clinical CSV export,
   not just the timeline. Affects Slice 0 acceptance. *Recommend: exclude from both.*
5. **Bottom-sheet vs drawer semantics.** Reuse `drawer` + a `position` attr (defer
   snap-points), or a distinct `sheet` primitive from the start? *Recommend: reuse
   `drawer` + `position`.*
6. **13-domain copy review.** Sign-off that category labels stay non-diagnostic before
   they ship in `CategoryDef` seed data.

**First action when greenlit:** Slice 0 — pure `.vox`, zero codegen, unblocks three
downstream slices. (Dev Client build slots in before Slice 3.)
