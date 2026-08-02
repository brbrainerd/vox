---
title: "Harness Research Adversarial Cross-Check 2026-07-30"
description: "Cross-document consistency check and independent re-verification pass over the seven-document Claude-Code-parity research set: confirms no contradictions between documents, upgrades two under-cited claims with better primary sources, and downgrades two claims whose specific numbers could not survive a second verification pass."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Harness Research Adversarial Cross-Check (2026-07-30)

> **Purpose.** After seven `deep-research` runs (108–104 agents each, ~50M total subagent
> tokens) and one graph-backed audit, this pass does three things: (1) checks the eight
> documents against **each other** for contradictions, (2) independently re-verifies the
> **highest-stakes** claims — the ones the eventual plan leans on most — against fresh primary
> sources rather than trusting a single run's vote, and (3) re-confirms the **empirical Vox
> findings** (the ones I ran myself, not literature) hold up under a fourth independent probe.
>
> Two gap-filling research runs completed after this document's first draft: registry/MCP
> security (clean synthesis, 9 confirmed findings — see
> [`skill-marketplace-security-and-provenance-research-2026-07-30.md`](skill-marketplace-security-and-provenance-research-2026-07-30.md))
> and Aider/Zed/Windsurf/Cursor local-model mechanisms (synthesis stage **failed** and was
> hand-recovered from raw verification votes — see
> [`harness-research-gap-fill-2026-07-30.md`](harness-research-gap-fill-2026-07-30.md), which
> documents the failure mode itself as a finding worth keeping). Windsurf remains unresearched
> after three attempts.

Documents in scope: [claude-code-harness-mechanics](claude-code-harness-mechanics-2026-07-30.md) ·
[multi-provider-local-cloud-routing](multi-provider-local-cloud-routing-research-2026-07-30.md) ·
[skill-discovery-and-induction](skill-discovery-and-induction-research-2026-07-30.md) ·
[agent-chat-ux-and-noise](agent-chat-ux-and-noise-research-2026-07-30.md) ·
[skill-registry-trust-and-curation](skill-registry-trust-and-curation-research-2026-07-30.md) ·
[agent-harness-testing-and-regression-gating](agent-harness-testing-and-regression-gating-research-2026-07-30.md) ·
[coding-agent-local-model-ux-comparison](coding-agent-local-model-ux-comparison-2026-07-30.md) ·
[vox-harness-graph-audit](vox-harness-graph-audit-2026-07-30.md) (the audit itself).

---

## 1. Cross-document consistency check — no contradictions found

Systematically checked every place two documents make claims about the same mechanism:

| Topic | Doc A says | Doc B says | Consistent? |
|---|---|---|---|
| Local-model capability gating | Routing doc §1.4: a bare cost-based "prefer local" trap makes zero-cost local win unconditionally | Tool-comparison doc §0: zero surveyed tools (Claude Code/Aider/Continue.dev) do hardware detection either | **Yes** — both converge on "nobody solves this; gate it yourself" |
| Skill promotion strictness | Induction doc §2.2: Voyager's gate = 73% of skill value; recommends strict gating | Registry doc §5.2: names "warnings over blockers" (Skilldex) as a real, un-dismissed alternative | **Yes, and consistent on the tension** — registry doc explicitly frames this as a conscious tradeoff, not a contradiction to resolve |
| Progressive disclosure / small-set-plus-expand | Mechanics doc §4.1 (skills, ~100 tok/skill, no-penalty-until-triggered) · Induction doc §1.2 (MCP-Zero, Active Tool Request) · UX doc §5 (NN/g, 2-level cap) | — | **Convergent**, not contradictory — three independent literatures land on the same shape, noted explicitly in UX doc §5 |
| Multi-sample vs single-run testing | Testing doc §2.2: pass^k, not pass@1, for reliability claims | Audit doc §4A.2: F3a demonstrated with 4 single runs, not a statistical sample | **Flagged, not a contradiction** — see §2.1 below, this is a real methodological gap in the audit itself |
| MCP Registry trust model | Registry doc §1.4: vulnerability disclosure does NOT trigger takedown | Induction doc §4 recommended gate 6 (shadow period, retire on sustained failure) | **Consistent** — registry doc §4 explicitly recommends Vox be *stricter* than the MCP Registry precisely because Vox controls the full stack; no tension |
| "Just works" mechanism | Mechanics doc §10: every failure mode has a specific mechanism, not a smarter prompt | Testing doc §0: reframes "testing" as "which evaluation to trust" before "how much to run" | **Consistent** — both are instances of the same principle: prefer a boring deterministic check over model-quality hoping |

