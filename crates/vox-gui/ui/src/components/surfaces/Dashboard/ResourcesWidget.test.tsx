// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('../../../hooks/useMemoryStatus', () => ({
  useMemoryStatus: () => ({ vectorCount: 12400, loading: false, error: null }),
}));

import { ResourcesWidget } from './ResourcesWidget';
import type { DashboardData } from '../../../types/dashboard';

// Fully-typed (no `as unknown as` cast) so the compiler validates the widget's data dependency.
const data: DashboardData = {
  peers: [{ id: 'p1', name: 'a', backend: 'cuda', online: true }, { id: 'p2', name: 'b', backend: 'cuda', online: false }],
  kpis: { budgetBurn: { label: 'Budget', value: 4.2, cap: 20, spark: [] }, mesh: { label: 'Mesh', value: 1, cap: 0, spark: [] }, queueDepth: { value: 0, spark: [] } },
  agents: [], stream: [], alerts: [], contextChips: [], skills: [],
};

describe('ResourcesWidget', () => {
  it('renders compute mesh (online peers), vector store, and token budget from real data', () => {
    render(<ResourcesWidget data={data} />);
    expect(screen.getByText('Compute Mesh')).toBeInTheDocument();
    expect(screen.getByText('1 peer')).toBeInTheDocument();        // 1 online
    expect(screen.getByText('Vector Store')).toBeInTheDocument();
    expect(screen.getByText('12.4k')).toBeInTheDocument();          // 12400 compacted
    expect(screen.getByText('Token Budget')).toBeInTheDocument();
    expect(screen.getByText('$4.20 / $20')).toBeInTheDocument();
  });
});
