import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';

const LLM_SPEND_POLL_MS = 60_000;

export function useLlmSpend(): { totalUsd: number | null } {
  const [totalUsd, setTotalUsd] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const spend = await voxTransport.getLlmSpend();
        if (!cancelled) setTotalUsd(spend.totalUsd);
      } catch {
        if (!cancelled) setTotalUsd(null);
      }
    };

    refresh();
    const id = window.setInterval(refresh, LLM_SPEND_POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  return { totalUsd };
}
