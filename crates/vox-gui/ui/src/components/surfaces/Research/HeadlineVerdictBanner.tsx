const BANNER_STYLE: Record<'contested' | 'high' | 'mixed', string> = {
  high: 'bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/30',
  mixed: 'bg-amber-500/15 text-amber-300 ring-1 ring-amber-500/30',
  contested: 'bg-red-500/15 text-red-300 ring-1 ring-red-500/30',
};

/**
 * One-line headline verdict summarizing a research run's overall trust
 * posture: high-confidence (no contested claims), mixed (some contested),
 * or contested (>30% of claims contested). Sits above the report body so
 * the reader gets a trust signal before diving into prose.
 */
export function HeadlineVerdictBanner({
  corroboratingSources,
  contestedClaims,
  totalClaims,
}: {
  confidenceTier: 'Direct' | 'Light' | 'DeepResearch';
  corroboratingSources: number;
  contestedClaims: number;
  totalClaims: number;
}) {
  const contestedRatio = totalClaims > 0 ? contestedClaims / totalClaims : 0;

  let tone: 'contested' | 'high' | 'mixed';
  let message: string;
  if (contestedRatio > 0.3) {
    tone = 'contested';
    message = `Contested — ${contestedClaims} of ${totalClaims} claims have conflicting evidence`;
  } else if (contestedClaims === 0) {
    tone = 'high';
    message = `High confidence — ${corroboratingSources} corroborating source${corroboratingSources === 1 ? '' : 's'}, no contested claims`;
  } else {
    tone = 'mixed';
    message = `Mixed evidence — ${contestedClaims} of ${totalClaims} claims contested, treat with care`;
  }

  return (
    <div
      className={`headline-verdict-banner mb-2 rounded-xl border border-border-subtle px-3 py-2 font-mono text-[11px] uppercase tracking-wide ${BANNER_STYLE[tone]}`}
    >
      {message}
    </div>
  );
}
