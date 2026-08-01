---
title: "How Coding Agents Expose Model Selection & Local Models — Verified Research 2026-07-30"
description: "Adversarially verified comparison of Claude Code, Aider, and Continue.dev's model-selection config schemas and local-model (Ollama/LM Studio) support: none of the three perform any hardware or VRAM detection before offering a local model, and Claude Code has no local-model provider option at all."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# How Coding Agents Expose Model Selection & Local Models (2026-07-30)

> **Provenance.** `deep-research` run `wf_c970605f-e82`: **104 agents, 7.3M subagent tokens,
> 0 errors** — clean, third attempt at this specific sub-question (the first two general-purpose
> UX runs, `wf_227d4095-e0f` and `wf_49688075-a18`, both hit session usage limits before
> reaching it — see [`agent-chat-ux-and-noise-research-2026-07-30.md`](agent-chat-ux-and-noise-research-2026-07-30.md)
> §9 and the routing doc's §8). Narrowing the question to *only* this sub-topic and re-running
> got a clean pass.
>
> **Coverage achieved:** solid, primary-sourced findings for **Claude Code, Aider, and
> Continue.dev**. **Coverage NOT achieved:** Zed (one page fetched, config schema unconfirmed),
> Windsurf (**zero surviving claims** — entirely unresearched), and Cursor (**not mentioned
> anywhere in the findings** — appears to have been dropped from the search entirely and needs
> a dedicated follow-up).

Companions: [`multi-provider-local-cloud-routing-research-2026-07-30.md`](multi-provider-local-cloud-routing-research-2026-07-30.md)
(the routing-engine research this closes the UX-facing half of),
[`agent-chat-ux-and-noise-research-2026-07-30.md`](agent-chat-ux-and-noise-research-2026-07-30.md).

---

## 0. The headline finding

**None of the three tools with solid primary-source coverage — Claude Code, Aider, or
Continue.dev — perform any hardware or VRAM detection before offering or recommending a local
model.** Local-model selection is manual everywhere it exists. Where a local model is too big
for available memory, the failure is reactive (a runtime OOM error with generic troubleshooting
advice), never preventive.

**This means Vox would not be behind the field by lacking hardware detection** — it would be
building a capability none of the surveyed tools have shipped. It is a genuine opportunity for
differentiation, not a gap to close defensively. See §5.

**Second headline finding: Claude Code has no local-model provider at all.** Not "supported but
undocumented" — the primary docs page was grepped and confirmed to contain **zero mentions** of
Ollama, LM Studio, VRAM, GPU, or hardware. Anthropic's own product is the least locally-capable
of the three surveyed tools. Vox's ambition to match "the same capacity that Ollama does"
(per the user's own framing) is not chasing Claude Code — it would be exceeding it.

---

## 1. Claude Code — single flat field, no local lane (confirmed, high)

### 1.1 Schema

A single flat `model` field in `settings.json` (or `CLAUDE_CODE_SUBAGENT_MODEL` env var, or
`--model` CLI flag), accepting either:

- an **alias** (`sonnet`, `opus`, `haiku`, `fable`), or
- a **provider-specific full model ID/ARN/deployment name**, depending on backend (Anthropic
  API, Bedrock, Microsoft Foundry, Google Cloud Agent Platform)

**No Ollama or LM Studio provider option, and no base-URL field for a local model server
anywhere in the schema** (confirmed 3-0, via exhaustive keyword search of the primary docs page
— zero hits for "Ollama," "LM Studio," "VRAM," "GPU," or "hardware").

### 1.2 Per-role assignment exists, but only across cloud backends

Subagents (Markdown files with YAML frontmatter) carry their own `model` field — alias, full
model ID, or `inherit` (the default). Additional override layers: the `Agent` tool's `model`
parameter, `CLAUDE_CODE_SUBAGENT_MODEL`, and skill/command frontmatter `model` fields.

**So Claude Code's per-role model story is real and well-designed** (this is genuinely good
prior art for Vox's own subagent model assignment) — **it just has no local option to assign
into any role.**

### 1.3 No hardware detection (confirmed, high)

Direct consequence of §1.1 — there is nothing to detect capability *for*.

---

## 2. Aider — per-role fields, local support unresolved (confirmed / partially unresolved)

### 2.1 Schema

Top-level `model` field in YAML config (`.aider.conf.yml`) or `--model` / `AIDER_MODEL`. Two
dedicated per-role fields:

| Field | Role |
|---|---|
| `weak_model_name` / `--weak-model` | commit messages, chat-history summarization |
| `editor_model_name` / `--editor-model` | file-editing in architect/editor mode, with its own `editor_edit_format` |

Per-model settings (not a single flat block) live in a separate `.aider.model.settings.yml`,
structured as a **YAML list of dictionaries** — closer to Continue.dev's array-of-models shape
than to Claude Code's single field.

> **⚠ Internal tension flagged, not resolved.** The main finding (confirmed, sourced to three
> mutually consistent docs pages) asserts `weak_model_name` is a genuine per-role field. A
> narrower, differently-worded claim — *"Aider supports a distinct 'weak-model' role assigned
> separately for commit messages **and** chat history summarization"* [as a single combined
> role] — was **refuted 1-2**. Read this as: the two sub-uses (commit messages; history
> summarization) may not be as cleanly unified under one role as the phrasing implied, not as a
> reversal of the field's existence.

### 2.2 The local-model mechanism is genuinely unresolved

> **⚠ Refuted 0-3 — do not restate:** that Aider configures Ollama/LM Studio specifically via
> `--openai-api-base` / `AIDER_OPENAI_API_BASE`. This was the natural guess and it **failed
> verification.**

**Open, not answered by this research:** what Aider's actual current mechanism is — a dedicated
`ollama/` model-name prefix, a separate env var, or something else. Aider clearly *has no
hardware detection* (confirmed 3-0, independent of the mechanism question), but the precise
config keys for pointing it at a local server remain unconfirmed.

---

## 3. Continue.dev — the most complete local-model story surveyed (confirmed, high)

### 3.1 Schema

Current schema is `config.yaml` (`config.json` is **deprecated** — note the migration,
relevant to Vox's own schema-stability planning). A top-level `models` array; each entry:

```yaml
models:
  - name: <label>
    provider: <provider-id>
    model: <model-name>
    apiBase: <url>
    roles: [chat, edit, apply, autocomplete, embed, rerank, summarize]  # default: [chat, edit, apply, summarize]
```

**Per-role assignment is done by listing roles on each model entry** — not via separate
top-level per-mode fields. This is a materially different (and arguably better) shape than
Claude Code's alias-per-field or Aider's named-field-per-role: **any model can serve any subset
of roles**, and a single config can express "use the small local model for autocomplete, the
frontier model for chat" declaratively.

### 3.2 Ollama configuration — exact and complete

```yaml
- name: Local Qwen
  provider: ollama
  model: qwen2.5-coder:1.5b
  apiBase: http://localhost:11434   # default; overridable for remote instances
```

Confirmed 3-0 with verbatim YAML examples quoted directly from Continue's own docs. Real model
names cited: `qwen2.5-coder:1.5b`, `deepseek-r1:32b`. **This is the closest published prior art
to what Vox needs** — a named `provider: ollama` entry sitting as a peer in the same array as
cloud providers, which is exactly the "any provider, not just OpenRouter" architecture the
routing research (routing doc §0, §1.1) already recommended on independent grounds.

### 3.3 No hardware detection — only reactive troubleshooting (confirmed 3-0)

The Ollama provider doc's only guidance for insufficient memory: **reduce `contextLength`, or
switch to a smaller model.** Triggered by a runtime error, not a pre-flight check. No
auto-detection language found anywhere on the page.

---

## 4. Zed and Windsurf — coverage gaps, stated plainly

**Zed** (confidence: medium): a single page (`zed.dev/docs/ai/llm-providers`) documents **five
distinct "model access paths"** — Zed-Hosted Models, API Access, Existing Subscription, Gateway,
and **Local Model** — for Zed's Agent, Inline Assistant, Git-commit, and thread-summary
features. A local-model path exists and is named, but its **actual config schema (the
equivalent of Continue's `apiBase`/`provider` keys) was not retrieved** — only one page was
fetched, and Zed's own separate "Use a Local Model" doc page was identified but not followed.
Whether Zed supports per-role model assignment across those four surfaces is also unconfirmed.

**Windsurf**: **zero surviving claims.** Not researched in this pass at all — this is a gap in
the research, not a finding that Windsurf lacks the capability.

**Cursor**: not present in any finding, confirmed or refuted, in this run's output. Also
unresearched, despite being named in the original question.

---

## 5. What this means for Vox — and it changes the framing

Cross-referencing the audit's empirical findings (audit doc §4A):

- **F9a** (zero local models reach Vox's catalog despite Ollama running with 3 models,
  including Vox's own `vox-mens-v1`) is **worse than the worst tool surveyed.** Even Claude
  Code, which has *no* local-model support at all, at least doesn't claim otherwise. Vox's
  `local_http=true` flag in the routing profile reporting itself enabled while contributing zero
  candidates is a specific defect none of the three surveyed tools would even have the surface
  area to exhibit.
- **Continue.dev's `provider`/`model`/`apiBase`/`roles` array is the schema to adopt**, not
  invent. It is simpler than Vox's existing `ChatProviderRouteKind` enum-of-structs while
  covering the same ground, and its per-role list (rather than Claude Code's per-role
  *duplication* across separate fields) generalizes better to Vox's multi-axis
  `SelectionAxes` — a model can be `roles: [local-preferred, codegen]` rather than needing a
  dedicated field per concern.
- **No hardware/VRAM detection exists anywhere in this survey.** This directly answers the
  user's framing — "our harness should have the same capacity that Ollama does, the ability to
  natively add any model." Ollama itself doesn't do capability-gating either (it fails at load
  time if a model won't fit); **the bar Vox should clear is Continue.dev's config completeness,
  not a hardware-detection feature nobody in this survey has built.** If Vox *does* build
  hardware detection (via `vox-plugin-nvml-probe`, already in-tree — routing doc §8 item 1),
  it would be ahead of the entire surveyed field, not catching up to it.
- **The schema-migration note matters operationally**: Continue.dev deprecated `config.json` for
  `config.yaml` — a real production tool went through exactly the kind of breaking config change
  Vox should plan a migration path for before shipping v1 of a routing policy file (routing doc
  §5, item 5).

---

## 6. Open questions

1. Zed's actual local-model config schema and per-surface model-assignment behavior —
   identified but not fetched.
2. Windsurf's entire model-selection and local-model story — unresearched.
3. Cursor's entire model-selection and local-model story — unresearched, dropped from this run
   despite being in the original question.
4. Aider's actual current mechanism for connecting to a local Ollama/LM Studio server, given
   that the natural guess (`--openai-api-base`) was refuted.

A fourth research pass, scoped to exactly these four items, would close out the tool-comparison
question completely.

---

## 7. Refuted ledger

| Claim | Vote |
|---|---|
| Aider supports a distinct "weak-model" role assigned separately for **both** commit messages and chat-history summarization as one unified role | **1-2** |
| Aider configures alternative/local providers via `--openai-api-base` / `AIDER_OPENAI_API_BASE` | **0-3** |
