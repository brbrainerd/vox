import { describe, it, expect } from 'vitest';
import { scanSource } from './honestyScan';

describe('scanSource', () => {
  it('flags placeholder text', () => {
    const v = scanSource('x.tsx', `return <div>Not yet implemented</div>;`);
    expect(v.map(x => x.kind)).toContain('placeholder');
  });
  it('flags an empty arrow handler', () => {
    const v = scanSource('x.tsx', `<button onClick={() => {}}>Go</button>`);
    expect(v.map(x => x.kind)).toContain('dead-handler');
  });
  it('passes a real handler', () => {
    const v = scanSource('x.tsx', `<button onClick={() => invoke('do_it')}>Go</button>`);
    expect(v).toHaveLength(0);
  });
  it('does NOT flag the Tailwind placeholder: pseudo-class', () => {
    const v = scanSource('x.tsx', `<input className="text-primary placeholder:text-muted" />`);
    expect(v).toHaveLength(0);
  });
  it('does NOT flag an input placeholder= attribute', () => {
    const v = scanSource('x.tsx', `<input placeholder="Search files…" />`);
    expect(v).toHaveLength(0);
  });
  it('does NOT flag "not wired" inside a comment', () => {
    const v = scanSource('x.tsx', `  // notifications are intentionally NOT wired yet`);
    expect(v).toHaveLength(0);
  });
  it('still flags placeholder used as prose', () => {
    const v = scanSource('x.tsx', `<p>This panel is a placeholder.</p>`);
    expect(v.map(x => x.kind)).toContain('placeholder');
  });
});
