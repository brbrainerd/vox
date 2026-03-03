import * as vscode from 'vscode';
import { VoxMcpClient } from './core/VoxMcpClient';
import { WorkspaceContextEngine } from './context/WorkspaceContextEngine';
import { ChatController } from './chat/ChatController';

export class SidebarProvider implements vscode.WebviewViewProvider {
    private _view?: vscode.WebviewView;
    private _chatController: ChatController;
    private _contextEngine = new WorkspaceContextEngine();

    constructor(
        private readonly _extensionUri: vscode.Uri,
        private readonly _mcp: VoxMcpClient,
    ) {
        this._chatController = new ChatController(this._mcp, messages => {
            this.postMessage({ type: 'chatHistory', value: messages });
        });
    }

    resolveWebviewView(view: vscode.WebviewView): void {
        this._view = view;
        view.webview.options = { enableScripts: true, localResourceRoots: [this._extensionUri] };
        view.webview.html = this._getHtml(view.webview);
        this._chatController.loadHistory();

        view.webview.onDidReceiveMessage(async (msg: { type: string; value?: string }) => {
            switch (msg.type) {
                case 'submitChat': {
                    const ctx = this._contextEngine.getActiveEditorContext();
                    const openFiles = this._contextEngine.getOpenFilePaths();
                    const editorCtxStr = ctx.filePath ? `[Active: ${ctx.filePath}:${ctx.line}]` : '';
                    const prompt = `${editorCtxStr} ${msg.value ?? ''}`.trim();
                    await this._chatController.submitMessage(prompt, openFiles);
                    break;
                }
                case 'clearChat':
                    await this._chatController.clearHistory();
                    break;
                case 'plan':
                    vscode.commands.executeCommand('vox.plan');
                    break;
                case 'command':
                    if (msg.value) vscode.commands.executeCommand(msg.value);
                    break;
            }
        });
    }

    postMessage(msg: unknown): void {
        this._view?.webview.postMessage(msg);
    }

