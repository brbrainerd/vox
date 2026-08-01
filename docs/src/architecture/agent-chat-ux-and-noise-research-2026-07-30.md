---
title: "Agent Chat UX, Noise Control & Panel Information Design — Verified Research 2026-07-30"
description: "Adversarially verified research into presenting agent work without drowning the user: ARIA live-region roles as a ready-made severity taxonomy, WCAG 4.1.3 as a binding requirement for status panels, NN/g progress-indicator thresholds, and the CHI 2024 finding that verification consumes 22.4% of session time — with every agent-specific mapping labelled as inference."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Agent Chat UX, Noise Control & Panel Information Design (2026-07-30)

> **Provenance.** `deep-research` run `wf_227d4095-e0f`: **106 agents, 6.9M subagent tokens,
> 0 errors.** 16 claims survived 3-vote verification.
>
> **⚠ Two disclosures that govern how this document should be read.**
>
> 1. **Half the research question has zero evidence.** The tool-by-tool comparison — how Claude
>    Code, Cursor, Continue.dev, Aider, Zed, and Windsurf expose model selection, settings
>    schemas, Ollama/LM Studio integration, and VRAM gating — produced **zero** claims,
>    confirmed *or* refuted. Every verifier reported the session WebSearch budget exhausted
>    (200/200); that half was **never executed**. It is being re-run as `wf_49688075-a18` with
>    the budget restored. **Report it as unresearched, never as a null result** — it must not
>    be read as "these tools lack local-model support."
> 2. **Adversarial counter-search could not run for ANY claim in this pass.** Verification
>    rested entirely on direct primary-source fetches confirming that sources say what was
>    attributed to them. Per standing project guidance, *"0 contradicting sources found" means
>    unverified, not refuted* — confidence is weaker than the 3-0 vote counts suggest.
>
> **What survives that limitation well:** the ARIA, WCAG, and NN/g findings are stable W3C
> Recommendations and long-standing published guidance. They do not need a live adversarial
> sweep to be trustworthy. The weaker material is flagged inline.

Companions: [`vox-harness-graph-audit-2026-07-30.md`](vox-harness-graph-audit-2026-07-30.md),
[`claude-code-harness-mechanics-2026-07-30.md`](claude-code-harness-mechanics-2026-07-30.md).
Prior Vox art: [`gui-frontend-design-principles-2026-06-14.md`](gui-frontend-design-principles-2026-06-14.md),
[`vox-gui-surface-map-2026-06-14.md`](vox-gui-surface-map-2026-06-14.md),
[`ai-first-plan-3-gui-intuitiveness-2026-07-02.md`](ai-first-plan-3-gui-intuitiveness-2026-07-02.md).

---

## 0. Normative strength — read this before citing anything below

The sources here differ **enormously** in authority, and flattening them would be the main way
to misuse this document:

| Source | Strength |
|---|---|
| **WCAG 2.2 SC 4.1.3** | **Level AA — binding for conformance** |
| **WAI-ARIA 1.2** | W3C Recommendation (June 2023) — normative semantics |
| **WCAG SC 2.2.4** | Level **AAA** — aspirational, not a conformance obligation |
| **Mozannar et al., CHI 2024** | The only **measured empirical result**; n=21, self-labelled, 2022-era inline autocomplete |
| **Microsoft HAX / Amershi CHI 2019** | Validated *heuristics*; authors decline the "normative baseline" framing |
| **NN/g, Google PAIR** | Expert assertion; **no reported methodology, sample, or effect size** |

**Every source here predates agent harnesses.** NN/g (2006/2014), WAI-ARIA, WCAG, PAIR, and HAX
(2019) say nothing about tool calls, token budgets, cost meters, dockable agent panels, or
interrupting a long-running autonomous agent. **Every agent-specific mapping below is
inference and is labelled as such.** Cite the principle to the source; label the mapping as
ours.

---

