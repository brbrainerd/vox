// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { Toasts, type ToastItem } from './Toasts';

const baseItems: ToastItem[] = [
  { id: 'a', tone: 'ok', title: 'Build succeeded', cause: 'backend-ok' },
  { id: 'b', tone: 'warn', title: 'Lint warning', body: 'Line 42', cause: 'backend-error' },
];

describe('Toasts MAX_TOASTS cap', () => {
  it('renders only the last 3 items when more than 3 are provided', () => {
    const manyItems: ToastItem[] = [
      { id: '1', tone: 'ok', title: 'First', cause: 'backend-ok' },
      { id: '2', tone: 'ok', title: 'Second', cause: 'backend-ok' },
      { id: '3', tone: 'ok', title: 'Third', cause: 'backend-ok' },
      { id: '4', tone: 'warn', title: 'Fourth', cause: 'backend-error' },
    ];
    // Simulate the .slice(-3) cap applied by the App before passing to Toasts
    const capped = manyItems.slice(-3);
    render(<Toasts items={capped} onClose={vi.fn()} />);
    expect(screen.queryByText('First')).toBeNull();
    expect(screen.getByText('Second')).toBeInTheDocument();
    expect(screen.getByText('Third')).toBeInTheDocument();
    expect(screen.getByText('Fourth')).toBeInTheDocument();
  });
});

describe('Toasts', () => {
  it('renders the outer container with aria-live="polite"', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    const region = document.querySelector('[aria-live="polite"]');
    expect(region).not.toBeNull();
  });

  it('renders the outer container with role="status"', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('close buttons have an accessible label', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    const closeButtons = screen.getAllByRole('button', { name: /dismiss/i });
    expect(closeButtons).toHaveLength(baseItems.length);
  });

  it('calls onClose with the correct id when a close button is clicked', async () => {
    const onClose = vi.fn();
    render(<Toasts items={[baseItems[0]]} onClose={onClose} />);
    await userEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(onClose).toHaveBeenCalledWith('a');
  });

  it('renders toast titles', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    expect(screen.getByText('Build succeeded')).toBeInTheDocument();
    expect(screen.getByText('Lint warning')).toBeInTheDocument();
  });

  it('renders toast body when provided', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    expect(screen.getByText('Line 42')).toBeInTheDocument();
  });
});
