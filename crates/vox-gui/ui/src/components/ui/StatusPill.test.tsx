// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { StatusPill } from './StatusPill';

describe('StatusPill Component', () => {
  it('renders status indicators matching the status tone', () => {
    render(<StatusPill tone="pass" label="Done" data-testid="status-pill" />);
    const pill = screen.getByTestId('status-pill');
    expect(pill).toHaveClass('text-emerald-300');
  });

  it('renders status glyph default matching tone', () => {
    const { container } = render(<StatusPill tone="fail" />);
    expect(container.textContent).toContain('!');
  });
});
