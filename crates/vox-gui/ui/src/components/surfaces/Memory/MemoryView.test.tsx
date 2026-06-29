// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { LanguageProvider } from '../../../hooks/useLanguage';

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
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    expect(screen.getByText(/Mnemosyne/i)).toBeDefined();
  });

  it('renders the Recent recalls section heading', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    expect(screen.getByText(/Recent recalls/i)).toBeDefined();
  });

  it('renders the Memory shards section heading', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    expect(screen.getByText(/Memory shards/i)).toBeDefined();
  });

  it('renders the Recall button', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    expect(screen.getByText('Recall')).toBeDefined();
  });

  it('every button carries an explicit type="button"', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('labels the recall search input (no placeholder-as-label)', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    expect(screen.getByLabelText('Recall query')).toBeDefined();
  });

  it('exposes the Auto-recall toggle with aria-pressed', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    const toggle = screen.getByRole('button', { name: /auto-recall/i });
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
  });

  it('marks the citations region as a polite live region', () => {
    render(<LanguageProvider><MemoryView pushToast={noopToast} /></LanguageProvider>);
    const region = screen.getByLabelText('Recall citations');
    expect(region.getAttribute('aria-live')).toBe('polite');
  });
});
