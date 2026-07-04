// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { IntentPanel } from './IntentPanel';
import { EMPTY_INTENT } from '../../../lib/intentSpec';

describe('IntentPanel', () => {
  it('exposes labelled fields for goal, constraints, acceptance and effort', () => {
    render(<IntentPanel intent={EMPTY_INTENT} onChange={() => {}} />);
    expect(screen.getByLabelText('Goal')).toBeDefined();
    expect(screen.getByLabelText('Constraints')).toBeDefined();
    expect(screen.getByLabelText('Acceptance criteria')).toBeDefined();
    expect(screen.getByLabelText('Effort')).toBeDefined();
  });
  it('reports field edits upward as partial patches', () => {
    const onChange = vi.fn();
    render(<IntentPanel intent={EMPTY_INTENT} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
    expect(onChange).toHaveBeenCalledWith({ goal: 'ship dark mode' });
    fireEvent.change(screen.getByLabelText('Constraints'), { target: { value: "don't touch auth" } });
    expect(onChange).toHaveBeenCalledWith({ constraints: "don't touch auth" });
    fireEvent.change(screen.getByLabelText('Acceptance criteria'), { target: { value: 'toggle persists' } });
    expect(onChange).toHaveBeenCalledWith({ acceptance: 'toggle persists' });
    fireEvent.change(screen.getByLabelText('Effort'), { target: { value: 'urgent' } });
    expect(onChange).toHaveBeenCalledWith({ effort: 'urgent' });
  });
});
