import React from 'react';
import { breadcrumbsForView, type BreadcrumbSegment } from '../../lib/navigation';

interface Props {
  viewKey: string;
  onNavigate?: (viewKey: string) => void;
}

export function BreadcrumbBar({ viewKey, onNavigate }: Props) {
  const segments = breadcrumbsForView(viewKey);
  if (segments.length === 0 || viewKey === 'chat') return null;

  const renderSegment = (seg: BreadcrumbSegment, index: number) => {
    const isLast = index === segments.length - 1;
    const content = (
      <span className={isLast ? 'text-zinc-200' : 'text-zinc-500'}>{seg.label}</span>
    );
    if (!isLast && onNavigate) {
      return (
        <button
          key={seg.key}
          type="button"
          onClick={() => onNavigate(seg.key)}
          className="font-display text-[11px] uppercase tracking-[0.14em] hover:text-zinc-300 transition"
          aria-label={`Navigate to ${seg.label}`}
        >
          {content}
        </button>
      );
    }
    return (
      <span
        key={seg.key}
        className="font-display text-[11px] uppercase tracking-[0.14em]"
        aria-current={isLast ? 'page' : undefined}
      >
        {content}
      </span>
    );
  };

  return (
    <nav aria-label="Breadcrumb" className="flex items-center gap-2 px-1 pb-2">
      {segments.map((seg, i) => (
        <React.Fragment key={seg.key}>
          {i > 0 && <span className="text-zinc-600 text-[10px]" aria-hidden="true">›</span>}
          {renderSegment(seg, i)}
        </React.Fragment>
      ))}
    </nav>
  );
}
