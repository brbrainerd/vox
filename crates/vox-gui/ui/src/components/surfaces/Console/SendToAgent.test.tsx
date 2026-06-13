// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

import { SendToAgent } from './SendToAgent';

const agents = [
  { id: 'a1', name: 'sci-runner', state: 'running' },
  { id: 'a2', name: 'quantize-01', state: 'queued' },
];

describe('SendToAgent', () => {
  beforeEach(() => cleanup());

  it('submits the body + chosen agent to onSend', () => {
    const onSend = vi.fn();
    render(
      <SendToAgent initialBody="vox scientia review" agents={agents} onSend={onSend} onClose={vi.fn()} />,
    );
    fireEvent.click(screen.getByText('Send'));
    expect(onSend).toHaveBeenCalledWith('a1', 'vox scientia review');
  });

  it('disables Send when the body is empty', () => {
    render(<SendToAgent initialBody="" agents={agents} onSend={vi.fn()} onClose={vi.fn()} />);
    expect((screen.getByText('Send') as HTMLButtonElement).disabled).toBe(true);
  });

  it('calls onClose from Cancel', () => {
    const onClose = vi.fn();
    render(<SendToAgent agents={agents} onSend={vi.fn()} onClose={onClose} />);
    fireEvent.click(screen.getByText('Cancel'));
    expect(onClose).toHaveBeenCalled();
  });
});
