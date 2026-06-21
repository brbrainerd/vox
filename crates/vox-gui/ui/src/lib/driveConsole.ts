// Mirror of contracts/gui/drive-console.v1.yaml (kept in sync by the BE parity gate).
export type ClutchId = 'free' | 'efficiency' | 'balanced' | 'genius';
export type RiskId = 'high' | 'moderate' | 'low';

export const CLUTCH_DETENTS: { id: ClutchId; label: string; hint: string }[] = [
  { id: 'free',       label: 'Free',   hint: 'Free models only' },
  { id: 'efficiency', label: 'Effic.', hint: 'Most out of the tokens you spend; delegates to free agents on simple tasks' },
  { id: 'balanced',   label: 'Bal.',   hint: 'Balanced cost/quality' },
  { id: 'genius',     label: 'Genius', hint: 'Most intelligent solutions; budget relaxed' },
];

export const RISK_POSTURES: { id: RiskId; label: string; tone: 'rose' | 'amber' | 'emerald' }[] = [
  { id: 'high',     label: 'High',     tone: 'rose' },
  { id: 'moderate', label: 'Moderate', tone: 'amber' },
  { id: 'low',      label: 'Low',      tone: 'emerald' },
];

export interface ControlState {
  clutch: ClutchId;
  risk: RiskId;
  safetyTokenBudget?: number; // optional override surfaced in the risk popover
}

export function defaultControl(): ControlState {
  return { clutch: 'efficiency', risk: 'moderate' };
}
