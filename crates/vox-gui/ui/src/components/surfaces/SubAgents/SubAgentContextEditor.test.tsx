// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const setContextMock = vi.fn().mockResolvedValue(undefined);
const getContextMock = vi.fn();
vi.mock('./subAgentClient', () => ({
  getContext: (...a: unknown[]) => getContextMock(...a),
  setContext: (...a: unknown[]) => setContextMock(...a),
}));
import { SubAgentContextEditor } from './SubAgentContextEditor';

const items = [
  { itemId: 'i1', role: 'user', itemKind: 'message', preview: 'use RS256', byteLen: 40, tokenEstimate: 10, pinned: true, fate: 'included' as const },
  { itemId: 'i2', role: 'tool', itemKind: 'tool_call', preview: 'log dump', byteLen: 4000, tokenEstimate: 1000, pinned: false, fate: 'dropped' as const },
];

describe('SubAgentContextEditor', () => {
  beforeEach(() => { setContextMock.mockClear(); getContextMock.mockReset(); getContextMock.mockResolvedValue(items); });
  it('renders the committed items for the window', async () => {
    render(<SubAgentContextEditor windowId="w2" maxTokens={8000} />);
    await waitFor(() => expect(screen.getByText('use RS256')).toBeDefined());
  });
  it('removing an item calls setContext without that id', async () => {
    render(<SubAgentContextEditor windowId="w2" maxTokens={8000} />);
    await waitFor(() => expect(screen.getByText('log dump')).toBeDefined());
    fireEvent.click(screen.getByLabelText('remove i2'));
    await waitFor(() => expect(setContextMock).toHaveBeenCalledWith('w2', ['i1']));
  });
});
