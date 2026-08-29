// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { GroundingCheckToggle } from './GroundingCheckToggle';

describe('GroundingCheckToggle', () => {
  it('renders off by default and reflects the hook-backed state', () => {
    render(<GroundingCheckToggle enabled={false} onToggle={vi.fn()} />);
    const btn = screen.getByRole('button', { name: /grounding check off/i });
    expect(btn).toHaveAttribute('aria-pressed', 'false');
    expect(btn).toHaveTextContent('grounding: off');
  });

  it('renders on when the persisted preference is true', () => {
    render(<GroundingCheckToggle enabled={true} onToggle={vi.fn()} />);
    const btn = screen.getByRole('button', { name: /grounding check on/i });
    expect(btn).toHaveAttribute('aria-pressed', 'true');
    expect(btn).toHaveTextContent('grounding: on');
  });

  it('calls onToggle with the flipped value on click', async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    render(<GroundingCheckToggle enabled={false} onToggle={onToggle} />);
    await user.click(screen.getByRole('button', { name: /grounding check off/i }));
    expect(onToggle).toHaveBeenCalledWith(true);
  });

  it('is a real <button type="button">, not a link or div', () => {
    render(<GroundingCheckToggle enabled={false} onToggle={vi.fn()} />);
    const btn = screen.getByRole('button', { name: /grounding check off/i });
    expect(btn.tagName).toBe('BUTTON');
    expect(btn).toHaveAttribute('type', 'button');
  });
});

// Wiring guard: the toggle renders inside the composer's own toolbar row
// (Loquela's `trailingSlot`), alongside ChatModelPicker — mirrors
// ChatModelPicker.test.tsx's "toolbar placement wiring" guard.
describe('GroundingCheckToggle toolbar placement wiring', () => {
  it('App.tsx passes GroundingCheckToggle as part of the Loquela trailingSlot', () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    const loquelaBlockMatch = appSrc.match(/const loquelaComposer = \(\s*<Loquela[\s\S]*?\/>\s*\);/);
    expect(loquelaBlockMatch).not.toBeNull();
    expect(loquelaBlockMatch?.[0]).toMatch(/<GroundingCheckToggle/);
  });
});

// Wiring guard (readFileSync idiom, mirroring ChatModelPicker.test.tsx's
// "model_override submit-payload wiring"): the toggle state must actually
// reach the params the daemon receives for SUBMIT_TASK as
// `grounding_check_enabled`, not just sit inert in the composer.
describe('grounding_check_enabled submit-payload wiring', () => {
  it('App.tsx threads the toggle state into the chat_turn input', () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    // The composer call site injects the persisted toggle state into the payload…
    expect(appSrc).toMatch(/grounding_check_enabled:\s*groundingCheckEnabled/);
    // …and handleLoquelaSubmit passes it to the single payload builder, which
    // maps it for BOTH executions (Task A3 — it used to reach only one branch).
    expect(appSrc).toMatch(/groundingCheckEnabled,/);
    const builderSrc = readFileSync(path.resolve(__dirname, '../../../lib/buildChatTurn.ts'), 'utf8');
    expect(builderSrc).toMatch(/grounding_check_enabled:\s*ctx\.groundingCheckEnabled\s*\?\?\s*null/);
  });

  it('the wired field name matches what the backend orchestrator parses off SUBMIT_TASK params', () => {
    const orchSrc = readFileSync(
      path.resolve(__dirname, '../../../../../../vox-orchestrator/src/orch_daemon/mod.rs'),
      'utf8',
    );
    expect(orchSrc).toMatch(/\.get\("grounding_check_enabled"\)/);
  });
});