    private _getHtml(webview: vscode.Webview): string {
        const nonce = getNonce();
        return /* html */`<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';"/>
<title>Vox AI</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
:root{
  --bg:var(--vscode-sideBar-background);
  --fg:var(--vscode-foreground);
  --input-bg:var(--vscode-input-background);
  --input-fg:var(--vscode-input-foreground);
  --input-border:var(--vscode-input-border);
  --btn-bg:var(--vscode-button-background);
  --btn-fg:var(--vscode-button-foreground);
  --accent:var(--vscode-focusBorder);
  --user-bubble:#1e3a5f;
  --asst-bubble:var(--vscode-editor-background);
  --border:var(--vscode-widget-border,rgba(255,255,255,.08));
  --code-bg:rgba(0,0,0,.3);
  --font:var(--vscode-font-family,system-ui);
  --mono:var(--vscode-editor-font-family,'Fira Code',monospace);
}
body{background:var(--bg);color:var(--fg);font-family:var(--font);font-size:13px;display:flex;flex-direction:column;height:100vh;overflow:hidden;}
.header{display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid var(--border);background:rgba(255,255,255,.03);}
.header h1{font-size:14px;font-weight:600;letter-spacing:.02em;display:flex;align-items:center;gap:6px;}
.status-dot{width:8px;height:8px;border-radius:50%;background:#4caf50;box-shadow:0 0 6px #4caf50;transition:background .3s;}
.header-actions{display:flex;gap:6px;}
.icon-btn{background:none;border:none;color:var(--fg);cursor:pointer;padding:4px 6px;border-radius:4px;opacity:.65;font-size:14px;transition:opacity .15s,background .15s;}
.icon-btn:hover{opacity:1;background:rgba(255,255,255,.08);}
.messages{flex:1;overflow-y:auto;padding:12px 10px;display:flex;flex-direction:column;gap:10px;scroll-behavior:smooth;}
.messages::-webkit-scrollbar{width:4px;}
.messages::-webkit-scrollbar-thumb{background:rgba(255,255,255,.15);border-radius:2px;}
.message{max-width:90%;padding:8px 12px;border-radius:10px;line-height:1.6;word-break:break-word;}
.message.user{align-self:flex-end;background:var(--user-bubble);border-radius:10px 10px 2px 10px;}
.message.assistant{align-self:flex-start;background:var(--asst-bubble);border:1px solid var(--border);border-radius:2px 10px 10px 10px;}
.message.system{align-self:center;opacity:.5;font-size:11px;font-style:italic;}
.message.streaming::after{content:'▊';animation:blink .9s step-end infinite;}
@keyframes blink{0%,100%{opacity:1}50%{opacity:0}}
.msg-meta{font-size:10px;opacity:.4;margin-top:4px;}
pre{background:var(--code-bg);border-radius:6px;padding:8px 10px;overflow-x:auto;font-family:var(--mono);font-size:12px;margin:6px 0;}
code{font-family:var(--mono);font-size:12px;}
.input-area{padding:10px 12px;border-top:1px solid var(--border);display:flex;flex-direction:column;gap:8px;}
.toolbar{display:flex;gap:6px;flex-wrap:wrap;}
.tool-btn{background:rgba(255,255,255,.06);border:1px solid var(--border);color:var(--fg);border-radius:6px;padding:4px 10px;font-size:11px;cursor:pointer;transition:background .15s;}
.tool-btn:hover{background:rgba(255,255,255,.12);}
.input-row{display:flex;gap:8px;align-items:flex-end;}
textarea{flex:1;background:var(--input-bg);color:var(--input-fg);border:1px solid var(--input-border);border-radius:8px;padding:8px 10px;font-family:var(--font);font-size:13px;resize:none;outline:none;min-height:36px;max-height:120px;transition:border-color .15s;}
textarea:focus{border-color:var(--accent);}
.send-btn{background:var(--btn-bg);color:var(--btn-fg);border:none;border-radius:8px;padding:8px 14px;cursor:pointer;font-size:14px;flex-shrink:0;transition:opacity .15s;}
.send-btn:hover{opacity:.85;}
.send-btn:disabled{opacity:.4;cursor:not-allowed;}
.empty{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;opacity:.4;gap:8px;text-align:center;padding:20px;}
.empty-icon{font-size:32px;}
.empty-text{font-size:12px;line-height:1.5;}
</style>
</head>
<body>
<div class="header">
  <h1><span class="status-dot" id="dot"></span> Vox AI</h1>
  <div class="header-actions">
    <button class="icon-btn" id="planBtn" title="Planning Mode">⚡</button>
    <button class="icon-btn" id="clearBtn" title="Clear">🗑</button>
  </div>
</div>
<div class="messages" id="messages">
  <div class="empty" id="emptyState">
    <div class="empty-icon">🤖</div>
    <div class="empty-text">Ask Vox anything about your code.<br/>Use @filename to reference files.</div>
  </div>
</div>
<div class="input-area">
  <div class="toolbar">
    <button class="tool-btn" id="btnEdit">✏ Edit</button>
    <button class="tool-btn" id="btnFix">🔧 Fix</button>
    <button class="tool-btn" id="btnExplain">💡 Explain</button>
    <button class="tool-btn" id="btnPlan">📋 Plan</button>
  </div>
  <div class="input-row">
    <textarea id="chatInput" placeholder="Ask Vox or use @filename..." rows="1"></textarea>
    <button class="send-btn" id="sendBtn">→</button>
  </div>
</div>
<script nonce="${nonce}">
const vscode=acquireVsCodeApi();
const messagesEl=document.getElementById('messages');
const emptyState=document.getElementById('emptyState');
const chatInput=document.getElementById('chatInput');
const sendBtn=document.getElementById('sendBtn');
const dot=document.getElementById('dot');

function escHtml(s){return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');}
function formatContent(text){
  return text
    .replace(/\`\`\`(\\w*)\\n([\\s\\S]*?)\`\`\`/g,(_,l,c)=>\`<pre><code>\${escHtml(c.trim())}</code></pre>\`)
    .replace(/\`([^\`]+)\`/g,(_,c)=>\`<code>\${escHtml(c)}</code>\`)
    .replace(/\\*\\*([^*]+)\\*\\*/g,'<strong>$1</strong>')
    .replace(/\\n/g,'<br/>');
}
function renderMessages(history){
  emptyState.style.display=history.length===0?'flex':'none';
  while(messagesEl.children.length>1)messagesEl.removeChild(messagesEl.lastChild);
  for(const msg of history){
    const div=document.createElement('div');
    div.className='message '+msg.role+(msg.is_streaming?' streaming':'');
    div.innerHTML=formatContent(msg.content||'');
    if(msg.model_used||msg.tokens){
      const meta=document.createElement('div');
      meta.className='msg-meta';
      meta.textContent=(msg.model_used||'')+(msg.tokens?' ('+msg.tokens.toLocaleString()+' tok)':'');
      div.appendChild(meta);
    }
    messagesEl.appendChild(div);
  }
  messagesEl.scrollTop=messagesEl.scrollHeight;
}
function sendMessage(){
  const text=chatInput.value.trim();
  if(!text)return;
  chatInput.value='';
  chatInput.style.height='auto';
  sendBtn.disabled=true;
  dot.style.background='#ff9800';
  vscode.postMessage({type:'submitChat',value:text});
}
sendBtn.onclick=sendMessage;
chatInput.addEventListener('keydown',e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();sendMessage();}});
chatInput.addEventListener('input',()=>{chatInput.style.height='auto';chatInput.style.height=Math.min(chatInput.scrollHeight,120)+'px';});
document.getElementById('clearBtn').onclick=()=>vscode.postMessage({type:'clearChat'});
document.getElementById('planBtn').onclick=()=>vscode.postMessage({type:'plan'});
document.getElementById('btnPlan').onclick=()=>vscode.postMessage({type:'plan'});
document.getElementById('btnEdit').onclick=()=>vscode.postMessage({type:'command',value:'vox.inlineEdit'});
document.getElementById('btnFix').onclick=()=>vscode.postMessage({type:'command',value:'vox.inlineFix'});
document.getElementById('btnExplain').onclick=()=>vscode.postMessage({type:'command',value:'vox.inlineExplain'});
window.addEventListener('message',ev=>{
  const msg=ev.data;
  if(msg.type==='chatHistory'){renderMessages(msg.value||[]);sendBtn.disabled=false;dot.style.background='#4caf50';}
});
</script>
</body>
</html>`;
    }
}

function getNonce(): string {
    let text = '';
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    for (let i = 0; i < 32; i++) text += chars.charAt(Math.floor(Math.random() * chars.length));
    return text;
}
