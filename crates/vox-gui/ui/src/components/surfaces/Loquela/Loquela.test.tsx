// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
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

  it('shows a Stop button while a task is in progress', () => {
    renderLoquela({ taskInProgress: true, currentTaskId: 7 });
    expect(screen.getByRole('button', { name: /stop/i })).toBeDefined();
    expect(screen.queryByRole('button', { name: /run/i })).toBeNull();
  });

  it('Enter interrupts the running task instead of submitting', () => {
    const onSubmit = vi.fn();
    const onInterrupt = vi.fn();
    renderLoquela({ taskInProgress: true, currentTaskId: 42, onSubmit, onInterrupt });
    const ta = screen.getByLabelText('Task composer');
    fireEvent.change(ta, { target: { value: 'next idea' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(onInterrupt).toHaveBeenCalledWith(42);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('Stop button click interrupts with the current task id', () => {
    const onInterrupt = vi.fn();
    renderLoquela({ taskInProgress: true, currentTaskId: 99, onInterrupt });
    fireEvent.click(screen.getByRole('button', { name: /stop/i }));
    expect(onInterrupt).toHaveBeenCalledWith(99);
  });

  it('Enter submits normally when no task is running', () => {
    const onSubmit = vi.fn();
    renderLoquela({ onSubmit });
    const ta = screen.getByLabelText('Task composer');
    fireEvent.change(ta, { target: { value: 'do a thing' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    expect(onSubmit).toHaveBeenCalled();
  });
});
