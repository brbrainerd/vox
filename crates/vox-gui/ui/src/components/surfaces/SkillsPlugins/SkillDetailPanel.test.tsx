// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { SkillDetailPanel } from './SkillDetailPanel';

describe('SkillDetailPanel', () => {
  it('renders skill info fields (no raw JSON)', () => {
    render(<SkillDetailPanel detail={{
      kind: 'skill-info',
      id: 'brainstorm', name: 'Brainstorm', version: '1.0.0',
      category: 'process', description: 'Generate ideas',
      tools: ['t1'], source: 'bundle', permissions: [], tags: ['ideation'],
    }} />);
    expect(screen.getByText('Brainstorm')).toBeInTheDocument();
    expect(screen.getByText('Generate ideas')).toBeInTheDocument();
    expect(screen.getByText('ideation')).toBeInTheDocument();
    expect(screen.queryByText(/^\{/)).not.toBeInTheDocument();
  });
  it('renders skill-use markdown body', () => {
    render(<SkillDetailPanel detail={{
      kind: 'skill-use', name: 'Brainstorm', description: 'd',
      body: '# Heading\nbody text',
    }} />);
    expect(screen.getByText('Brainstorm')).toBeInTheDocument();
    expect(screen.getByText(/body text/)).toBeInTheDocument();
  });
});
