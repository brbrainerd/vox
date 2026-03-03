import React, { useState, useEffect } from "react";

interface MarketplaceItem {
  id: string;
  name: string;
  artifact_type: string;
  description: string | null;
  version: string;
  downloads: number;
  avg_rating: number;
  author_id: string;
}

export function Marketplace() {
  const [artifacts, setArtifacts] = useState<MarketplaceItem[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchArtifacts();
  }, []);

  async function fetchArtifacts(query?: string) {
    try {
      setLoading(true);
      const url = query
        ? `/api/marketplace?q=${encodeURIComponent(query)}`
        : "/api/marketplace";
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setArtifacts(data.artifacts || []);
      setError(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }

  function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    fetchArtifacts(search || undefined);
  }

  function renderStars(rating: number): string {
    const full = Math.floor(rating);
    const half = rating - full >= 0.5 ? "½" : "";
    return "★".repeat(full) + half + "☆".repeat(5 - Math.ceil(rating));
  }

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>Marketplace</h2>
        <button className="btn-primary" onClick={() => alert("Use `vox publish` to publish artifacts")}>
          Publish Artifact
        </button>
      </div>

      <form onSubmit={handleSearch} className="search-form">
        <input
          type="text"
          placeholder="Search marketplace..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="search-input"
        />
        <button type="submit" className="btn-secondary">Search</button>
      </form>

      {loading && <p className="loading">Loading marketplace...</p>}
      {error && <p className="error">⚠ {error}</p>}
      {!loading && artifacts.length === 0 && (
        <p className="empty">No published artifacts yet.</p>
      )}

      <ul className="item-list">
        {artifacts.map((a) => (
          <li key={a.id} className="item-card">
            <div className="item-header">
              <strong>{a.name}</strong>
              <span className="badge">{a.artifact_type}</span>
            </div>
            <p className="item-desc">{a.description || "No description"}</p>
            <div className="item-footer">
              <span className="version">v{a.version}</span>
              <span className="rating">{renderStars(a.avg_rating)}</span>
              <span className="downloads">{a.downloads} downloads</span>
              <span className="author">by {a.author_id}</span>
              <button className="btn-primary" onClick={() => alert(`Install: vox add ${a.name}`)}>
                Review & Install
              </button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
