// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';
import { LanguageProvider } from '../../../hooks/useLanguage';

const INTENTIONS = [
  { id: 'a', parent: 'Routing', branch: 'Cost', phase: 'Active', conf: 0.6, note: 'Favor cheap models' },
  { id: 'b', parent: 'Routing', branch: 'Quality', phase: 'Validated', conf: 0.8, note: 'Favor best models' },
];

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'get_routing_intentions') return Promise.resolve(INTENTIONS);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { Matrix } from './Matrix';

describe('Matrix', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Routing Policies heading after load', async () => {
    render(<LanguageProvider><Matrix pushToast={vi.fn()} /></LanguageProvider>);
    expect(await screen.findByText('Routing Policies')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<LanguageProvider><Matrix pushToast={vi.fn()} /></LanguageProvider>);
    await screen.findByText('Routing Policies');
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('hex cells expose aria-pressed and an aria-label', async () => {
    render(<LanguageProvider><Matrix pushToast={vi.fn()} /></LanguageProvider>);
    const cell = await screen.findByLabelText(/Cost routing axis/i);
    expect(cell.getAttribute('aria-pressed')).toBeDefined();
  });

  it('weight meter exposes role=progressbar', async () => {
    render(<LanguageProvider><Matrix pushToast={vi.fn()} /></LanguageProvider>);
    await waitFor(() => expect(screen.getAllByRole('progressbar').length).toBeGreaterThan(0));
  });

  it('shows an empty state when there are no intentions', async () => {
    invokeMock.mockImplementationOnce(() => Promise.resolve([]));
    render(<LanguageProvider><Matrix pushToast={vi.fn()} /></LanguageProvider>);
    expect(await screen.findByText(/No routing policies active/i)).toBeTruthy();
  });

  it('Active/Planning hex cell labels use brass text, never the old cyan', async () => {
    invokeMock.mockImplementationOnce(() => Promise.resolve([
      { id: 'a', parent: 'Routing', branch: 'Cost', phase: 'Active', conf: 0.6, note: 'x' },
      { id: 'p', parent: 'Routing', branch: 'Speed', phase: 'Planning', conf: 0.4, note: 'y' },
    ]));
    render(<LanguageProvider><Matrix pushToast={vi.fn()} /></LanguageProvider>);
    const activeCell = await screen.findByLabelText(/Cost routing axis/i);
    const planningCell = await screen.findByLabelText(/Speed routing axis/i);
    const activeLabel = activeCell.querySelector('.font-display.text-\\[13px\\]');
    const planningLabel = planningCell.querySelector('.font-display.text-\\[13px\\]');
    expect(activeLabel?.className).toContain('text-brass');
    expect(activeLabel?.className).not.toContain('text-cyan-300');
    expect(planningLabel?.className).toContain('text-brass');
    expect(planningLabel?.className).not.toContain('text-cyan-300');
  });
});
