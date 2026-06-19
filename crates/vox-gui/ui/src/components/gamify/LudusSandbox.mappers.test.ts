// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { moodFromPhase, integrityFromDiag } from './LudusSandbox.mappers';

describe('sandbox mappers', () => {
  it('maps agent phase to citizen mood', () => {
    expect(moodFromPhase('Executing')).toBe('Excited');
    expect(moodFromPhase('Paused')).toBe('Tired');
    expect(moodFromPhase('Doubted')).toBe('Sad');
    expect(moodFromPhase('Verifying')).toBe('Excited');
    expect(moodFromPhase('Planning')).toBe('Neutral');
    expect(moodFromPhase('Validated')).toBe('Happy');
  });

  it('maps diagnostics to building integrity', () => {
    expect(integrityFromDiag({ errors: 0, warnings: 0 })).toBe('intact');
    expect(integrityFromDiag({ errors: 3, warnings: 0 })).toBe('cracked');
    expect(integrityFromDiag({ errors: 0, warnings: 5 })).toBe('intact');
  });
});