## 1. ARIA live-region roles are a ready-made severity taxonomy (confirmed 3-0)

This is the highest-leverage finding in the document, because it means **the severity taxonomy
a harness needs already exists as a W3C Recommendation** — no invention required.

| Role | Implicit `aria-live` | Implicit `aria-atomic` | Agent-harness mapping *(inference)* |
|---|---|---|---|
| `alert` | **assertive** | `true` | Approval-required prompts; failures |
| `status` | polite | `true` | Run status |
| `log` | polite | **`false`** | High-frequency tool-call streams |
| `timer` | **off** | — | **Not announced at all** |
| `marquee` | **off** | — | **Not announced at all** |

WAI-ARIA defines live regions as *"perceivable regions of a web page that are typically updated
as a result of an external event when user focus may be elsewhere"* — precisely an agent panel
streaming status beside a chat input the user is typing in.

> **Three nuances the verifiers pulled out:**
> 1. `log` is `aria-atomic="false"` (unlike `alert` and `status`) — it announces *increments*,
>    which is exactly right for an append-only tool-call stream.
> 2. **`log` and `timer` are not interchangeable.** Routing tool-call streams to `timer`
>    suppresses announcement **entirely**; `log` announces politely.
> 3. **Spec-implicit values are not real screen-reader behaviour.** NVDA, JAWS, and VoiceOver
>    diverge in practice for `log` and `status`.
>
> **Scope limit:** *"a web page"* — this binds web/Electron/webview harnesses (Vox Axis is
> Tauri, so it binds), **not** a terminal TUI.

### 1.1 `assertive` must be rare (confirmed 3-0)

MDN, verbatim: assertive *"should only be used for time-sensitive/critical notifications that
absolutely require the user's immediate attention. Generally, a change to an assertive live
region will interrupt any announcement a screen reader is currently making. As such, it can be
extremely annoying and disruptive and should only be used sparingly."*

Because `role="alert"` carries **implicit assertive**, this governs alert usage too. In an
agent harness, only approval prompts and failures plausibly qualify. **Streaming tool-call
chatter must never be assertive.**

### 1.2 ⚠ `role="alert"` must not carry interactive controls — this is a live Vox defect

Two hard constraints:

- APG, verbatim: *"Because alerts are intended to provide important and potentially
  time-sensitive information without interfering with the user's ability to continue working,
  it is crucial they do not affect keyboard focus."* The pattern's Keyboard Interaction section
  is **"Not applicable"**, and APG redirects to the **Alert Dialog** pattern when interrupting
  the workflow *is* required.
- MDN, verbatim: *"The alert role should only be used for text content, not interactive
  elements such as links or buttons."*
- MDN also: a live region **must pre-exist in the DOM** — dynamically injecting an
  already-populated `role="alert"` element *"generally does not lead to an announcement."*

**Consequence for Vox:** approval prompts that need a button belong in an **alert dialog or a
persistent panel affordance**, *not* a toast. Vox's `SecretaryToast` and approvals surface both
need auditing against this — a toast carrying an actionable control is the exact anti-pattern
named here.

> **Caveat:** this is authoring guidance, not browser enforcement. An author who calls
> `.focus()` on the toast still steals focus.

---

## 2. WCAG SC 4.1.3 is binding (confirmed 3-0)

**Level AA.** Normative text, verbatim:

> "In content implemented using markup languages, status messages can be programmatically
> determined through role or properties such that they can be presented to the user by
> assistive technologies **without receiving focus**."

Intent, verbatim: *"to make users aware of important changes in content that are not given
focus, and to do so in a way that doesn't unnecessarily interrupt their work."*

WCAG 2.2 is current (Oct 2023, editorially updated Dec 2024), and **4.1.3 was retained while
4.1.1 was obsoleted** — this is not a stale 2.1 artifact. The spec's own definition of a status
message (results of an action, waiting state, progress, existence of an error) maps directly
onto an agent activity feed.

