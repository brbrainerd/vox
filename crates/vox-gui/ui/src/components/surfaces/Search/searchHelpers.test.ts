import { describe, it, expect } from 'vitest';
import { renderHighlights } from './searchHelpers';

describe('renderHighlights', () => {
  it('returns a single un-marked segment when query is empty', () => {
    const result = renderHighlights('hello world', '');
    expect(result).toEqual([{ text: 'hello world', mark: false }]);
  });

  it('returns a single un-marked segment when query is only whitespace', () => {
    const result = renderHighlights('hello world', '   ');
    expect(result).toEqual([{ text: 'hello world', mark: false }]);
  });

  it('marks a single token', () => {
    const result = renderHighlights('hello world', 'world');
    expect(result).toEqual([
      { text: 'hello ', mark: false },
      { text: 'world', mark: true },
    ]);
  });

  it('is case-insensitive', () => {
    const result = renderHighlights('Hello World', 'hello');
    expect(result).toEqual([
      { text: 'Hello', mark: true },
      { text: ' World', mark: false },
    ]);
  });

  it('marks multiple tokens from a multi-word query', () => {
    const result = renderHighlights('the quick brown fox', 'quick fox');
    const marked = result.filter(s => s.mark).map(s => s.text.toLowerCase());
    expect(marked).toContain('quick');
    expect(marked).toContain('fox');
  });

  it('returns un-marked segment when no token matches', () => {
    const result = renderHighlights('hello world', 'xyz');
    expect(result).toEqual([{ text: 'hello world', mark: false }]);
  });

  it('handles an empty snippet', () => {
    const result = renderHighlights('', 'query');
    expect(result).toEqual([{ text: '', mark: false }]);
  });

  it('marks token that appears multiple times', () => {
    const result = renderHighlights('cat and cat', 'cat');
    const marked = result.filter(s => s.mark);
    expect(marked).toHaveLength(2);
    expect(marked.every(s => s.text === 'cat')).toBe(true);
  });

  it('handles regex special characters in tokens safely', () => {
    const result = renderHighlights('price is $5.00', '$5.00');
    const marked = result.filter(s => s.mark);
    expect(marked).toHaveLength(1);
    expect(marked[0].text).toBe('$5.00');
  });

  it('produces non-overlapping, char-safe segments that reconstruct the original snippet', () => {
    const snippet = 'Rust ownership and borrowing';
    const result = renderHighlights(snippet, 'rust borrow');
    const reconstructed = result.map(s => s.text).join('');
    expect(reconstructed).toBe(snippet);
  });
});
