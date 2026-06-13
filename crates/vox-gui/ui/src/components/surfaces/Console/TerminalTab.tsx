import React, { useEffect, useRef } from 'react';
import { Terminal, type IDisposable } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import {
  ptySpawn,
  ptyWrite,
  ptyClose,
  listenPtyOutput,
  listenPtyExit,
} from '../../../transport';
import { createBlockReducer, type Block, type Osc633Kind } from './osc633';

/** A line the parent wants written to this PTY. `seq` changes each submit so the
 *  effect re-fires even when the same text is sent twice. */
export interface PendingLine {
  text: string;
  seq: number;
}

interface Props {
  tabId: string;
  pendingLine: PendingLine | null;
  /** Called when an OSC 633 command/output block completes (shell integration). */
  onBlock?: (block: Block) => void;
}

/**
 * Renders one PTY-backed terminal via xterm.js. Spawns the PTY on mount, streams
 * output in, and forwards both interactive keystrokes (xterm onData) and
 * parent-submitted lines to the backend.
 */
export function TerminalTab({ tabId, pendingLine, onBlock }: Props) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const onBlockRef = useRef(onBlock);
  onBlockRef.current = onBlock;

  useEffect(() => {
    const term = new Terminal({ convertEol: true, fontFamily: 'monospace', fontSize: 13 });
    const fit = new FitAddon();
    term.loadAddon(fit);
    if (hostRef.current) term.open(hostRef.current);
    try {
      fit.fit();
    } catch {
      /* jsdom: no layout */
    }
    termRef.current = term;

    // OSC 633 shell-integration: build command/output blocks and paint a
    // per-block exit-status dot. Defensive — any failure leaves raw scrollback.
    const reducer = createBlockReducer();
    const decorations: IDisposable[] = [];
    term.parser.registerOscHandler(633, (data: string) => {
      const sep = data.indexOf(';');
      const kind = (sep === -1 ? data : data.slice(0, sep)) as Osc633Kind;
      const payload = sep === -1 ? undefined : data.slice(sep + 1);
      const active = term.buffer?.active;
      const cursorLine = active ? active.baseY + active.cursorY : 0;
      reducer.onMarker(kind, payload, cursorLine);
      if (kind === 'D') {
        const b = reducer.latestCompleted();
        if (b) {
          const enriched = { ...b, output: captureOutput(term, b.startLine, b.endLine) };
          onBlockRef.current?.(enriched);
          paintStatusDot(term, b, decorations);
        }
      }
      return true; // handled
    });

    term.onData((d) => {
      ptyWrite(tabId, d).catch(() => {});
    });

    let disposed = false;
    let unOut: (() => void) | undefined;
    let unExit: (() => void) | undefined;
    listenPtyOutput((id, data) => {
      if (id === tabId) term.write(data);
    }).then((u) => (disposed ? u() : (unOut = u)));
    listenPtyExit((id) => {
      if (id === tabId) term.write('\r\n[process exited]\r\n');
    }).then((u) => (disposed ? u() : (unExit = u)));

    ptySpawn(tabId, term.cols || 80, term.rows || 24).catch(() => {});

    return () => {
      disposed = true;
      unOut?.();
      unExit?.();
      decorations.forEach((d) => {
        try {
          d.dispose();
        } catch {
          /* already disposed with the term */
        }
      });
      ptyClose(tabId).catch(() => {});
      term.dispose();
    };
  }, [tabId]);

  useEffect(() => {
    if (pendingLine && termRef.current) {
      ptyWrite(tabId, `${pendingLine.text}\n`).catch(() => {});
    }
  }, [pendingLine, tabId]);

  return <div ref={hostRef} aria-label="terminal" style={{ height: '100%', width: '100%' }} />;
}

/**
 * Read the terminal buffer between a block's command line and its end line to
 * recover the command's output text. Best-effort: returns undefined if the
 * buffer API is unavailable (e.g. jsdom in tests).
 */
function captureOutput(term: Terminal, startLine: number, endLine: number): string | undefined {
  try {
    const buf = term.buffer?.active;
    if (!buf || typeof buf.getLine !== 'function') return undefined;
    const out: string[] = [];
    // +1 skips the command line itself; output lives between exec and the next prompt.
    for (let i = startLine + 1; i <= endLine; i++) {
      const line = buf.getLine(i);
      if (line) out.push(line.translateToString(true));
    }
    return out.join('\n').replace(/\n+$/, '');
  } catch {
    return undefined;
  }
}

/**
 * Paint a small exit-status dot in the gutter at a block's command line:
 * green (exit 0), red (non-zero), neutral (unknown). Best-effort — xterm's
 * marker/decoration APIs are unavailable in some environments (e.g. jsdom).
 */
function paintStatusDot(term: Terminal, block: Block, sink: IDisposable[]): void {
  try {
    const active = term.buffer?.active;
    const cursorAbs = active ? active.baseY + active.cursorY : 0;
    // registerMarker takes an offset relative to the cursor's current line.
    const marker = term.registerMarker(block.startLine - cursorAbs);
    if (!marker) return;
    const color =
      block.exitCode === null ? '#9ca3af' : block.exitCode === 0 ? '#22c55e' : '#ef4444';
    const dec = term.registerDecoration({ marker, width: 1 });
    if (!dec) {
      sink.push(marker);
      return;
    }
    dec.onRender((el: HTMLElement) => {
      el.style.backgroundColor = color;
      el.style.borderRadius = '50%';
      el.style.width = '6px';
      el.style.height = '6px';
      el.style.marginTop = '5px';
    });
    sink.push(dec, marker);
  } catch {
    /* decoration is cosmetic; never break the terminal over it */
  }
}