### 2.1 This makes audit finding F10 a conformance issue, not a polish issue

The audit found that **28 of 33** Vox dashboard widgets render via `SurfaceMiniRender` with
`aria-hidden="true"`. Under 4.1.3 that is not merely low information density — a live status
surface hidden from assistive technology **fails a Level AA criterion**. The severity of F10
should be raised accordingly.

> **Two scope limits:** 4.1.3 binds markup UIs, not a terminal TUI. And it is a **delivery
> guarantee** imposing **no** requirement about coalescing, throttling, or severity tiers — a
> live panel is separately governed by SC 2.2.2 (pause/stop/hide), 1.4.13, and 2.4.3.
>
> A verifier **explicitly rejected** framing 4.1.3 as a progressive-disclosure analogue.
> Over-applied `aria-live` is itself a known verbosity hazard the criterion does not address.

---

## 3. Coalescing has an accessibility rationale, not just an attention one (confirmed 3-0)

APG, verbatim: *"Frequent interruptions inhibit usability for people with visual and cognitive
disabilities, which makes meeting the requirements of WCAG 2.0 success criterion 2.2.4 more
difficult."*

SC 2.2.4: *"Interruptions can be postponed or suppressed by the user, except interruptions
involving an emergency"* — aimed at users with attention deficit disorders, low vision, and
blind screen-reader users.

**So the remedy the standard actually prescribes is user-controllable postponement or
suppression.** A harness should offer **both** coalescing **and** a user control to defer or
mute status alerts.

> **Two qualifications:** SC 2.2.4 is **Level AAA**, so "must coalesce" would overstate the
> conformance obligation. And coalescing is an *application* of the frequency concern rather
> than the criterion's literal requirement.

### 3.1 Applied to Vox's measured toast distribution

The audit measured 179 toast invocations, **125 `warn` / 118 `backend-error`**, capped at
`MAX_TOASTS = 3` by **oldest-drop truncation**, with **no dedupe**.

Against this section:

- **Oldest-drop truncation is the wrong direction.** It discards the *first* error — usually
  the root cause — and keeps the cascade. Coalescing ("×5") preserves the root cause and
  reduces volume simultaneously.
- **No dedupe + a 2-second `APPROVALS_POLL_MS` loop** means one persistent backend failure
  re-announces every 2 seconds. Under `role="status"` that is polite-but-relentless screen-reader
  flooding; if any of those are assertive, it is disruptive by MDN's own description.
- **There is no user control to defer or mute.** That is the specific remedy SC 2.2.4 names.

---

## 4. Progress: spinners stop carrying information at 10 seconds (confirmed 3-0)

NN/g's duration taxonomy:

| Wait | Correct indicator |
|---|---|
| < 1s | none |
| 2–10s | looped/indeterminate animation *("should be reserved for actions that take between 2-10 seconds")* |
| **≥ 10s** | **percent-done progress indicator** |

Verbatim: *"if a spinner is rotating indefinitely, users cannot be sure if the system is still
working or if it's stopped, so they may decide to abandon the task entirely."* Nielsen (1993)
independently gives the same 10s limit.

**Critically, the source anticipates the nondeterminism objection that agent runs raise.** It
names the estimation difficulty, criticizes defaulting to spinners *because they "don't require
estimating the duration"*, and prescribes fallbacks: *"a general time estimate. (Don't try to be
exact…)"* and **step counts instead of percentages**.

**So step/phase progress is a sanctioned substitute where percent-done is uncomputable.** An
agent turn lasting minutes must show *step-of-N*, *current phase*, or *elapsed + phase* — never
a bare spinner. Vox has `PhaseChip` and `ChatExecutionRail` already; this is the standards
justification for making them primary rather than optional.

> **⚠ Refuted — do not cite:** the widely-repeated "progress bar makes users wait ~3× longer"
> figure. Also refuted: the WCAG 2.2.3 toast auto-dismiss-timeout constraint, and the framing of
> off/polite/assertive as a clean three-tier severity scale.

