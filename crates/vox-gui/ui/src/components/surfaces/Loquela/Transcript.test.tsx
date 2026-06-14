// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Transcript } from './Transcript';
import type { ChatMessage } from '../../../lib/chatCorrelation';

const msg = (over: Partial<ChatMessage>): ChatMessage => ({
  id: 'm1',
  role: 'assistant',
  text: 'hello',
  status: 'done',
  runId: 'r1',
  ...over,
});

describe('Transcript', () => {
  it('renders nothing when there are no messages', () => {
    const { container } = render(<Transcript messages={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it('exposes the stream as a polite log region', () => {
    render(<Transcript messages={[msg({})]} />);
    const log = screen.getByRole('log');
    expect(log.getAttribute('aria-live')).toBe('polite');
  });
});
