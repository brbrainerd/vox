/// Embedded templates for scaffolding a complete web application.
/// These are baked into the compiler binary so no external files are needed.

pub fn index_html() -> &'static str {
r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Vox App</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet">
</head>
<body>
  <div id="root"></div>
  <script type="module" src="/src/main.tsx"></script>
</body>
</html>
"#
}

pub fn main_tsx(component_name: &str) -> String {
    format!(
r#"import React from "react";
import ReactDOM from "react-dom/client";
import {{ {component_name} }} from "./generated/{component_name}";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <{component_name} />
  </React.StrictMode>
);
"#
    )
}

pub fn index_css() -> &'static str {
r#"/* Vox Generated App — Dark Theme Design System */
:root {
  --bg-primary: #0f1117;
  --bg-secondary: #1a1d27;
  --bg-tertiary: #252836;
  --bg-accent: #2d3142;
  --text-primary: #e8eaf0;
  --text-secondary: #9ca3b4;
  --text-muted: #6b7280;
  --accent: #6366f1;
  --accent-hover: #818cf8;
  --accent-glow: rgba(99, 102, 241, 0.25);
  --success: #34d399;
  --error: #f87171;
  --border: #2e3244;
  --border-focus: #6366f1;
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --shadow-sm: 0 1px 3px rgba(0,0,0,0.3);
  --shadow-md: 0 4px 12px rgba(0,0,0,0.4);
  --shadow-lg: 0 8px 32px rgba(0,0,0,0.5);
  --font: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --transition: 200ms cubic-bezier(0.4, 0, 0.2, 1);
}

*, *::before, *::after {
  margin: 0; padding: 0; box-sizing: border-box;
}

html, body, #root {
  height: 100%; width: 100%;
  font-family: var(--font);
  background: var(--bg-primary);
  color: var(--text-primary);
  -webkit-font-smoothing: antialiased;
}

/* === Chat Layout === */
.chat-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  max-width: 800px;
  margin: 0 auto;
  background: var(--bg-primary);
}

.chat-container h1 {
  text-align: center;
  padding: 20px;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
  color: var(--text-primary);
  border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, var(--bg-secondary) 0%, var(--bg-primary) 100%);
}

/* === Messages Area === */
.messages {
  flex: 1;
  overflow-y: auto;
  padding: 24px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  scroll-behavior: smooth;
}

.messages::-webkit-scrollbar { width: 6px; }
.messages::-webkit-scrollbar-track { background: transparent; }
.messages::-webkit-scrollbar-thumb {
  background: var(--bg-accent);
  border-radius: 3px;
}

.message {
  max-width: 75%;
  padding: 12px 16px;
  border-radius: var(--radius-md);
  font-size: 0.95rem;
  line-height: 1.5;
  animation: messageIn 300ms ease-out;
  box-shadow: var(--shadow-sm);
  word-wrap: break-word;
}

.message.user {
  align-self: flex-end;
  background: linear-gradient(135deg, var(--accent) 0%, #4f46e5 100%);
  color: #fff;
  border-bottom-right-radius: var(--radius-sm);
}

.message.ai, .message.assistant {
  align-self: flex-start;
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border);
  border-bottom-left-radius: var(--radius-sm);
}

.message.error {
  align-self: flex-start;
  background: rgba(248, 113, 113, 0.1);
  color: var(--error);
  border: 1px solid rgba(248, 113, 113, 0.3);
}

@keyframes messageIn {
  from { opacity: 0; transform: translateY(8px); }
  to   { opacity: 1; transform: translateY(0); }
}

/* === Input Area === */
.input-area {
  display: flex;
  gap: 10px;
  padding: 16px;
  border-top: 1px solid var(--border);
  background: var(--bg-secondary);
}

.chat-input {
  flex: 1;
  padding: 12px 16px;
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  font-family: var(--font);
  font-size: 0.95rem;
  outline: none;
  transition: border-color var(--transition), box-shadow var(--transition);
}

.chat-input:focus {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 3px var(--accent-glow);
}

.chat-input::placeholder { color: var(--text-muted); }

.send-btn {
  padding: 12px 24px;
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: var(--radius-md);
  font-family: var(--font);
  font-size: 0.95rem;
  font-weight: 600;
  cursor: pointer;
  transition: background var(--transition), transform var(--transition), box-shadow var(--transition);
}

.send-btn:hover {
  background: var(--accent-hover);
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
}

.send-btn:active {
  transform: translateY(0);
  box-shadow: var(--shadow-sm);
}

/* === Responsive === */
@media (max-width: 640px) {
  .chat-container { max-width: 100%; }
  .message { max-width: 85%; }
  .chat-container h1 { font-size: 1.25rem; padding: 16px; }
}
"#
}

pub fn package_json() -> &'static str {
r#"{
  "name": "vox-generated-app",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.7.0",
    "vite": "^6.0.0"
  }
}
"#
}

pub fn vite_config(backend_port: u16) -> String {
    format!(
r#"import {{ defineConfig }} from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({{
  plugins: [react()],
  server: {{
    proxy: {{
      "/api": {{
        target: "http://127.0.0.1:{backend_port}",
        changeOrigin: true,
      }},
    }},
  }},
  build: {{
    outDir: "dist",
  }},
}});
"#
    )
}

pub fn tsconfig_json() -> &'static str {
r#"{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
"#
}