---

## 5. Progressive disclosure, and its conditions (confirmed 3-0 / 2-1)

NN/g's two-part rule, verbatim: *"Initially, show users only a few of the most important
options"* / *"Offer a larger set of specialized options upon request"* — with the warning that
designs beyond **2 disclosure levels** *"typically have low usability because users often get
lost when moving between the levels."*

Claimed benefit, verbatim: *"Progressive disclosure thus improves 3 of usability's 5 components:
learnability, efficiency of use, and error rate."*

**But it is strictly conditional**, and both conditions are quoted:

1. *"You must get the right split between initial and secondary features… You have to disclose
   everything that users frequently need up front, so that they have to progress to the
   secondary display only on rare occasions."*
2. *"It must be obvious how users progress from the primary to the secondary disclosure
   levels"* — with the control in a clearly visible spot, labelled to set expectations.

**A collapsed agent-activity panel with a buried expand control forfeits the benefit entirely,
and burying approvals or diffs that reviewers need routinely inverts the principle.**

> Attribute as *"Nielsen asserts"*, not *"studies show"* — no study, sample, or effect size is
> cited. The article predates agent UIs entirely; the tool-call/diff application is our
> inference. The article is silent on memorability and satisfaction, so "net gain" is a mild
> extrapolation.

**Note the convergence:** this two-level cap is the same shape as Claude Code's skill
progressive disclosure (mechanics doc §4.1) and the same shape as tool-retrieval architectures
(induction doc §1). Three independent literatures land on *small always-visible set + expand on
demand*. That is a strong signal for Vox's panel design.

---

## 6. What actually belongs in the panel (confidence: medium)

**The single strongest empirical anchor available**, and it points somewhere specific.

Mozannar, Bansal, Fourney & Horvitz (CHI 2024, arXiv 2210.14306) — 21 programmers
retrospectively labelling their own GitHub Copilot sessions under the CUPS taxonomy:

> *"the 'verifying suggestion' state takes up the most time at **22.4%** (sN=12.97), it is the
> top state for 6 participants and in the top 3 states for 14 out of 21 participants taking up
> at least 10% of session time for all but one participant."*

The authors note verification **continues after acceptance**, so 22.4% is a **floor**, and they
themselves draw the design implication: verification is a first-class cost introduced by
code-recommendation systems.

**Design consequence:** the panel should optimize **cheap verification** — diffs, plan/todo
state, *what changed and why* — over maximizing streamed output volume. Raw token throughput is
not the product; reviewable deltas are.

> **Three limits keeping this at medium:** cross-user *mean* rather than modal (top state for
> only 6 of 21); n=21 with self-labelled retrospective telemetry; and it measures **2022-era
> inline single-suggestion Copilot**, not multi-step agentic harnesses. The extrapolation
> direction favours the claim — agents produce larger, harder-to-verify diffs — but it is an
> extrapolation.
>
> **⚠ Refuted 0-3 — do not cite:** the 51.5% oversight-overhead total from the same paper, and
> its behaviour-conditioned suggestion-coalescing proposal.

### 6.1 What this does NOT establish

Explicitly unaddressed by any verified source: **token/context budget meters, cost display,
summary tiles vs live thumbnails, and dockable-panel ergonomics.** Vox's `ContextWindowMeter`
and `CostWidget` are *not* validated by this research — they are reasonable, and unevidenced.

This matters for audit finding **F10** (thumbnails vs summaries): the recommendation to replace
60%-scale thumbnails with summary tiles rests on (a) the `aria-hidden` conformance failure in
§2.1, which is solid, and (b) the verification-cost argument in §6, which is directional. It
does **not** rest on direct evidence that summary tiles beat thumbnails — **no such evidence was
found**, and that is an open question, not a settled one.

---

