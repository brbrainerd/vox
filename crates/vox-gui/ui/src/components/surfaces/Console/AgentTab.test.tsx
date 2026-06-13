// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

let emit:
  | ((e: { id: number; timestamp_ms: number; kind: { type: string; agent_id?: string } }) => void)
  | null = null;
vi.mock('../../../transport', () => ({
  listenAgentEvents: vi.fn().mockImplementation((cb: any) => {
    emit = cb;
    return Promise.resolve(() => {});
  }),
}));

import { AgentTab } from './AgentTab';

describe('AgentTab', () => {
  beforeEach(() => {
    cleanup();
    emit = null;
  });

  it('appends events matching its agent id', async () => {
    render(<AgentTab agentId="a1" />);
    await waitFor(() => expect(emit).toBeTruthy());
    emit!({ id: 1, timestamp_ms: 0, kind: { type: 'task_started', agent_id: 'a1' } });
    await waitFor(() => expect(screen.getByText(/task_started/)).toBeTruthy());
  });

  it('ignores events for other agents', async () => {
    render(<AgentTab agentId="a1" />);
    await waitFor(() => expect(emit).toBeTruthy());
    emit!({ id: 2, timestamp_ms: 0, kind: { type: 'task_started', agent_id: 'other' } });
    expect(screen.getByText('waiting for events…')).toBeTruthy();
  });
});
