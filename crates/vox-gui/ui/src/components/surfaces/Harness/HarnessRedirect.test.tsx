// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

import { HarnessRedirect } from './HarnessRedirect';

describe('HarnessRedirect', () => {
  beforeEach(() => cleanup());

  it('renders title containing composer or Loquela', () => {
    render(<HarnessRedirect onFocusComposer={vi.fn()} />);
    const title = screen.getByText('Quick Harness lives in the composer');
    expect(title.textContent?.toLowerCase()).toMatch(/composer|loquela/);
  });

  it('calls onFocusComposer when action button is clicked', () => {
    const onFocusComposer = vi.fn();
    render(<HarnessRedirect onFocusComposer={onFocusComposer} />);

    fireEvent.click(screen.getByRole('button', { name: 'Focus composer' }));

    expect(onFocusComposer).toHaveBeenCalledTimes(1);
  });
});
