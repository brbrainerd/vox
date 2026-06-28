import React from 'react';
import { Glass } from '../../ui/Glass';
import { useLlmSpend } from '../../../hooks/useLlmSpend';

export function CostWidget() {
  const { totalUsd } = useLlmSpend();
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">OpenRouter Spend</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-emerald-300">
        {totalUsd == null ? '—' : `$${totalUsd.toFixed(2)}`}
      </span>
      <span className="text-[10px] text-text-muted">{totalUsd == null ? 'awaiting cost daemon' : 'total this period'}</span>
    </Glass>
  );
}
