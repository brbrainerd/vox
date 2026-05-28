import * as vscode from 'vscode';
import * as path from 'path';
import { VoxMcpClient } from '../core/VoxMcpClient';

const AUDIO_EXT_RE = /\.(wav|mp3|flac|ogg|m4a|webm)$/i;

function isLikelyAudioUri(u: vscode.Uri): boolean {
    return u.scheme === 'file' && AUDIO_EXT_RE.test(u.fsPath);
}

function requireWorkspaceFolder(): vscode.WorkspaceFolder | undefined {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
        void vscode.window.showWarningMessage('Open a workspace folder to use Oratio speech tools.');
    }
    return folder;
}

/** Workspace-relative MCP `path`, or copy under `.vox/tmp/` when the file is outside the folder. */
async function resolveMcpAudioPath(
    folder: vscode.WorkspaceFolder,
    file: vscode.Uri,
): Promise<string> {
    const owning = vscode.workspace.getWorkspaceFolder(file);
    if (owning && owning.index === folder.index) {
        const rel = vscode.workspace.asRelativePath(file, false).replace(/\\/g, '/');
        if (
            !rel.startsWith('..') &&
            !path.posix.isAbsolute(rel) &&
            isLikelyAudioUri(file)
        ) {
            return rel;
        }
    }
    return importAudioToWorkspaceRel(folder, file);
}

async function pickAudioFile(fromContext?: vscode.Uri): Promise<vscode.Uri | undefined> {
    if (fromContext) {
        if (isLikelyAudioUri(fromContext)) return fromContext;
        void vscode.window.showWarningMessage(
            'Not an audio file. Use a .wav / .mp3 / … file, or run from the palette to pick one.',
        );
        return undefined;
    }
    const uris = await vscode.window.showOpenDialog({
        canSelectMany: false,
        title: 'Vox: Choose audio file',
        filters: { Audio: ['wav', 'mp3', 'flac', 'ogg', 'm4a', 'webm'], All: ['*'] },
    });
    return uris?.[0];
}

async function ensureVoxTmp(uri: vscode.Uri): Promise<void> {
    const tmp = vscode.Uri.joinPath(uri, '.vox', 'tmp');
    await vscode.workspace.fs.createDirectory(tmp);
}

/** Copy or use path relative to workspace root for MCP `path` arguments. */
async function importAudioToWorkspaceRel(
    folder: vscode.WorkspaceFolder,
    source: vscode.Uri,
): Promise<string> {
    await ensureVoxTmp(folder.uri);
    const base = path.basename(source.fsPath).replace(/[^a-zA-Z0-9._-]/g, '_') || 'recording.wav';
    const destName = `import_${Date.now()}_${base}`;
    const dest = vscode.Uri.joinPath(folder.uri, '.vox', 'tmp', destName);
    await vscode.workspace.fs.copy(source, dest, { overwrite: true });
    return path.posix.join('.vox/tmp', destName);
}

function showResultInDocument(title: string, body: string): void {
    void vscode.workspace.openTextDocument({ language: 'json', content: body }).then((doc) => {
        void vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Beside });
    });
}

export function registerOratioSpeechCommands(
    context: vscode.ExtensionContext,
    mcp: VoxMcpClient,
): void {
    context.subscriptions.push(
        vscode.commands.registerCommand('vox.oratio.transcribeFile', async (fromContext?: vscode.Uri) => {
            if (!mcp.connected) {
                void vscode.window.showWarningMessage('MCP not connected.');
                return;
            }
            if (!mcp.isToolAvailable('vox_oratio_transcribe')) {
                void vscode.window.showWarningMessage(
                    'Server does not advertise `vox_oratio_transcribe` (Oratio/STT).',
                );
                return;
            }
            const picked = await pickAudioFile(fromContext);
            if (!picked) return;
            const folder = vscode.workspace.getWorkspaceFolder(picked);
            if (!folder) {
                void vscode.window.showWarningMessage(
                    'Audio file must be inside an open workspace folder (or use voice capture).',
                );
                return;
            }
            const rel = await resolveMcpAudioPath(folder, picked);
            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Oratio: transcribing…',
                    cancellable: false,
                },
                async () => {
                    const result = await mcp.oratioTranscribe(rel);
                    const text =
                        typeof result === 'string'
                            ? result
                            : JSON.stringify(result, null, 2);
                    mcp.outputChannel.appendLine(`[Oratio transcribe] ${text}`);
                    showResultInDocument('Oratio transcript', text);
                },
            );
        }),
        vscode.commands.registerCommand('vox.oratio.speechToCodeFile', async (fromContext?: vscode.Uri) => {
            if (!mcp.connected) {
                void vscode.window.showWarningMessage('MCP not connected.');
                return;
            }
            if (!mcp.isToolAvailable('vox_speech_to_code')) {
                void vscode.window.showWarningMessage(
                    'Server does not advertise `vox_speech_to_code`.',
                );
                return;
            }
            const picked = await pickAudioFile(fromContext);
            if (!picked) return;
            const folder = vscode.workspace.getWorkspaceFolder(picked);
            if (!folder) {
                void vscode.window.showWarningMessage(
                    'Audio file must be inside an open workspace folder (or use voice capture).',
                );
                return;
            }
            const rel = await resolveMcpAudioPath(folder, picked);
            const validate =
                (await vscode.window.showQuickPick(['Yes (HIR repair)', 'No (draft only)'], {
                    placeHolder: 'Run validation / repair?',
                }))?.startsWith('Yes') ?? true;
            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Oratio: speech → code…',
                    cancellable: false,
                },
                async () => {
                    const result = await mcp.speechToCode({
                        path: rel,
                        validate,
                        session_id: 'vscode-oratio',
                    });
                    const text =
                        typeof result === 'string'
                            ? result
                            : JSON.stringify(result, null, 2);
                    mcp.outputChannel.appendLine(`[speech_to_code] ${text}`);
                    showResultInDocument('Speech to code result', text);
                },
            );
        }),
        vscode.commands.registerCommand('vox.oratio.voiceCaptureTranscribe', () => {
            openOratioVoicePanel(context, mcp, 'transcribe');
        }),
        vscode.commands.registerCommand('vox.oratio.voiceCaptureSpeechToCode', () => {
            openOratioVoicePanel(context, mcp, 'speechToCode');
        }),
    );
}

