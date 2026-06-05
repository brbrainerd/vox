import { describe, it, expect } from 'vitest';
import { deriveStages, groupByStage, RESEARCH_STAGES, PUBLICATION_STAGES } from './pipeline';

describe('deriveStages', () => {
  it('marks every stage done when completed', () => {
    const s = deriveStages('completed');
    expect(s.planning).toBe('done');
    expect(s.completed).toBe('done');
  });
  it('marks every stage error on failure/orphan', () => {
    expect(deriveStages('failed').synthesizing).toBe('error');
    expect(deriveStages('orphaned').retrieving).toBe('error');
  });
  it('shows queued done and the rest pending while active', () => {
    const s = deriveStages('active');
    expect(s.queued).toBe('done');
    expect(s.completed).toBe('pending');
  });
});

describe('groupByStage', () => {
  it('buckets manifests by state and keeps empty stages', () => {
    const groups = groupByStage([
      { publication_id: 'a', content_type: 'paper', state: 'draft', created_at_ms: 1, updated_at_ms: 1 },
      { publication_id: 'b', content_type: 'paper', state: 'published', created_at_ms: 2, updated_at_ms: 2 },
    ]);
    expect(groups.draft.map(m => m.publication_id)).toEqual(['a']);
    expect(groups.published).toHaveLength(1);
    expect(groups.approved).toEqual([]);
  });
  it('exposes the canonical stage order', () => {
    expect(RESEARCH_STAGES[0]).toBe('queued');
    expect(PUBLICATION_STAGES).toContain('submitted');
  });
});
