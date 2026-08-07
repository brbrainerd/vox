// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MessageBubble, ChatTranscript } from './ChatTranscript';
import type { ChatMessage } from '../../../lib/chatCorrelation';
import { listHarnessIssuesForSession } from '../Scientia/harnessIssuesApi';

vi.mock('../Scientia/harnessIssuesApi', () => ({
  listHarnessIssuesForSession: vi.fn(),
}));

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

describe('ChatTranscript harness issue summary strip', () => {
  beforeEach(() => {
    vi.mocked(listHarnessIssuesForSession).mockReset();
  });

  it('shows a detected issue fetched for the session, struck through when dismissed', async () => {
    vi.mocked(listHarnessIssuesForSession).mockResolvedValue([
      {
        id: 1,
        source: 'chat_session',
        session_key: 's1',
        target_path: null,
        detected_at_ms: 1_750_000_000_000,
        category: 'stub',
        severity: 'medium',
        summary: 'looks stubbed',
        evidence_json: '{}',
        status: 'dismissed',
      },
    ]);

    render(<ChatTranscript messages={[msg({})]} sessionId="s1" />);

    const row = await screen.findByTestId('transcript-harness-issue-1');
    expect(row).toHaveTextContent('Issue detected (dismissed): looks stubbed');
    expect(row.className).toContain('line-through');
    await waitFor(() => expect(listHarnessIssuesForSession).toHaveBeenCalledWith('s1'));
  });

  it('does not fetch or render anything when no sessionId is provided', () => {
    render(<ChatTranscript messages={[msg({})]} />);
    expect(listHarnessIssuesForSession).not.toHaveBeenCalled();
    expect(screen.queryByTestId(/transcript-harness-issue-/)).not.toBeInTheDocument();
  });
});
