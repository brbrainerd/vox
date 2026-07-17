// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { BackendAvailability, type ProviderStatus } from './BackendAvailability';

const statuses: ProviderStatus[] = [
  { provider: 'OpenRouter', key_present: true, is_local: false, local_reachable: null, local_models: [] },
  { provider: 'Anthropic', key_present: false, is_local: false, local_reachable: null, local_models: [] },
  { provider: 'Ollama', key_present: true, is_local: true, local_reachable: false, local_models: [] },
];

describe('BackendAvailability', () => {
  it('renders one row per backend with key and reachability state', () => {
    render(<BackendAvailability statuses={statuses} />);
    expect(screen.getByRole('listitem', { name: /OpenRouter/i })).toHaveTextContent(/key/i);
    expect(screen.getByRole('listitem', { name: /Anthropic/i })).toHaveTextContent(/no key/i);
    expect(screen.getByRole('listitem', { name: /Ollama/i })).toHaveTextContent(/offline/i);
  });

  it('renders nothing for an empty status list', () => {
    const { container } = render(<BackendAvailability statuses={[]} />);
    expect(container.firstChild).toBeNull();
  });
});
