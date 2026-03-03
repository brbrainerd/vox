import React, { useState, useEffect } from "react";

interface Workflow {
  id: string;
  name: string;
  description: string | null;
  version: string;
  status: string;
}

export function WorkflowBrowser() {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchWorkflows();
  }, []);

  async function fetchWorkflows() {
    try {
      setLoading(true);
      const res = await fetch("/api/workflows");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setWorkflows(data.workflows || []);
      setError(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }

  const filtered = workflows.filter(
    (w) =>
      w.name.toLowerCase().includes(search.toLowerCase()) ||
      (w.description || "").toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="panel">
      <h2>Workflow Browser</h2>
      <input
        type="text"
        placeholder="Search workflows..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="search-input"
      />
      {loading && <p className="loading">Loading workflows...</p>}
      {error && <p className="error">⚠ {error}</p>}
      {!loading && filtered.length === 0 && (
        <p className="empty">No workflows found.</p>
      )}
      <ul className="item-list">
        {filtered.map((w) => (
          <li key={w.id} className="item-card">
            <div className="item-header">
              <strong>{w.name}</strong>
              <span className={`badge ${w.status}`}>{w.status}</span>
            </div>
            <p className="item-desc">{w.description || "No description"}</p>
            <div className="item-footer">
              <span className="version">v{w.version}</span>
              <button className="btn-secondary">View Diagram</button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