function voiceCaptureHtml(nonce: string, panelMode: 'transcribe' | 'speechToCode'): string {
    const modeLit = JSON.stringify(panelMode);
    return `<!DOCTYPE html>
<html><head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src 'unsafe-inline';">
<style>
body { font-family: var(--vscode-font-family); color: var(--vscode-foreground); padding: 12px; }
button { margin: 6px 6px 6px 0; padding: 8px 12px; cursor: pointer; }
.status { margin-top: 10px; opacity: 0.85; font-size: 12px; }
.error { color: var(--vscode-errorForeground); }
</style></head><body>
<p><strong>Vox Oratio</strong> — microphone captures PCM WAV in-browser (no server-side mic).</p>
<button id="start">Start</button>
<button id="stop" disabled>Stop & send</button>
<p class="status" id="st">Idle.</p>
<script nonce="${nonce}">
(function() {
  const vscode = acquireVsCodeApi();
  const mode = ${modeLit};
  const startBtn = document.getElementById('start');
  const stopBtn = document.getElementById('stop');
  const st = document.getElementById('st');
  let audioCtx = null;
  let mediaStream = null;
  let processor = null;
  let sourceNode = null;
  const sampleBuffers = [];

  function setStatus(msg, isErr) {
    st.textContent = msg;
    st.className = 'status' + (isErr ? ' error' : '');
  }

  function floatTo16BitPCM(float32) {
    const out = new DataView(new ArrayBuffer(float32.length * 2));
    for (let i = 0; i < float32.length; i++) {
      let s = Math.max(-1, Math.min(1, float32[i]));
      out.setInt16(i * 2, s < 0 ? s * 0x8000 : s * 0x7FFF, true);
    }
    return new Uint8Array(out.buffer);
  }

  function encodeWavMono(float32Chunks, sampleRate) {
    let total = 0;
    for (const c of float32Chunks) total += c.length;
    const merged = new Float32Array(total);
    let off = 0;
    for (const c of float32Chunks) {
      merged.set(c, off);
      off += c.length;
    }
    const pcm = floatTo16BitPCM(merged);
    const numChannels = 1;
    const bitsPerSample = 16;
    const blockAlign = (numChannels * bitsPerSample) / 8;
    const byteRate = sampleRate * blockAlign;
    const dataSize = pcm.length;
    const buf = new ArrayBuffer(44 + dataSize);
    const v = new DataView(buf);
    const w = (o, s) => { for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i)); };
    w(0, 'RIFF');
    v.setUint32(4, 36 + dataSize, true);
    w(8, 'WAVE');
    w(12, 'fmt ');
    v.setUint32(16, 16, true);
    v.setUint16(20, 1, true);
    v.setUint16(22, numChannels, true);
    v.setUint32(24, sampleRate, true);
    v.setUint32(28, byteRate, true);
    v.setUint16(32, blockAlign, true);
    v.setUint16(34, bitsPerSample, true);
    w(36, 'data');
    v.setUint32(40, dataSize, true);
    new Uint8Array(buf, 44).set(pcm);
    return buf;
  }

  function bytesToBase64(buffer) {
    const u8 = new Uint8Array(buffer);
    let binary = '';
    const chunk = 8192;
    for (let i = 0; i < u8.length; i += chunk) {
      binary += String.fromCharCode.apply(null, u8.subarray(i, Math.min(i + chunk, u8.length)));
    }
    return btoa(binary);
  }

  startBtn.onclick = async () => {
    try {
      sampleBuffers.length = 0;
      mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      audioCtx = new AudioContext();
      const src = audioCtx.createMediaStreamSource(mediaStream);
      const bufferSize = 4096;
      processor = audioCtx.createScriptProcessor(bufferSize, 1, 1);
      processor.onaudioprocess = (e) => {
        const data = e.inputBuffer.getChannelData(0);
        sampleBuffers.push(new Float32Array(data));
      };
      sourceNode = src;
      src.connect(processor);
      processor.connect(audioCtx.destination);
      startBtn.disabled = true;
      stopBtn.disabled = false;
      setStatus('Recording…');
    } catch (e) {
      setStatus('Mic error: ' + (e && e.message ? e.message : String(e)), true);
    }
  };

  stopBtn.onclick = () => {
    try {
      if (processor) {
        processor.disconnect();
        processor = null;
      }
      if (sourceNode) {
        sourceNode.disconnect();
        sourceNode = null;
      }
      if (mediaStream) {
        mediaStream.getTracks().forEach((t) => t.stop());
        mediaStream = null;
      }
      if (!audioCtx) {
        setStatus('Nothing recorded.', true);
        startBtn.disabled = false;
        stopBtn.disabled = true;
        return;
      }
      const sr = audioCtx.sampleRate;
      audioCtx.close();
      audioCtx = null;
      if (sampleBuffers.length === 0) {
        setStatus('No samples captured.', true);
        startBtn.disabled = false;
        stopBtn.disabled = true;
        return;
      }
      const wav = encodeWavMono(sampleBuffers, sr);
      vscode.postMessage({ type: 'voxOratioWav', mode: mode, base64: bytesToBase64(wav), sampleRate: sr });
      setStatus('Sent ' + wav.byteLength + ' bytes WAV @ ' + sr + ' Hz.');
    } catch (e) {
      setStatus('Stop error: ' + (e && e.message ? e.message : String(e)), true);
    }
    startBtn.disabled = false;
    stopBtn.disabled = true;
  };
})();
</script></body></html>`;
}

