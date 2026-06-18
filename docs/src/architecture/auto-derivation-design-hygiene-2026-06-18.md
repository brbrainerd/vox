---
title: "Design Hygiene for Auto-Derived UI and Observability"
description: "Cross-cutting design principles for features that derive artifacts (UI, observability) from program structure: persistence is not UI intent (opt-in), capture is not meaning, selective-by-default, advise-not-gate, escape hatches, and K-complexity discipline. Distilled from the auto-GUI and auto-debugging research."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-18"
---

# Design Hygiene for Auto-Derived UI and Observability

**Status:** Design principles (cross-cutting). Distilled from [auto-GUI](auto-gui-from-pure-logic-research-2026-06-18.md), [auto-debugging](auto-debugging-zero-annotation-research-2026-06-18.md), and [error-surfacing](error-surfacing-dual-audience-research-2026-06-18.md) research.
**Audience:** any future session designing a "derive X from program structure" feature in Vox.

---

## 0. The unifying move

Both auto-GUI and auto-debugging are the same move: **derive an artifact from the program's structure instead of hand-writing it** (presentation layer from the type graph; observability layer from the execution graph). The principles below keep that move honest.

## 1. Declaring a thing is not asking for its surface (OPT-IN)

A `@table` declares *"persist this data"*, **not** *"build me a screen."* Conflating storage with presentation forces UIs nobody asked for.

- **Rule:** auto-GUI is **opt-in per type** via an explicit marker; persistence alone never generates UI.
- **Precedent:** Django's admin uses explicit registration; auto-register-**all** is known to cause conflicts when any model needs customization. ([Django admin](https://docs.djangoproject.com/en/3.2/ref/contrib/admin/))
- **Generalization:** any derivation that produces a user-visible or cost-bearing artifact should be opt-in, not implied by an unrelated declaration.

## 2. Capture is not meaning (STRUCTURE vs SEMANTICS)

Automatic capture (eBPF, an interpreter event hook) yields *structure* — calls, branches, timings, errors — **for free**. It does **not** yield *meaning* (which event is a significant domain event, what severity). ([OTel OBI limits](https://opentelemetry.io/docs/zero-code/obi/))

- **Rule:** pair automatic capture with an explicit **inference/hint** layer for meaning; never claim capture alone is observability.

## 3. Selective by default (NO TRACE-SPAM)

Developers log only ~30–42% of catch blocks; blanket instrumentation over-logs.

- **Rule:** surface a high-signal default (e.g. Notice+); make verbosity opt-in. Consider a **budget** (Log2: max events per interval) so volume is bounded by construction. ([Log2](https://www.usenix.org/conference/atc15/technical-session/presentation/ding))

## 4. Advise, do not gate (BOUNDED INFERENCE)

Automatic severity inference tops out ~84% AUC (DeepLV); LLM root-cause analysis solves a minority of hard cases (OpenRCA).

- **Rule:** inferred severity and LLM diagnosis are **advisory** — they rank and suggest; humans/CI decide. Keep a rules-first v1 with the same interface as a future learned model, so the model is a swap, not a rewrite.

## 5. Escape hatches everywhere (GENERATED ≠ FROZEN)

Generation targets the structurally-regular envelope; the intent/affordance gap is real (auto-GUI §5).

- **Rule:** every generated artifact is overridable per-instance (a generated admin view can be replaced by a hand-authored one; a global kill-switch exists, e.g. `VOX_EMIT_ADMIN`). Default to generated, opt out locally. The hint layer should be **typed** (VUV tokens; a `uiSchema` analogue), not stringly.

## 6. Reuse the format, don't fork it (ONE ENVELOPE)

- **Rule:** dual-audience output (human + LLM) extends the existing diagnostic envelope (`VoxCompilerDiagnosticPayload`), not a second parallel format. Curate (severity-rank) before piping to an LLM — telemetry quality dominates.

## 7. K-complexity discipline (REMOVE, DON'T ADD)

The point of these features is to **remove** per-site labor (hand-written forms; `print` lines; manual level choice), replacing N sites with one generic mechanism + a small policy.

- **Rule:** a derivation feature that adds significant *language surface* per use site has failed its own premise. Track B adds **zero** new language syntax (flags/config/crate only); Track A adds one optional marker. Prefer that profile.

## 8. Checklist for any future "derive X from structure" feature

- [ ] Is it opt-in, or does it fire from an unrelated declaration? (must be opt-in if user-visible/cost-bearing)
- [ ] Does it separate captured *structure* from inferred/hinted *meaning*?
- [ ] Is it selective/budgeted by default?
- [ ] Is inference advisory (not gating), with a rules-first v1?
- [ ] Is every generated artifact overridable, with a typed hint layer and a kill-switch?
- [ ] Does it reuse the existing output/diagnostic envelope?
- [ ] Does it remove more language surface than it adds?

## Sources
Django admin · OTel OBI · Log2 · DeepLV · OpenRCA — see the linked research docs for full citations.
