import { useCallback } from 'react';
import { useLocalStorage } from '../../../hooks/useLocalStorage';

export const ONBOARDING_DISMISSED_KEY = 'vox_onboarding_dismissed';

/** Dispatched on `window` by `replay()` so a currently-mounted `OnboardingWizard`
 * can force itself open immediately, in the same session — without this, a
 * user who already has a secret/local model configured (shouldShow's
 * fresh-install condition never holds for them) would click "Replay setup
 * wizard" in Settings and see nothing happen until they reloaded the page. */
export const ONBOARDING_REPLAY_EVENT = 'vox:onboarding-replay';

export interface OnboardingGateInput {
  secretCount: number;
  localModelCount: number;
}

export interface OnboardingGateResult {
  shouldShow: boolean;
  dismiss: () => void;
  replay: () => void;
}

/** Gate + persisted dismissal for the first-run onboarding wizard. */
export function useOnboardingGate({ secretCount, localModelCount }: OnboardingGateInput): OnboardingGateResult {
  const [dismissed, setDismissed] = useLocalStorage<boolean>(ONBOARDING_DISMISSED_KEY, false);

  const isFreshInstall = secretCount === 0 && localModelCount === 0;
  const shouldShow = isFreshInstall && !dismissed;

  const dismiss = useCallback(() => setDismissed(true), [setDismissed]);
  // Clearing `dismissed` is necessary but not sufficient to make `shouldShow`
  // true again (it also requires the fresh-install condition, which a
  // returning user with a key already configured will never satisfy) — so
  // also broadcast an explicit "force open" signal any mounted wizard can
  // react to immediately, same-session, without relying on a reload.
  const replay = useCallback(() => {
    setDismissed(false);
    if (typeof window !== 'undefined') {
      window.dispatchEvent(new CustomEvent(ONBOARDING_REPLAY_EVENT));
    }
  }, [setDismissed]);

  return {
    shouldShow,
    dismiss,
    replay,
  };
}
