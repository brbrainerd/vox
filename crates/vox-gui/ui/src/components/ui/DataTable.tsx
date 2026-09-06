import React, { useState } from 'react';
import { cn } from '../../lib/cn';
import { Button } from './Button';

export interface ColumnDef<T> {
  key: string;
  header: string;
  width?: number;
  sortable?: boolean;
  render?: (row: T) => React.ReactNode;
}

export interface DataTableProps<T> {
  rows: T[];
  columns: ColumnDef<T>[];
  groupBy?: (row: T) => string;
  selectable?: boolean;
  onRowAction?: (id: string, action: string) => void;
  emptyState?: React.ReactNode;
  loading?: boolean;
  getRowId: (row: T) => string;
  density?: 'compact' | 'default' | 'comfortable';
}

const DENSITY_CLASS = {
  compact: 'px-2 py-1 text-[11px]',
  default: 'px-4 py-2 text-sm',
  comfortable: 'px-6 py-4 text-base',
};

export function DataTable<T>({
  rows,
  columns,
  groupBy,
  selectable = false,
  onRowAction,
  emptyState,
  loading = false,
  getRowId,
  density = 'default',
}: DataTableProps<T>) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  if (loading) {
    return (
      <div className="w-full flex flex-col gap-2 py-4">
        {[1, 2, 3].map(i => (
          <div key={i} className="h-10 w-full bg-overlay-subtle border border-border-subtle rounded-lg animate-pulse" />
        ))}
      </div>
    );
  }

  if (rows.length === 0) {
    return <div className="w-full py-6">{emptyState || <div className="text-center text-text-muted text-sm">No data available</div>}</div>;
  }

  const toggleGroup = (group: string) => {
    setCollapsedGroups(curr => {
      const next = new Set(curr);
      if (next.has(group)) next.delete(group);
      else next.add(group);
      return next;
    });
  };

  const toggleSelectAll = () => {
    setSelectedIds(curr => {
      if (curr.size === rows.length) return new Set();
      return new Set(rows.map(getRowId));
    });
  };

  const toggleSelectRow = (id: string) => {
    setSelectedIds(curr => {
      const next = new Set(curr);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  // Grouping rows
  const grouped: Record<string, T[]> = {};
  if (groupBy) {
    rows.forEach(r => {
      const key = groupBy(r);
      (grouped[key] ??= []).push(r);
    });
  } else {
    grouped[''] = rows;
  }

  return (
    <div className="w-full overflow-x-auto rounded-xl border border-border-subtle bg-bg-base/20 backdrop-blur-xl">
      {selectable && selectedIds.size > 0 && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-border-subtle bg-brass/10 text-brass text-xs">
          <span>{selectedIds.size} rows selected</span>
          <div className="flex items-center gap-2">
            <Button size="xs" variant="primary" onClick={() => onRowAction?.(Array.from(selectedIds).join(','), 'bulk-pause')}>
              Pause
            </Button>
            <Button size="xs" variant="danger" onClick={() => onRowAction?.(Array.from(selectedIds).join(','), 'bulk-cancel')}>
              Cancel
            </Button>
          </div>
        </div>
      )}
      <table className="w-full border-collapse text-left">
        <thead>
          <tr className="border-b border-border-subtle bg-overlay-subtle">
            {selectable && (
              <th className={cn("w-10", DENSITY_CLASS[density])}>
                <input 
                  type="checkbox" 
                  checked={selectedIds.size === rows.length && rows.length > 0}
                  onChange={toggleSelectAll}
                  className="rounded-sm border-border-subtle bg-bg-base text-brass focus:ring-brass/40"
                  aria-label="Select all rows"
                />
              </th>
            )}
            {columns.map(col => (
              <th 
                key={col.key} 
                className={cn("font-semibold text-text-muted tracking-wide uppercase text-[10px]", DENSITY_CLASS[density])}
                style={{ width: col.width }}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {Object.entries(grouped).map(([groupName, groupRows]) => {
            const isCollapsed = collapsedGroups.has(groupName);
            return (
              <React.Fragment key={groupName}>
                {groupBy && (
                  <tr className="bg-overlay-subtle border-b border-border-subtle">
                    <td colSpan={columns.length + (selectable ? 1 : 0)} className="px-3 py-1.5">
                      <button
                        type="button"
                        onClick={() => toggleGroup(groupName)}
                        className="flex items-center gap-1.5 font-mono text-[10px] tracking-widest uppercase text-text-muted hover:text-text-secondary"
                        aria-expanded={!isCollapsed}
                      >
                        <span>{isCollapsed ? '▶' : '▼'}</span>
                        <span>{groupName} ({groupRows.length})</span>
                      </button>
                    </td>
                  </tr>
                )}
                {!isCollapsed && groupRows.map(row => {
                  const id = getRowId(row);
                  const isSelected = selectedIds.has(id);
                  return (
                    <tr 
                      key={id} 
                      className={cn(
                        "border-b border-border-subtle last:border-0 hover:bg-overlay-subtle transition-colors",
                        isSelected && "bg-brass/2"
                      )}
                    >
                      {selectable && (
                        <td className={cn("w-10", DENSITY_CLASS[density])}>
                          <input 
                            type="checkbox" 
                            checked={isSelected}
                            onChange={() => toggleSelectRow(id)}
                            className="rounded-sm border-brass/40 bg-bg-base text-brass focus:ring-brass/40"
                            aria-label={`Select row ${id}`}
                          />
                        </td>
                      )}
                      {columns.map(col => (
                        <td key={col.key} className={DENSITY_CLASS[density]}>
                          {col.render ? col.render(row) : (row as any)[col.key]}
                        </td>
                      ))}
                    </tr>
                  );
                })}
              </React.Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