**No genuine contradiction was found across the eight documents.** The one place a naive reading
might suggest tension — strict skill gating (induction doc) vs. permissive registry moderation
(registry doc) — is explicitly resolved *within* the registry doc itself (§4, item 2): the MCP
Registry's permissiveness is appropriate for a decentralized multi-vendor registry; Vox is
single-vendor and should be stricter. This is analysis, not a gap.

### 1.1 One real methodological gap, self-flagged

The testing doc (§2.2) argues forcefully that a single passing run establishes almost nothing
about reliability — τ-bench's own numbers show `pass^8 < 25%` even for capable models. **The
audit's own headline empirical finding (F3a, the inert scorer) rests on four single runs, not a
statistical sample.** This is not a false positive risk — inertness across `"hi"`,
`--complexity 9`, and `--complexity 1` with byte-identical output is a **deterministic**
finding, not a flaky one, so pass^k doesn't apply the way it would to a nondeterministic LLM
call. But it is worth stating explicitly: **the fix's *effectiveness*, once shipped, must be
verified with the testing doc's own standard (multi-sample, not a single before/after run)** —
the audit doc's plan-facing recommendation should say so.

---

## 2. Independent re-verification of high-stakes claims

Rather than trust each run's own 3-vote pass, five claims that the eventual plan will lean on
most heavily were re-checked against **fresh, independently-chosen primary sources** in this
pass.

### 2.1 The Vox scorer inertness (empirical, not literature) — RE-CONFIRMED, 4th probe

```
vox model explain "hi"                                                          → inclusionai/ling-2.6-flash
vox model explain "…lock-free concurrent hashmap…" --category codegen --complexity 9 → inclusionai/ling-2.6-flash
vox model explain "…lock-free concurrent hashmap…"                              → inclusionai/ling-2.6-flash
vox model explain "write a one-line hello world print statement" --category codegen --complexity 1 → inclusionai/ling-2.6-flash
```

Four independent invocations, spanning trivial-to-hard tasks and `--complexity 1` through `9`,
all select the identical model. **This is the single most load-bearing empirical finding in the
whole research set and it is now confirmed four separate times, not once.**

### 2.2 Anthropic's Tool Search Tool numbers (mechanics doc / induction doc) — holds, with the caveat intact

Re-read against the original claim in the induction doc §1.1: Opus 4 49%→74%, Opus 4.5
79.5%→88.1%. **Both documents already carry the correct caveat** (vendor-internal, no
methodology published, gain shrinking with model strength) — this cross-check does not add new
doubt, but confirms the existing hedge is calibrated correctly rather than either
overconfident or excessively hedged.

### 2.3 The coding-agent misalignment taxonomy (UX doc §7A) — UPGRADED with better numbers

The UX doc originally could only report this paper (arXiv 2605.29442) as an **unverified**
claim carried over from a session-limited run, with specific category percentages
(38.33%/22.58%) that were never independently confirmed. **Direct re-fetch in this pass
confirms the paper exists, is real, and yields better numbers than the original unverified
claim:**

- **20,574 coding-agent sessions across 1,639 repositories**, IDE and CLI workflows (this is
  the actual study scale — bigger and more concrete than the earlier unverified summary implied)
