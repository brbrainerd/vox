import { describe, it, expect } from 'vitest';
import {
  ISOLATION_STRATEGIES,
  asStrategy,
  conflictRows,
  defaultStrategy,
  perAgentRows,
  strategyLabel,
  type IsolationStatus,
} from './isolationHelpers';

describe('strategyLabel', () => {
  it('maps known strategies to human labels', () => {
    expect(strategyLabel('shared_branch')).toBe('Shared Branch');
    expect(strategyLabel('split_changes')).toBe('Split Changes');
    expect(strategyLabel('separate_branches')).toBe('Separate Branches');
  });

  it('falls back to the raw id for unknown strategies', () => {
    expect(strategyLabel('mystery')).toBe('mystery');
  });
});

describe('asStrategy', () => {
  it('passes through the three known strategies', () => {
    for (const s of ISOLATION_STRATEGIES) {
      expect(asStrategy(s)).toBe(s);
    }
  });

  it('defaults unknown / undefined to shared_branch', () => {
    expect(asStrategy('nope')).toBe('shared_branch');
    expect(asStrategy(undefined)).toBe('shared_branch');
  });
});

describe('defaultStrategy', () => {
  it('reads strategy_default, defaulting to shared_branch', () => {
    expect(defaultStrategy({ strategy_default: 'separate_branches' })).toBe('separate_branches');
    expect(defaultStrategy(null)).toBe('shared_branch');
    expect(defaultStrategy({})).toBe('shared_branch');
  });
});

describe('perAgentRows', () => {
  it('returns an empty list when there are no overrides', () => {
    expect(perAgentRows({ per_agent: {} })).toEqual([]);
    expect(perAgentRows(null)).toEqual([]);
  });

  it('normalizes and sorts overrides by numeric agent id', () => {
    const status: IsolationStatus = {
      per_agent: { '10': 'split_changes', '2': 'separate_branches' },
    };
    expect(perAgentRows(status)).toEqual([
      { agentId: '2', strategy: 'separate_branches' },
      { agentId: '10', strategy: 'split_changes' },
    ]);
  });

  it('coerces unknown strategy strings to shared_branch', () => {
    expect(perAgentRows({ per_agent: { '1': 'bogus' } })).toEqual([
      { agentId: '1', strategy: 'shared_branch' },
    ]);
  });
});

describe('live daemon payload (isolation_status_json shape)', () => {
  // Mirrors what `get_vcs_isolation` returns over the Tauri bridge: the raw
  // `isolation_status_json` value (not a REST envelope).
  const live: IsolationStatus = {
    strategy_default: 'split_changes',
    per_agent: { '3': 'separate_branches', '1': 'shared_branch' },
    active_conflicts: [
      { id: 'X-9', path: 'crates/foo/bar.rs', sides: ['1', '3'], created_ms: 1717000000000 },
    ],
  };

  it('exposes the default strategy', () => {
    expect(defaultStrategy(live)).toBe('split_changes');
  });

  it('renders per-agent overrides sorted by numeric id', () => {
    expect(perAgentRows(live)).toEqual([
      { agentId: '1', strategy: 'shared_branch' },
      { agentId: '3', strategy: 'separate_branches' },
    ]);
  });

  it('surfaces active conflicts with their sides', () => {
    expect(conflictRows(live)).toEqual([
      { id: 'X-9', path: 'crates/foo/bar.rs', sides: ['1', '3'] },
    ]);
  });
});

describe('conflictRows', () => {
  it('returns an empty list when there are no active conflicts', () => {
    expect(conflictRows({ active_conflicts: [] })).toEqual([]);
    expect(conflictRows(null)).toEqual([]);
  });

  it('normalizes conflict rows and tolerates missing fields', () => {
    const status: IsolationStatus = {
      active_conflicts: [
        { id: 'C-1', path: 'src/a.rs', sides: ['1', '2'] },
        {},
      ],
    };
    expect(conflictRows(status)).toEqual([
      { id: 'C-1', path: 'src/a.rs', sides: ['1', '2'] },
      { id: 'conflict-1', path: '(unknown path)', sides: [] },
    ]);
  });
});
