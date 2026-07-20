// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { Icon } from './Icons';

// Reproduces a live bug: the sidebar's nav icon lookup falls back to
// `Icon.file` (a generic document icon) whenever the mapped key isn't
// defined — 'message' (chat), 'book' (knowledge), 'folder' (workspace), and
// 'terminal' (commands) were all missing, so 4 of the 9 top-level sidebar
// entries rendered as identical generic document icons.
describe('Icon registry', () => {
  it.each(['message', 'book', 'folder', 'terminal'])(
    "defines a distinct '%s' icon, not a fallback to Icon.file",
    (key) => {
      const IconCmp = (Icon as Record<string, React.FC<React.SVGProps<SVGSVGElement>>>)[key];
      expect(IconCmp).toBeDefined();

      const { container: named } = render(<IconCmp data-testid="icon" />);
      const { container: file } = render(<Icon.file data-testid="icon" />);
      expect(named.innerHTML).not.toBe(file.innerHTML);
    },
  );
});
