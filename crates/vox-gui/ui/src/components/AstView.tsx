import React from 'react';

export const AstView = ({ ast = null, activeFile }: { ast?: unknown; activeFile?: string }) => {
  return (
    <div
      className="h-full flex flex-col"
      style={{ background: 'var(--vscode-sideBar-background, #09090b)' }}
    >
      <header className="px-10 py-8 border-b border-white/5 bg-white/[0.01]">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-3xl font-black text-white tracking-tight flex items-center gap-4">
            AST <span className="text-blue-500">Inspector</span>
          </h2>
        </div>
        <span className="text-[10px] font-mono text-zinc-500 uppercase tracking-widest">
          Active: {activeFile || 'No .vox file'}
        </span>
      </header>

      <div className="flex-1 overflow-y-auto p-8">
        {!ast ? (
          <div className="text-zinc-500 text-sm font-medium">
            Open a <code className="text-blue-400">.vox</code> file to load an AST from{' '}
            <code className="text-blue-400">vox_compiler::ast_inspect</code>.
          </div>
        ) : (
          <div className="glass rounded-[2rem] border border-white/5 p-8 font-mono text-xs leading-relaxed text-zinc-400">
            <pre>{JSON.stringify(ast, null, 2)}</pre>
          </div>
        )}
      </div>
    </div>
  );
};
