// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === 'get_memory_status') {
      return Promise.resolve({
        corpus_counts: { memory: 100, knowledge: 200, chunk: 50 },
        shards: [],
        recent_recalls: [],
        embedding_dim: 768,
      });
    }
    return Promise.resolve(null);
  }),
}));

const noopToast = () => {};

import { MemoryView } from './MemoryView';

describe('MemoryView', () => {
  it('renders the Mnemosyne heading', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText(/Mnemosyne/i)).toBeDefined();
  });

  it('renders the Recent recalls section heading', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText(/Recent recalls/i)).toBeDefined();
  });

  it('renders the Memory shards section heading', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText(/Memory shards/i)).toBeDefined();
  });

  it('renders the Recall button', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText('Recall')).toBeDefined();
  });
});
