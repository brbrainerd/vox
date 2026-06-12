import React, { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import {
  ptySpawn,
  ptyWrite,
  ptyClose,
  listenPtyOutput,
  listenPtyExit,
} from '../../../transport';

/** A line the parent wants written to this PTY. `seq` changes each submit so the
 *  effect re-fires even when the same text is sent twice. */
export interface PendingLine {
  text: string;
  seq: number;
}

interface Props {
  tabId: string;
  pendingLine: PendingLine | null;
}

/**
 * Renders one PTY-backed terminal via xterm.js. Spawns the PTY on mount, streams
 * output in, and forwards both interactive keystrokes (xterm onData) and
 * parent-submitted lines to the backend.
 */
export function TerminalTab({ tabId, pendingLine }: Props) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);

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

    term.onData((d) => {
      ptyWrite(tabId, d).catch(() => {});
    });

    let unOut: (() => void) | undefined;
    let unExit: (() => void) | undefined;
    listenPtyOutput((id, data) => {
      if (id === tabId) term.write(data);
    }).then((u) => (unOut = u));
    listenPtyExit((id) => {
      if (id === tabId) term.write('\r\n[process exited]\r\n');
    }).then((u) => (unExit = u));

    ptySpawn(tabId, term.cols || 80, term.rows || 24).catch(() => {});

    return () => {
      unOut?.();
      unExit?.();
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
