import React from 'react';
import { Glass } from '../../ui/Glass';
import { CommandCatalogForm } from '../../CommandCatalogForm';

export function Catalog({ skills = [] }: any) {
  const catalog = {
    generated_from: 'tauri:get_command_catalog',
    entries: skills ?? [],
  };

  return (
    <div className="flex flex-col gap-5 p-5">
      <Glass className="p-5">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Command Center</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Compiled CLI catalog with generated execution forms</p>
          </div>
        </div>
      </Glass>
      <div className="h-[calc(100vh-290px)] min-h-[560px]">
        <CommandCatalogForm catalog={catalog} />
      </div>
    </div>
  );
}
