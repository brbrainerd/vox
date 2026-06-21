// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const controlMock = vi.fn().mockResolvedValue(undefined);
vi.mock('./subAgentClient', () => ({ control: (...a: unknown[]) => controlMock(...a) }));
import { SubAgentControls } from './SubAgentControls';

describe('SubAgentControls', () => {
  beforeEach(() => controlMock.mockClear());
  it('pause dispatches a pause action for the window', async () => {
    render(<SubAgentControls windowId="w2" status="running" />);
    fireEvent.click(screen.getByLabelText('pause w2'));
    await waitFor(() => expect(controlMock).toHaveBeenCalledWith('w2', { kind: 'pause' }));
  });
  it('overrule sends the typed note', async () => {
    render(<SubAgentControls windowId="w2" status="running" />);
    fireEvent.change(screen.getByLabelText('overrule note'), { target: { value: 'stop, wrong file' } });
    fireEvent.click(screen.getByLabelText('overrule w2'));
    await waitFor(() => expect(controlMock).toHaveBeenCalledWith('w2', { kind: 'overrule', note: 'stop, wrong file' }));
  });
});
