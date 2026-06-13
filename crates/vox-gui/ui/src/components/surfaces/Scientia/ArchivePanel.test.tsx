// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

// Completion report before autofill: 40% complete, license_spdx missing, no
// provenance yet. After autofill it jumps to 80% with one autofill provenance
// entry. The mock dispatches per command name, and the completion report flips
// from BEFORE to AFTER once run_autofill has been called (the panel reloads).
const REPORT_BEFORE = {
  completeness_0_100: 40,
  required_missing: ['license_spdx'],
  inferred_ok: ['title'],
  human_only_pending: ['funding_statement'],
  field_provenance: [],
};
const REPORT_AFTER = {
  completeness_0_100: 80,
  required_missing: [],
  inferred_ok: ['title', 'license_spdx'],
  human_only_pending: ['funding_statement'],
  field_provenance: [{ field: 'license_spdx', origin: 'autofill:repo_license', notes: null }],
};
const ARCHIVE_STATUS = {
  swhid: null,
  swh_task_status: null,
  zenodo_doi: null,
  zenodo_state: null,
};

let autofilled = false;

const invokeMock = vi.fn((cmd: string, _args?: unknown) => {
  if (cmd === 'get_completion_report') {
    return Promise.resolve(autofilled ? REPORT_AFTER : REPORT_BEFORE);
  }
  if (cmd === 'get_archive_status') return Promise.resolve(ARCHIVE_STATUS);
  if (cmd === 'run_autofill') {
    autofilled = true;
    return Promise.resolve({
      fills: [
        { field: 'license_spdx', value: '"MIT"', origin: 'autofill:repo_license', notes: null },
      ],
      human_only_remaining: ['funding_statement'],
      completeness_before: 40,
      completeness_after: 80,
    });
  }
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { ArchivePanel } from './ArchivePanel';

describe('ArchivePanel', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
    autofilled = false;
  });

  async function loadPub() {
    render(<ArchivePanel pushToast={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText('publication id'), {
      target: { value: 'pub-1' },
    });
    fireEvent.click(screen.getByText('Load'));
  }

  it('renders the completeness meter (40) and the required-missing checklist item', async () => {
    await loadPub();
    const meter = await screen.findByLabelText('completeness percent');
    expect(meter.textContent).toContain('40');
    expect(screen.getByText('license_spdx')).toBeTruthy();
  });

  it('Auto-fill calls run_autofill and re-renders with the after-completeness + an autofill provenance chip', async () => {
    await loadPub();
    // Wait for the initial load (40%) so the Auto-fill button is present.
    await screen.findByLabelText('completeness percent');

    fireEvent.click(screen.getByText('Auto-fill'));

    await waitFor(() => {
      expect(invokeMock.mock.calls.some(([cmd]) => cmd === 'run_autofill')).toBe(true);
    });
    // Re-rendered with the raised completeness.
    await waitFor(() => {
      expect(screen.getByLabelText('completeness percent').textContent).toContain('80');
    });
    // A provenance chip with an "autofill:" origin now appears.
    expect(screen.getByText(/autofill:repo_license/)).toBeTruthy();
  });
});
