import { createContext, useContext, useState, useCallback, type ReactNode } from 'react';
import { type Lang, labelFor, currentLang } from '../lib/lexicon';

interface LangCtx { lang: Lang; setLang: (l: Lang) => void }
const Ctx = createContext<LangCtx | null>(null);

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(currentLang);
  const setLang = useCallback((l: Lang) => {
    try { window.localStorage.setItem('vox.lang', l); } catch { /* ignore */ }
    setLangState(l);
  }, []);
  return <Ctx.Provider value={{ lang, setLang }}>{children}</Ctx.Provider>;
}

export function useLang(): LangCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('useLang must be used within LanguageProvider');
  return ctx;
}

export function useLabel(id: string): string {
  return labelFor(id, useLang().lang);
}
