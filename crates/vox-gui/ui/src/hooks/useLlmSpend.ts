import { useEffect, useState } from 'react';
import { voxTransport, type LlmSpendDto } from '../transport';
import { sanitizeErrorForToast } from '../lib/backendGuard';

/** Contract default; overridden by the status bar's `spendPollSeconds` option. */
export const LLM_SPEND_POLL_MS = 60_000;

/**
 * Recorded LLM spend and the caps the budget guard enforces against it.
 *
 * The backend (`get_llm_spend`) has always returned the caps alongside the
 * spend; this hook used to project the payload down to `totalUsd` and drop the
 * rest, so the status bar showed *lifetime* spend against *no* cap while the
 * cap that actually blocks dispatch is the *daily* one. All five fields are
 * surfaced now, plus an explicit `error` so a failed fetch is distinguishable
 * from a fresh install with nothing recorded yet.
 */
export interface LlmSpendState {
  sessionUsd: number | null;
  dayUsd: number | null;
  totalUsd: number | null;
  dailyBudgetUsd: number | null;
  perSessionBudgetUsd: number | null;
  warnThresholdPct: number | null;
  error: string | null;
}

const EMPTY: LlmSpendState = {
  sessionUsd: null,
  dayUsd: null,
  totalUsd: null,
  dailyBudgetUsd: null,
  perSessionBudgetUsd: null,
  warnThresholdPct: null,
  error: null,
};

function num(v: unknown): number | null {
  return typeof v === 'number' && Number.isFinite(v) ? v : null;
}

export function useLlmSpend(pollMs: number = LLM_SPEND_POLL_MS): LlmSpendState {
  const [state, setState] = useState<LlmSpendState>(EMPTY);

  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const s: LlmSpendDto | null = await voxTransport.getLlmSpend();
        if (cancelled) return;
        if (s == null) {
          setState({ ...EMPTY, error: 'no spend data returned' });
          return;
        }
        setState({
          sessionUsd: num(s.sessionUsd),
          dayUsd: num(s.dayUsd),
          totalUsd: num(s.totalUsd),
          dailyBudgetUsd: num(s.dailyBudgetUsd),
          perSessionBudgetUsd: num(s.perSessionBudgetUsd),
          warnThresholdPct: num(s.warnThresholdPct),
          error: null,
        });
      } catch (e) {
        // Scrubbed via sanitizeErrorForToast rather than stringified raw: this
        // message reaches the status bar tile's tooltip, so it goes through the
        // same treatment as any other user-facing error text. Enforced by
        // guards/toastBodyGuard, which greps this directory for raw coercions.
        if (!cancelled) setState({ ...EMPTY, error: sanitizeErrorForToast(e) });
      }
    };

    refresh();
    const id = window.setInterval(refresh, pollMs);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [pollMs]);

  return state;
}
