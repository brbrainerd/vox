/**
 * Pure OSC 633/133 shell-integration block reducer for the Vox Console.
 *
 * The spawned shell emits markers that delimit command/output blocks:
 *   A           prompt start
 *   B           prompt end / command-line start (no-op for the block model)
 *   E;<command> the command line (encoded)
 *   C           pre-execution (output begins)
 *   D;<exit>    command finished, with exit code
 *
 * This module is DOM-free and deterministic so it is unit-testable in isolation.
 */

export type Osc633Kind = 'A' | 'B' | 'C' | 'D' | 'E';

export interface Block {
  id: number;
  command: string;
  exitCode: number | null;
  startLine: number;
  endLine: number;
  running: boolean;
}

/**
 * Decode the OSC 633 `E` command payload. VS Code encodes control characters
 * and separators as `\xNN` escapes (e.g. `\x0a` for newline, `\x3b` for `;`).
 */
export function decodeCommand(raw: string): string {
  return raw.replace(/\\x([0-9a-fA-F]{2})/g, (_m, hex) =>
    String.fromCharCode(parseInt(hex, 16)),
  );
}

interface MutBlock extends Block {}

export interface BlockReducer {
  onMarker(kind: Osc633Kind, payload: string | undefined, cursorLine: number): void;
  blocks(): Block[];
  latestCompleted(): Block | null;
}

export function createBlockReducer(): BlockReducer {
  const done: MutBlock[] = [];
  let open: MutBlock | null = null;
  let nextId = 1;
  let latest: Block | null = null;

  const finalize = (exitCode: number | null, endLine: number) => {
    if (!open) return;
    open.exitCode = exitCode;
    open.running = false;
    open.endLine = endLine;
    const frozen: Block = { ...open };
    done.push(frozen);
    if (exitCode !== null) latest = frozen;
    open = null;
  };

  return {
    onMarker(kind, payload, cursorLine) {
      switch (kind) {
        case 'A': {
          // A new prompt starts. If a block is still open (no D seen), close it
          // with an unknown exit so we never lose it.
          if (open) finalize(null, cursorLine);
          open = {
            id: nextId++,
            command: '',
            exitCode: null,
            startLine: cursorLine,
            endLine: cursorLine,
            running: false,
          };
          break;
        }
        case 'E':
          if (open) open.command = decodeCommand(payload ?? '');
          break;
        case 'C':
          if (open) open.running = true;
          break;
        case 'D': {
          if (!open) break; // stray D — nothing to close
          const code = payload != null && payload !== '' ? Number(payload) : null;
          finalize(Number.isFinite(code as number) ? (code as number) : null, cursorLine);
          break;
        }
        case 'B':
          // prompt-end; not needed for the Core block model.
          break;
      }
    },
    blocks() {
      return open ? [...done, { ...open }] : [...done];
    },
    latestCompleted() {
      return latest;
    },
  };
}
