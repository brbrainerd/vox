import React from 'react';
import { breadcrumbsForView, type BreadcrumbSegment } from '../../lib/navigation';
import { recordGamifyGuiEvent } from '../../lib/gamifyGuiEvents';

interface Props {
  viewKey: string;
  onNavigate?: (viewKey: string) => void;
  gamifyEnabled?: boolean;
  onResetLayout?: () => void;
}

export function BreadcrumbBar({ viewKey, onNavigate, gamifyEnabled, onResetLayout }: Props) {
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
          onClick={() => {
            void recordGamifyGuiEvent(
              'breadcrumb_navigation',
              { from: viewKey, to: seg.key },
              { enabled: gamifyEnabled },
            );
            onNavigate(seg.key);
          }}
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
    <nav aria-label="Breadcrumb" className="flex items-center justify-between px-1 pb-2">
      <div className="flex items-center gap-2">
        {segments.map((seg, i) => (
          <React.Fragment key={seg.key}>
            {i > 0 && <span className="text-zinc-600 text-[10px]" aria-hidden="true">›</span>}
            {renderSegment(seg, i)}
          </React.Fragment>
        ))}
      </div>
      {onResetLayout && (
        <button
          type="button"
          onClick={onResetLayout}
          className="text-zinc-500 hover:text-zinc-300 font-display text-[10px] uppercase tracking-[0.14em] transition px-2 py-0.5 rounded hover:bg-white/[0.04]"
        >
          Reset layout
        </button>
      )}
    </nav>
  );
}
