// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MessageBubble } from './ChatTranscript';
import type { ChatMessage } from '../../../lib/chatCorrelation';

function msg(overrides: Partial<ChatMessage>): ChatMessage {
  return {
    id: 'm1',
    role: 'assistant',
    text: 'reply',
    status: 'done',
    runId: 'r1',
    ...overrides,
  };
}

describe('MessageBubble grounding-check badge', () => {
  it('shows a low-confidence badge on an assistant message flagged by the grounding check', () => {
    render(<MessageBubble message={msg({ groundingFlagged: true })} />);
    expect(screen.getByText(/low confidence/i)).toBeInTheDocument();
  });

  it('does not show the badge when the message was not flagged', () => {
    render(<MessageBubble message={msg({ groundingFlagged: false })} />);
    expect(screen.queryByText(/low confidence/i)).not.toBeInTheDocument();
  });

  it('does not show the badge on user messages even if somehow flagged', () => {
    render(<MessageBubble message={msg({ role: 'user', groundingFlagged: true })} />);
    expect(screen.queryByText(/low confidence/i)).not.toBeInTheDocument();
  });
});
