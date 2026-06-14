// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

import { CoverageView } from './CoverageView';

describe('CoverageView', () => {
  it('renders the Surface Coverage heading', () => {
    render(<CoverageView pushToast={() => {}} />);
    expect(screen.getByText('Surface Coverage')).toBeTruthy();
  });

  it('gives the table an accessible name via caption', () => {
    render(<CoverageView pushToast={() => {}} />);
    expect(screen.getByRole('table', { name: /surface representation coverage/i })).toBeTruthy();
  });

  it('marks column headers with scope=col', () => {
    render(<CoverageView pushToast={() => {}} />);
    const headers = screen.getAllByRole('columnheader');
    expect(headers.length).toBe(3);
    for (const h of headers) expect(h.getAttribute('scope')).toBe('col');
  });
});
