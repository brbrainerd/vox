import React from 'react';

interface SegmentOption {
  id: string;
  label: string;
  hint?: string;
  tone?: string;
}

interface SegmentProps {
  value: string;
  onChange: (id: string) => void;
  options: SegmentOption[];
  size?: 'xs' | 'sm';
}

export function Segment({ value, onChange, options, size = 'sm' }: SegmentProps) {
  const pad = size === 'xs' ? 'px-2 py-0.5 text-[10px]' : 'px-2.5 py-1 text-[11px]';
  return (
    <div className="inline-flex items-center rounded-md border border-white/10 bg-black/30 p-0.5">
      {options.map((o) => {
        const on = value === o.id;
        return (
          <button
            type="button"
            key={o.id}
            title={o.hint}
            aria-pressed={on}
            onClick={() => onChange(o.id)}
            className={`${pad} font-display uppercase tracking-[0.15em] rounded-[5px] transition ${on ? (o.tone || 'bg-white/10 text-zinc-50') : 'text-zinc-500 hover:text-zinc-300'}`}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
