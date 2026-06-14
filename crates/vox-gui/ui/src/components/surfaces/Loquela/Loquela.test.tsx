// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue([]) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../../../transport', () => ({
  voxTransport: { listModels: () => Promise.resolve([]) },
}));

import { Loquela } from './Loquela';

function renderLoquela(over: Partial<React.ComponentProps<typeof Loquela>> = {}) {
  return render(
    <Loquela
      chips={[]}
      setChips={() => {}}
      onSubmit={() => {}}
      activeSkill={null}
      setActiveSkill={() => {}}
      skills={[]}
      {...over}
    />,
  );
}

describe('Loquela', () => {
  it('labels the composer textarea (no placeholder-as-label)', () => {
    renderLoquela();
    expect(screen.getByLabelText('Task composer')).toBeDefined();
  });

  it('every button carries an explicit type="button"', () => {
    renderLoquela();
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('icon-only attach controls expose accessible names', () => {
    renderLoquela();
    expect(screen.getByRole('button', { name: /attach local file/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /attach a url/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /voice input/i })).toBeDefined();
  });

  it('tier and skill menus expose aria-expanded', () => {
    renderLoquela();
    expect(
      screen.getByRole('button', { name: /choose model tier/i }).getAttribute('aria-expanded'),
    ).toBe('false');
    expect(
      screen.getByRole('button', { name: /choose skill/i }).getAttribute('aria-expanded'),
    ).toBe('false');
  });
});
