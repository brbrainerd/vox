import React from 'react';

export interface KpiTileProps {
  label: string;
  value: string | number;
  /** Signed delta; positive renders verdigris ▲, negative renders fail ▼. */
  delta?: number;
}

/**
 * A single HUD metric tile: engraved label over a Cinzel tabular value with an
 * optional signed delta. Mirrors the TopHud KPI in the live app.
 */
export function KpiTile({ label, value, delta }: KpiTileProps) {
  const hasDelta = delta != null;
  const up = hasDelta && delta >= 0;
  return (
    <div className="ds-kpi">
      <span className="ds-kpi-label">{label}</span>
      <span className="ds-kpi-value">
        {value}
        {hasDelta && (
          <span className={up ? 'ds-kpi-delta-up' : 'ds-kpi-delta-down'} style={{ marginLeft: 6 }}>
            {up ? '▲' : '▼'} {Math.abs(delta)}
          </span>
        )}
      </span>
    </div>
  );
}
