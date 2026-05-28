import React from 'react';

/** Theme-aware panel shell (VS Code webview tokens + fallbacks). */
export function Panel({
    title,
    children,
    className = '',
}: {
    title?: string;
    children: React.ReactNode;
    className?: string;
}) {
    return (
        <section
            className={`rounded-2xl border p-6 ${className}`}
            style={{
                background: 'var(--vscode-sideBar-background, rgba(255,255,255,0.03))',
                borderColor: 'var(--vscode-panel-border, rgba(255,255,255,0.08))',
                color: 'var(--vscode-sideBar-foreground, inherit)',
            }}
        >
            {title && (
                <h3 className="text-xs font-bold uppercase tracking-widest mb-4 opacity-80">{title}</h3>
            )}
            {children}
        </section>
    );
}
