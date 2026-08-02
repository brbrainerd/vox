---
title: "Free-Tier Onboarding: Budget Enforcement, OAuth Key Flow, Wizard GUI — Design"
description: "Design spec for sub-project A (expanded): user-configurable budget enforcement, an RFC-8252 loopback OAuth flow for zero-key onboarding, and the Vox Axis onboarding wizard that drives it, in dependency order."
category: "architecture"
status: "current"
training_eligible: false
---

# Free-Tier Onboarding: Budget Enforcement, OAuth Key Flow, Wizard GUI

> **Revision note (second audit round).** A follow-up adversarial pass (live web-verification of
> OAuth/axum claims, a code-audit re-checking every claim below against the current codebase, and a
> spec-completeness review) found several inaccuracies in the original draft, now corrected inline
> and cross-referenced from the implementation plan
> ([`2026-08-01-free-tier-onboarding.md`](../plans/2026-08-01-free-tier-onboarding.md)), which is the
> more current source of truth for exact code: (1) §1.3's `RouteError` type never existed in this
> codebase — the plan uses a purpose-built `BudgetGuardError` instead; (2) §2.2 step 6's
> `SecretKind::OAuthRefreshToken` isn't actually how `OPENROUTER_API_KEY` is stored — the real,
> already-wired path is `vox_secrets::set_registry_token`, which the plan uses; (3) §2.4's claim that
> `state` is "mandatory" is now known to be unverifiable against OpenRouter's actual documented OAuth
> contract, which doesn't mention `state` being echoed back — see the corrected §2.4 below; (4) §3.1's
> claim that `VersionMismatchBanner.tsx`/`BackendBanner.tsx` persist dismissal is wrong — both use
> plain in-memory `useState`, not `localStorage`; the plan uses the codebase's real
> `useLocalStorage` hook instead.

## 0. Overview

