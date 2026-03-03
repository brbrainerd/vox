import React, { useState, useEffect } from "react";

interface FeedbackItem {
  prompt: string;
  response: string;
  rating: number | null;
  feedback_type: string;
}

export function FeedbackPanel() {
  const [feedback, setFeedback] = useState<FeedbackItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchFeedback();
  }, []);

  async function fetchFeedback() {
    try {
      setLoading(true);
      const res = await fetch("/api/feedback");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setFeedback(data.training_pairs || []);
      setError(null);
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }

  async function exportJsonl() {
    try {
      const jsonl = feedback
        .map((f) => JSON.stringify(f))
        .join("\n");
      const blob = new Blob([jsonl], { type: "application/x-ndjson" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "feedback_export.jsonl";
      a.click();
      URL.revokeObjectURL(url);
    } catch (err: any) {
      setError(err.message);
    }
  }

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>Feedback Panel</h2>
        <button className="btn-primary" onClick={exportJsonl}>
          Export JSONL
        </button>
      </div>

      {loading && <p className="loading">Loading feedback...</p>}
      {error && <p className="error">⚠ {error}</p>}
      {!loading && feedback.length === 0 && (
        <p className="empty">No feedback interactions recorded yet.</p>
      )}

      <table className="feedback-table">
        <thead>
          <tr>
            <th>Prompt</th>
            <th>Response (excerpt)</th>
            <th>Type</th>
            <th>Rating</th>
          </tr>
        </thead>
        <tbody>
          {feedback.map((f, i) => (
            <tr key={i}>
              <td className="prompt-cell" title={f.prompt}>
                {f.prompt.length > 80 ? f.prompt.slice(0, 80) + "…" : f.prompt}
              </td>
              <td className="response-cell" title={f.response}>
                {f.response.length > 80 ? f.response.slice(0, 80) + "…" : f.response}
              </td>
              <td>
                <span className={`badge ${f.feedback_type}`}>{f.feedback_type}</span>
              </td>
              <td>
                {f.rating !== null ? (
                  <span className={f.rating >= 3 ? "rating-good" : "rating-bad"}>
                    {f.rating}/5
                  </span>
                ) : (
                  <span className="rating-none">—</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
