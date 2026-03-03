import React, { useState, useEffect } from "react";

interface Snippet {
  id: number;
  language: string;
  title: string;
  code: string;
  description: string | null;
  tags: string | null;
}

export function SnippetArchive() {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Form state
  const [showForm, setShowForm] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [newLang, setNewLang] = useState("vox");
  const [newCode, setNewCode] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [newTags, setNewTags] = useState("");

  useEffect(() => {
    fetchSnippets();
  }, []);

  async function fetchSnippets(query?: string) {
    try {
      setLoading(true);
      const url = query
        ? `/api/snippets?q=${encodeURIComponent(query)}`
        : "/api/snippets";
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setSnippets(data.snippets || []);
      setError(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }

  async function handleSave() {
    if (!newTitle.trim() || !newCode.trim()) return;
    try {
      const res = await fetch("/api/snippets", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          language: newLang,
          title: newTitle,
          code: newCode,
          description: newDesc || null,
          tags: newTags ? JSON.stringify(newTags.split(",").map((t: string) => t.trim())) : null,
        }),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setShowForm(false);
      setNewTitle("");
      setNewCode("");
      setNewDesc("");
      setNewTags("");
      await fetchSnippets();
    } catch (err: any) {
      setError(err.message);
    }
  }

  function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    fetchSnippets(search || undefined);
  }

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>Snippet Archive</h2>
        <button className="btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? "Cancel" : "Save New Snippet"}
        </button>
      </div>

      {showForm && (
        <div className="form-section">
          <label>
            Title
            <input type="text" value={newTitle} onChange={(e) => setNewTitle(e.target.value)} placeholder="My useful snippet" />
          </label>
          <label>
            Language
            <input type="text" value={newLang} onChange={(e) => setNewLang(e.target.value)} placeholder="vox" />
          </label>
          <label>
            Code
            <textarea value={newCode} onChange={(e) => setNewCode(e.target.value)} placeholder="fn example():" rows={6} />
          </label>
          <label>
            Description
            <input type="text" value={newDesc} onChange={(e) => setNewDesc(e.target.value)} placeholder="What this snippet does" />
          </label>
          <label>
            Tags (comma-separated)
            <input type="text" value={newTags} onChange={(e) => setNewTags(e.target.value)} placeholder="utility, math, parsing" />
          </label>
          <button className="btn-primary" onClick={handleSave}>Save</button>
        </div>
      )}

      <form onSubmit={handleSearch} className="search-form">
        <input
          type="text"
          placeholder="Search snippets..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="search-input"
        />
        <button type="submit" className="btn-secondary">Search</button>
      </form>

      {loading && <p className="loading">Loading snippets...</p>}
      {error && <p className="error">⚠ {error}</p>}
      {!loading && snippets.length === 0 && (
        <p className="empty">No snippets found.</p>
      )}

      <ul className="item-list">
        {snippets.map((s) => (
          <li key={s.id} className="item-card snippet-card">
            <div className="item-header">
              <strong>{s.title}</strong>
              <span className="badge">{s.language}</span>
            </div>
            {s.description && <p className="item-desc">{s.description}</p>}
            <pre className="code-block"><code>{s.code}</code></pre>
            {s.tags && (
              <div className="tags">
                {JSON.parse(s.tags).map((tag: string) => (
                  <span key={tag} className="tag">{tag}</span>
                ))}
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
