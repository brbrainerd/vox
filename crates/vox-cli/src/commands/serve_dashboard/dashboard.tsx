import React, { useState } from "react";
import { WorkflowBrowser } from "./WorkflowBrowser";
import { SkillBrowser } from "./SkillBrowser";
import { AgentBuilder } from "./AgentBuilder";
import { SnippetArchive } from "./SnippetArchive";
import { Marketplace } from "./Marketplace";
import { FeedbackPanel } from "./FeedbackPanel";
import "./index.css";

export function Dashboard() {
  const [activeTab, setActiveTab] = useState("workflows");

  const tabs = [
    { id: "workflows", label: "Workflows" },
    { id: "skills", label: "Skills" },
    { id: "agents", label: "Agents" },
    { id: "snippets", label: "Snippets" },
    { id: "marketplace", label: "Marketplace" },
    { id: "feedback", label: "Feedback" },
  ];

  return (
    <div className="dashboard-container">
      <aside className="sidebar">
        <div className="sidebar-header">
          <h2>Vox Dashboard</h2>
        </div>
        <nav className="sidebar-nav">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              className={`nav-button ${activeTab === tab.id ? "active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </nav>
      </aside>
      <main className="dashboard-content">
        {activeTab === "workflows" && <WorkflowBrowser />}
        {activeTab === "skills" && <SkillBrowser />}
        {activeTab === "agents" && <AgentBuilder />}
        {activeTab === "snippets" && <SnippetArchive />}
        {activeTab === "marketplace" && <Marketplace />}
        {activeTab === "feedback" && <FeedbackPanel />}
      </main>
    </div>
  );
}
