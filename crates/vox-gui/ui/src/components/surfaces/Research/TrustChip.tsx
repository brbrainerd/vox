export type TrustSignal =
  | { kind: 'formal'; venueType: string; retracted: boolean }
  | { kind: 'corroborated'; sourceCount: number }
  | { kind: 'uncorroborated' };

const TRUST_STYLE: Record<string, string> = {
  formal: 'bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/30',
  retracted: 'bg-red-500/15 text-red-300 ring-1 ring-red-500/30',
  corroborated: 'bg-cyan/10 text-cyan ring-1 ring-cyan/30',
  uncorroborated: 'bg-overlay-subtle text-text-muted ring-1 ring-border-subtle/30',
};

function chip(cls: string, label: string) {
  return <span className={`rounded-sm px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider ${cls}`}>{label}</span>;
}

/**
 * 3-tier citation trust chip: formal (venue type, retraction check),
 * corroborated (N independent sources), or uncorroborated (single source).
 */
export function TrustChip({ signal }: { signal: TrustSignal }) {
  if (signal.kind === 'formal') {
    if (signal.retracted) {
      return chip(TRUST_STYLE.retracted, 'RETRACTED');
    }
    return chip(TRUST_STYLE.formal, `${signal.venueType} · not retracted`);
  }
  if (signal.kind === 'corroborated') {
    return chip(
      TRUST_STYLE.corroborated,
      `Confirmed by ${signal.sourceCount} independent source${signal.sourceCount === 1 ? '' : 's'}`
    );
  }
  return chip(TRUST_STYLE.uncorroborated, 'Single source — not independently corroborated');
}
