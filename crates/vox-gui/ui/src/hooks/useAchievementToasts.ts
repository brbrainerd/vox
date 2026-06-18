import { useCallback, useState } from 'react';
import type { GuiEventResultDto } from '../lib/gamifyGuiEvents';

export interface AchievementToastItem {
  id: string;
  title: string;
  body?: string;
}

let toastSeq = 0;

function nextToastId(): string {
  toastSeq += 1;
  return `achievement-toast-${toastSeq}`;
}

function formatToastBody(result: GuiEventResultDto): string | undefined {
  const parts: string[] = [];
  if (result.xpGranted > 0) parts.push(`+${result.xpGranted} XP`);
  if (result.lumensGranted > 0) parts.push(`+${result.lumensGranted} lumens`);
  return parts.length > 0 ? parts.join(' · ') : undefined;
}

/** Ambient achievement / XP toast queue (suppressed in Serious mode). */
export function useAchievementToasts(gamifyEnabled: boolean, gamifyMode: string) {
  const [toasts, setToasts] = useState<AchievementToastItem[]>([]);

  const handleGuiEventResult = useCallback(
    (result: GuiEventResultDto | null | undefined) => {
      if (!gamifyEnabled || gamifyMode === 'serious' || result == null) return;
      const xp = result.xpGranted ?? 0;
      const lumens = result.lumensGranted ?? 0;
      if (xp === 0 && lumens === 0 && !result.achievementTitle) return;

      const title = result.achievementTitle ?? (xp > 0 ? 'XP' : 'Reward');
      const body = formatToastBody(result);

      setToasts((prev) => [
        ...prev,
        {
          id: nextToastId(),
          title,
          body,
        },
      ]);
    },
    [gamifyEnabled, gamifyMode],
  );

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, handleGuiEventResult, dismissToast };
}
