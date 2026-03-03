import React, { useState, useEffect } from "react";

interface Skill {
  id: string;
  name: string;
  description: string | null;
  version: string;
  downloads: number;
  tags: string | null;
}

export function SkillBrowser() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchSkills();
  }, []);

  async function fetchSkills() {
    try {
      setLoading(true);
      const res = await fetch("/api/skills");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setSkills(data.skills || []);
      setError(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }

  const filtered = skills.filter(
    (s) =>
      s.name.toLowerCase().includes(search.toLowerCase()) ||
      (s.description || "").toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="panel">
      <h2>Skill Browser</h2>
      <input
        type="text"
        placeholder="Search skills..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="search-input"
      />
      {loading && <p className="loading">Loading skills...</p>}
      {error && <p className="error">⚠ {error}</p>}
      {!loading && filtered.length === 0 && (
        <p className="empty">No skills found.</p>
      )}
      <ul className="item-list">
        {filtered.map((s) => (
          <li key={s.id} className="item-card">
            <div className="item-header">
              <strong>{s.name}</strong>
              <span className="downloads">{s.downloads} installs</span>
            </div>
            <p className="item-desc">{s.description || "No description"}</p>
            <div className="item-footer">
              <span className="version">v{s.version}</span>
              {s.tags && (
                <div className="tags">
                  {JSON.parse(s.tags).map((tag: string) => (
                    <span key={tag} className="tag">{tag}</span>
                  ))}
                </div>
              )}
              <button className="btn-primary" onClick={() => alert(`Install: vox add ${s.name}`)}>
                Install
              </button>
              <button className="btn-secondary">View Examples</button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
