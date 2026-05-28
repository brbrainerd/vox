/**
 * CodeBlock — shiki-powered syntax-highlighted code block component.
 *
 * Renders a raw source string with full token-level syntax highlighting via
 * the lazy shiki singleton from `../utils/highlight`. While shiki resolves
 * (async), renders a plain `<pre>` so there is no flash of empty content.
 * On error degrades to the same `<pre>` fallback without throwing.
 */
import React, { useEffect, useRef, useState } from 'react';
import { highlightCode } from '../utils/highlight';

interface CodeBlockProps {
    /** Raw source text — no surrounding fences. */
    code: string;
    /** Language identifier (e.g. `"rust"`, `"vox"`, `"typescript"`). */
    lang: string;
}

export function CodeBlock({ code, lang }: CodeBlockProps) {
    const [html, setHtml] = useState<string>('');
    const aliveRef = useRef(true);

    useEffect(() => {
        aliveRef.current = true;
        highlightCode(code, lang || 'text', true)
            .then((h) => {
                if (aliveRef.current) setHtml(h);
            })
            .catch(() => {
                // highlightCode already swallows errors, but defend here too
                if (aliveRef.current) setHtml('');
            });
        return () => {
            aliveRef.current = false;
        };
    }, [code, lang]);

    if (!html) {
        // Plain skeleton until shiki resolves — no empty flash
        return (
            <pre className="shiki-block shiki-loading">
                <code>{code}</code>
            </pre>
        );
    }

    // shiki output is sanitized HTML — dangerouslySetInnerHTML is safe here.
    // The `shiki-block` class overrides shiki's default background so it
    // inherits the extension's glass-morphism container style.
    return (
        <div
            className="shiki-block"
            // biome-ignore lint/security/noDangerouslySetInnerHtml: shiki output is safe
            dangerouslySetInnerHTML={{ __html: html }}
        />
    );
}
