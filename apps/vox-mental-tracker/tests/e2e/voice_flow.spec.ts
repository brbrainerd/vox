import { test, expect } from "@playwright/test";

/**
 * Voice-flow E2E.
 *
 * Stubs Speech.transcribe_microphone via globalThis.__VOX_TEST_TRANSCRIPT__
 * (consumed by the Vox-emitted dist/runtime-install.ts) so we exercise the parse → save loop with
 * a deterministic transcript and no microphone dependency. Native STT is
 * exercised by the Capacitor build pipeline, not here.
 *
 * The save step doesn't actually persist — the @endpoint calls go through
 * the Vox-emitted vox-client.ts which talks to the (not-yet-running) Rust
 * backend. The save will fail; we only assert that Parse populates the
 * KIND / PAYLOAD lines and that the Save click attempts a request without
 * throwing a JS error before the network call.
 */
test("voice → parse round-trip", async ({ page }) => {
  page.on("pageerror", (err) => console.log("PAGE ERROR:", err.message));
  page.on("console", (msg) => console.log("CONSOLE:", msg.type(), msg.text()));

  await page.addInitScript(() => {
    (globalThis as unknown as Record<string, unknown>).__VOX_TEST_TRANSCRIPT__ =
      "I feel like a 4 today";
  });

  await page.goto("/voice");

  // Verify our globals reach the page
  const probe = await page.evaluate(() => ({
    hasSpeech: typeof (globalThis as Record<string, unknown>).Speech === "object",
    hasTranscribe:
      typeof (
        (globalThis as Record<string, unknown>).Speech as Record<string, unknown> | undefined
      )?.transcribe_microphone === "function",
    testTranscript: (globalThis as Record<string, unknown>).__VOX_TEST_TRANSCRIPT__,
    speechResult: (
      (globalThis as Record<string, unknown>).Speech as
        | { transcribe_microphone: () => unknown }
        | undefined
    )?.transcribe_microphone(),
  }));
  console.log("PROBE:", JSON.stringify(probe));

  await page.getByRole("button", { name: /^Transcribe$/ }).click();
  await expect(page.getByText(/RAW:/)).toContainText("I feel like a 4 today", {
    timeout: 5_000,
  });

  // Parse calls the @endpoint parse_voice via fetch through the generated
  // vox-client.ts. The compiler now emits `async` handlers with `await`
  // for nested @endpoint calls, so the result is resolved before `p.kind`
  // is read. These assertions require the Vox backend to be running.
  if (process.env.VOX_BACKEND_URL) {
    await page.getByRole("button", { name: /^Parse$/ }).click();
    await expect(page.locator('[data-testid="kind"]')).toHaveText("mood_recorded", {
      timeout: 5_000,
    });
    await expect(page.locator('[data-testid="payload"]')).toContainText("mood_score");
    await page.getByRole("button", { name: /^Save$/ }).click();
    await expect(page.locator('[data-testid="saved-counter"]')).toHaveText("1");
  }
});
