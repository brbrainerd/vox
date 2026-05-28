import React from "react";
import ReactDOM from "react-dom/client";
import { VoxManifestApp } from "./vox-manifest-router";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <VoxManifestApp />
  </React.StrictMode>
);
