import { useCallback } from 'react';
import { useLocalStorage } from '../../../hooks/useLocalStorage';

export const ONBOARDING_DISMISSED_KEY = 'vox_onboarding_dismissed';

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
  const replay = useCallback(() => setDismissed(false), [setDismissed]);

  return {
    shouldShow,
    dismiss,
    replay,
  };
}