## 7. Steering and interruption (confirmed 3-0)

Microsoft's HAX Toolkit / Amershi et al. (CHI 2019) — **18 validated guidelines**, of which five
apply directly:

| # | Guideline | Application *(inference)* |
|---|---|---|
| **G3** | Time services based on context | Evaluate whether to notify during a critical task or postpone |
| **G7** | Support efficient invocation | Cheap to start/redirect an agent |
| **G8** | Support efficient dismissal | Cheap to stop |
| **G9** | Support efficient correction | Cheap to fix a wrong turn |
| **G10** | Scope services when in doubt | Narrow the action when intent is ambiguous |

The paper states: *"We propose 18 generally applicable design guidelines for human-AI
interaction"*, distilled from *"over 150 AI-related design recommendations"* (168 candidates →
35 concepts → 20 → 18) through four evaluation phases including 49 practitioners heuristically
evaluating 20 AI-infused products and 11 expert reviewers.

> **Three caveats:** validation was against **2019-era AI-infused products**, not agentic
> harnesses — *"apply to"* is more accurate than *"govern"*. **"Coalescing" appears nowhere** —
> G3 covers timing and deferral but says nothing about batching or merging repeated
> notifications, so that mapping is looser extrapolation. And the authors frame the set as
> heuristics and *"a resource to practitioners"*, self-reporting that the source list *"may not
> be exhaustive."*

**G10 is the one that indicts Vox's secretary most directly** (audit F2). "Scope services when
in doubt" is the precise inverse of *silently converting an ambiguous ten-word chat message
into an orchestrator task at 85% asserted confidence*. When intent is uncertain, the guideline
says **narrow the action** — propose, don't dispatch.

Google PAIR adds (confidence: medium, 2-1): keep *"engagement requests strategic, minimal, and
allow for easy dismissal"*, noting *"users are multitasking and your product isn't their primary
focus."*

> PAIR's sentence scopes to **feedback and engagement solicitation**, not the full toast surface
> — extending it to general notification volume is our inference. Asserted design guidance, no
> methodology or effect sizes.

---

## 7A. Oversight is proactive, not just retrospective (confirmed by direct verification, 2026-07-30)

> **Updated 2026-07-30 by the adversarial cross-check** (`harness-research-adversarial-crosscheck-2026-07-30.md`
> §2.3–2.4): the misalignment-taxonomy and steering-feature numbers below were re-fetched from
> corrected primary sources. The earlier unverified category percentages (38.33% / 22.58%) and
> the earlier unverified steering numbers (50–100+ messages, 4.9/5 rating, "2 of 8") are
> **retracted** — see the callouts inline.

