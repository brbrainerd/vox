// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { Toasts, type ToastItem } from './Toasts';

const baseItems: ToastItem[] = [
  { id: 'a', tone: 'ok', title: 'Build succeeded' },
  { id: 'b', tone: 'warn', title: 'Lint warning', body: 'Line 42' },
];

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
