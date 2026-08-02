// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const SESSIONS = [
  { id: 1, status: 'completed', query_text: 'What is Vox?', started_at_ms: 0, finished_at_ms: 1 },
];

const DETAIL_NO_CLAIMS = {
  session: SESSIONS[0],
  report_markdown: 'The sky is blue.',
  artifact_json: null,
};

const DETAIL_WITH_CLAIMS = {
  session: SESSIONS[0],
  report_markdown: 'The sky is blue.',
  artifact_json: null,
  confidence_tier: 'DeepResearch',
  source_count: 3,
  citation_precision: 1.0,
  claims: [
    {
      claim_id: 'c1',
      text: 'The sky is blue.',
      verdict: 'Supported',
      confidence: 0.9,
      resample_stability: 0.8,
      citation_urls: ['https://example.com/a'],
      corroboration_count: 1,
    },
  ],
};

let detailResponse: unknown = DETAIL_NO_CLAIMS;

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'list_research_sessions') return Promise.resolve(SESSIONS);
  if (cmd === 'get_research_session_detail') return Promise.resolve(detailResponse);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { LanguageProvider } from '../../../hooks/useLanguage';
import { ResearchView } from './ResearchView';

describe('ResearchView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
    detailResponse = DETAIL_NO_CLAIMS;
  });

  it('renders the Research heading', () => {
    render(<LanguageProvider><ResearchView pushToast={vi.fn()} /></LanguageProvider>);
    expect(screen.getByText('Research')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<LanguageProvider><ResearchView pushToast={vi.fn()} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('What is Vox?')).toBeTruthy());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('the research query input is labeled', () => {
    render(<LanguageProvider><ResearchView pushToast={vi.fn()} /></LanguageProvider>);
    expect(screen.getByLabelText('Research question')).toBeTruthy();
  });

  it('exposes the session history as role=list', async () => {
    render(<LanguageProvider><ResearchView pushToast={vi.fn()} /></LanguageProvider>);
    await waitFor(() => expect(screen.getAllByRole('list').length).toBeGreaterThan(0));
    expect(screen.getAllByRole('listitem').length).toBe(SESSIONS.length);
  });

  it('renders the raw report only when the detail has no claim/citation data (current backend shape)', async () => {
    render(<LanguageProvider><ResearchView pushToast={vi.fn()} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('What is Vox?')).toBeTruthy());
    screen.getByText('What is Vox?').closest('button')!.click();
    await waitFor(() => expect(screen.getByText('The sky is blue.')).toBeTruthy());
    expect(screen.queryByText(/High confidence/i)).toBeNull();
    expect(screen.queryByText(/claims verified/i)).toBeNull();
  });

  it('renders the headline banner and claim accordion when the detail carries claim/citation data', async () => {
    detailResponse = DETAIL_WITH_CLAIMS;
    render(<LanguageProvider><ResearchView pushToast={vi.fn()} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('What is Vox?')).toBeTruthy());
    screen.getByText('What is Vox?').closest('button')!.click();
    await waitFor(() => expect(screen.getByText(/High confidence/i)).toBeTruthy());
    expect(screen.getByText(/1 claim verified · 0 contested · 3 sources/i)).toBeTruthy();
    // existing raw-report render is still present, unchanged
    expect(screen.getByText('The sky is blue.')).toBeTruthy();
  });
});
