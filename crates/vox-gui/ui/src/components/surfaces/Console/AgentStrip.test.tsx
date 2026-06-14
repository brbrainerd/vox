// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

import { AgentStrip } from './AgentStrip';

describe('AgentStrip', () => {
  beforeEach(() => cleanup());

  it('renders a chip per agent with its state', () => {
    render(
      <AgentStrip
        agents={[
          { id: 'a1', name: 'sci-runner', state: 'running' },
          { id: 'a2', name: 'quantize-01', state: 'queued' },
        ]}
        onOpen={vi.fn()}
      />,
    );
    expect(screen.getByText(/sci-runner/)).toBeTruthy();
    expect(screen.getByText(/quantize-01/)).toBeTruthy();
  });

  it('calls onOpen with the agent id when a chip is clicked', () => {
    const onOpen = vi.fn();
    render(
      <AgentStrip agents={[{ id: 'a1', name: 'sci-runner', state: 'running' }]} onOpen={onOpen} />,
    );
    fireEvent.click(screen.getByText(/sci-runner/));
    expect(onOpen).toHaveBeenCalledWith('a1');
  });

  it('gives each chip a descriptive aria-label and explicit button type', () => {
    render(
      <AgentStrip agents={[{ id: 'a1', name: 'sci-runner', state: 'running' }]} onOpen={vi.fn()} />,
    );
    const chip = screen.getByLabelText(/open agent sci-runner/i);
    expect(chip).toBeTruthy();
    expect(chip.getAttribute('type')).toBe('button');
  });
});
