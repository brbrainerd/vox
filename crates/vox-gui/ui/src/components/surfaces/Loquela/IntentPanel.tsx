import React from 'react';
import type { Effort, IntentFields } from '../../../lib/intentSpec';

interface IntentPanelProps {
  intent: IntentFields;
  onChange: (patch: Partial<IntentFields>) => void;
}

const FIELD_CLS =
  'w-full rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1.5 text-[12px] text-text-primary placeholder:text-text-muted focus:outline-hidden focus:ring-1 focus:ring-brass/40';
const LABEL_CLS = 'font-display text-[9px] uppercase tracking-[0.22em] text-text-muted';

export function IntentPanel({ intent, onChange }: IntentPanelProps) {
  return (
    <div className="mt-2 grid grid-cols-1 gap-2 border-t border-border-subtle pt-2 sm:grid-cols-2" data-testid="intent-panel">
      <label className="flex flex-col gap-1 sm:col-span-2">
        <span className={LABEL_CLS}>Goal</span>
        <input aria-label="Goal" value={intent.goal} placeholder="What outcome should the agent achieve?"
          onChange={(e) => onChange({ goal: e.target.value })} className={FIELD_CLS} />
      </label>
      <label className="flex flex-col gap-1">
        <span className={LABEL_CLS}>Constraints</span>
        <textarea aria-label="Constraints" rows={2} value={intent.constraints} placeholder="Boundaries: don't touch X, keep API stable…"
          onChange={(e) => onChange({ constraints: e.target.value })} className={`${FIELD_CLS} resize-none`} />
      </label>
      <label className="flex flex-col gap-1">
        <span className={LABEL_CLS}>Acceptance criteria</span>
        <textarea aria-label="Acceptance criteria" rows={2} value={intent.acceptance} placeholder="How you'll judge the work is done"
          onChange={(e) => onChange({ acceptance: e.target.value })} className={`${FIELD_CLS} resize-none`} />
      </label>
      <label className="flex flex-col gap-1">
        <span className={LABEL_CLS}>Effort</span>
        <select aria-label="Effort" value={intent.effort}
          onChange={(e) => onChange({ effort: e.target.value as Effort })} className={FIELD_CLS}>
          <option value="">default</option>
          <option value="background">background — when idle</option>
          <option value="normal">normal</option>
          <option value="urgent">urgent — jump the queue</option>
        </select>
      </label>
    </div>
  );
}
