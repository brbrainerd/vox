// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { SubAgentActivityStream } from './SubAgentActivityStream';
import { useSubAgentStore } from './subAgentStore';

describe('SubAgentActivityStream', () => {
  beforeEach(() => useSubAgentStore.getState().reset());
  it('renders the selected window events, labelling retrieval pulls', () => {
    const s = useSubAgentStore.getState();
    s.pushEvent('w2', { id: 1, timestamp_ms: 1, kind: { type: 'task_started' } });
    s.pushEvent('w2', { id: 2, timestamp_ms: 2, kind: { type: 'context_pull', hash: 'abc', from_window: 'w1' } });
    render(<SubAgentActivityStream windowId="w2" />);
    expect(screen.getByText('task_started')).toBeDefined();
    expect(screen.getByText(/pulled abc from w1/i)).toBeDefined();
  });
});
