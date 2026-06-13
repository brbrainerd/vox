import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock the Tauri core invoke bridge so the fetch can run outside Tauri.
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { fetchCostRollup, providerRows, quarterlyRows } from './costRollup';
import type { CostRollup } from './costRollup';

const SAMPLE: CostRollup = {
  this_quarter: {
    extraction_usd: 1.25,
    critic_usd: 0.0,
    novelty_retrieval_usd: 0.5,
    scholarly_submission_usd: 0.0,
    total_usd: 1.75,
  },
  per_finding_average_usd: 0.875,
  by_provider: [
    { provider: 'anthropic', usd: 1.0 },
    { provider: 'openai', usd: 0.75 },
  ],
};

describe('fetchCostRollup (A6)', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('invokes the execute_command bridge with the scientia cost path and parses stdout', async () => {
    invokeMock.mockResolvedValue({
      exit_code: 0,
      stdout: JSON.stringify(SAMPLE),
      stderr: '',
    });

    const rollup = await fetchCostRollup();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    const [cmd, payload] = invokeMock.mock.calls[0];
    expect(cmd).toBe('execute_command');
    expect(payload).toMatchObject({ path: ['scientia', 'cost'] });
    expect(rollup).toEqual(SAMPLE);
  });

  it('throws when the command exits non-zero', async () => {
    invokeMock.mockResolvedValue({ exit_code: 1, stdout: '', stderr: 'boom' });
    await expect(fetchCostRollup()).rejects.toThrow(/boom/);
  });
});

describe('cost view-model helpers (A6)', () => {
  it('maps per-provider totals into rendered rows', () => {
    const rows = providerRows(SAMPLE);
    expect(rows).toEqual([
      { provider: 'anthropic', usd: '$1.00' },
      { provider: 'openai', usd: '$0.75' },
    ]);
  });

  it('maps quarterly phase totals into labelled rows', () => {
    const rows = quarterlyRows(SAMPLE);
    expect(rows).toEqual([
      { label: 'Extraction', usd: '$1.25' },
      { label: 'Critic', usd: '$0.00' },
      { label: 'Novelty retrieval', usd: '$0.50' },
      { label: 'Scholarly submission', usd: '$0.00' },
      { label: 'Total', usd: '$1.75' },
    ]);
  });

  it('treats an all-zero / empty rollup as a zero state', () => {
    const empty: CostRollup = {
      this_quarter: {
        extraction_usd: 0,
        critic_usd: 0,
        novelty_retrieval_usd: 0,
        scholarly_submission_usd: 0,
        total_usd: 0,
      },
      per_finding_average_usd: 0,
      by_provider: [],
    };
    expect(providerRows(empty)).toEqual([]);
    expect(empty.this_quarter.total_usd).toBe(0);
  });
});
