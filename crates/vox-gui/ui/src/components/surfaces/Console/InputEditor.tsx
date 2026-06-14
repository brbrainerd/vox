import React, { useEffect, useRef, useState } from 'react';
import { discoverySuggest, type Suggestion } from '../../../transport';

interface Props {
  onSubmit: (line: string) => void;
  /** Called with the currently-highlighted suggestion's action id (for the rail). */
  onActiveSuggestion: (actionId: string | null) => void;
}

/**
 * The console prompt. The shell never receives a keystroke until Enter, so we own
 * completion entirely: as the user types, the top catalog suggestion renders as
 * ghost text after the cursor; Tab/→ accepts it, Enter submits the line.
 */
export function InputEditor({ onSubmit, onActiveSuggestion }: Props) {
  const [value, setValue] = useState('');
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const debounce = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reqSeq = useRef(0);

  useEffect(() => {
    if (debounce.current) clearTimeout(debounce.current);
    if (!value.startsWith('vox')) {
      setSuggestions([]);
      onActiveSuggestion(null);
      return;
    }
    debounce.current = setTimeout(() => {
      // Guard against out-of-order resolutions: only the latest request may
      // commit state, so a slow older query can't clobber fresher suggestions.
      const req = ++reqSeq.current;
      discoverySuggest(value, 8)
        .then((s) => {
          if (req !== reqSeq.current) return;
          setSuggestions(s);
          onActiveSuggestion(s[0]?.action_id ?? null);
        })
        .catch(() => {
          if (req !== reqSeq.current) return;
          setSuggestions([]);
          onActiveSuggestion(null);
        });
    }, 120);
    return () => {
      if (debounce.current) clearTimeout(debounce.current);
    };
  }, [value, onActiveSuggestion]);

  // The ghost is the remaining text of the top completion beyond what's typed.
  const top = suggestions[0];
  const typedTail = value.replace(/^vox\s*/, '');
  const ghost =
    top && top.completion.startsWith(typedTail) && typedTail.length > 0
      ? top.completion.slice(typedTail.length)
      : '';

  const acceptGhost = () => {
    if (ghost) setValue(`vox ${top!.completion}`);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === 'Tab' || e.key === 'ArrowRight') && ghost) {
      e.preventDefault();
      acceptGhost();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const line = value.trim();
      if (line) onSubmit(line);
      setValue('');
      setSuggestions([]);
    }
  };

  return (
    <div style={{ position: 'relative', fontFamily: 'monospace' }}>
      <span
        aria-hidden
        style={{ position: 'absolute', left: 0, color: '#9ca3af', pointerEvents: 'none' }}
      >
        {value}
        <span data-testid="ghost">{ghost}</span>
      </span>
      <input
        role="textbox"
        aria-label="console input"
        aria-autocomplete="inline"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onKeyDown}
        spellCheck={false}
        autoComplete="off"
        style={{
          width: '100%',
          background: 'transparent',
          border: 'none',
          outline: 'none',
          fontFamily: 'monospace',
        }}
      />
      <span
        role="status"
        aria-live="polite"
        style={{
          position: 'absolute',
          width: 1,
          height: 1,
          overflow: 'hidden',
          clip: 'rect(0 0 0 0)',
          whiteSpace: 'nowrap',
        }}
      >
        {top ? `Suggestion: ${top.completion}` : ''}
      </span>
    </div>
  );
}
