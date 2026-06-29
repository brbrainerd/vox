// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

const mockBudget = vi.hoisted(() => ({
  max_context_tokens: 128_000,
  reserved_tokens: 10_000,
  threshold_tokens: 102_400,
  usable_tokens: 118_000,
  strategy: 'balanced',
  used_tokens: 0,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === 'get_context_budget') return Promise.resolve(mockBudget);
    return Promise.resolve(null);
  }),
}));

import { ChatExecutionRail } from './ChatExecutionRail';
import { LanguageProvider } from '../../../hooks/useLanguage';

const sampleKpis = {
  activeAgents: { value: 3 },
  queueDepth: { value: 7 },
  mesh: { peers: 2 },
};

describe('ChatExecutionRail', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('renders task list section with aria-label Active tasks', () => {
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[{ id: 't1', title: 'Fix CI', status: 'running' }]}
          kpis={sampleKpis}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );
    expect(screen.getByRole('region', { name: /active tasks/i })).toBeInTheDocument();
    expect(screen.getByText('Fix CI')).toBeInTheDocument();
    expect(screen.getByText(/running/i)).toBeInTheDocument();
  });

  it('shows resource strip with agents, queue depth, and mesh peers from props', () => {
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );
    expect(screen.getByTestId('execution-rail-agents')).toHaveTextContent('3');
    expect(screen.getByTestId('execution-rail-queue')).toHaveTextContent('7');
    expect(screen.getByTestId('execution-rail-mesh')).toHaveTextContent('2 peers');
  });

  it('shows OpenRouter cost segment when openrouterSpendUsd is provided', () => {
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          openrouterSpendUsd={1.25}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );
    const segment = screen.getByTestId('execution-rail-openrouter');
    expect(segment).toHaveTextContent(/openrouter/i);
    expect(segment).toHaveTextContent('$1.25');
  });

  it('hides OpenRouter segment when openrouterSpendUsd is omitted', () => {
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );
    expect(screen.queryByTestId('execution-rail-openrouter')).toBeNull();
  });

  it('shows current model label when activeModel is provided', () => {
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          activeModel="claude-sonnet-4"
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );
    const segment = screen.getByTestId('execution-rail-model');
    expect(segment).toHaveTextContent(/model/i);
    expect(segment).toHaveTextContent('claude-sonnet-4');
  });

  it('navigates when resource segments are clicked', async () => {
    const onNavigate = vi.fn();
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          onNavigate={onNavigate}
        />
      </LanguageProvider>,
    );

    await user.click(screen.getByTestId('execution-rail-agents'));
    expect(onNavigate).toHaveBeenCalledWith('agents');

    await user.click(screen.getByTestId('execution-rail-queue'));
    expect(onNavigate).toHaveBeenCalledWith('runs');

    await user.click(screen.getByTestId('execution-rail-mesh'));
    expect(onNavigate).toHaveBeenCalledWith('compute');
  });

  it('renders intent map section with up to three intent lines linking to matrix', async () => {
    const onNavigate = vi.fn();
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          intents={['claude-sonnet-4 · exploit', 'Alt: gpt-4o', 'Alt: gemini-pro', 'extra']}
          onNavigate={onNavigate}
        />
      </LanguageProvider>,
    );

    const region = screen.getByRole('region', { name: /intent map/i });
    expect(region).toBeInTheDocument();
    expect(screen.getByText('claude-sonnet-4 · exploit')).toBeInTheDocument();
    expect(screen.getByText('Alt: gpt-4o')).toBeInTheDocument();
    expect(screen.getByText('Alt: gemini-pro')).toBeInTheDocument();
    expect(screen.queryByText('extra')).toBeNull();

    await user.click(screen.getByRole('button', { name: /claude-sonnet-4 · exploit/i }));
    expect(onNavigate).toHaveBeenCalledWith('matrix');
  });

  it('omits intent map when intents prop is empty', () => {
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          intents={[]}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );
    expect(screen.queryByRole('region', { name: /intent map/i })).toBeNull();
  });

  it('can collapse and expand the rail panel', async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );

    expect(screen.getByRole('region', { name: /active tasks/i })).toBeInTheDocument();
    const collapse = screen.getByRole('button', { name: /collapse execution rail/i });
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    await user.click(collapse);
    expect(screen.queryByRole('region', { name: /active tasks/i })).toBeNull();

    const expand = screen.getByRole('button', { name: /expand execution rail/i });
    expect(expand.getAttribute('aria-expanded')).toBe('false');
    await user.click(expand);
    expect(screen.getByRole('region', { name: /active tasks/i })).toBeInTheDocument();
  });

  it('persists collapsed state in localStorage', async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={sampleKpis}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );

    await user.click(screen.getByRole('button', { name: /collapse execution rail/i }));
    expect(localStorage.getItem('gui.chat.execution_rail_collapsed.v1')).toBe('true');
  });

  it('renders ContextWindowMeter after budget loads', async () => {
    const defaultProps = {
      tasks: [],
      kpis: { activeAgents: { value: 0 }, queueDepth: { value: 0 }, mesh: { peers: 0 } },
      onNavigate: vi.fn(),
    };
    render(<LanguageProvider><ChatExecutionRail {...defaultProps} /></LanguageProvider>);
    await waitFor(() => {
      expect(screen.getByRole('meter')).toBeInTheDocument();
    });
  });

  it('passes used_tokens to ContextWindowMeter so it reflects real fill percentage', async () => {
    // Override the mock to return 25% usage (250 / 1000).
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_context_budget') {
        return Promise.resolve({
          max_context_tokens: 1000,
          reserved_tokens: 0,
          threshold_tokens: 800,
          usable_tokens: 1000,
          strategy: 'balanced',
          used_tokens: 250,
        });
      }
      return Promise.resolve(null);
    });

    render(
      <LanguageProvider>
        <ChatExecutionRail
          tasks={[]}
          kpis={{ activeAgents: { value: 0 }, queueDepth: { value: 0 }, mesh: { peers: 0 } }}
          onNavigate={vi.fn()}
        />
      </LanguageProvider>,
    );

    await waitFor(() => {
      const meter = screen.getByRole('meter');
      expect(meter).toBeInTheDocument();
      // aria-label should say 25% full
      expect(meter.getAttribute('aria-label')).toContain('25%');
      // aria-valuenow should be the real used_tokens value
      expect(meter.getAttribute('aria-valuenow')).toBe('250');
    });
  });
});

