// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

// resolve_default_task_policy defaults to the same values as the local
// hardcoded defaultControl() fallback, so tests that don't care about this
// fetch keep seeing the composer settle on efficiency/moderate as before.
const { mockInvoke, defaultMockInvokeImpl } = vi.hoisted(() => {
  const defaultMockInvokeImpl = (cmd: string) => {
    if (cmd === 'resolve_default_task_policy') {
      return Promise.resolve({ clutch: 'efficiency', risk: 'moderate' });
    }
    return Promise.resolve([]);
  };
  return { mockInvoke: vi.fn(defaultMockInvokeImpl), defaultMockInvokeImpl };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));
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
  afterEach(() => {
    // Reset both call history and any per-test mockImplementation override
    // (e.g. from the resolve_default_task_policy test below) so a failing
    // assertion mid-test can't leak a stale implementation into later tests.
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(defaultMockInvokeImpl);
  });

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

  it('composer root has no p-4 inset (aligns flush with the chat transcript)', () => {
    renderLoquela();
    const root = screen.getByTestId('loquela-composer');
    expect(root.className).not.toContain('p-4');
  });

  it('Run button height matches the textarea min-height (h-9 vs min-h-[36px])', () => {
    renderLoquela();
    expect(screen.getByRole('button', { name: /run/i }).className).toContain('h-9');
  });

  it('secondary controls live in the toolbar row, not the input row', () => {
    renderLoquela();
    const ta = screen.getByLabelText('Task composer');
    const inputRow = ta.parentElement?.parentElement as HTMLElement;
    const attach = screen.getByRole('button', { name: /attach local file/i });
    expect(inputRow.contains(attach)).toBe(false);
    expect(inputRow.contains(screen.getByRole('button', { name: /voice input/i }))).toBe(false);
    expect(inputRow.contains(screen.getByRole('button', { name: /run/i }))).toBe(true);
  });

  it('intent panel is collapsed by default and toggles open', () => {
    renderLoquela();
    expect(screen.queryByLabelText('Goal')).toBeNull();
    const toggle = screen.getByRole('button', { name: /structured intent/i });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(toggle);
    expect(screen.getByLabelText('Goal')).toBeDefined();
  });

  it('serializes intent fields into the submitted description and priority', () => {
    const onSubmit = vi.fn();
    renderLoquela({ onSubmit });
    fireEvent.click(screen.getByRole('button', { name: /structured intent/i }));
    fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
    fireEvent.change(screen.getByLabelText('Acceptance criteria'), { target: { value: 'toggle persists' } });
    fireEvent.change(screen.getByLabelText('Effort'), { target: { value: 'urgent' } });
    const ta = screen.getByLabelText('Task composer');
    fireEvent.change(ta, { target: { value: 'add a theme switch' } });
    fireEvent.keyDown(ta, { key: 'Enter' });
    const payload = onSubmit.mock.calls[0][0];
    expect(payload.description).toContain('add a theme switch');
    expect(payload.description).toContain('## Goal\nship dark mode');
    expect(payload.description).toContain('## Acceptance criteria\ntoggle persists');
    expect(payload.priority).toBe('urgent');
  });

  it('goal alone is submittable without free text', () => {
    const onSubmit = vi.fn();
    renderLoquela({ onSubmit });
    fireEvent.click(screen.getByRole('button', { name: /structured intent/i }));
    fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
    fireEvent.click(screen.getByRole('button', { name: /run/i }));
    expect(onSubmit.mock.calls[0][0].description).toBe('ship dark mode');
  });

  it('collapses the intent panel after a structured submit', () => {
    const onSubmit = vi.fn();
    renderLoquela({ onSubmit });
    fireEvent.click(screen.getByRole('button', { name: /structured intent/i }));
    fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
    fireEvent.click(screen.getByRole('button', { name: /run/i }));
    expect(screen.queryByLabelText('Goal')).toBeNull();
    expect(screen.getByRole('button', { name: /structured intent/i }).getAttribute('aria-expanded')).toBe('false');
  });

  it('renders trailingSlot at the far right of the toolbar row, after mic/attach/link controls', () => {
    renderLoquela({ trailingSlot: <button type="button">Model: auto</button> });
    const slotButton = screen.getByRole('button', { name: /model: auto/i });
    const micButton = screen.getByRole('button', { name: /voice input/i });
    // Both live in the same toolbar row; the slot must come after the
    // left-side attach/mic controls in DOM order (source order + ml-auto is
    // what pushes it visually to the right).
    expect(
      micButton.compareDocumentPosition(slotButton) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('omits the trailing slot container entirely when trailingSlot is not provided', () => {
    renderLoquela();
    expect(screen.queryByRole('button', { name: /model:/i })).toBeNull();
  });

  it('renders a Resume button when the current agent is paused, calling onResume with the agent', () => {
    const onResume = vi.fn();
    renderLoquela({
      agentPaused: true,
      currentAgent: { id: 'a1' } as any,
      onResume,
    });
    const resumeBtn = screen.getByRole('button', { name: /resume/i });
    fireEvent.click(resumeBtn);
    expect(onResume).toHaveBeenCalledWith({ id: 'a1' });
  });

  it('the Run button carries its own keyboard-shortcut hint, with no other disconnected shortcut hint elsewhere', () => {
    renderLoquela();
    const runButton = screen.getByRole('button', { name: /run/i });
    expect(runButton).toHaveTextContent('⌘↵');
    // Reproduces a live bug: a bare "⌘↵" kbd hint used to render alone at
    // the end of the toolbar row, disconnected from any button — it must
    // not exist anywhere outside the Run button now.
    const allKbds = document.querySelectorAll('kbd');
    const kbdsOutsideRunButton = Array.from(allKbds).filter((k) => !runButton.contains(k));
    expect(kbdsOutsideRunButton.map((k) => k.textContent)).not.toContain('⌘↵');
  });

  it('seeds the DriveConsole default from resolve_default_task_policy instead of a hardcoded guess', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'resolve_default_task_policy') {
        return Promise.resolve({ clutch: 'free', risk: 'high' });
      }
      return Promise.resolve([]);
    });
    renderLoquela();
    await waitFor(() => {
      const freeButton = screen.getByRole('radio', { name: /free/i });
      expect(freeButton).toHaveAttribute('aria-checked', 'true');
    });
    // No manual restore needed here — the top-level afterEach resets
    // mockInvoke back to defaultMockInvokeImpl even if this test fails
    // before reaching this point.
  });

  it('does not clobber a user-made clutch pick if resolve_default_task_policy resolves afterward', async () => {
    let resolveDefault: (value: { clutch: string; risk: string }) => void = () => {};
    const deferred = new Promise<{ clutch: string; risk: string }>((resolve) => {
      resolveDefault = resolve;
    });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'resolve_default_task_policy') {
        return deferred;
      }
      return Promise.resolve([]);
    });
    renderLoquela();

    // User picks "Genius" before the backend's default has resolved.
    const geniusButton = screen.getByRole('radio', { name: /genius/i });
    fireEvent.click(geniusButton);
    expect(geniusButton).toHaveAttribute('aria-checked', 'true');

    // The slow backend fetch now resolves with a DIFFERENT value — it must
    // not silently revert the user's already-made choice.
    resolveDefault({ clutch: 'free', risk: 'high' });
    await waitFor(() => {
      const freeButton = screen.getByRole('radio', { name: /^free$/i });
      expect(freeButton).toHaveAttribute('aria-checked', 'false');
    });
    expect(geniusButton).toHaveAttribute('aria-checked', 'true');
  });

  it('keeps the hardcoded default when resolve_default_task_policy resolves to null (e.g. an unmocked IPC call)', async () => {
    // Regression test: a naive `.then((resolved) => setControl(resolved))`
    // would set `control` to `null`, crashing DriveConsole's render (it reads
    // `control.risk`) — this reproduces exactly that shape of response, as
    // seen from a harness whose default mock resolves unmocked commands to
    // `null` rather than rejecting.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'resolve_default_task_policy') {
        return Promise.resolve(null);
      }
      return Promise.resolve([]);
    });
    renderLoquela();
    await waitFor(() => {
      const efficButton = screen.getByRole('radio', { name: /effic/i });
      expect(efficButton).toHaveAttribute('aria-checked', 'true');
    });
  });
});
