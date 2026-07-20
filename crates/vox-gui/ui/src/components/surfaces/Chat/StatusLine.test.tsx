// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { StatusLine } from './StatusLine';

describe('StatusLine', () => {
  it('renders phase and elapsed seconds as a single line', () => {
    render(<StatusLine phase="Verify" elapsedMs={12_340} />);
    expect(screen.getByText(/Verify/)).toBeInTheDocument();
    expect(screen.getByText(/12s/)).toBeInTheDocument();
  });

  it('rounds down to whole seconds', () => {
    render(<StatusLine phase="Act" elapsedMs={999} />);
    expect(screen.getByText(/0s/)).toBeInTheDocument();
  });
});
