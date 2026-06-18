// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import React from 'react';

import { NoveltyEvidencePanel } from './NoveltyEvidencePanel';
import type { NoveltyAssessment } from './noveltyApi';

const notNovel: NoveltyAssessment = {
  verdict_kind: 'not_novel',
  closest_hit_uri: 'https://doi.org/10.1/closest',
  closest_score: 0.92,
  excluded_future_hits: 1,
  conflicts: [],
  signals: {
    max_semantic: 0.92,
    max_lexical: 0.4,
    near_hit_count: 3,
    top_hit_citations: 120,
    sources_succeeded: 2,
  },
  prior_art: [
    {
      work_uri: 'https://doi.org/10.1/closest',
      title: 'A very similar prior work',
      year: 2021,
      cited_by_count: 120,
      semantic_score: 0.92,
    },
  ],
  insufficient_evidence_reason: null,
};

const contradicted: NoveltyAssessment = {
  verdict_kind: 'contradicted',
  closest_hit_uri: 'https://openalex.org/W1000000007',
  closest_score: null,
  excluded_future_hits: 0,
  conflicts: [
    {
      claim_text: '',
      conflict_score: 0.5,
      supporting: [{ work_uri: 'https://openalex.org/W1000000006', excerpt: null }],
      contradicting: [
        {
          work_uri: 'https://openalex.org/W1000000007',
          excerpt: 'this work contradicts the proposed approach',
        },
      ],
    },
  ],
  signals: {
    max_semantic: 0.9,
    max_lexical: null,
    near_hit_count: 2,
    top_hit_citations: 200,
    sources_succeeded: 1,
  },
  prior_art: [
    {
      work_uri: 'https://openalex.org/W1000000006',
      title: 'Supporting prior art',
      year: 2022,
      cited_by_count: 200,
      semantic_score: 0.9,
    },
    {
      work_uri: 'https://openalex.org/W1000000007',
      title: 'Contradicting prior art',
      year: 2021,
      cited_by_count: 80,
      semantic_score: 0.9,
    },
  ],
  insufficient_evidence_reason: null,
};

const insufficient: NoveltyAssessment = {
  verdict_kind: 'insufficient_evidence',
  closest_hit_uri: null,
  closest_score: null,
  excluded_future_hits: 0,
  insufficient_evidence_reason: 'all prior-art sources failed or returned empty',
  conflicts: [],
  signals: {
    max_semantic: null,
    max_lexical: null,
    near_hit_count: 0,
    top_hit_citations: null,
    sources_succeeded: 0,
  },
  prior_art: [],
};

describe('NoveltyEvidencePanel', () => {
  beforeEach(() => cleanup());

  it('renders the verdict chip and closest prior art for a not_novel assessment', () => {
    render(<NoveltyEvidencePanel assessment={notNovel} />);
    expect(screen.getByText('Not novel')).toBeTruthy();
    expect(screen.getByText('A very similar prior work')).toBeTruthy();
  });

  it('renders the contradicted verdict chip and evidence conflicts section', () => {
    render(<NoveltyEvidencePanel assessment={contradicted} />);
    expect(screen.getByText('Contradicted')).toBeTruthy();
    expect(screen.getByText('Evidence conflicts')).toBeTruthy();
    expect(screen.getByText('Contradicting prior art')).toBeTruthy();
  });

  it('shows the retrieval-failure banner and assess_novelty reason for insufficient_evidence', () => {
    render(<NoveltyEvidencePanel assessment={insufficient} />);
    expect(screen.getByText('Insufficient evidence')).toBeTruthy();
    expect(
      screen.getByText(/Retrieval failed or never ran — do not treat as novel\./),
    ).toBeTruthy();
    expect(screen.getByText('all prior-art sources failed or returned empty')).toBeTruthy();
    expect(screen.getByText('No prior-art hits.')).toBeTruthy();
  });

  it('renders prior-art hits as a semantic list and hides the decorative warning glyph', () => {
    const { container, rerender } = render(<NoveltyEvidencePanel assessment={notNovel} />);
    expect(screen.getByRole('list')).toBeTruthy();
    rerender(<NoveltyEvidencePanel assessment={insufficient} />);
    expect(container.querySelector('[aria-hidden="true"]')?.textContent).toBe('⚠');
  });
});
