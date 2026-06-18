/**
 * Visualization colors for SVG / xterm / canvas contexts where Tailwind classes
 * do not apply. Values align with the default Tailwind palette and
 * `tokens/primitive.json` semantic ramps.
 */
export const viz = {
  zinc100: '#f4f4f5',
  gray400: '#9ca3af',
  zinc500: '#71717a',
  void: '#09090b',
  white: '#ffffff',
  emerald400: '#34d399',
  emerald500: '#22c55e',
  red500: '#ef4444',
  cyan400: '#22d3ee',
  amber400: '#fbbf24',
  violet400: '#a78bfa',
} as const;

export type PhaseKey = 'Validated' | 'Active' | 'Doubted' | 'Speculative' | 'Executing' | 'Planning' | 'Verifying' | 'Paused' | 'Root';

const PHASE_STROKE: Record<string, string> = {
  Validated: viz.emerald400,
  Active: viz.cyan400,
  Doubted: viz.amber400,
  Speculative: viz.violet400,
  Executing: 'rgb(var(--brass))',
  Planning: viz.cyan400,
  Verifying: viz.violet400,
  Paused: viz.zinc500,
  Root: viz.white,
};

export function phaseStroke(phase: string): string {
  return PHASE_STROKE[phase] ?? viz.zinc500;
}

export function phaseFill(stroke: string, conf: number): string {
  if (stroke.startsWith('rgb(var(--brass')) {
    return `rgba(212, 175, 55, ${0.06 + conf * 0.18})`;
  }
  const hex = stroke.replace('#', '');
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const a = 0.06 + conf * 0.18;
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

export function shardSparkColor(shard: { hot?: boolean; dirty?: boolean }): string {
  if (shard.hot) return 'rgb(var(--brass))';
  if (shard.dirty) return viz.amber400;
  return viz.zinc500;
}

export function terminalExitColor(exitCode: number | null): string {
  if (exitCode === null) return viz.gray400;
  if (exitCode === 0) return viz.emerald500;
  return viz.red500;
}
