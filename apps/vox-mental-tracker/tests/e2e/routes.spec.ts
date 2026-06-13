import { test, expect } from "@playwright/test";

const ROUTES: { path: string; heading: RegExp }[] = [
  { path: "/", heading: /Mental Health Tracker/i },
  { path: "/mood", heading: /Log your mood/i },
  { path: "/timeline", heading: /Timeline/i },
  { path: "/weekly", heading: /Weekly summary/i },
  { path: "/export", heading: /Exports/i },
  { path: "/voice", heading: /Voice/i },
  { path: "/settings", heading: /Settings/i },
];

for (const { path, heading } of ROUTES) {
  test(`route ${path} renders + screenshot`, async ({ page }) => {
    await page.goto(path);
    // The Vox NavBar is on every route — proves the app shell mounted.
    // exact: true so the NavBar "Home" doesn't collide with a page's "← Home".
    await expect(page.getByRole("link", { name: "Home", exact: true })).toBeVisible();
    // Route-specific heading.
    await expect(page.getByRole("heading", { name: heading })).toBeVisible();
    const slug = path === "/" ? "home" : path.replace(/\//g, "");
    await page.screenshot({ path: `tests/e2e/__screens__/${slug}.png`, fullPage: true });
  });
}

test("settings has no destructive clear/delete control", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByRole("button", { name: /clear|delete|reset|wipe/i })).toHaveCount(0);
});