function openOratioVoicePanel(
    context: vscode.ExtensionContext,
    mcp: VoxMcpClient,
    mode: 'transcribe' | 'speechToCode',
): void {
    const folder = requireWorkspaceFolder();
    if (!folder) return;
    if (!mcp.connected) {
        void vscode.window.showWarningMessage('MCP not connected.');
        return;
    }
    const tool = mode === 'transcribe' ? 'vox_oratio_transcribe' : 'vox_speech_to_code';
    if (!mcp.isToolAvailable(tool)) {
        void vscode.window.showWarningMessage('Server does not advertise `' + tool + '`.');
        return;
    }
    void mcp.oratioStatus().then((status) => {
        const wsUrl = status?.streaming?.stream_ws_url;
        if (wsUrl) {
            mcp.outputChannel.appendLine(`[Oratio streaming] ${wsUrl}`);
        }
    });
    const nonce = String(Date.now());
    const panel = vscode.window.createWebviewPanel(
        'voxOratioVoice',
        mode === 'transcribe' ? 'Vox: Oratio voice (transcribe)' : 'Vox: Oratio voice (speech→code)',
        vscode.ViewColumn.Beside,
        { enableScripts: true, retainContextWhenHidden: false },
    );
    panel.webview.html = voiceCaptureHtml(nonce, mode);

    panel.webview.onDidReceiveMessage(
        async (msg: { type?: string; base64?: string; mode?: string }) => {
            if (msg?.type !== 'voxOratioWav' || !msg.base64) return;
            const voiceName = `vscode_voice_${Date.now()}.wav`;
            const rel = path.posix.join('.vox/tmp', voiceName);
            await ensureVoxTmp(folder.uri);
            const dest = vscode.Uri.joinPath(folder.uri, '.vox', 'tmp', voiceName);
            try {
                const buf = Buffer.from(msg.base64, 'base64');
                await vscode.workspace.fs.writeFile(dest, buf);
                panel.dispose();
                await vscode.window.withProgress(
                    {
                        location: vscode.ProgressLocation.Notification,
                        title:
                            msg.mode === 'speechToCode'
                                ? 'Oratio: speech → code…'
                                : 'Oratio: transcribing…',
                        cancellable: false,
                    },
                    async () => {
                        if (msg.mode === 'speechToCode') {
                            const result = await mcp.speechToCode({
                                path: rel,
                                validate: true,
                                session_id: 'vscode-oratio-voice',
                            });
                            const text =
                                typeof result === 'string'
                                    ? result
                                    : JSON.stringify(result, null, 2);
                            mcp.outputChannel.appendLine(`[voice speech_to_code] ${text}`);
                            showResultInDocument('Speech to code result', text);
                        } else {
                            const result = await mcp.oratioTranscribe(rel);
                            const text =
                                typeof result === 'string'
                                    ? result
                                    : JSON.stringify(result, null, 2);
                            mcp.outputChannel.appendLine(`[voice transcribe] ${text}`);
                            showResultInDocument('Oratio transcript', text);
                        }
                    },
                );
            } catch (e) {
                const m = e instanceof Error ? e.message : String(e);
                void vscode.window.showErrorMessage('Failed to save or transcribe: ' + m);
            }
        },
        undefined,
        context.subscriptions,
    );
}
