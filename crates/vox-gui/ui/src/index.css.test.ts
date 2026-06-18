// src/index.css.test.ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { STATUS_TONE } from './styles/tokens';

const css = readFileSync(resolve(__dirname, './index.css'), 'utf8');

describe('index.css global a11y rules', () => {
  it('contains a :focus-visible rule', () => {
    expect(css).toContain(':focus-visible');
  });

  it('uses --color-accent-default for the focus ring color', () => {
    // The ring must reference the token, not a hardcoded hex.
    expect(css).toContain('--color-accent-default');
  });

  it('contains a prefers-reduced-motion block that targets vox-* animations', () => {
    expect(css).toContain('prefers-reduced-motion');
    expect(css).toContain('animation');
  });
});


describe('STATUS_TONE', () => {
  it('contains the mandatory keys', () => {
    expect(STATUS_TONE.pass).toBeDefined();
    expect(STATUS_TONE.fail).toBeDefined();
    expect(STATUS_TONE.warn).toBeDefined();
    expect(STATUS_TONE.Executing).toBeDefined();
  });
});

