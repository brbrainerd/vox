// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from './Dialog';

function SampleDialog({ open }: { open?: boolean }) {
  return (
    <Dialog defaultOpen={open}>
      <DialogTrigger asChild>
        <button type="button">Open</button>
      </DialogTrigger>
      <DialogContent>
        <DialogTitle>Confirm action</DialogTitle>
        <DialogDescription>This action cannot be undone.</DialogDescription>
        <p>Dialog body content.</p>
      </DialogContent>
    </Dialog>
  );
}

describe('Dialog', () => {
  it('does not render dialog content before trigger is clicked', () => {
    render(<SampleDialog />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('opens when the trigger is clicked', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    });
  });

  it('renders the title inside the open dialog', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByText('Confirm action')).toBeInTheDocument();
    });
  });

  it('renders the description inside the open dialog', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByText('This action cannot be undone.')).toBeInTheDocument();
    });
  });

  it('closes when the Escape key is pressed', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    });
    await userEvent.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('renders in open state when defaultOpen is true', async () => {
    render(<SampleDialog open={true} />);
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    });
  });
});
