import { describe, it, expect } from 'vitest';
import { BASALT, TRAVERTINE, contrastRatio } from '../contrastTokens';

const PAIRS: Array<[string, 'text' | 'ui']> = [
  ['textPrimaryOnBase', 'text'], ['textSecondaryOnSurface', 'text'],
  ['accentOnBase', 'ui'], ['accentSecondaryOnSurface', 'ui'],
];

describe.each([['basalt', BASALT], ['travertine', TRAVERTINE]] as const)('%s contrast', (_name, scope) => {
  it.each(PAIRS)('%s meets AA', (key, kind) => {
    const ratio = contrastRatio(scope[key].fg, scope[key].bg);
    expect(ratio).toBeGreaterThanOrEqual(kind === 'text' ? 4.5 : 3.0);
  });
});
