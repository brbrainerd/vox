// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

import { IsolationPanel } from './IsolationPanel';
import type { IsolationStatus } from './isolationHelpers';

const STATUS: IsolationStatus = {
  default_strategy: 'shared_branch',
  per_agent: {},
  conflicts: [],
} as unknown as IsolationStatus;

describe('IsolationPanel', () => {
  beforeEach(() => cleanup());

  it('labels the strategy select (no placeholder-as-label)', () => {
    render(<IsolationPanel status={STATUS} onSetDefault={vi.fn()} />);
    expect(screen.getByLabelText('Default strategy')).toBeTruthy();
  });

  it('confirms before changing the default isolation strategy (does not fire immediately)', () => {
    const onSetDefault = vi.fn();
    render(<IsolationPanel status={STATUS} onSetDefault={onSetDefault} />);
    fireEvent.change(screen.getByLabelText('Default strategy'), {
      target: { value: 'separate_branches' },
    });
    // Destructive: must not call through until confirmed.
    expect(onSetDefault).not.toHaveBeenCalled();
    // A confirm dialog appears.
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('fires onSetDefault only after the confirm action', () => {
    const onSetDefault = vi.fn();
    render(<IsolationPanel status={STATUS} onSetDefault={onSetDefault} />);
    fireEvent.change(screen.getByLabelText('Default strategy'), {
      target: { value: 'separate_branches' },
    });
    fireEvent.click(screen.getByRole('button', { name: /change strategy/i }));
    expect(onSetDefault).toHaveBeenCalledWith('separate_branches');
  });

  it('every button carries an explicit type="button"', () => {
    render(<IsolationPanel status={STATUS} onSetDefault={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('Default strategy'), {
      target: { value: 'separate_branches' },
    });
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });
});
