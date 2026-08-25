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
    const rows = parseDiscoverResults(res);
    expect(rows).toHaveLength(1);
    expect(rows[0].id).toBe('n1');
  });

  it('maps surface: ids to a viewKey but keeps the real label', () => {
    const res = { result: { hits: [{ node_id: 'surface:voxgraph', label: 'VoxGraph' }] } };
    const row = parseDiscoverResults(res)[0];
    expect(row.viewKey).toBe('voxgraph');
    // `label ?? vk ?? id`: a supplied label outranks the derived view key.
    expect(row.label).toBe('VoxGraph');
  });

  it('returns [] on an error envelope even when a payload rides along', () => {
    // The payload matters: without it this case passes with the is_error guard
    // deleted, because the parser falls through to an empty array anyway.
    const res = { is_error: true, result: { hits: [{ node_id: 'x', label: 'X' }] } };
    expect(parseDiscoverResults(res)).toEqual([]);
  });
});
