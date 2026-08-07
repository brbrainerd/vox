// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { TaskPolicySection } from './TaskPolicySection';

describe('TaskPolicySection', () => {
  beforeEach(() => {
    cleanup();
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_task_policy_overrides') {
        return Promise.resolve({
          category: { CodeGen: { clutch: 'efficiency', risk: 'moderate' } },
          source: {},
        });
      }
      return Promise.resolve(undefined);
    });
  });

  it('renders existing overrides', async () => {
    render(<TaskPolicySection />);
    await waitFor(() => expect(screen.getByText(/CodeGen/i)).toBeInTheDocument());
    expect(screen.getByText(/efficiency/i)).toBeInTheDocument();
  });

  it('clears an override on remove click', async () => {
    render(<TaskPolicySection />);
    await waitFor(() => expect(screen.getByText(/CodeGen/i)).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('clear_task_policy_override', {
        scopeKind: 'category',
        scopeKey: 'CodeGen',
      })
    );
  });

  it('offers only not-yet-configured categories/sources in the add control, and adds one', async () => {
    render(<TaskPolicySection />);
    await waitFor(() => expect(screen.getByText(/CodeGen/i)).toBeInTheDocument());

    // CodeGen already has an override (from the mock) — it must not appear as
    // an addable option; Automated (a source) must, since none are configured.
    const addScopeSelect = screen.getByLabelText(/add override for/i);
    const optionLabels = Array.from(addScopeSelect.querySelectorAll('option')).map((o) => o.textContent);
    expect(optionLabels.some((l) => l?.includes('CodeGen'))).toBe(false);
    expect(optionLabels.some((l) => l?.includes('Automated'))).toBe(true);

    fireEvent.change(addScopeSelect, { target: { value: 'source:Automated' } });
    fireEvent.click(screen.getByRole('button', { name: /^add$/i }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_task_policy_override', {
        scopeKind: 'source',
        scopeKey: 'Automated',
        clutch: undefined,
        risk: undefined,
      })
    );
  });
});