This spec covers sub-project **A** from the decomposition note
([`2026-08-01-free-tier-model-selection-decomposition.md`](2026-08-01-free-tier-model-selection-decomposition.md)),
expanded per user direction to include the full onboarding wizard GUI (originally sub-project C)
and full user-configurable budget enforcement (originally risk #4 in the research doc), rather than
shipping a minimal stub of either. Source audit and research:
[`free-tier-model-selection-and-onboarding-research-2026-08-01.md`](../../src/architecture/free-tier-model-selection-and-onboarding-research-2026-08-01.md).

**Goal**: a brand-new Vox user with zero API keys and no Ollama installed reaches a working chat
state without hitting a terminal error, with a real spend safety net in place before that flow
exists, and without recreating the same opaque failure a few hours later when a free tier's rate
limit is hit.

**Non-goals** (separate sub-projects, per the decomposition note):
- **B** — ingesting external model-rating registries (Epoch AI / OpenRouter `/benchmarks` /
  Artificial Analysis) as a cold-start scoring prior. This spec's wizard renders whatever
  `quality_score` value is available today (including a placeholder/default if none), but does not
  build the ingestion pipeline that would populate it meaningfully.
- **D** — wiring the hardware-fit badge (`vox-plugin-nvml-probe` → Fit/Won't-fit/Unknown badge).
  Independent, can land in parallel with this work by a separate session.
- Any changes to the core `SelectionAxes`/`decide()` scoring algorithm itself. This spec only adds
  new *inputs it can act on* (a budget guard, a newly-provisioned credential) — it does not touch
  `scoring.rs`'s ranking logic.
- Non-OpenRouter OAuth providers (e.g., a hypothetical Gemini OAuth flow). The wizard's "I already
  have a key" and "use a local model" paths cover the non-OpenRouter cases for v1; a second OAuth
  provider is future work if OpenRouter proves too central a dependency in practice (research doc
  risk #14).

## 1. Phase 1 — Budget enforcement

### 1.1 Problem

`daily_budget_usd` (default $5) and `per_session_budget_usd` (default $1) exist on `VoxConfig`
(`crates/vox-config/src/config/vox_config.rs:19-20,52-53`), are readable/writable from the GUI
(`crates/vox-gui/src/commands/user_config.rs`), and are labeled "soft cap on spend" in the key
manifest (`crates/vox-llm-config/src/keys.rs:138-139`) — but no code path reads them to block, warn,
or downgrade anything. This is a pre-existing gap, not introduced by this spec, but Phase 2/3 make a
first-time, likely inexperienced user more exposed to it (a "free" onboarding flow is exactly where
someone is least likely to have separately checked whether spend caps actually work).

### 1.2 Design

A new `budget_guard` module in `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/`
(sibling to the existing `resolve.rs`/`provider_auth.rs`) runs immediately before dispatch, after a
model has been selected but before the request is sent:

1. Read the effective caps from `VoxConfig` (existing accessor pattern, no new storage).
2. Read cumulative spend via the existing `VoxDb::llm_spend_summary(session_id)` aggregate
   (already built per prior cost-tracking-SSOT work — reuse, do not reimplement).
3. Compare against both `daily_budget_usd` and `per_session_budget_usd` independently (either cap
   can trip first).
4. At **80%** of either cap (the warn threshold — see below for configurability): return a
   `BudgetWarning` alongside the normal dispatch result; callers (CLI/GUI) surface it as a
   non-blocking notice.
5. At **100%** of either cap: return `Err(BudgetGuardError::Exceeded { scope: Daily | Session,
   cap_usd: f64, spent_usd: f64 })` (see §1.3's correction — this is a purpose-built error type, not
   a `RouteError` variant) and refuse dispatch.

**User-configurable warn threshold**: add `budget_warn_threshold_pct: f32` (default `0.8`) to
`VoxConfig`, surfaced in the same Settings section as the existing budget fields, registered through
`vox-llm-config`'s key registry (the existing SSOT — do not hand-roll a second config path). Users
who want hard-block-only behavior set it to `1.0` (warning and block become the same event); users
who want more headroom lower it.

**Scope decision**: this guard applies uniformly to *all* dispatch, not just OAuth-provisioned free
keys — a pre-existing paid-key user benefits from the same fix. This is intentional: building a
narrower guard that only applies to free-tier keys would be a second, inconsistent enforcement path
and contradicts the "single source of truth" pattern this codebase already uses elsewhere (research
doc §1.1's telemetry-first architecture, the `vox-llm-config` SSOT precedent).

**Interaction with the free OAuth-provisioned key (Phase 2)**: no special-casing. The same guard
applies. The wizard's last screen (Phase 3) shows the user their current caps and warn threshold as
part of setup, so a new user's first experience of the budget system is informed, not accidental.

### 1.3 API surface

- `vox_orchestrator_mcp::llm_bridge::model_route_policy::budget_guard::check(&LlmSpendSummary, f64,
  f64, f32) -> Result<Option<BudgetWarning>, BudgetGuardError>` — **not** a `RouteError` variant; a
  code-audit confirmed no `RouteError` type exists anywhere in this codebase (every fallible path in
  `model_route_policy/` returns plain `Result<_, String>`). `BudgetGuardError` is a small,
  purpose-built `thiserror` enum instead, converted to `String` only at the boundary where it needs
  to join the existing `Result<_, String>` convention — see the implementation plan's Task 3/4 for
  the exact shape.
- Callers distinguish `BudgetExceeded` from the existing `NoCredential`-shaped errors (§2.3 below
  formalizes the full error taxonomy) by checking for `BudgetGuardError`'s distinguishing message
  prefix, not by matching a shared discriminated-union type — there isn't one today, and inventing
  one purely for this would touch more of `model_route_policy` than this spec's scope justifies.
- GUI: extend `user_config.rs`'s existing get/set Tauri commands to include
  `budget_warn_threshold_pct`; no new commands needed.
- CLI: `vox doctor`'s LLM routing check gains a budget-status line (current spend vs. caps), reusing
  the existing check's output format.

## 2. Phase 2 — OAuth free-key flow

### 2.1 Problem

Dispatching any OpenRouter model — including `:free`-tier ones — requires `OPENROUTER_API_KEY` to
be set (`crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs:26-30`). A zero-key user hits a
terminal error today (`resolve.rs:392-397`). Getting a key today means leaving Vox, signing up on
OpenRouter's website, generating a key, and pasting it into `KeysSecretsSection` — five steps with
no guidance.

### 2.2 Design

Implement RFC 8252's desktop-preferred pattern: a loopback HTTP server, not a custom URI scheme
(research doc §3.3 — custom schemes are vulnerable to scheme-squatting on desktop; OpenRouter's own
docs explicitly support `localhost`/`127.0.0.1` callback URLs "for local CLI tools").

New crate `vox-oauth-pkce` (**not** a module inside `vox-secrets`, correcting the original draft:
`vox-secrets` is deliberately a low-layer crate with no HTTP client/server capability at all — no
`reqwest`, no `axum` — so an OAuth flow that needs to bind a listener and make outbound HTTP calls
cannot live there without breaking that crate's layering. The new crate depends on `axum`+`reqwest`
(already workspace dependencies) and the new `open` crate for launching the system browser; it does
**not** depend on `vox-secrets` itself — the caller (CLI/Tauri command) persists the returned key,
keeping the OAuth crate's responsibility limited to "get a token," not "know how Vox stores
secrets"):

1. Generate a PKCE `code_verifier` (random 43–128 char string) and `code_challenge` (S256 hash of
   the verifier), plus a random `state` value.
2. Bind a `127.0.0.1:0` TCP listener (OS-assigned free port) — never a fixed port, to avoid
   colliding with anything already listening.
3. Open the system default browser via the `open` crate (confirmed via web search: no
   webbrowser-launching crate already exists anywhere in this workspace's dependency tree, so this
   is a new, small dependency, not a reuse of an existing one as the original draft assumed) to
   `https://openrouter.ai/auth?callback_url=http://127.0.0.1:<port>&code_challenge=<challenge>&code_challenge_method=S256&state=<state>`.
4. The loopback listener accepts exactly one request, reads the `code` query param, and shuts down
   *gracefully* (via `axum`'s `with_graceful_shutdown`, triggered only after the response has been
   built — **not** a raw task `abort()`, a documented failure mode for this exact "one-shot loopback
   callback" pattern where an abort can race an in-flight response and leave the browser looking at
   a connection reset instead of a success page). On `state`: validate it **only when present** —
   web-verified against OpenRouter's live OAuth docs that they do not document echoing `state` back
   on the callback at all, so requiring it would make every real login fail a check the provider
   never promised to honor. This is an interim, explicitly-flagged position — the implementation
   plan requires empirically confirming the real callback's shape (via a manual smoke test) before
   this ships to real users, and tightening back to required-and-matching if OpenRouter's real
   behavior turns out to include it after all. The PKCE `code_verifier` check at exchange time
   remains the real security boundary regardless of how `state` resolves.
5. Exchange the code server-side: `POST https://openrouter.ai/api/v1/auth/keys` with `{code,
   code_verifier, code_challenge_method: "S256"}` → response contains the user-scoped API key
   (response field is `key`, web-verified against OpenRouter's own documented example).
6. Persist via the existing, already-wired secret path: `vox_secrets::set_registry_token("openrouter",
   &key, None)` — **not** `set_secret(..., SecretKind::OAuthRefreshToken)` as originally drafted.
   `set_registry_token` is exactly what the GUI's existing `set_secret` Tauri command already calls
   for `OPENROUTER_API_KEY` (its `SecretSpec` has `auth_registry: Some("openrouter")`), so this is
   the real "identical to a manually-entered key" path, not `SecretKind::OAuthRefreshToken` (which
   isn't actually wired to this secret at all, and — as an OpenRouter-issued permanent API key
   rather than a refresh token — was arguably a mismatched fit for that variant regardless).
7. Return success/failure to the caller (CLI or Tauri command) — the flow is synchronous from the
   caller's perspective (blocks on the loopback accept with a reasonable timeout, e.g. 120s, so a
   user who closes the browser tab doesn't hang the app indefinitely). On `open`-crate failure (no
   default browser, sandboxed/headless environment), the error carries the constructed auth URL
   alongside it, so the caller can offer a copyable/clickable fallback link instead of a dead end.

**Timeout / abandonment handling**: if no callback arrives within the timeout, the listener is
closed and a clear `OAuthTimedOut` error returned (distinct from `NoCredential` — the user started
but didn't finish, vs. never started). The wizard (Phase 3) surfaces this as "didn't complete —
try again" rather than the generic zero-key message.

**Accepted residual risk (recorded during implementation code review): local-process injection.**
Because `state` absence is accepted by design (§2.2 step 4 — OpenRouter doesn't document echoing it
back, so requiring it would break every real login), any other local process that discovers the
ephemeral loopback port within the callback timeout window (e.g. via `netstat`) could race a forged
`GET /callback?code=...` ahead of the real browser redirect and, via the single-shot take-once
channel, "win" — causing an attacker-controlled `code` to be exchanged instead of the legitimate
one. This is an inherent property of RFC 8252's loopback pattern (§8.4's local-injection threat
class), not something this implementation introduces or could close without contradicting
OpenRouter's own contract — PKCE's `code_verifier` binding protects against passive
interception/replay, not this active local race. Accepted as a conscious tradeoff, not an
unexamined gap, given the alternative (requiring `state`) breaks the flow outright per the
now-twice-verified absence of that parameter in OpenRouter's real callback contract.

**Entry points**:
- CLI: `vox secrets login --provider openrouter --oauth` (or folded into the existing `vox secrets
  login` command with a provider flag — follow whatever pattern `login_shared::run_login`
  already uses for its other providers, do not invent a second CLI shape).
- GUI: new Tauri command `oauth_login_openrouter() -> Result<SecretStatus, String>` in
  `crates/vox-gui/src/commands/`, called by the wizard (Phase 3).

### 2.3 Error taxonomy (formalizes what Phase 1 introduced)

Three distinct error classes replace today's single generic "no LLM model available" message,
surfaced with distinct copy in both CLI and GUI:

| Error | Trigger | User-facing action |
|---|---|---|
| `NoCredential` | No provider key configured, no local model available (today's only case) | Launch wizard / "get a free key" CTA |
| `RateLimited` | A configured key exists but the provider (OpenRouter) returned 429 — free tier's own 50/day cap | "Free tier limit reached, resets at \<time\> — add your own key for unlimited, or wait" |
| `BudgetExceeded` | Phase 1's guard tripped — Vox's own configured cap | "Daily/session budget of $X reached — raise it in Settings or wait for reset" |
| `OAuthTimedOut` | Phase 2's flow abandoned mid-flight | "Didn't complete — try again" (wizard only, not a dispatch-time error) |

`vox doctor`'s LLM routing check (`checks_standard/llm_routing.rs`) is updated to report which of
these states applies, instead of a single FAIL — including a genuine PASS state for "free tier
active, no issues."

### 2.4 Security notes (from research doc §3.3–3.4, corrected in the second audit round)

- PKCE (`code_challenge`/`code_verifier`) is mandatory regardless of transport — protects the code
  exchange even though the loopback approach avoids the scheme-squatting risk a custom URI scheme
  would carry. This remains the real security boundary.
- **`state` is checked when present, but is not required to be present** (corrected from the
  original draft's "mandatory" claim — see §2.2 step 4 for why: OpenRouter's documented OAuth
  contract doesn't mention echoing it back). This is a deliberate, interim, empirically-unverified
  position, not a security downgrade Vox chose casually — flagged as a hard gate item in the
  implementation plan to confirm and, if needed, revert before real users see this flow.
- The exchange (`code` → token) happens entirely in the Rust process; no intermediate JS/webview
  step handles the raw code.
- No abuse-prevention logic is added on Vox's side beyond the above — rate-limiting/anti-farming for
  the free tier itself is OpenRouter's responsibility at their authorize screen (research doc §3.4:
  OpenRouter states multi-accounting doesn't extend limits; no evidence of active exploitation).
  Vox does not add anything that would make scripted, headless invocation of this flow easier (e.g.,
  no non-interactive/automation-friendly variant of this command). This flow requires a real system
  browser and real human interaction with OpenRouter's own signup/bot-detection surface — Vox's
  contribution here is strictly less automatable than a script hitting OpenRouter directly would be,
  which is why risk #13 in the carried-forward register (§6) is downgraded rather than repeated
  unchanged.
- **New: post-storage verification is not the same as "the key was received."** A key string
  returned by the token exchange, and successfully written to the secret store, is not proof the key
  actually works (malformed, immediately-revoked, or provider-side-broken keys would all still
  "succeed" at this point). The wizard (§3.1) adds a real, minimal connectivity check between storing
  the key and showing the user a success screen — this was a genuine gap in the original draft,
  which let a stored-but-broken key present as fully working.

## 3. Phase 3 — Onboarding wizard GUI

### 3.1 Design

New top-level Axis surface: `crates/vox-gui/ui/src/components/surfaces/Onboarding/OnboardingWizard.tsx`
(plus per-screen sub-components in the same directory), registered in
`crates/vox-gui/ui/src/components/layout/Sidebar.tsx` only as an entry point for the *replay* path
(Settings → Onboarding) — on first launch it is not a sidebar item but an overlay/modal shown
automatically when the visibility gate is met, matching the already-designed-but-unbuilt §9.5
first-run-tour pattern in `vox-gui-design-review-2026.md`.

**Visibility gate** (risk #11 from the research doc): shown when *all* of —
- `list_secret_status` reports zero configured provider secrets, **and**
- no local model detected (reuse `BackendAvailability.tsx`'s existing local-model detection), **and**
- a new `onboarding_dismissed` flag, persisted via the codebase's real `useLocalStorage` hook
  (`crates/vox-gui/ui/src/hooks/useLocalStorage.ts`), is unset. **Corrected from the original
  draft**: `VersionMismatchBanner.tsx`/`BackendBanner.tsx` were assumed to persist their own dismiss
  state and be reusable for this — a code-audit found both actually use plain in-memory `useState`
  (dismissal resets on every reload), so there was nothing there to reuse. `useLocalStorage` is the
  codebase's real persisted-dismiss pattern (already used elsewhere, e.g. `KeysSecretsSection`'s
  `vox_secrets_groups` collapse state) and is what the implementation plan actually uses.

**Screen 1 — entry paths** (three, per user decision):
1. **"Get a free key"** → calls Phase 2's `oauth_login_openrouter()` Tauri command, shows progress
   (browser opened, waiting for callback, with the Phase 2 timeout surfaced as a retry option on
   `OAuthTimedOut`, and a copyable fallback link on browser-open failure per §2.2 step 7), then —
   before showing any success state — calls a new minimal `verify_openrouter_key()` check (§2.4) to
   confirm the stored key actually works, not just that a key string was received.
2. **"I already have an API key"** → deep-links directly into `SettingsView.tsx`'s existing
   `KeysSecretsSection` (the missing jump-link identified in the research doc's audit, §1.4) —
   no new key-entry UI, reuse what exists.
3. **"Use a local model"** → reuses `BackendAvailability.tsx`'s Ollama detection; if found, confirms
   and proceeds; if not found, links to Ollama's install instructions (external link, no in-app
   installer — out of scope).

**Screen 2 — confirmation**: shows the resolved provider/model state (e.g., "Connected via
OpenRouter free tier" or "Using local llama3.1:8b via Ollama"), and — per the OpenCode cautionary
tale in the research doc (§2.4) — is honest about what's actually driving model choice today: if
sub-project B hasn't landed yet, this screen says "Auto mode picks based on cost and your usage
history" rather than implying an intelligence ranking that doesn't exist yet.

**Screen 3 — budget setup**: shows Phase 1's `daily_budget_usd`/`per_session_budget_usd`/
`budget_warn_threshold_pct` with sensible defaults pre-filled, editable inline, and explicit copy
distinguishing Vox's own spend cap from the *provider's* free-tier rate limit (research doc §3.4 —
these are two different ceilings and conflating them was flagged as a risk). Completing this screen
sets the `onboarding_dismissed` flag.

**`ModelsView.tsx` changes** (small, bundled with this phase since they're the same surface):
add a free-tier filter toggle next to the existing green "free" badge (§1.4 audit finding — the
badge exists, the filter doesn't); render the existing `quality_score` field (§1.4 — currently
defined but dead) using whatever value the registry provides today, with the honesty caveat from
Screen 2 applied consistently (if the value is a placeholder, label it as such rather than
presenting it as a real ranking).

### 3.2 Data flow

```
User launches Vox (fresh install)
  → gate check (zero secrets + zero local models + flag unset) → wizard shown
  → Screen 1: user picks a path
      "Get a free key"   → Tauri oauth_login_openrouter() → Phase 2 flow → secret stored
                         → verify_openrouter_key() → confirmed working (or shown a retry, not success)
      "I have a key"     → deep-link to KeysSecretsSection → user pastes → secret stored
      "Use local model"  → Ollama detection → confirmed, no secret needed
  → Screen 2: confirmation, honest capability copy
  → Screen 3: budget setup → VoxConfig updated via existing user_config.rs commands
  → onboarding_dismissed = true
  → wizard closes → next dispatch passes Phase 1's budget guard and finds a credential
```

### 3.3 Testing

Playwright coverage for: the visibility-gate matrix (zero/non-zero secrets × zero/non-zero local
models × flag set/unset — 8 cases, only one shows the wizard), each of the three entry paths
(mocking the Tauri commands, not a live OpenRouter round-trip), the replay path from Settings, and
`ModelsView.tsx`'s new filter toggle. Reuse existing GUI test patterns/fixtures rather than building
a new harness.

## 4. Rollout

Ship behind a feature flag (repo's existing `build_flags.yaml`-style convention) covering Phases
1–3 together, since Phase 3 has no independent value without Phases 1–2 and Phase 1 alone is safe to
enable immediately (it only adds enforcement to an already-defined, already-user-visible config
surface). Recommended flag sequencing: enable Phase 1 by default first (low risk, closes a real
gap); gate Phases 2–3 behind a flag until the loopback OAuth flow has been manually verified on
Windows, macOS, and Linux (the platform-specific risk flagged in the research doc §7, open question
1) — do not assume RFC 8252's loopback pattern "just works" identically across all three without a
manual pass on each.

## 5. Success metrics

Per the research doc §6: wizard completion rate, time-to-first-successful-inference, per-step
drop-off, and the metric that most directly measures whether this spec did its job — % of new
installs that reach a working chat state without hitting a terminal error. Any of these that would
leave the device as telemetry must go through the existing opt-in path
(`telemetry-trust-ssot.md`/ADR-023) — this spec does not add a bespoke reporting call.

## 6. Risk carry-forward

All items in the research doc's 16-item risk register that touch recommendations A/C are addressed
by name above (redirect mechanism → §2.2/2.4, now including the corrected `state`-leniency and
graceful-shutdown handling; secret storage path → §2.2 step 6, now `set_registry_token`, not
`SecretKind::OAuthRefreshToken`; rate-limit vs. no-key messaging → §2.3, but see the implementation
plan's Task 12 vs. 12b split — the taxonomy exists on paper, wiring it into the *live* dispatch path
rather than only an on-demand `vox doctor` diagnostic is Task 12b specifically, not automatic; budget
caps → all of §1; wizard re-show/gate → §3.1, now `useLocalStorage`-backed; a11y/i18n → deferred,
English-only v1, explicitly noted here as a scope exclusion, not silently dropped; single point of
failure on OpenRouter → mitigated by Screen 1's three paths, not eliminated — still a named
dependency risk for future work). Risks tied to B (external registry) and D (hardware badge) are out
of scope for this spec by design (§0 non-goals).

**Updates from the second audit round:**
- **Risk #13 (mass account-creation vector) is downgraded**, not carried forward unchanged. As
  actually specified (§2.4), the flow requires a real system browser and real human interaction with
  OpenRouter's own signup/bot-detection surface, with no non-interactive variant — Vox's contribution
  here is strictly less automatable than a script hitting OpenRouter directly, not a shortcut around
  its defenses.
- **Risk #12's Clavis per-OS-profile-scoping sub-item was never actually answered**, only the
  a11y/i18n half of it was — a code-audit did not resolve whether Clavis secret storage is
  per-OS-user-profile scoped (relevant to the shared-machine scenario). This remains genuinely open;
  flagging it explicitly here rather than letting it silently disappear between the risk register and
  this carry-forward section, which is what happened in the original draft.
- **New risk surfaced in this round, not in the original 16**: a stored-but-non-working key
  presenting as a success state (addressed by the new `verify_openrouter_key` step, §2.4/§3.1) —
  worth folding into the research doc's risk register retroactively if that document is revised
  again, since it's the same class of problem as risk #10 (registry without a real quality signal
  degrading silently) — an unverified "it worked" claim degrading silently.
