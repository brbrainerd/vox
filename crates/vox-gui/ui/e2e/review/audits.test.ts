// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { auditIconsInPage, auditOverflowInPage } from './audits';

describe('auditIconsInPage', () => {
  it('flags zero-size svgs, drawless svgs, and broken imgs; passes healthy ones', () => {
    document.body.innerHTML = `
      <svg id="ok"><path d="M0 0h16v16z"/></svg>
      <svg id="drawless"></svg>
      <svg id="zero"><path d="M0 0h16v16z"/></svg>
      <img id="broken" src="x.png" alt="icon" />
    `;
    // jsdom rects are 0x0 — emulate rendered sizes explicitly.
    const rect16 = () => ({ width: 16, height: 16 }) as DOMRect;
    (document.getElementById('ok') as any).getBoundingClientRect = rect16;
    (document.getElementById('drawless') as any).getBoundingClientRect = rect16;
    // 'zero' keeps the 0x0 default -> zero-size branch.
    // jsdom imgs: complete=false by default -> force the loaded-but-broken shape.
    const broken = document.getElementById('broken') as HTMLImageElement;
    Object.defineProperty(broken, 'complete', { value: true });
    // naturalWidth is 0 in jsdom already.

    const issues = auditIconsInPage();
    expect(issues.some((i) => i.kind === 'empty-svg' && i.id === 'drawless')).toBe(true);
    expect(issues.some((i) => i.kind === 'zero-size-svg' && i.id === 'zero')).toBe(true);
    expect(issues.some((i) => i.kind === 'broken-img' && i.id === 'broken')).toBe(true);
    expect(issues.some((i) => i.id === 'ok')).toBe(false);
  });
});

describe('auditOverflowInPage', () => {
  it('reports body horizontal overflow', () => {
    Object.defineProperty(document.body, 'scrollWidth', { value: 1600, configurable: true });
    Object.defineProperty(document.body, 'clientWidth', { value: 1440, configurable: true });
    expect(auditOverflowInPage().bodyHorizontalOverflowPx).toBe(160);
  });
});
