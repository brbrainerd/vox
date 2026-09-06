import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOnboardingGate, ONBOARDING_REPLAY_EVENT } from './useOnboardingGate';
import { KeysSecretsSection } from '../Settings/SettingsView';
import type { Toast } from '../../../types/tauri';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';

interface SecretStatusRow {
  id: string;
  isPresent: boolean;
}

interface ProviderStatusRow {
  provider: string;
  is_local: boolean;
  local_reachable: boolean | null;
}

type WizardScreen = 'entry' | 'oauth-in-progress' | 'verifying' | 'has-key' | 'local-model' | 'budget' | 'confirmation';

/** Single static id shared by every screen's heading so the outer dialog's
 * `aria-labelledby` always resolves to a visible, on-screen heading. */
const WIZARD_HEADING_ID = 'onboarding-wizard-heading';

export function OnboardingWizard({ pushToast, gamifyEnabled }: { pushToast: (t: Toast) => void; gamifyEnabled?: boolean }) {
  const [secretCount, setSecretCount] = useState<number | null>(null);
  const [localModelCount, setLocalModelCount] = useState<number | null>(null);
  const [screen, setScreen] = useState<WizardScreen>('entry');
  const [oauthError, setOauthError] = useState<string | null>(null);
  const [oauthFallbackUrl, setOauthFallbackUrl] = useState<string | null>(null);
  const [hasKeyWarning, setHasKeyWarning] = useState<string | null>(null);
  const [localModelWarning, setLocalModelWarning] = useState<string | null>(null);
  // Explicit "force open" signal from Settings' "Replay setup wizard" button
  // (see useOnboardingGate's `replay()`), separate from the automatic
  // fresh-install gate below — a user who already has a key/local model
  // configured would otherwise never see `gate.shouldShow` go true again.
  const [forceOpen, setForceOpen] = useState(false);

  useEffect(() => {
    // The wizard stays mounted for the app's whole lifetime, so `screen` (and
    // its transient per-screen state) would otherwise still be wherever the
    // user last left it — replaying from Settings must always start fresh at
    // 'entry', not reopen mid-flow or on 'confirmation'.
    const onReplay = () => {
      setScreen('entry');
      setOauthError(null);
      setOauthFallbackUrl(null);
      setHasKeyWarning(null);
      setLocalModelWarning(null);
      setForceOpen(true);
    };
    window.addEventListener(ONBOARDING_REPLAY_EVENT, onReplay);
    return () => window.removeEventListener(ONBOARDING_REPLAY_EVENT, onReplay);
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const secrets = await invoke<SecretStatusRow[]>('list_secret_status');
        setSecretCount(secrets.filter((s) => s.isPresent).length);
      } catch {
        setSecretCount(0);
      }
      try {
        const providers = await invoke<ProviderStatusRow[]>('inference_provider_status');
        setLocalModelCount(providers.filter((p) => p.is_local && p.local_reachable === true).length);
      } catch {
        setLocalModelCount(0);
      }
    })();
  }, []);

  const gate = useOnboardingGate({
    secretCount: secretCount ?? 0,
    localModelCount: localModelCount ?? 0,
  });
  // Visible either because the automatic fresh-install gate says so, or
  // because the user explicitly asked to replay it from Settings.
  const isVisible = gate.shouldShow || forceOpen;

  // Dismissing must also clear `forceOpen` — otherwise a replayed-then-dismissed
  // wizard would stay stuck considering itself "forced open" (never properly
  // closes, or reopens immediately on next mount).
  const handleDismiss = () => {
    gate.dismiss();
    setForceOpen(false);
    // Reset to 'entry' so a later replay (which stays mounted across the
    // app's lifetime) never reopens directly on whatever screen the user
    // dismissed from (e.g. 'confirmation' or mid-flow on 'has-key'/'local-model'/'budget').
    setScreen('entry');
    setOauthError(null);
    setOauthFallbackUrl(null);
    setHasKeyWarning(null);
    setLocalModelWarning(null);
  };

  // Escape dismisses the whole wizard, same action as "Skip for now" — mirrors
  // AchievementsDrawer.tsx's window-keydown pattern. Registered unconditionally
  // (hooks can't be conditional) but only wired up while the wizard is visible.
  useEffect(() => {
    if (!isVisible) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') handleDismiss();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isVisible]);

  if (secretCount === null || localModelCount === null || !isVisible) {
    return null;
  }

  // Re-check for an actually-configured key before letting `has-key` advance —
  // without this a user could click Done with nothing entered, sail through to
  // confirmation, dismiss the gate, and never see onboarding again despite
  // secretCount/localModelCount staying at zero.
  const handleHasKeyDone = async () => {
    setHasKeyWarning(null);
    try {
      const secrets = await invoke<SecretStatusRow[]>('list_secret_status');
      if (secrets.some((s) => s.isPresent)) {
        setScreen('budget');
        return;
      }
    } catch {
      // fall through to warning — same fail-closed treatment as the initial check
    }
    setHasKeyWarning('Add a key above before continuing, or go back and pick a different option — Skip for now is still always available.');
  };

  // Same idea for `local-model`: re-check reachability before advancing.
  const handleLocalModelDone = async () => {
    setLocalModelWarning(null);
    try {
      const providers = await invoke<ProviderStatusRow[]>('inference_provider_status');
      if (providers.some((p) => p.is_local && p.local_reachable === true)) {
        setScreen('budget');
        return;
      }
    } catch {
      // fall through to warning — same fail-closed treatment as the initial check
    }
    setLocalModelWarning('No local model detected yet. Install Ollama and pull a model above before continuing, or go back and pick a different option — Skip for now is still always available.');
  };

  const startOAuth = async () => {
    setScreen('oauth-in-progress');
    setOauthError(null);
    setOauthFallbackUrl(null);
    try {
      const result = await invoke<{ success: boolean; error: string | null; fallbackUrl: string | null }>('oauth_login_openrouter');
      if (!result.success) {
        setOauthError(result.error ?? 'Unknown error');
        setOauthFallbackUrl(result.fallbackUrl ?? null);
        setScreen('entry');
        return;
      }
      setScreen('verifying');
      const works = await invoke<boolean>('verify_openrouter_key').catch(() => false);
      if (works) {
        setScreen('budget');
      } else {
        setOauthError('Key saved, but a test request failed — check your connection and try again.');
        setOauthFallbackUrl(null);
        setScreen('entry');
      }
    } catch (err) {
      setOauthError(sanitizeErrorForToast(err));
      setOauthFallbackUrl(null);
      setScreen('entry');
    }
  };

  return (
    <div role="dialog" aria-modal="true" aria-labelledby={WIZARD_HEADING_ID} className="fixed inset-0 z-70 flex items-center justify-center bg-black/60">
      <div className="max-w-lg w-full rounded-xl border border-border-subtle bg-bg-base p-6 shadow-2xl">
        {screen === 'entry' && (
          <>
            <h1 id={WIZARD_HEADING_ID} className="font-display text-xl font-semibold text-text-primary">
              Get started with Vox
            </h1>
            <p className="mt-2 text-sm text-text-muted">
              Vox needs a model to talk to. Pick whichever fits you best — you can change this anytime in Settings.
            </p>
            {oauthError && (
              <div role="alert" className="mt-3 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
                {oauthError}
                {oauthFallbackUrl && (
                  <>
                    {' '}
                    <a href={oauthFallbackUrl} target="_blank" rel="noreferrer" className="underline">
                      Open this link manually
                    </a>
                    .
                  </>
                )}
              </div>
            )}
            <div className="mt-4 flex flex-col gap-2">
              <button type="button" onClick={startOAuth} className="rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90">
                Get a free key
              </button>
              <button type="button" onClick={() => setScreen('has-key')} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle">
                I already have an API key
              </button>
              <button type="button" onClick={() => setScreen('local-model')} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle">
                Use a local model
              </button>
            </div>
            <button type="button" onClick={handleDismiss} className="mt-4 text-[11px] text-text-muted hover:text-text-primary">
              Skip for now
            </button>
          </>
        )}
        {screen === 'oauth-in-progress' && (
          <>
            <h1 id={WIZARD_HEADING_ID} className="font-display text-xl font-semibold text-text-primary">Waiting for OpenRouter…</h1>
            <p className="mt-2 text-sm text-text-muted">
              A browser window opened — sign in or create a free OpenRouter account, then come back here.
            </p>
          </>
        )}
        {screen === 'verifying' && (
          <>
            <h1 id={WIZARD_HEADING_ID} className="font-display text-xl font-semibold text-text-primary">Checking your key…</h1>
            <p className="mt-2 text-sm text-text-muted">Confirming it actually works before we finish setup.</p>
          </>
        )}
        {screen === 'has-key' && (
          <>
            <h1 id={WIZARD_HEADING_ID} className="font-display text-xl font-semibold text-text-primary">Add your API key</h1>
            <div className="mt-4">
              <KeysSecretsSection pushToast={pushToast} gamifyEnabled={gamifyEnabled} />
            </div>
            {hasKeyWarning && (
              <div role="alert" className="mt-3 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
                {hasKeyWarning}
              </div>
            )}
            <div className="mt-4 flex gap-2">
              <button type="button" onClick={() => setScreen('entry')} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle">
                Back
              </button>
              <button type="button" onClick={handleHasKeyDone} className="rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90">
                Done
              </button>
            </div>
          </>
        )}
        {screen === 'local-model' && (
          <>
            <h1 id={WIZARD_HEADING_ID} className="font-display text-xl font-semibold text-text-primary">Use a local model</h1>
            <p className="mt-2 text-sm text-text-muted">
              Install{' '}
              <a href="https://ollama.com/download" target="_blank" rel="noreferrer" className="text-brass underline">
                Ollama
              </a>
              , pull a model, then come back — Vox will detect it automatically.
            </p>
            {localModelWarning && (
              <div role="alert" className="mt-3 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-[12px] text-red-300">
                {localModelWarning}
              </div>
            )}
            <div className="mt-4 flex gap-2">
              <button type="button" onClick={() => setScreen('entry')} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle">
                Back
              </button>
              <button type="button" onClick={handleLocalModelDone} className="rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90">
                Done
              </button>
            </div>
          </>
        )}
        {screen === 'budget' && (
          <BudgetSetupScreen headingId={WIZARD_HEADING_ID} pushToast={pushToast} onBack={() => setScreen('entry')} onContinue={() => setScreen('confirmation')} />
        )}
        {screen === 'confirmation' && (
          <>
            <h1 id={WIZARD_HEADING_ID} className="font-display text-xl font-semibold text-text-primary">You&apos;re set up</h1>
            <p className="mt-2 text-sm text-text-muted">
              Auto mode picks a model based on cost and your usage history as it builds up.
            </p>
            <button type="button" onClick={handleDismiss} className="mt-4 rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90">
              Start using Vox
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** Mirrors Rust `UserConfigFieldDto` (crates/vox-gui/src/commands/user_config.rs) —
 * same wire shape SettingsView's RuntimeConfigSection consumes. */
interface UserConfigFieldDto {
  key: string;
  label: string;
  hint: string;
  group: string;
  kind: string;
  options: string[];
  default: string;
  currentValue: string;
}

/** Screen 3: review/edit the budget caps set in Phase 1, before finishing onboarding.
 * Reuses the existing `get_user_config`/`set_user_config` commands — no new Tauri
 * commands needed for this screen. */
function BudgetSetupScreen({
  headingId,
  pushToast,
  onBack,
  onContinue,
}: {
  headingId: string;
  pushToast: (t: Toast) => void;
  onBack: () => void;
  onContinue: () => void;
}) {
  const [daily, setDaily] = useState('5');
  const [perSession, setPerSession] = useState('1');
  const [warnPct, setWarnPct] = useState('80');
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const fields = await invoke<UserConfigFieldDto[]>('get_user_config');
        const byKey = Object.fromEntries(fields.map((f) => [f.key, f.currentValue]));
        if (byKey.daily_budget_usd) setDaily(byKey.daily_budget_usd);
        if (byKey.per_session_budget_usd) setPerSession(byKey.per_session_budget_usd);
        if (byKey.budget_warn_threshold_pct) {
          const pct = Number(byKey.budget_warn_threshold_pct) * 100;
          setWarnPct(`${pct}`);
        }
      } finally {
        setLoaded(true);
      }
    })();
  }, []);

  const save = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await invoke('set_user_config', { key: 'daily_budget_usd', value: daily });
      await invoke('set_user_config', { key: 'per_session_budget_usd', value: perSession });
      await invoke('set_user_config', { key: 'budget_warn_threshold_pct', value: String(Number(warnPct) / 100) });
      onContinue();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Could not save spending limits', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <h1 id={headingId} className="font-display text-xl font-semibold text-text-primary">Set your spending limits</h1>
      <p className="mt-2 text-sm text-text-muted">
        This is Vox&apos;s own cap on spend — separate from any free-tier limit your provider applies. You&apos;ll get a warning before it blocks anything.
      </p>
      {loaded && (
        <div className="mt-4 space-y-3">
          <label className="block text-[11px] text-text-muted">
            Daily budget (USD)
            <input
              type="number"
              min="0"
              step="0.5"
              value={daily}
              onChange={(e) => setDaily(e.target.value)}
              className="mt-1 w-full rounded-lg border border-border-subtle bg-transparent px-2 py-1 text-sm text-text-primary"
            />
          </label>
          <label className="block text-[11px] text-text-muted">
            Per-session budget (USD)
            <input
              type="number"
              min="0"
              step="0.25"
              value={perSession}
              onChange={(e) => setPerSession(e.target.value)}
              className="mt-1 w-full rounded-lg border border-border-subtle bg-transparent px-2 py-1 text-sm text-text-primary"
            />
          </label>
          <label className="block text-[11px] text-text-muted">
            Warn me at (% of cap)
            <input
              type="number"
              min="0"
              max="100"
              step="5"
              value={warnPct}
              onChange={(e) => setWarnPct(e.target.value)}
              className="mt-1 w-full rounded-lg border border-border-subtle bg-transparent px-2 py-1 text-sm text-text-primary"
            />
          </label>
        </div>
      )}
      <div className="mt-4 flex gap-2">
        <button type="button" onClick={onBack} disabled={busy} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle disabled:opacity-40">
          Back
        </button>
        <button type="button" onClick={save} disabled={busy} className="rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90 disabled:opacity-40">
          {busy ? 'Saving…' : 'Save and continue'}
        </button>
      </div>
    </>
  );
}
