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
});
