import React from 'react';
import { createRoot } from 'react-dom/client';
import { Dashboard } from './dashboard';

const container = document.getElementById('root');
if (container) {
  const root = createRoot(container);
  root.render(<Dashboard />);
}
