import { describe, it, expect } from 'vitest';
import { createBlockReducer, decodeCommand, renderBlockForAgent, type Block } from './osc633';

describe('osc633 block reducer', () => {
  it('builds a completed block from A → E → C → D', () => {
    const r = createBlockReducer();
    r.onMarker('A', undefined, 5);
    r.onMarker('E', 'git status', 5);
    r.onMarker('C', undefined, 6);
    r.onMarker('D', '0', 9);
    const b = r.latestCompleted();
    expect(b).toBeTruthy();
    expect(b!.command).toBe('git status');
    expect(b!.exitCode).toBe(0);
    expect(b!.running).toBe(false);
    expect(b!.startLine).toBe(5);
    expect(b!.endLine).toBe(9);
    expect(r.blocks().length).toBe(1);
  });

  it('captures non-zero exit codes', () => {
    const r = createBlockReducer();
    r.onMarker('A', undefined, 0);
    r.onMarker('E', 'false', 0);
    r.onMarker('C', undefined, 0);
    r.onMarker('D', '1', 1);
    expect(r.latestCompleted()!.exitCode).toBe(1);
  });

  it('finalizes a prior open block with null exit when a new A arrives', () => {
    const r = createBlockReducer();
    r.onMarker('A', undefined, 0);
    r.onMarker('E', 'sleep 100', 0);
    r.onMarker('C', undefined, 0);
    // no D — user starts a new prompt; the prior block is finalized (null exit)
    // and a fresh open block begins, so blocks() now has both.
    r.onMarker('A', undefined, 3);
    expect(r.blocks().length).toBe(2);
    expect(r.blocks()[0].command).toBe('sleep 100');
    expect(r.blocks()[0].exitCode).toBeNull();
    // A null-exit finalize is not a "completed" block for send purposes.
    expect(r.latestCompleted()).toBeNull();
  });

  it('drops a D with no open block', () => {
    const r = createBlockReducer();
    r.onMarker('D', '0', 2);
    expect(r.blocks().length).toBe(0);
    expect(r.latestCompleted()).toBeNull();
  });

  it('ignores B (prompt-end) without throwing', () => {
    const r = createBlockReducer();
    r.onMarker('A', undefined, 0);
    r.onMarker('B', undefined, 0);
    r.onMarker('E', 'ls', 0);
    r.onMarker('C', undefined, 0);
    r.onMarker('D', '0', 0);
    expect(r.latestCompleted()!.command).toBe('ls');
  });

  it('renders a block for the agent composer (command + output + exit)', () => {
    const b: Block = {
      id: 1,
      command: 'git status',
      exitCode: 0,
      startLine: 0,
      endLine: 2,
      running: false,
      output: 'On branch main\nnothing to commit',
    };
    const text = renderBlockForAgent(b, 'typed line');
    expect(text).toBe('$ git status\nOn branch main\nnothing to commit\n(exit 0)');
  });

  it('falls back to the typed line when no block is present', () => {
    expect(renderBlockForAgent(null, 'vox scientia review')).toBe('vox scientia review');
  });

  it('renders unknown exit and omits empty output', () => {
    const b: Block = {
      id: 2,
      command: 'sleep 1',
      exitCode: null,
      startLine: 0,
      endLine: 0,
      running: false,
      output: '   ',
    };
    expect(renderBlockForAgent(b, 'x')).toBe('$ sleep 1\n(exit unknown)');
  });

  it('decodes OSC 633 percent/backslash-encoded command text', () => {
    // VS Code encodes \n, ; and \ in the E payload.
    expect(decodeCommand('echo\\x0ahi')).toBe('echo\nhi');
    expect(decodeCommand('a\\x3bb')).toBe('a;b');
    expect(decodeCommand('plain text')).toBe('plain text');
  });
});
