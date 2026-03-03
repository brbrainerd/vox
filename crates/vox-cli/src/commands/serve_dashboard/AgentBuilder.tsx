import React, { useState, useEffect } from "react";

interface Agent {
  id: string;
  name: string;
  description: string | null;
  system_prompt: string | null;
  tools: string | null;
  version: string;
  is_public: boolean;
}

export function AgentBuilder() {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Form state for creating a new agent
  const [agentName, setAgentName] = useState("");
  const [systemPrompt, setSystemPrompt] = useState("");
  const [tools, setTools] = useState("");
  const [testQuery, setTestQuery] = useState("");
  const [testResponse, setTestResponse] = useState("");

  useEffect(() => {
    fetchAgents();
  }, []);

  async function fetchAgents() {
    try {
      setLoading(true);
      const res = await fetch("/api/agents");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setAgents(data.agents || []);
      setError(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }

  async function handleSave() {
    if (!agentName.trim()) return;
    try {
      const res = await fetch("/api/agents", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          id: `agent-${Date.now()}`,
          name: agentName,
          description: null,
          system_prompt: systemPrompt || null,
          tools: tools || null,
          model_config: null,
          version: "0.1.0",
          is_public: false,
        }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      await fetchAgents(); // Refresh list
      setAgentName("");
      setSystemPrompt("");
      setTools("");
    } catch (err: any) {
      setError(err.message);
    }
  }

  return (
    <div className="panel">
      <h2>Agent Builder</h2>
      {error && <p className="error">⚠ {error}</p>}

      <div className="form-section">
        <h3>Create / Edit Agent</h3>
        <label>
          Name
          <input
            type="text"
            value={agentName}
            onChange={(e) => setAgentName(e.target.value)}
            placeholder="my-agent"
          />
        </label>
        <label>
          System Prompt
          <textarea
            value={systemPrompt}
            onChange={(e) => setSystemPrompt(e.target.value)}
            placeholder="You are a helpful assistant..."
            rows={4}
          />
        </label>
        <label>
          Tools (JSON array)
          <input
            type="text"
            value={tools}
            onChange={(e) => setTools(e.target.value)}
            placeholder='["search_kb", "run_sql"]'
          />
        </label>
        <button className="btn-primary" onClick={handleSave}>
          Save Agent
        </button>
      </div>

      <div className="form-section">
        <h3>Test Agent</h3>
        <input
          type="text"
          placeholder="Ask a test question..."
          value={testQuery}
          onChange={(e) => setTestQuery(e.target.value)}
        />
        <button className="btn-secondary" onClick={() => setTestResponse("Agent response will appear here (not yet connected to Populi)")}>
          Send
        </button>
        {testResponse && <pre className="test-response">{testResponse}</pre>}
      </div>

      <div className="form-section">
        <h3>Existing Agents</h3>
        {loading && <p className="loading">Loading agents...</p>}
        <ul className="item-list">
          {agents.map((a) => (
            <li key={a.id} className="item-card">
              <div className="item-header">
                <strong>{a.name}</strong>
                <span className={`badge ${a.is_public ? "public" : "private"}`}>
                  {a.is_public ? "public" : "private"}
                </span>
              </div>
              <p className="item-desc">{a.description || "No description"}</p>
              <div className="item-footer">
                <span className="version">v{a.version}</span>
                <button className="btn-secondary" onClick={() => {
                  setAgentName(a.name);
                  setSystemPrompt(a.system_prompt || "");
                  setTools(a.tools || "");
                }}>
                  Edit
                </button>
              </div>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
