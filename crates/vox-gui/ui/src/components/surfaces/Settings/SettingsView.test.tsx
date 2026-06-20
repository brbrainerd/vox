// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockSetGuiPreference = vi.fn().mockResolvedValue(undefined);

const SECRET_ROW = {
  id: 'db-token',
  canonicalEnv: 'VOX_DB_TOKEN',
  scopeDescription: 'Turso database token',
  taxonomySlug: 'database',
  authRegistry: null,
  required: true,
  isPresent: false,
  status: 'missing',
  redacted: '',
  source: null,
  remediation: 'Set via vault',
};

const invokeMock = vi.fn((cmd: string, args?: { key?: string }) => {
  if (cmd === 'get_orchestrator_config') return Promise.resolve({});
  if (cmd === 'list_secret_status') return Promise.resolve([SECRET_ROW]);
  if (cmd === 'secrets_backend_status') {
    return Promise.resolve({
      backendMode: 'vault',
      profile: 'dev',
      strict: false,
      available: true,
      detail: null,
    });
  }
  if (cmd === 'set_secret') return Promise.resolve(true);
  if (cmd === 'get_telemetry_consent') {
    return Promise.resolve({ state: 'denied', remoteAllowed: false, masterEnabled: true, installId: 'inst-abc123' });
  }
  if (cmd === 'set_telemetry_consent') {
    return Promise.resolve({ state: 'granted', remoteAllowed: true, masterEnabled: true, installId: 'inst-abc123' });
  }
  if (cmd === 'signing_key_status') {
    return Promise.resolve({
      nodeId: 'node-abc',
      algorithm: 'ed25519',
      fingerprint: 'fp-deadbeef',
      pubkeyHex: '00',
      present: true,
    });
  }
  if (cmd === 'rotate_signing_key') {
    return Promise.resolve({
      nodeId: 'node-abc',
      algorithm: 'ed25519',
      fingerprint: 'fp-cafebabe',
      pubkeyHex: '01',
      present: true,
    });
  }
  return Promise.resolve(null);
});

// Mock Tauri invoke — SettingsView fires multiple invoke() calls on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args as { key?: string }),
}));

const recordGamifyMock = vi.fn().mockResolvedValue(null);
vi.mock('../../../lib/gamifyGuiEvents', () => ({
  recordGamifyGuiEvent: (...args: unknown[]) => recordGamifyMock(...args),
}));

// Mock Tauri event listen — transport.ts calls listen() on import.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock voxTransport — SettingsView calls getRoutingSummaryLive on mount.
vi.mock('../../../transport', () => ({
  voxTransport: {
    getRoutingSummaryLive: vi.fn().mockResolvedValue({ routing_priority: null }),
    setRoutingPriority: vi.fn().mockResolvedValue(undefined),
    getGuiPreference: vi.fn().mockResolvedValue(null),
    get setGuiPreference() {
      return mockSetGuiPreference;
    },
  },
}));

// Mock PriorityChainEditor — it makes its own invoke calls.
vi.mock('./PriorityChainEditor', () => ({
  PriorityChainEditor: () => <div data-testid="priority-chain-editor" />,
}));

import { SettingsView } from './SettingsView';

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('SettingsView', () => {
  beforeEach(() => {
    invokeMock.mockClear();
    recordGamifyMock.mockClear();
  });

  it('renders without crashing', () => {
    expect(() =>
      render(<SettingsView pushToast={vi.fn()} />, { wrapper })
    ).not.toThrow();
  });

  it('all buttons have type=button', () => {
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    const buttons = screen.getAllByRole('button');
    buttons.forEach(btn => {
      expect(btn.getAttribute('type')).toBe('button');
    });
  });

  it('search input is accessible via aria-label', () => {
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    const searchInput = screen.getByLabelText('Search settings');
    expect(searchInput).toBeDefined();
  });

  it('renders section nav items for all 12 settings sections', () => {
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    // The nav renders one button per section label (12 sections defined in SECTIONS).
    const navButtons = screen.getAllByRole('button');
    // At least the 12 nav section buttons should be present.
    expect(navButtons.length).toBeGreaterThanOrEqual(12);
    // Spot-check a few known section labels (use getAllByText since headings also appear).
    expect(screen.getAllByText('Orchestrator').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Theme').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Keybinds').length).toBeGreaterThanOrEqual(1);
  });

  it('announces preference save via aria-live when theme changes', async () => {
    const user = userEvent.setup();
    mockSetGuiPreference.mockClear();
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    await user.click(screen.getByRole('button', { name: /theme/i }));
    const voidTheme = await screen.findByRole('button', { name: /void/i });
    await user.click(voidTheme);
    await waitFor(() => expect(mockSetGuiPreference).toHaveBeenCalledWith('gui.theme', 'void'));
    expect(await screen.findByRole('status')).toHaveTextContent(/saved/i);
  });

  it('fires secret_rotated when set_secret succeeds', async () => {
    const user = userEvent.setup();
    render(<SettingsView pushToast={vi.fn()} gamifyEnabled />, { wrapper });
    await user.click(screen.getByRole('button', { name: /keys & secrets/i }));
    const input = await screen.findByPlaceholderText('Paste new value…');
    await user.type(input, 'super-secret-value');
    fireEvent.click(screen.getByRole('button', { name: 'save' }));
    await waitFor(() => {
      expect(recordGamifyMock).toHaveBeenCalledWith(
        'secret_rotated',
        { key: 'VOX_DB_TOKEN' },
        { enabled: true },
      );
    });
  });

  it('opts in to anonymous contribution via the real telemetry consent command', async () => {
    const user = userEvent.setup();
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    await user.click(screen.getByRole('button', { name: /^telemetry$/i }));
    // Consent snapshot loaded — install id is shown.
    await screen.findByText(/anonymous install id/i);
    // Denied → toggle is off; clicking it must call the REAL consent command.
    fireEvent.click(screen.getByRole('button', { name: /toggle off/i }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('set_telemetry_consent', { grant: true });
    });
  });

  it('fires signing_key_rotated when rotate succeeds on an existing key', async () => {
    const user = userEvent.setup();
    vi.stubGlobal('prompt', vi.fn(() => 'master-password'));
    render(<SettingsView pushToast={vi.fn()} gamifyEnabled />, { wrapper });
    await user.click(screen.getByRole('button', { name: /signing keys/i }));
    await screen.findByText('fp-deadbeef');
    fireEvent.click(screen.getByRole('button', { name: 'rotate' }));
    await waitFor(() => {
      expect(recordGamifyMock).toHaveBeenCalledWith(
        'signing_key_rotated',
        expect.objectContaining({ node_id: 'node-abc', fingerprint: 'fp-cafebabe' }),
        { enabled: true },
      );
    });
    vi.unstubAllGlobals();
  });
});
