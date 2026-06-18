---
title: "Automatic GUIs from Pure Logic — Feasibility Research"
description: "Prior art, feasibility, gains, and limits of deriving user interfaces automatically from program types/logic/data structures, with a Vox-specific gap analysis. Confirms type-driven and naked-objects UI generation as production-proven within the structurally-regular envelope."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-18"
---

# Automatic GUIs from Pure Logic — Feasibility Research

**Status:** Research / feasibility (no implementation).
**Companions:** [design-hygiene](auto-derivation-design-hygiene-2026-06-18.md) · [auto-debugging](auto-debugging-zero-annotation-research-2026-06-18.md) · [error-surfacing](error-surfacing-dual-audience-research-2026-06-18.md).
**Method:** Deep-research harness (adversarial 3-vote verification) + paced operator web search + read-only Vox codebase audit.
**Confidence grading:** ★★★ primary-source confirmed (or adversarially verified) · ★★ reputable single/secondary source · ★ inferred.

---

## 1. Thesis

A UI can be **derived from a standardized, reflective representation of a program's types/data** rather than hand-written per screen. This is mechanically grounded and production-proven — *within the structurally-regular envelope* (CRUD, admin, internal tools, data exploration). Outside that envelope the **intent/affordance gap** requires a small typed hint layer.

## 2. Confirmed prior art ★★★

- **Naked Objects** auto-generates the entire presentation layer from domain-object definitions by letting objects present themselves in a standardized way (Pawson PhD thesis; embodied by Apache Causeway/Isis). *(adversarially verified 3-0; [thesis](https://causeway.apache.org/docs/latest/_attachments/Pawson-Naked-Objects-thesis.pdf))*
- **Production-proven, not a prototype:** the Irish DSFA benefits system — Child Benefit Administration replacement live Nov 2002 (believed first operational naked-objects app); 1,000+ officers administering billions of euro/year. *(3-0)* This is the strongest evidence auto-GUI scales past demos.
- **Type-driven generation is mechanical:** GHC `DeriveGeneric` auto-derives a full sum-of-products structural representation of any datatype from one `deriving Generic` clause; AutoForms derives a GUI component directly from that structure with no per-type boilerplate. *(3-0 / 2-0; [GHC generics](https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/generics.html), [AutoForms](https://wiki.haskell.org/AutoForms/Tutorial))*
- **Values as UIs:** Conal Elliott's *Eros* manifests pure values as interactive GUIs (tangible values), unifying creation and execution. *(2-0; [Eros](http://conal.net/papers/Eros/eros.pdf))* Caveat: the "derives from types via DeepArrow" framing did **not** verify; only the value-manifestation framing survived.

## 3. Shipping mainstream practice ★★

- **react-jsonschema-form / react-admin** generate a React form "for any data, sight unseen" from a JSON Schema, with a separate **`uiSchema`** for presentation — the industry's answer to the affordance gap (structure auto-derived, presentation lightly hinted). ([rjsf](https://rjsf-team.github.io/react-jsonschema-form/docs/), [react-admin](https://marmelab.com/react-admin/JsonSchemaForm.html))
- **Admin/internal-tool generators** (Django admin, Hasura, Retool, PostgREST) dominate commercially in exactly the structurally-regular domain Naked Objects identified.
- **Projectional editors** (JetBrains MPS) derive the *editing* UI from language structure — the same principle applied to source rather than data. ([MPS](https://en.wikipedia.org/wiki/JetBrains_MPS))

## 4. Quantified gains ★★

The gain is **elimination of CRUD/admin boilerplate**, concentrated where data is structurally regular: one type definition becomes schema + validator + client + form + (with the naked-objects step) a full admin surface. The gain is **not** elimination of *design* — none of the prior art claims consumer-grade or graphics-rich UI from types.

## 5. The hard limit — intent/affordance gap ★★★

Structure tells you *what data exists and how it's typed*; it does not tell you **salience** (which of 40 fields matters), **affordance** (this string is a "send" action vs a label), **flow** (wizards, progressive disclosure, empty/error/loading states), or **brand**. Naked Objects uses one generic reflective viewer and is explicitly criticized as inappropriate for consumer-facing UIs. Verdict: **automatable for the structurally-regular envelope; needs a small typed hint layer beyond it.** The design question is "how small and how typed can the hint layer be" — see [design-hygiene §opt-in](auto-derivation-design-hygiene-2026-06-18.md).

## 6. Vox-specific gap analysis (codebase audit)

Vox is **~70% into this envelope already**:

| Capability | Status | Location |
|---|---|---|
| `@form` → full React form (state, validation, async submit, ARIA) | shipped | `crates/vox-codegen-ts/src/form_emit.rs` |
| CLI `clap` metadata → dynamic GUI form controls (reflection) | shipped | `crates/vox-cli/src/command_catalog.rs` |
| One Vox type → TS interface + Zod + DB validator + wire format | shipped | `crates/vox-codegen-ts/src/schema/type_maps.rs` |
| VoxDB schema → TS interfaces | shipped | `crates/vox-codegen-ts/src/schema/from_hir.rs` |
| `@reactive component` → React hooks/TSX | shipped | `crates/vox-codegen-ts/src/reactive/mod.rs` |
| **Type/`@table` → full admin CRUD UI (naked-objects step)** | **gap** | — |
| **Typed field inference beyond primitives** (enum→select, branded scalars, nested, lists) | **partial** | `hir_type_to_input_type()` = `int/float/bool/timestamp/else→text` |

VUV (Vox's typed UI syntax) is the enabler the Haskell prior art lacks: its design tokens are **enumerations, not CSS strings**, so a generated UI's choices are themselves typed and pickable. The genuine novelty for Vox is therefore the **naked-objects step** plus richer typed field inference — see implementation plan `../../superpowers/plans/2026-06-18-track-a-naked-objects-auto-gui.md`.

## 7. Open questions

1. What fraction of real apps fall outside the structurally-derivable envelope, and what *minimal* typed hint closes the gap (the `uiSchema` analogue)?
2. Does VUV's typed-token model let a generated admin UI be safely user-customized in-place (pickers over enumerated tokens)?

## Sources
Pawson thesis · GHC generics · AutoForms · Eros · Naked objects (Wikipedia) · MPS (Wikipedia) · react-jsonschema-form · react-admin · Django admin — full URLs inline above.
