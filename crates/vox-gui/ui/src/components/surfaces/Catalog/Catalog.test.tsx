// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { LanguageProvider } from '../../../hooks/useLanguage';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(null) }));

import { Catalog } from './Catalog';

describe('Catalog', () => {
  it('renders the Command Center heading', () => {
    render(<LanguageProvider><Catalog skills={[]} /></LanguageProvider>);
    expect(screen.getByText(/command center/i)).toBeDefined();
  });

  it('renders without crashing when no skills are provided', () => {
    expect(() => render(<LanguageProvider><Catalog /></LanguageProvider>)).not.toThrow();
  });
});
