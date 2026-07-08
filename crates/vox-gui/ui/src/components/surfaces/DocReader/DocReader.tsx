import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useVoxQuery } from '../../../hooks/useVoxQuery';
import { voxTransport } from '../../../transport';
import { Button } from '../../ui/Button';
import { docPathFromTab } from '../../../hooks/useWorkbenchTabs';

interface DocReaderProps {
  tabId: string;
}

export function DocReader({ tabId }: DocReaderProps) {
  const path = docPathFromTab(tabId);
  const q = useVoxQuery(['doc', path], () => invoke<string>('read_doc_markdown', { path }), {
    enabled: path.length > 0,
  });

  if (q.isLoading) {
    return (
      <article className="p-6 text-text-muted font-display text-sm" data-testid="doc-reader">
        Loading documentation…
      </article>
    );
  }

  if (q.isError) {
    return (
      <article className="p-6 text-amber-400 font-mono text-sm" data-testid="doc-reader" role="alert">
        Failed to load doc: {q.error.message}
      </article>
    );
  }

  return (
    <article className="max-w-none p-4" data-testid="doc-reader">
      <header className="mb-4 border-b border-border-subtle pb-3">
        <h1 className="font-display text-lg text-text-primary">{path.split('/').pop()}</h1>
        <p className="mt-1 font-mono text-[11px] text-text-muted">{path}</p>
      </header>
      <pre className="whitespace-pre-wrap font-mono text-[12px] leading-relaxed text-text-secondary">
        {q.data ?? ''}
      </pre>
      <div className="mt-4">
        <Button
          className="text-[11px]"
          onClick={() => {
            voxTransport.openLocator({ kind: 'file', value: path }).catch(() => {});
          }}
        >
          Open in editor
        </Button>
      </div>
    </article>
  );
}
