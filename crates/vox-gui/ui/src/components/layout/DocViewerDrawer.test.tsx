// @vitest-environment jsdom
import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DocViewerDrawer } from './DocViewerDrawer';

vi.mock('../surfaces/DocReader/DocReader', () => ({
  DocReader: ({ path }: { path: string }) => <div data-testid="doc-reader-stub">{path}</div>,
}));

describe('DocViewerDrawer', () => {
  it('renders nothing when doc is null', () => {
    const { container } = render(
      <DocViewerDrawer doc={null} onClose={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the doc title, a close button, and passes path to DocReader', () => {
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={vi.fn()} />,
    );
    expect(screen.getByText('Foo Guide')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Close doc' })).toBeInTheDocument();
    expect(screen.getByTestId('doc-reader-stub')).toHaveTextContent('docs/foo.md');
  });

  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn();
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Close doc' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn();
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={onClose} />,
    );
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when the scrim is clicked', () => {
    const onClose = vi.fn();
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole('button', { name: /close doc overlay/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('moves focus into the drawer on open', () => {
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={vi.fn()} />,
    );
    expect(screen.getByRole('button', { name: 'Close doc' })).toHaveFocus();
  });

  it('restores focus to the trigger element after close', () => {
    function Harness() {
      const [open, setOpen] = React.useState(false);
      return (
        <div>
          <button type="button" onClick={() => setOpen(true)}>
            Open doc
          </button>
          <DocViewerDrawer
            doc={open ? { path: 'docs/foo.md', title: 'Foo Guide' } : null}
            onClose={() => setOpen(false)}
          />
        </div>
      );
    }

    render(<Harness />);
    const trigger = screen.getByRole('button', { name: 'Open doc' });
    trigger.focus();
    fireEvent.click(trigger);

    expect(screen.getByRole('button', { name: 'Close doc' })).toHaveFocus();

    fireEvent.click(screen.getByRole('button', { name: 'Close doc' }));

    expect(trigger).toHaveFocus();
  });
});
