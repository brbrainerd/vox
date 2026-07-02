import { describe, expect, it } from 'vitest';
import { toSliceRows } from './CodeRabbitView';

describe('toSliceRows', () => {
  it('maps manifest chunks to rows with planned status by default', () => {
    const manifest = {
      chunks: [
        { order: 1, name: 'crate_vox_db', files: ['a.rs', 'b.rs'] },
        { order: 2, name: '06_docs_src', files: ['x.md'] },
      ],
    };
    const rows = toSliceRows(manifest, null);
    expect(rows).toEqual([
      { name: 'crate_vox_db', files: 2, status: 'planned', pr: null },
      { name: '06_docs_src', files: 1, status: 'planned', pr: null },
    ]);
  });

  it('overlays run-state status and PR numbers by chunk name', () => {
    const manifest = { chunks: [{ order: 1, name: 'crate_vox_db', files: ['a.rs'] }] };
    const report = { run_state: { chunks: [{ name: 'crate_vox_db', pr_number: 42, status: 'completed' }] } };
    const rows = toSliceRows(manifest, report);
    expect(rows[0]).toEqual({ name: 'crate_vox_db', files: 1, status: 'completed', pr: 42 });
  });

  it('handles empty/missing manifest', () => {
    expect(toSliceRows(null, null)).toEqual([]);
    expect(toSliceRows({}, { run_state: null })).toEqual([]);
  });
});
