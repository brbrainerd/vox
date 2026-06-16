import { describe, it, expect } from 'vitest';
import { phaseStroke, phaseFill, terminalExitColor, viz } from './visualTokens';

describe('visualTokens', () => {
  it('phaseStroke maps known phases', () => {
    expect(phaseStroke('Validated')).toBe(viz.emerald400);
    expect(phaseStroke('Unknown')).toBe(viz.zinc500);
  });

  it('phaseFill scales alpha with confidence', () => {
    expect(phaseFill(viz.emerald400, 0)).toContain('rgba(52, 211, 153');
    expect(phaseFill(viz.emerald400, 1)).toContain('0.24');
  });

  it('terminalExitColor encodes success and failure', () => {
    expect(terminalExitColor(null)).toBe(viz.gray400);
    expect(terminalExitColor(0)).toBe(viz.emerald500);
    expect(terminalExitColor(1)).toBe(viz.red500);
  });
});
