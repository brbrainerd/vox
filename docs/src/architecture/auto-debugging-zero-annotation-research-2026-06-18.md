---
title: "Zero-Annotation Severity-Graded Debugging — Feasibility Research"
description: "Prior art and feasibility for surfacing program behavior without manual print/log statements: omniscient/time-travel debugging, automatic instrumentation, runtime selectivity, and automatic severity inference. Confirms capture-without-annotation is production-proven; selectivity and inferred severity are the design problems."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-18"
---

# Zero-Annotation Severity-Graded Debugging — Feasibility Research

**Status:** Research / feasibility (no implementation).
**Companions:** [error-surfacing](error-surfacing-dual-audience-research-2026-06-18.md) (presenting what's captured) · [design-hygiene](auto-derivation-design-hygiene-2026-06-18.md) · [auto-GUI](auto-gui-from-pure-logic-research-2026-06-18.md).
**Method:** Two deep-research passes (the verify phase was repeatedly rate-limited; a third paced operator-search pass closed the gaps) + Vox codebase audit.
**Confidence grading:** ★★★ confirmed · ★★ reputable source · ★ inferred.

> **Verification note:** across two harness runs the adversarial verify phase hit `Server is temporarily limiting requests`, recording many true claims as `0-0` (abstain) rather than refuted. The third pass fetched the primary sources directly. Grades below reflect the *direct-fetch* evidence, not the rate-limited tallies.

---

## 1. Thesis

Surfacing what a program does **without hand-written `print`/`log`** is not speculative — it is a solved-in-principle, partly-commercialized problem. The open work is **selectivity** (avoid trace-spam) and **severity** (what's worth surfacing), not **capture**.

## 2. Capture without annotation is production-proven ★★★

- **Omniscient / time-travel debugging.** Pernosco (built on **rr** recordings) records an execution, then replays with binary instrumentation to build a queryable database of all CPU-level state — giving instant access to full program state at any point and automatic control-/data-flow visualization, **with no source instrumentation**. *(adversarially verified 2-0 / 3-0; [pernos.co](https://pernos.co/), [vision](https://pernos.co/about/vision/))*
- **Automatic instrumentation.** OpenTelemetry eBPF Instrumentation (OBI) inspects executables + the OS network stack to capture spans/RED metrics **with zero code or config changes**, kernel-side, at **1–5% CPU overhead**. ([OTel OBI](https://opentelemetry.io/docs/zero-code/obi/), [demystifying](https://opentelemetry.io/blog/2025/demystifying-auto-instrumentation/))
  - **Critical limit:** eBPF captures *structural* telemetry (calls, timings, HTTP/gRPC spans) but **cannot derive in-process business/semantic events or custom attributes** — those need a language agent or hints; and it needs Linux + root. → automatic capture gives you *structure for free, meaning is not free.*

## 3. Selectivity has a proven mechanism ★★★

**Log2** (USENIX ATC 2015, Microsoft Research + UIUC): a cost-aware logging mechanism. Given a **budget** (max log volume per interval), it makes the **runtime "whether-to-log" decision** via two-phase filtering — cheaply discard irrelevant logs, then cache/emit useful ones within budget — at **negligible overhead**. ([Log2](https://www.usenix.org/conference/atc15/technical-session/presentation/ding))

Empirical motivation: developers log only **~30–42% of catch blocks and ~8–9% of checked call sites** ([Where Do Developers Log, ICSE 2014 SEIP](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/07/ICSE-2014-SEIP-Where-Do-Developers-Log-An-Empirical-Study-on-Logging-Practices-in-Industry.pdf)) — so **blanket instrumentation over-logs.** Selectivity is the whole game.

## 4. Automatic severity inference is tractable ★★★

- **Where-to-log** is learnable from code structure: a C4.5 classifier predicts logged sites at ~90% F-score on industrial systems (*Where Do Developers Log*); LogAdvisor frames placement as supervised classification (84–93% balanced accuracy).
- **What level** is learnable: **DeepLV** (ICSE 2021) suggests log levels via ordinal neural networks over syntactic context + message, **AUC ≈ 83.7**. ([DeepLV](https://ece.uwaterloo.ca/~wshang/pubs/Zhenhao2021ICSE.pdf))
- **Design consequence:** ~84% is the realistic ceiling → **severity should advise, not gate.** A rules-first v1 is safe; a learned model is a later swap behind the same interface.

## 5. Interrogative debugging ★★

Ko's **Whyline** answers "why did / why didn't" questions about runtime behavior via dynamic slicing, with a reported ~8× debugging-time reduction. ([Whyline](https://faculty.washington.edu/ajko/papers/Ko2004Whyline.pdf)) Conceptually adjacent to "ask the trace why X happened" — a natural LLM-facing query mode (see [error-surfacing](error-surfacing-dual-audience-research-2026-06-18.md)).

## 6. K-complexity argument ★★★

Implicit instrumentation **removes** per-site labor (writing `print`, choosing levels) and replaces N hand-placed sites with one generic mechanism + a small policy. The cost moves from *source text* (where it rots) to *runtime/config* (centralized, toggleable). The constraint, repeatedly evidenced, is **selectivity** — capture is cheap, deciding what to surface is the hard part.

## 7. Vox-specific gap analysis (codebase audit)

Vox has a **mature observability foundation but a silent runtime**:

| Capability | Status | Location |
|---|---|---|
| L1 telemetry facade (trace context, event types, sensitivity S0–S3) | shipped | `crates/vox-telemetry/` |
| Trace-context propagation across A2A/MCP/LLM | shipped | `crates/vox-telemetry/src/span.rs` |
| **Automatic interpreter execution-event stream** | **gap** | `vox-compiler::eval::*` emits only final result / explicit `print()`; natural hook = `track_step` (`eval/mod.rs:583`) |
| **Inferred severity** (vs hand-set `Error`/`Warning`) | **gap** | severity always explicit today |

Implementation plan: `../../superpowers/plans/2026-06-18-track-b-zero-annotation-debugging.md`.

## 8. Open questions

1. Overhead ceiling for interpreter-level capture before it perturbs program timing/behavior.
2. Adopt Log2's **per-run event budget** as a refinement of rules-based severity?

## Sources
Pernosco · OTel OBI · Log2 (USENIX ATC'15) · Where Do Developers Log · DeepLV · Whyline — full URLs inline above.
