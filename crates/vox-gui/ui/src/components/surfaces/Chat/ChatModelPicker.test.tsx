// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { ChatModelPicker } from './ChatModelPicker';

describe('ChatModelPicker', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_model_cards') {
        return [
          { id: 'openai/gpt-5.2-mini', provider: 'openai' },
          { id: 'anthropic/claude-opus-4.7', provider: 'anthropic' },
        ];
      }
      if (cmd === 'inference_provider_status') {
        return [
          { provider: 'OpenAI', key_present: true, is_local: false, local_reachable: null },
          { provider: 'Anthropic', key_present: true, is_local: false, local_reachable: null },
        ];
      }
      return null;
    });
  });

  it('loads the catalog on open and reports a pick via onApplied — never set_active_model', async () => {
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<ChatModelPicker activeModel="openai/gpt-5.2-mini" onApplied={onApplied} />);
    await user.click(screen.getByRole('button', { name: /model: openai\/gpt-5\.2-mini/i }));
    await user.click(await screen.findByRole('option', { name: 'anthropic/claude-opus-4.7' }));
    expect(onApplied).toHaveBeenCalledWith('anthropic/claude-opus-4.7');
    // Honest wiring: set_active_model only touches the GUI process and is never
    // read by the daemon serving chat — the pick must NOT ride it.
    expect(invoke).not.toHaveBeenCalledWith('set_active_model', expect.anything());
  });

  it('offers auto-route to clear the override', async () => {
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<ChatModelPicker activeModel="anthropic/claude-opus-4.7" onApplied={onApplied} />);
    await user.click(screen.getByRole('button', { name: /model: anthropic/i }));
    await user.click(await screen.findByRole('option', { name: /auto-route/i }));
    expect(onApplied).toHaveBeenCalledWith(null);
  });

  it('opens the listbox upward (bottom-full) so it clears the bottom-docked composer', async () => {
    const user = userEvent.setup();
    render(<ChatModelPicker activeModel={null} />);
    await user.click(screen.getByRole('button', { name: /model: auto-route/i }));
    const listbox = await screen.findByRole('listbox', { name: /pick model/i });
    expect(listbox.className).toContain('bottom-full');
    expect(listbox.className).not.toContain('mt-1');
  });

  it('closes the listbox on Escape', async () => {
    const user = userEvent.setup();
    render(<ChatModelPicker activeModel={null} />);
    await user.click(screen.getByRole('button', { name: /model: auto-route/i }));
    await screen.findByRole('listbox', { name: /pick model/i });
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox', { name: /pick model/i })).toBeNull();
  });

  it('closes the listbox on outside pointerdown', async () => {
    const user = userEvent.setup();
    render(<ChatModelPicker activeModel={null} />);
    await user.click(screen.getByRole('button', { name: /model: auto-route/i }));
    await screen.findByRole('listbox', { name: /pick model/i });
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole('listbox', { name: /pick model/i })).toBeNull();
  });

  it('disables a model whose provider has no key configured, and refuses the pick on click', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_model_cards') {
        return [
          { id: 'openai/gpt-5.2-mini', provider: 'openai' },
          { id: 'anthropic/claude-opus-4.7', provider: 'anthropic' },
        ];
      }
      if (cmd === 'inference_provider_status') {
        return [
          { provider: 'OpenAI', key_present: false, is_local: false, local_reachable: null },
          { provider: 'Anthropic', key_present: true, is_local: false, local_reachable: null },
        ];
      }
      return null;
    });
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<ChatModelPicker activeModel={null} onApplied={onApplied} />);
    await user.click(screen.getByRole('button', { name: /model: auto-route/i }));
    const unavailableOption = await screen.findByRole('option', { name: /openai\/gpt-5\.2-mini/i });
    expect(unavailableOption).toBeDisabled();
    await user.click(unavailableOption);
    expect(onApplied).not.toHaveBeenCalled();
  });

  it('disables a local provider model when the cached health probe reports unreachable', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_model_cards') return [{ id: 'ollama/llama3', provider: 'ollama' }];
      if (cmd === 'inference_provider_status') {
        return [{ provider: 'Ollama', key_present: true, is_local: true, local_reachable: false }];
      }
      return null;
    });
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<ChatModelPicker activeModel={null} onApplied={onApplied} />);
    await user.click(screen.getByRole('button', { name: /model: auto-route/i }));
    const option = await screen.findByRole('option', { name: /ollama\/llama3/i });
    expect(option).toBeDisabled();
    await user.click(option);
    expect(onApplied).not.toHaveBeenCalled();
  });
});

