---
title: "Free-Tier Onboarding: Budget Enforcement, OAuth Key Flow, Wizard GUI — Design"
description: "Design spec for sub-project A (expanded): user-configurable budget enforcement, an RFC-8252 loopback OAuth flow for zero-key onboarding, and the Vox Axis onboarding wizard that drives it, in dependency order."
category: "architecture"
status: "current"
training_eligible: false
---

# Free-Tier Onboarding: Budget Enforcement, OAuth Key Flow, Wizard GUI

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
5. At **100%** of either cap: return `Err(RouteError::BudgetExceeded { scope: Daily | Session, cap:
   f64, spent: f64 })` and refuse dispatch.

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

- `vox_orchestrator_mcp::llm_bridge::model_route_policy::budget_guard::check(session_id, &VoxConfig,
  &VoxDb) -> Result<Option<BudgetWarning>, RouteError::BudgetExceeded>`
- New `RouteError` variant: `BudgetExceeded { scope: BudgetScope, cap_usd: f64, spent_usd: f64 }`,
  distinct from the existing `NoCredential`-shaped errors (§2.3 below formalizes the full error
  taxonomy).
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

New module `crates/vox-secrets/src/oauth/openrouter_pkce.rs` (co-located with the secret store it
writes into, since PKCE code-verifier state never needs to leave this crate):

1. Generate a PKCE `code_verifier` (random 43–128 char string) and `code_challenge` (S256 hash of
   the verifier), plus a random `state` value.
2. Bind a `127.0.0.1:0` TCP listener (OS-assigned free port) — never a fixed port, to avoid
   colliding with anything already listening.
3. Open the system default browser (existing pattern: check for a `webbrowser`-equivalent crate
   already in the dependency tree before adding a new one) to
   `https://openrouter.ai/auth?callback_url=http://127.0.0.1:<port>&code_challenge=<challenge>&code_challenge_method=S256`.
4. The loopback listener accepts exactly one request, validates the returned `state` matches, reads
   the `code` query param, and immediately shuts the listener down (no lingering open port).
5. Exchange the code server-side: `POST https://openrouter.ai/api/v1/auth/keys` with `{code,
   code_verifier, code_challenge_method: "S256"}` → response contains the user-scoped API key.
6. Persist via the existing secret store: `vox_secrets::set_secret(SecretId::OpenRouterApiKey,
   value, SecretKind::OAuthRefreshToken)` — the `OAuthRefreshToken` kind already exists in
   `crates/vox-secrets/src/spec/types.rs`, this flow is its first real consumer.
7. Return success/failure to the caller (CLI or Tauri command) — the flow is synchronous from the
   caller's perspective (blocks on the loopback accept with a reasonable timeout, e.g. 120s, so a
   user who closes the browser tab doesn't hang the app indefinitely).

**Timeout / abandonment handling**: if no callback arrives within the timeout, the listener is
closed and a clear `OAuthTimedOut` error returned (distinct from `NoCredential` — the user started
but didn't finish, vs. never started). The wizard (Phase 3) surfaces this as "didn't complete —
try again" rather than the generic zero-key message.

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

### 2.4 Security notes (from research doc §3.3–3.4)

- PKCE (`code_challenge`/`code_verifier`) is mandatory regardless of transport — protects the code
  exchange even though the loopback approach avoids the scheme-squatting risk a custom URI scheme
  would carry.
- `state` is mandatory and checked before accepting the callback — binds the callback to this
  specific flow instance, blocking CSRF-style account-mixup.
- The exchange (`code` → token) happens entirely in the Rust process; no intermediate JS/webview
  step handles the raw code.
- No abuse-prevention logic is added on Vox's side beyond the above — rate-limiting/anti-farming for
  the free tier itself is OpenRouter's responsibility at their authorize screen (research doc §3.4:
  OpenRouter states multi-accounting doesn't extend limits; no evidence of active exploitation).
  Vox does not add anything that would make scripted, headless invocation of this flow easier (e.g.,
  no non-interactive/automation-friendly variant of this command).

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
- a new `onboarding_dismissed` flag (persisted the same way `VersionMismatchBanner.tsx`/
  `BackendBanner.tsx` already persist their own dismiss state — reuse that mechanism, do not invent
  a third) is unset.

**Screen 1 — entry paths** (three, per user decision):
1. **"Get a free key"** → calls Phase 2's `oauth_login_openrouter()` Tauri command, shows progress
   (browser opened, waiting for callback, with the Phase 2 timeout surfaced as a retry option on
   `OAuthTimedOut`), then confirms success via `list_secret_status`.
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
by name above (redirect mechanism → §2.2/2.4; secret storage path → §2.2 step 6; rate-limit vs.
no-key messaging → §2.3; budget caps → all of §1; wizard re-show/gate → §3.1; a11y/i18n → deferred,
English-only v1, explicitly noted here as a scope exclusion, not silently dropped; single point of
failure on OpenRouter → mitigated by Screen 1's three paths, not eliminated — still a named
dependency risk for future work). Risks tied to B (external registry) and D (hardware badge) are out
of scope for this spec by design (§0 non-goals).
