import React, { useState, useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  VSCodeButton,
  VSCodePanelView,
  VSCodePanelTab,
  VSCodePanels,
  VSCodeBadge,
  VSCodeTextArea
} from "@vscode/webview-ui-toolkit/react";

//@ts-ignore
const vscode = acquireVsCodeApi();

interface ProviderStatus {
  provider: string;
  model: string;
  configured: boolean;
  calls_used: number;
  daily_limit: number;
  remaining: number;
}

interface VoxStatus {
  providers: ProviderStatus[];
  cost_today_usd: number;
}

function AIProviderDashboard({ status }: { status: VoxStatus | null }) {
  if (!status) return (
     <div style={{ padding: '10px', opacity: 0.5, fontSize: '0.9em' }}>
       $(sync~spin) Detecting AI providers...
     </div>
  );

  return (
    <div style={{
      border: '1px solid var(--vscode-widget-border)',
      padding: '12px',
      marginBottom: '15px',
      borderRadius: '6px',
      backgroundColor: 'var(--vscode-sideBar-background)',
      boxShadow: '0 2px 4px rgba(0,0,0,0.1)'
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
        <span style={{ fontWeight: 600, fontSize: '0.95em' }}>AI PROVIDERS</span>
        <VSCodeButton appearance="icon" title="Change Model" onClick={() => vscode.postMessage({ type: 'pickModel' })}>
           <span className="codicon codicon-settings-gear"></span>
        </VSCodeButton>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
        {status.providers.map(p => {
          if (!p.configured && p.provider !== 'ollama') return null;
          const pct = p.daily_limit > 0 ? (p.calls_used / p.daily_limit) * 100 : 0;
          const color = p.configured ? (pct > 90 ? 'var(--vscode-charts-red)' : 'var(--vscode-charts-green)') : 'var(--vscode-disabledForeground)';
          const remStr = p.remaining === -1 ? '∞' : `${p.remaining}/${p.daily_limit}`;

          return (
            <div key={p.provider + p.model} style={{ fontSize: '0.85em' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px' }}>
                <span style={{ opacity: p.configured ? 1 : 0.5 }}>
                  {p.provider === 'google' ? '★ ' :  p.provider === 'ollama' ? '⬡ ' : '☁ ' }
                  {p.model.replace('gemini-', '').replace('models', '')}
                </span>
                <span style={{ fontWeight: '500' }}>{remStr}</span>
              </div>
              {p.daily_limit > 0 && (
                <div style={{ height: '3px', background: 'var(--vscode-widget-border)', borderRadius: '2px', overflow: 'hidden' }}>
                  <div style={{ width: `${pct}%`, height: '100%', background: color, transition: 'width 0.3s' }}></div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {status.cost_today_usd > 0 && (
         <div style={{ marginTop: '10px', paddingTop: '8px', borderTop: '1px solid var(--vscode-widget-border)', fontSize: '0.8em', opacity: 0.7, textAlign: 'right' }}>
            Cost today: ${status.cost_today_usd.toFixed(4)}
         </div>
      )}
    </div>
  );
}

function App() {
  const [messages, setMessages] = useState<{ role: string, content: string }[]>([]);
  const [input, setInput] = useState("");
  const [agentEvents, setAgentEvents] = useState<any[]>([]);
  const [voxStatus, setVoxStatus] = useState<VoxStatus | null>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    window.addEventListener('message', event => {
      const message = event.data;
      if (message.type === 'taskResult') {
        const textRender = typeof message.value === 'string' ? message.value : JSON.stringify(message.value, null, 2);
        setMessages(prev => [...prev, { role: 'assistant', content: "Result: " + textRender }]);
      } else if (message.type === 'voxStatus') {
        setVoxStatus(message.value);
      } else if (message.type === 'agentEvent') {
        if (message.value.event_type === 'TokenStreamed') {
          let text = "";
          try {
             const payload = JSON.parse(message.value.payload || "{}");
             text = payload.TokenStreamed?.text || payload.text || "";
          } catch (e) {}
          if (text) {
              setMessages(prev => {
                 const newMsgs = [...prev];
                 if (newMsgs.length > 0 && newMsgs[newMsgs.length - 1].role === 'assistant') {
                     newMsgs[newMsgs.length - 1].content += text;
                 } else {
                     newMsgs.push({ role: 'assistant', content: text });
                 }
                 return newMsgs;
              });
          }
        } else {
          setAgentEvents(prev => [...prev.slice(-49), message.value]); // keep last 50 events
        }
      }
    });
  }, []);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSubmit = () => {
    if (!input) return;
    setMessages(prev => [...prev, { role: 'user', content: input }]);
    vscode.postMessage({ type: 'submitTask', value: input });
    setInput("");
  };

  return (
    <div style={{ padding: "0" }}>
      <VSCodePanels>
        <VSCodePanelTab id="tab-chat">CHAT</VSCodePanelTab>
        <VSCodePanelTab id="tab-agents">AGENTS <VSCodeBadge>3</VSCodeBadge></VSCodePanelTab>

        <VSCodePanelView id="view-chat" style={{ padding: '1rem', flexDirection: 'column' }}>
          <h3>Vox Chat (MCP)</h3>

          <div style={{ flex: 1, overflowY: 'auto', marginBottom: '10px', minHeight: '200px' }}>
            {messages.map((m, i) => (
               <div key={i} style={{
                 marginBottom: "12px",
                 padding: "8px",
                 borderRadius: "4px",
                 backgroundColor: m.role === 'user' ? 'var(--vscode-editor-selectionBackground)' : 'transparent',
                 color: 'var(--vscode-editor-foreground)'
               }}>
                 <div style={{ fontWeight: 'bold', marginBottom: '4px', color: m.role === 'user' ? 'var(--vscode-charts-blue)' : 'var(--vscode-charts-green)' }}>
                   {m.role === 'user' ? 'You' : 'Vox Agent'}
                 </div>
                 <ReactMarkdown
                    remarkPlugins={[remarkGfm]}
                    components={{
                      code({node, inline, className, children, ...props}: any) {
                        const match = /language-(\w+)/.exec(className || '')
                        const isCodeBlock = !inline && match;

                        return isCodeBlock ? (
                          <div style={{ position: 'relative', marginTop: '10px', marginBottom: '10px' }}>
                            <div style={{ display: 'flex', justifyContent: 'space-between', background: 'var(--vscode-editor-background)', padding: '4px 8px', fontSize: '0.8em', borderTopLeftRadius: '4px', borderTopRightRadius: '4px' }}>
                              <span>{match[1]}</span>
                              <VSCodeButton
                                appearance="icon"
                                onClick={() => {
                                  // extract file path from first line if possible, else prompt or use default
                                  const text = String(children).replace(/\n$/, '');
                                  const firstLine = text.split('\n')[0];
                                  let path = "src/main.vox";
                                  if (firstLine.startsWith('// ') || firstLine.startsWith('# ')) {
                                      path = firstLine.substring(3).trim();
                                  }
                                  vscode.postMessage({ type: 'applyChanges', value: { path, content: text }});
                                }}
                              >
                                <span className="codicon codicon-check">Apply File</span>
                              </VSCodeButton>
                            </div>
                            <pre style={{ margin: 0, padding: '10px', background: 'var(--vscode-editor-inactiveSelectionBackground)', overflowX: 'auto' }}>
                              <code className={className} {...props}>
                                {children}
                              </code>
                            </pre>
                          </div>
                        ) : (
                          <code className={className} style={{background: 'var(--vscode-editor-inactiveSelectionBackground)', padding: '2px 4px', borderRadius: '4px'}} {...props}>
                            {children}
                          </code>
                        )
                      }
                    }}
                 >{m.content}</ReactMarkdown>
               </div>
            ))}
            <div ref={chatEndRef} />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
             <VSCodeTextArea
                value={input}
                onInput={(e: any) => setInput(e.target.value)}
                placeholder="Message Vox or an @agent..."
                rows={3}
                style={{ width: '100%' }}
             />
             <VSCodeButton onClick={handleSubmit}>Send</VSCodeButton>
          </div>
        </VSCodePanelView>

        <VSCodePanelView id="view-agents" style={{ padding: '1rem', flexDirection: 'column' }}>
          <h3>Mission Control</h3>

          <AIProviderDashboard status={voxStatus} />

          <div style={{ marginBottom: '20px' }}>
            <h4>Active Agent Grid</h4>
            <div style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
              gap: '10px'
            }}>
              {Object.entries(
                agentEvents.reduce((acc: any, ev) => {
                  acc[ev.agent_id || 'system'] = ev;
                  return acc;
                }, {})
              ).map(([id, lastEv]: [string, any]) => (
                <div key={id} style={{
                  border: '1px solid var(--vscode-widget-border)',
                  padding: '10px',
                  borderRadius: '4px',
                  backgroundColor: 'var(--vscode-editor-background)'
                }}>
                  <div style={{ fontWeight: 'bold' }}>{id}</div>
                  <div style={{ fontSize: '0.8em', opacity: 0.7 }}>
                    Status: {lastEv.event_type || 'Idle'}
                  </div>
                  <VSCodeBadge style={{ marginTop: '4px' }}>
                    {new Date(lastEv.timestamp || Date.now()).toLocaleTimeString()}
                  </VSCodeBadge>
                </div>
              ))}
            </div>
          </div>

          <h4>Event Timeline</h4>
          <div style={{ flex: 1, overflowY: 'auto' }}>
            {agentEvents.slice().reverse().map((ev, i) => (
              <div key={i} style={{
                border: '1px solid var(--vscode-widget-border)',
                padding: '10px',
                marginBottom: '10px',
                borderRadius: '4px',
                fontSize: '0.9em'
              }}>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <strong>Agent: {ev.agent_id || 'System'}</strong>
                  <span style={{ color: 'var(--vscode-charts-green)' }}>{new Date(ev.timestamp || Date.now()).toLocaleTimeString()}</span>
                </div>
                <div style={{ marginTop: '4px', display: 'flex', gap: '8px', alignItems: 'center' }}>
                    <VSCodeBadge>{ev.event_type || ev.type || 'Event'}</VSCodeBadge>
                    <span style={{ opacity: 0.8 }} title={ev.payload || ev.message || JSON.stringify(ev.data)}>
                       {(ev.payload || ev.message || JSON.stringify(ev.data))?.toString().substring(0, 100)}...
                    </span>
                </div>
              </div>
            ))}
            {agentEvents.length === 0 && (
               <p style={{ opacity: 0.5 }}>Waiting for agent activity...</p>
            )}
          </div>
        </VSCodePanelView>
      </VSCodePanels>
    </div>
  );
}

const rootElement = document.getElementById("root");
if (rootElement) {
  createRoot(rootElement).render(<App />);
}