// Wiring guard: the picker renders inside the composer's own toolbar row
// (Loquela's `trailingSlot`), not as a separate row ChatSurface stacks below
// the composer — reported live as overlapping the execution-rail toggle
// before this move.
describe('ChatModelPicker toolbar placement wiring', () => {
  it('App.tsx passes ChatModelPicker as Loquela trailingSlot', () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    const loquelaBlockMatch = appSrc.match(/const loquelaComposer = \(\s*<Loquela[\s\S]*?\/>\s*\);/);
    expect(loquelaBlockMatch).not.toBeNull();
    expect(loquelaBlockMatch?.[0]).toMatch(/trailingSlot=\{[\s\S]*?<ChatModelPicker/);
  });
});

// Wiring guard (readFileSync idiom, mirroring Phase 1's ErrorBoundary.test.tsx):
// the picked model must reach the submit payload App sends to the daemon.
describe('model_override submit-payload wiring', () => {
  it('App.tsx threads the pick into the submit_orchestrator_task input', () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    // handleLoquelaSubmit maps the payload field into the daemon input…
    expect(appSrc).toMatch(/model_override:\s*payload\.model_override\s*\?\?\s*null/);
    // …and the composer call site injects the picker state into the payload.
    expect(appSrc).toMatch(/model_override:\s*chatModelOverride/);
  });
});

// Wiring guard: free-text chat submissions must be tagged `task_category:
// 'chat'` so the daemon routes them through the one-shot chat fast path
// instead of the 6-phase agentic pipeline — but NOT `/spawn`, which reuses
// this exact submit path to dispatch a real sub-agent.
//
// Historical note: this used to be derived in App.tsx from
// `payload.mode === 'act'`, which was silently always false in practice —
// Loquela's composer defaults its own internal `mode` state to "act" for
// EVERY normal submission (see Loquela.tsx's `useState("act")`), not just
// for /spawn, so real chat messages were never tagged 'chat' and always
// fell through to the full agentic pipeline. task_category is now set
// explicitly at each real call site instead, and App.tsx just forwards it.
//
// Fix Task 4 (gui-axis-chat-harness-fixes) made this a user-visible choice:
// Loquela's composer now exposes an explicit "Quick chat" / "Background
// task" toggle (`executionMode`), defaulting to 'chat', and only tags
// task_category: 'chat' when that mode is selected.
describe('task_category submit-payload wiring', () => {
  it('App.tsx forwards payload.task_category verbatim, not derived from mode', () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    expect(appSrc).toMatch(/task_category:\s*payload\.task_category\s*\?\?\s*undefined/);
    expect(appSrc).not.toMatch(/task_category:\s*payload\.mode/);
  });

  it("Loquela's composer send() tags 'chat' submissions via the executionMode toggle", () => {
    const loquelaSrc = readFileSync(
      path.resolve(__dirname, '../Loquela/Loquela.tsx'),
      'utf8',
    );
    expect(loquelaSrc).toMatch(/task_category:\s*executionMode === 'chat' \? 'chat' : undefined/);
  });

  it("/spawn's direct dispatch payload does not set task_category, leaving it agentic", () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    const spawnBlockMatch = appSrc.match(
      /base === '\/spawn'\) \{\s*void handleLoquelaSubmit\(\{[^}]*\}\);/,
    );
    expect(spawnBlockMatch).not.toBeNull();
    expect(spawnBlockMatch?.[0]).not.toMatch(/task_category/);
  });
});
