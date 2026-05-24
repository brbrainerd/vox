import React from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';

interface WidgetContainerProps {
  id: string;
  title: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
  onRemove: (id: string) => void;
  isCollapsed?: boolean;
  onToggleCollapse?: (id: string) => void;
}

export function WidgetContainer({
  id,
  title,
  icon,
  children,
  onRemove,
  isCollapsed = false,
  onToggleCollapse,
}: WidgetContainerProps) {
  return (
    <Glass className="mb-4 flex flex-col overflow-hidden transition-all duration-300">
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/5 bg-white/[0.02]">
        <div className="flex items-center gap-2">
          {icon && <span className="text-zinc-400">{icon}</span>}
          <h2 className="font-display text-[13px] font-semibold tracking-wide text-zinc-100">{title}</h2>
        </div>
        <div className="flex items-center gap-2">
          {onToggleCollapse && (
            <button
              onClick={() => onToggleCollapse(id)}
              className="rounded p-1 text-zinc-500 hover:bg-white/5 hover:text-zinc-300 transition"
              title={isCollapsed ? "Expand" : "Collapse"}
            >
              {isCollapsed ? <Icon.chevronDown className="size-4" /> : <Icon.chevronUp className="size-4" />}
            </button>
          )}
          <button
            onClick={() => onRemove(id)}
            className="rounded p-1 text-zinc-500 hover:bg-rose-500/20 hover:text-rose-400 transition"
            title="Remove Widget"
          >
            <Icon.x className="size-4" />
          </button>
        </div>
      </div>
      {!isCollapsed && <div className="p-4 relative">{children}</div>}
    </Glass>
  );
}
