import { describe, it, expect } from 'vitest';
import { CLUTCH_DETENTS, RISK_POSTURES, defaultControl, type ControlState } from './driveConsole';

describe('driveConsole contract', () => {
  it('exposes four clutch detents in order', () => {
    expect(CLUTCH_DETENTS.map(d => d.id)).toEqual(['free', 'efficiency', 'balanced', 'genius']);
  });
  it('exposes three risk postures', () => {
    expect(RISK_POSTURES.map(r => r.id)).toEqual(['high', 'moderate', 'low']);
  });
  it('defaults to efficiency + moderate', () => {
    const s: ControlState = defaultControl();
    expect(s.clutch).toBe('efficiency');
    expect(s.risk).toBe('moderate');
  });
});
