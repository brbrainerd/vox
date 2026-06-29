// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { LanguageProvider, useLang, useLabel } from './useLanguage';

function Probe() {
  const { lang, setLang } = useLang();
  return (
    <div>
      <span data-testid="label">{useLabel('mercatus')}</span>
      <button onClick={() => setLang(lang === 'en' ? 'la' : 'en')}>flip</button>
    </div>
  );
}

describe('useLanguage', () => {
  beforeEach(() => window.localStorage.clear());

  it('defaults to English', () => {
    render(<LanguageProvider><Probe /></LanguageProvider>);
    expect(screen.getByTestId('label').textContent).toBe('Market');
  });
  it('flips to Latin and persists', () => {
    render(<LanguageProvider><Probe /></LanguageProvider>);
    act(() => screen.getByText('flip').click());
    expect(screen.getByTestId('label').textContent).toBe('Mercatus');
    expect(window.localStorage.getItem('vox.lang')).toBe('la');
  });
  it('hydrates from localStorage', () => {
    window.localStorage.setItem('vox.lang', 'la');
    render(<LanguageProvider><Probe /></LanguageProvider>);
    expect(screen.getByTestId('label').textContent).toBe('Mercatus');
  });
});
