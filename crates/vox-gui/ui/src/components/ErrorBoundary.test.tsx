// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import React from 'react';
import { ErrorBoundary } from './ErrorBoundary';

function Boom(): React.ReactElement {
  throw new Error('kaboom in shell');
}

describe('ErrorBoundary', () => {
  // React logs the caught error to console.error; silence it for a clean run.
  let spy: ReturnType<typeof vi.spyOn>;
  beforeEach(() => { spy = vi.spyOn(console, 'error').mockImplementation(() => {}); });
  afterEach(() => { spy.mockRestore(); });

  it('renders the recovery screen instead of white-screening when a child throws', () => {
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText('Display Runtime Error')).toBeTruthy();
    expect(screen.getByText('kaboom in shell')).toBeTruthy();
    expect(screen.getByRole('button', { name: /recover state/i })).toBeTruthy();
  });

  it('renders children unchanged when nothing throws', () => {
    render(
      <ErrorBoundary>
        <div data-testid="ok-body">fine</div>
      </ErrorBoundary>,
    );
    expect(screen.getByTestId('ok-body')).toBeTruthy();
  });
});

describe('main.tsx wiring (C3 regression: boundary existed but was imported nowhere)', () => {
  it('wraps the app tree in ErrorBoundary', () => {
    const main = readFileSync(resolve(__dirname, '../main.tsx'), 'utf8');
    expect(main).toContain("import { ErrorBoundary } from './components/ErrorBoundary'");
    const open = main.indexOf('<ErrorBoundary>');
    expect(open).toBeGreaterThan(-1);
    expect(open).toBeLessThan(main.indexOf('<App />'));
    expect(main).toContain('</ErrorBoundary>');
  });
});
