/**
 * Per-capture in-page audits. Each function is passed to page.evaluate(fn)
 * — Playwright serializes the SOURCE, so keep them fully self-contained.
 */

export interface IconIssue {
  kind: 'zero-size-svg' | 'empty-svg' | 'broken-img';
  id: string;
  testid: string;
  selectorHint: string;
}

export function auditIconsInPage(): IconIssue[] {
  const issues: IconIssue[] = [];
  // getAttribute('class'): el.className is an SVGAnimatedString on SVG.
  const hint = (el: Element) =>
    `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}.${(el.getAttribute('class') || '').split(/\s+/)[0]}`;
  for (const svg of Array.from(document.querySelectorAll('svg'))) {
    const r = svg.getBoundingClientRect();
    const drawable = svg.querySelector('path, circle, rect, line, polyline, polygon, use, text');
    if (r.width === 0 || r.height === 0) {
      issues.push({ kind: 'zero-size-svg', id: svg.id, testid: svg.getAttribute('data-testid') ?? '', selectorHint: hint(svg) });
    } else if (!drawable) {
      issues.push({ kind: 'empty-svg', id: svg.id, testid: svg.getAttribute('data-testid') ?? '', selectorHint: hint(svg) });
    }
  }
  for (const img of Array.from(document.querySelectorAll('img'))) {
    if (img.complete && img.naturalWidth === 0) {
      issues.push({ kind: 'broken-img', id: img.id, testid: img.getAttribute('data-testid') ?? '', selectorHint: hint(img) });
    }
  }
  return issues;
}

export interface OverflowReport {
  bodyHorizontalOverflowPx: number;
  scrollHostHorizontalOverflowPx: number;
  contentHeightPx: number;
}

export function auditOverflowInPage(): OverflowReport {
  const body = document.body;
  const host = document.querySelector('[data-testid="surface-scroll-host"]') as HTMLElement | null;
  return {
    bodyHorizontalOverflowPx: Math.max(0, body.scrollWidth - body.clientWidth),
    scrollHostHorizontalOverflowPx: host ? Math.max(0, host.scrollWidth - host.clientWidth) : 0,
    contentHeightPx: Math.max(body.scrollHeight, document.documentElement.scrollHeight),
  };
}
