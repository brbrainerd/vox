import { useLocalStorage } from '../../../hooks/useLocalStorage';

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
  const [dismissed, setDismissed] = useLocalStorage<boolean>('vox_onboarding_dismissed', false);

  const isFreshInstall = secretCount === 0 && localModelCount === 0;
  const shouldShow = isFreshInstall && !dismissed;

  return {
    shouldShow,
    dismiss: () => setDismissed(true),
    replay: () => setDismissed(false),
  };
}
