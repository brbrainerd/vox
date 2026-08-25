import { describe, expect, it } from 'vitest';
import { parseDiscoverResults } from './Omnibar';

describe('parseDiscoverResults', () => {
  it('reads the hits[] key that vox_search_structural actually returns', () => {
    // Shape copied verbatim from graph_tools.rs graphify_search payload.
    const res = {
      result: {
        corpus_id: 'repo-code-graph',
        searched_at: '2026-08-24T00:00:00Z',
        hits: [
          {
            node_id: 'crates_vox_gui_src_lib',
            label: 'vox_gui::lib',
            score: 0.9,
            knowledge_id: 'k1',
          },
        ],
      },
    };
    const rows = parseDiscoverResults(res);
    expect(rows).toHaveLength(1);
    expect(rows[0].id).toBe('crates_vox_gui_src_lib');
    // The tool supplies a human label; using the raw id would be a regression.
    expect(rows[0].label).toBe('vox_gui::lib');
  });

  it('still reads the legacy results[] key', () => {
    const res = { result: { results: [{ node_id: 'n1' }] } };
    expect(parseDiscoverResults(res)).toHaveLength(1);
  });

  it('maps surface: ids to a viewKey', () => {
    const res = { result: { hits: [{ node_id: 'surface:voxgraph', label: 'VoxGraph' }] } };
    expect(parseDiscoverResults(res)[0].viewKey).toBe('voxgraph');
  });

  it('returns [] on an error envelope', () => {
    expect(parseDiscoverResults({ is_error: true })).toEqual([]);
  });
});
