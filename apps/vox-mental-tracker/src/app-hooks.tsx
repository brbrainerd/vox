// App-owned web bootstrap hooks, consumed by the Vox-emitted `dist/entry.tsx`.
//
// The emitted entry mounts <VoxApp/> generically; everything app-specific that
// can't be expressed in `.vox` (a React error boundary, PWA service-worker
// registration) lives here so the app keeps ownership of it without
// re-introducing a hand-written bootstrap.
//
//   wrapApp(app) — wrap the Vox app root (here: crash-logging error boundary).
//   onBoot()     — one-time startup side effects (here: register the service worker).
import React from "react";
import { ErrorBoundary } from "./ErrorBoundary";
import { registerServiceWorker } from "./sync";

export function wrapApp(app: React.ReactElement): React.ReactElement {
  return <ErrorBoundary>{app}</ErrorBoundary>;
}

export function onBoot(): void {
  void registerServiceWorker();
}