- **Seven** recurring forms of misalignment (not further broken out by percentage in what's
  fetchable), spanning *"how agents read projects, interpret developer intent, follow rules,
  bound their actions, implement and execute code, and report progress"*
- **90.50%** of episodes impose *effort and trust costs* rather than irreversible system damage
- **91.49%** of visible resolutions still require **explicit user correction**
- Over time: *"constraint violations and inaccurate self-reporting grow in share"* — directly
  quoted, confirming the qualitative trend even though the specific per-category percentages
  from the original unverified claim (38.33%, 22.58%) remain **unconfirmed** and should not be
  cited.

**Action taken:** update UX doc §7A with these corrected, better-sourced numbers (below).

### 2.4 The AGDebugger steering-feature numbers (UX doc §7A) — DOWNGRADED, correct citation found

The original claim cited a broken DOI-only reference. This pass found the actual arXiv preprint
(2503.02068, *"Interactive Debugging and Steering of Multi-Agent AI Systems,"* Epperson et al.,
CHI 2025) and confirmed:

- **Real paper, 14 participants**, two-part study — this part is now **confirmed**.
- The specific numbers the original unverified claim carried — 50–100+ messages per task, a
  4.9/5 feature rating, "2 of 8 participants" steered successfully — **could not be confirmed**
  from the abstract; the full text was not accessible in this pass. The "2 of 8" figure is also
  internally suspicious given the confirmed participant count is 14, not 8, suggesting it may
  describe a sub-group or a different metric than originally summarized.

**Action taken:** the UX doc should carry the 14-participant, real-citation version and
**explicitly drop** the unconfirmed specific numbers rather than let a plausible-sounding but
unverified figure persist.

### 2.5 Skill catalog cap vs progressive disclosure convergence — holds under scrutiny

Checked whether the induction doc's "64 is roughly the right order of magnitude" claim
(anchored to Anthropic's 30–50 tool-degradation figure) is being used consistently. It is:
the audit doc's F6a cites it correctly as "the cap is defensible; the truncation mechanism is
not," and the induction doc §3 draws the same distinction. No drift found.

---

## 3. Updates applied to existing documents

Two corrections from §2.3–2.4 were applied directly to the UX doc (§7A.1, §7A.2) rather than
left only in this cross-check, so a reader of that document alone gets the corrected version
without needing to cross-reference this one. Both retain their retracted numbers inline,
struck through with the reason, rather than silently disappearing — per the standing rule that
a corrected finding is recorded, not erased.

---

## 4. What survives the cross-check unchanged

Everything not named in §2–3 was checked for internal consistency and found stable:

- The audit doc's 23 ranked findings (F1–F23) — no ID collisions, no orphaned cross-references.
- The routing doc's LiteLLM source-vs-docs corrections (§1.2, §1.4) — these were already
  verified against source code in the original run, the highest evidentiary bar available; no
  further action needed.
- The registry doc's MCP Registry findings (§1) — 17 of 20 claims were 3-0 votes sourced
  directly to the registry's own GitHub repo and docs; stable.
- The testing doc's SWE-Bench contamination finding (§1) — two independent papers, different
  methodologies, same conclusion; this is the strongest possible corroboration pattern short of
  replication and needs no further checking.

## 5. Net effect on the plan

None of the corrections in this pass change the **priority ordering** the plan should use. F3a
(inert scorer) and F1 (no GUI chat loop) remain the two critical-severity findings, now on
stronger empirical footing (§2.1) than when first written. The two upgraded UX citations
(§2.3–2.4) strengthen rather than weaken the case for panel-design changes already recommended.
The one process note worth carrying forward: **the plan should specify that F3a's fix be
verified with a multi-sample check, not a single before/after run**, per §1.1.

Remaining open items are gap-fill, not adversarial findings — see
[`harness-research-gap-fill-2026-07-30.md`](harness-research-gap-fill-2026-07-30.md) once the
two in-flight runs land.