A follow-up pass (`wf_49688075-a18`, re-running §7's tool-comparison gap) hit a session usage
limit mid-verification, but surfaced one paper rich enough to verify directly rather than
discard: **Chen et al., "Emergent Forms of Oversight Work in Developer-Agent Collaboration"**
(arXiv 2606.05391), confirmed by direct primary-source fetch.

**Study:** 17 experienced developers, interview-based, exploratory: *"Drawing on interviews
with 17 experienced developers, we conduct an exploratory inquiry examining what forms of
emergent oversight work developers perform, when, and how."*

**Four distinct forms of oversight**, verbatim: *"at least four forms of emergent oversight
work: **a priori control, co-planning, real-time monitoring, and post hoc review**."*

**The paper's central corrective claim** — and it is a direct challenge to how Vox's own panel
surfaces are currently framed (diffs, post-hoc approval): *"We show that oversight work is not
only reactive and retrospective, as portrayed in existing research, but also **preventative and
proactive**."*

A named failure mode, directly relevant to F2 (the secretary's substring-match auto-dispatch)
and to §6's verification-cost finding: developers adopt the heuristic of *"using test results
as guarantees for code correctness"* — a shortcut that degrades oversight quality exactly the
way alarm fatigue degrades alarm response.

**Consequence for the panel design in §6 and §8:** a panel built only around *post hoc
review* — diffs after the fact — covers **one of four** documented oversight forms. Vox needs
surfaces for the other three: **a priori control** (constraining before the run — permission
modes, the secretary proposing rather than dispatching per §7), **co-planning** (the `PlanPanel`
Vox already has), and **real-time monitoring** (the `ChatExecutionRail`/`PhaseChip` surfaces
Vox already has, but framed by §4's spinner findings). The gap is that these four are currently
built as *disconnected surfaces* rather than a *deliberate four-form model* — naming the model
is what lets Vox check coverage.

### 7A.1 The misalignment taxonomy, corrected (confirmed, direct re-fetch 2026-07-30)

The coding-agent misalignment paper is real: **arXiv 2605.29442**, analyzing **20,574
coding-agent sessions across 1,639 repositories**, spanning both IDE and CLI workflows —
*"operationalize[d] misalignment as a breakdown made visible through developer pushback,"*
annotated along four axes: form, cause, cost, resolution. **Seven** recurring forms are
identified, spanning *"how agents read projects, interpret developer intent, follow rules,
bound their actions, implement and execute code, and report progress."*

Two hard numbers, quoted directly:

- **90.50%** of episodes impose *effort and trust costs* rather than irreversible system
  damage — directly supporting the UX doc's framing (§8) that harness UX should optimize for
  *reviewability*, not just blocking catastrophic actions.
- **91.49%** of visible resolutions still require **explicit user correction** — a strong,
  independent corroboration of the CHI 2024 finding in §6 (verification dominates session
  time): here, correction is not just costly, it is **almost always necessary**.

And, quoted directly on the trend that motivates the audit's secretary finding (F2): *"while
overall rates decline, constraint violations and inaccurate self-reporting grow in share"*
over time — i.e. as agents get better at simple execution, the failure modes that remain
concentrate exactly in **rule-following** and **honest self-reporting**, the two things Vox's
secretary (substring-matching, unverified confidence percentages) is worst positioned to catch.

> **⚠ Retracted:** the earlier per-category percentages (38.33% constraint violation, 22.58%
> inaccurate self-reporting) and the CLI-worse-than-IDE claim could **not** be re-confirmed from
> the accessible page content and should not be cited. The paper's *existence*, *scale*, and the
> two headline percentages above are now solid; the finer breakdown is not.

### 7A.2 The steering-feature paper, corrected (confirmed, direct re-fetch 2026-07-30)

The correct citation is **arXiv 2503.02068**, *"Interactive Debugging and Steering of
Multi-Agent AI Systems"* (Epperson, Bansal, Dibia, Fourney, Gerrits, Zhu, Amershi — CHI 2025),
not the ACM DOI originally carried (which 403'd on re-fetch). Confirmed: **a two-part user
study with 14 participants**, evaluating **AGDebugger**, a tool built around browsing/sending
messages and *"the ability to edit and reset prior agent messages,"* with an *"overview
visualization for navigating complex message histories"* — direct prior art for exactly the
`ChatExecutionRail`/transcript-navigation problem in Vox's own panel design (§6). The paper's
own framing of *"the importance of interactive message resets for debugging"* corroborates
§7A's four-forms argument: resets are **real-time monitoring** tooling that supports
**a priori control** retroactively.

> **⚠ Retracted:** the specific numbers originally carried (50–100+ messages per task, a
> 4.9/5 feature rating, "2 of 8 participants" steered successfully) could **not** be confirmed
> from the abstract, and the "2 of 8" figure is suspect on its face — the confirmed participant
> count is **14**, not 8. Do not cite these numbers. The qualitative finding (message
> edit-and-reset is an important, validated steering pattern) stands; the specific figures do
> not.

## 8. Consolidated recommendations for Vox

Ordered by evidential strength, strongest first.

| # | Recommendation | Basis | Strength |
|---|---|---|---|
| 1 | Remove `aria-hidden="true"` from live dashboard widgets; give them real status semantics | WCAG 4.1.3 **AA** | **Binding** |
| 2 | Map toast tones onto ARIA roles: approvals/failures → `alert`, run status → `status`, tool-call stream → `log` | WAI-ARIA 1.2 | Normative |
| 3 | Move actionable approvals out of toasts into an alert dialog or persistent affordance | APG + MDN (`alert` must not carry controls) | Normative |
| 4 | Replace oldest-drop truncation with **coalescing** (`×N`), preserving the first/root error | SC 2.2.4 + APG | AAA + inference |
| 5 | Add a user control to defer/mute status notifications | SC 2.2.4's literal remedy | AAA |
| 6 | Never show a bare spinner for an agent turn — show step-of-N or phase | NN/g 10s rule + its own step-count fallback | Expert assertion |
| 7 | Prioritize diffs / plan state / what-changed in the panel over streamed volume | Mozannar CHI 2024 (22.4%) | Empirical, n=21 |
| 8 | Cap panel disclosure at **two** levels with an obvious, labelled expand control | NN/g | Expert assertion |
| 9 | Make the secretary propose rather than dispatch when intent is ambiguous | HAX G10 | Validated heuristic |
| 10 | Dedupe repeated identical errors from poll loops | §3.1 | Inference |
| 11 | Build panel/surface coverage around all four oversight forms — a priori control, co-planning, real-time monitoring, post hoc review — not post-hoc diffs alone | §7A (Chen et al. 2606.05391) | Qualitative, n=17 |

---

## 9. Open questions

1. **The entire tool-comparison half** — Claude Code / Cursor / Continue.dev / Aider / Zed /
   Windsurf model-selection schemas, per-mode assignment, Ollama & LM Studio integration, and
   VRAM/hardware gating. **Still unresearched after two attempts** (`wf_227d4095-e0f` and its
   re-run `wf_49688075-a18` both hit the session usage limit before completing this half). A
   third, narrower pass scoped to only this sub-question is needed.
1a. **Multi-agent debugging message-volume/steering findings and the coding-agent misalignment
   taxonomy** (arXiv 2605.29442, ACM 3706598.3713581) — real citations surfaced, not yet
   independently confirmed. Worth a dedicated re-verification pass; see §7A.
2. **What is the right coalescing window?** SC 2.2.4 and HAX G3 establish that frequency matters
   and interruptions should be postponable; **neither specifies a threshold**, and the one
   empirical coalescing proposal in the corpus was refuted.
3. **Summary tiles vs live thumbnails — is there any evidence either way?** No verified source
   addresses it, nor token/context budget meters, cost display, or dockable-panel ergonomics.
4. **How should determinate progress be constructed for a nondeterministic agent turn?** NN/g
   sanctions step counts and rough estimates, but there is no evidence on whether step-of-N plan
   progress, elapsed-time-plus-phase, or token-budget consumption best resolves
   still-working-vs-hung.
5. **Does high-frequency token streaming into an ARIA live region overwhelm real screen
   readers**, and what throttling or `role="log"` batching is empirically usable? The specs
   define semantics but say nothing about behaviour under sustained update rates. **This is a
   real risk for Vox's streaming transcript** and nobody has published an answer.

---

## 10. Refuted ledger — do not restate

| Claim | Vote |
|---|---|
| "A progress bar makes users wait ~3× longer" | refuted |
| WCAG 2.2.3 constrains toast auto-dismiss timeouts | refuted |
| off/polite/assertive form a clean three-tier severity scale | refuted |
| PAIR high-stakes control-retention claim | refuted |
| Mozannar 51.5% oversight-overhead total | **0-3** |
| Mozannar behaviour-conditioned suggestion-coalescing proposal | **0-3** |
| HAX G1/G2 confidence-disclosure claim (would have bridged to capability gating) | refuted |
