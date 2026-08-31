// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ModelBadge } from './ModelBadge';

describe('ModelBadge', () => {
  const attr = {
    model: 'claude-opus',
    reqTokens: 4200,
    respTokens: 1100,
    costUsd: 0.06,
    selectionReason: 'scored',
    latencyMs: 820,
  };

  it('shows model + tokens collapsed', () => {
    render(<ModelBadge {...attr} />);
    expect(screen.getByText(/claude-opus/)).toBeTruthy();
    expect(screen.queryByText(/scored/)).toBeNull(); // detail hidden by default
  });

  it('reveals detail on activate (keyboard reachable)', () => {
    render(<ModelBadge {...attr} />);
    fireEvent.click(screen.getByRole('button', { name: /claude-opus/i }));
    expect(screen.getByText(/scored/)).toBeTruthy();
    expect(screen.getByText(/820/)).toBeTruthy();
  });

  it('renders unknown when no model', () => {
    render(<ModelBadge model={undefined} />);
    expect(screen.getByText(/model unknown/i)).toBeTruthy();
  });

  // Phase B / Task B2: honest selection labels — classified from the resolved
  // model, not the request, per docs/superpowers/plans/2026-08-28-chat-harness-unification.md.
  describe('selection source labels', () => {
    it('shows "Your pick" for user_override', () => {
      render(
        <ModelBadge
          model="claude-opus"
          selection={{ model: 'claude-opus', source: 'user_override', rationale: null }}
        />
      );
      fireEvent.click(screen.getByRole('button', { name: /claude-opus/i }));
      expect(screen.getByText(/Your pick/i)).toBeTruthy();
    });

    it('shows "Auto-routed" for auto_routed', () => {
      render(
        <ModelBadge
          model="claude-opus"
          selection={{ model: 'claude-opus', source: 'auto_routed', rationale: null }}
        />
      );
      fireEvent.click(screen.getByRole('button', { name: /claude-opus/i }));
      expect(screen.getByText(/Auto-routed/i)).toBeTruthy();
    });

    it('shows "Fell back" plus the rationale for fallback', () => {
      render(
        <ModelBadge
          model="free-x"
          selection={{
            model: 'free-x',
            source: 'fallback',
            rationale: "requested `gone` is not in the registry",
          }}
        />
      );
      fireEvent.click(screen.getByRole('button', { name: /free-x/i }));
      expect(screen.getByText(/Fell back/i)).toBeTruthy();
      expect(screen.getByText(/not in the registry/i)).toBeTruthy();
    });
  });
});
