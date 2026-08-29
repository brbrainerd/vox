import { describe, it, expect } from 'vitest';
import { buildChatTurn, CHAT_TURN_KEYS } from './buildChatTurn';

const full = {
  description: 'harden the crypto invariants',
  priority: 'urgent', active_skill: 'ponytail', tier: 'cloud',
  dry_run: false, clutch: 'genius', risk: 'low',
  context: [
    { kind: 'file', ref: 'crates/vox-crypto/src/lib.rs' },
    { kind: 'agent', ref: 'A-01' },
  ],
  execution_mode: 'chat' as const,
};

describe('buildChatTurn', () => {
  it('carries every composer control', () => {
    const out = buildChatTurn(full, {
      sessionId: 's1', modelOverride: 'openrouter/anthropic/claude-opus-5',
      groundingCheckEnabled: true,
    });
    expect(out.model_override).toBe('openrouter/anthropic/claude-opus-5');
    expect(out.tier).toBe('cloud');
    expect(out.clutch).toBe('genius');
    expect(out.risk).toBe('low');
    expect(out.priority).toBe('urgent');
    expect(out.grounding_check_enabled).toBe(true);
    // agent chips are not files
    expect(out.context_files).toEqual(['crates/vox-crypto/src/lib.rs']);
  });

  it('takes execution from the composer switch, not a legacy sentinel', () => {
    expect(buildChatTurn(full, { sessionId: 's1' }).execution).toBe('sync');
    expect(buildChatTurn({ ...full, execution_mode: 'task' }, { sessionId: 's1' }).execution)
      .toBe('background');
  });

  it('emits a stable key set', () => {
    const out = buildChatTurn(full, { sessionId: 's1' });
    expect(Object.keys(out).sort()).toEqual([...CHAT_TURN_KEYS].sort());
  });

  it('has no intent field', () => {
    // Loquela folds intent into `description` via composeDescription. A field
    // here would be permanently null, and double-counted if Loquela ever
    // emitted it without removing the fold.
    expect(CHAT_TURN_KEYS).not.toContain('intent');
  });
});
