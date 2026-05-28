import assert from "node:assert/strict";

import { extractRoutePathsFromManifest, extractVoxClientEndpoints } from "./artifacts.ts";

const sampleManifest = `
export const voxRoutes: VoxRoute[] = [
  { path: "/", component: Home },
  { path: "/about", component: About },
]
`;

assert.deepStrictEqual(extractRoutePathsFromManifest(sampleManifest), ["/", "/about"]);

const sampleClient = `
export async function listItems(): Promise<unknown> {
  return $get("/api/query/listItems");
}
export async function save(): Promise<void> {
  return $post("/api/mutation/save", {});
}
`;

const endpoints = extractVoxClientEndpoints(sampleClient);
assert.deepStrictEqual(endpoints, ["listItems", "save"]);

console.log("artifacts ingest tests ok");
