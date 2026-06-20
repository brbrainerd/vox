import React from 'react';
import { Glass } from './Glass';
import { cn } from '../../lib/cn';

const ACCENT_COLORS = {
  cyan: 'text-cyan-400',
  amber: 'text-amber-400',
  emerald: 'text-emerald-400',
  violet: 'text-violet-400',
  brass: 'text-brass',
  zinc: 'text-text-muted',
  sky: 'text-sky-400',
};

export interface KpiProps extends React.HTMLAttributes<HTMLDivElement> {
  label: string;
  value: string | number;
  unit?: string;
  delta?: number;
  trend?: 'up' | 'down' | 'flat';
  accent?: keyof typeof ACCENT_COLORS;
  sparkData?: number[];
  icon?: React.ReactNode;
  onClick?: () => void;
  children?: React.ReactNode;
  as?: React.ElementType;
}

export function Kpi({
  label,
  value,
  unit = '',
  delta,
  trend = 'flat',
  accent = 'brass',
  sparkData,
  icon,
  onClick,
  className,
  children,
  as,
  ...props
}: KpiProps) {
  const isClickable = !!onClick;
  const Comp = as || (isClickable ? 'button' : 'div');
  
  return (
    <Glass
      as={Comp}
      size="sm"
      interactive={isClickable}
      onClick={onClick}
      className={cn("flex flex-col select-none", className)}
      {...props}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-[10px] overline uppercase tracking-widest text-text-muted font-medium truncate">
          {label}
        </span>
        {icon && <span className="text-text-muted flex shrink-0">{icon}</span>}
      </div>

      <div className="flex items-baseline gap-1 mt-1">
        <span className={cn("font-mono font-bold tracking-tight text-[18px] tabular-nums", ACCENT_COLORS[accent])}>
          {value}
        </span>
        {unit && <span className="text-xs text-text-muted font-medium">{unit}</span>}
        
        {delta !== undefined && (
          <span className={cn(
            "ml-auto font-mono text-[10px] font-semibold flex items-center tabular-nums",
            trend === 'up' ? 'text-emerald-400' : trend === 'down' ? 'text-red-400' : 'text-text-muted'
          )}>
            {trend === 'up' ? '▲' : trend === 'down' ? '▼' : '■'}
            {Math.abs(delta)}
          </span>
        )}
      </div>

      {children}
    </Glass>
  );
}

Kpi.Sub = function KpiSub({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[11px] text-text-muted mt-1 leading-none select-text">
      {children}
    </div>
  );
};
