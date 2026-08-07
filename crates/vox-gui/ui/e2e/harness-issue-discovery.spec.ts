import { test, expect } from '@playwright/test';

/**
 * Tauri invoke mock for the harness issue discovery review flow (Scientia
 * surface, "Harness Issues" tab). Self-contained per Playwright's
 * `addInitScript` contract — no captured module scope.
 */
function installHarnessIssuesMock(): void {
  try {
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'scientia'], activeTab: 'scientia' }),
    );
    localStorage.setItem('vox_sidebar_mode', 'default');
  } catch {
    // sandboxed contexts may deny localStorage
  }

  (window as any).__TAURI_CALLS__ = [];

  const pendingIssue = {
    id: 42,
    source: 'corpus_scan',
    session_key: null,
    target_path: 'examples/golden/foo.vox',
    detected_at_ms: Date.now() - 1_000,
    category: 'staleness',
    severity: 'medium',
    summary: 'Golden example foo.vox references a retired API',
    evidence_json: '{}',
    status: 'pending',
  };

  (window as any).__TAURI_INTERNALS__ = {
    transformCallback: (cb: (...args: unknown[]) => unknown) => {
      const id = `cb_${Math.random().toString(36).slice(2)}`;
      (window as any)[id] = cb;
      return id;
    },
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
      switch (cmd) {
        case 'get_initial_view':
          return 'scientia';
        case 'get_build_info':
          return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
        case 'get_orchestrator_status_bin':
          return new Uint8Array([0x80]);
        case 'get_orchestrator_status':
          return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
        case 'get_action_manifest':
          return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
        case 'get_gui_preference':
          return null;
        case 'get_gamify_settings':
          return { enabled: false, mode: 'off' };
        case 'get_identity_summary':
          return { display_name: 'tester@vox', os_user: 'tester' };
        case 'get_active_model':
          return null;
        case 'get_selection_policy':
          return { chain: [], free_tier: true };
        case 'vox_docs_index':
          return [];
        case 'list_secret_status':
          return [{ id: 'OPENROUTER_API_KEY', isPresent: true }];
        case 'inference_provider_status':
          return [];
        case 'list_harness_issues':
          return [pendingIssue];
        case 'list_harness_fix_proposals':
          return [];
        case 'record_harness_issue_decision':
          return null;
        case 'propose_harness_issue_fix':
          return 1;
        case 'scan_training_corpus':
          return 3;
        default:
          return null;
      }
    },
  };
}

test.describe('Harness issue discovery review flow', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installHarnessIssuesMock);
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await page.getByRole('tab', { name: 'Harness Issues' }).click();
  });

  test('shows the pending issue and confirms it into a proposed fix', async ({ page }) => {
    await expect(page.getByText('Golden example foo.vox references a retired API')).toBeVisible();

    await page.getByRole('button', { name: 'Confirm & propose fix' }).click();

    await expect
      .poll(() =>
        page.evaluate(() =>
          (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'record_harness_issue_decision'),
        ),
      )
      .toHaveLength(1);
    const decisionCalls = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'record_harness_issue_decision'),
    );
    expect(decisionCalls[0].args).toMatchObject({ issueId: 42, decision: 'confirmed' });

    const proposeCalls = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'propose_harness_issue_fix'),
    );
    expect(proposeCalls).toHaveLength(1);
    expect(proposeCalls[0].args).toMatchObject({
      issueId: 42,
      targetPath: 'examples/golden/foo.vox',
    });
  });

  test('scanning the training corpus toasts the found count', async ({ page }) => {
    await page.getByRole('button', { name: 'Scan training corpus' }).click();

    await expect(page.getByRole('status').getByText(/3 new issue\(s\) found/)).toBeVisible();
  });
});
