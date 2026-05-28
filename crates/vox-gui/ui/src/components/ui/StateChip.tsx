import React from 'react';

type Tone = 'success' | 'running' | 'warning' | 'neutral';

const toneStyle: Record<Tone, React.CSSProperties> = {
    success: {
        background: 'var(--vscode-testing-iconPassed, #34d399)22',
        color: 'var(--vscode-testing-iconPassed, #34d399)',
        borderColor: 'var(--vscode-testing-iconPassed, #34d399)',
    },
    running: {
        background: 'var(--vscode-textLink-foreground, #60a5fa)22',
        color: 'var(--vscode-textLink-foreground, #60a5fa)',
        borderColor: 'var(--vscode-textLink-foreground, #60a5fa)',
    },
    warning: {
        background: 'var(--vscode-editorWarning-foreground, #eab308)22',
        color: 'var(--vscode-editorWarning-foreground, #ca8a04)',
        borderColor: 'var(--vscode-editorWarning-foreground, #ca8a04)',
    },
    neutral: {
        background: 'var(--vscode-badge-background, rgba(120,120,120,0.2))',
        color: 'var(--vscode-descriptionForeground, inherit)',
        borderColor: 'var(--vscode-panel-border, rgba(255,255,255,0.08))',
    },
};

export function StateChip({ label, tone }: { label: string; tone: Tone }) {
    return (
        <span
            className="px-3 py-1 rounded-lg text-[9px] font-extrabold uppercase tracking-widest border"
            style={toneStyle[tone]}
        >
            {label}
        </span>
    );
}
