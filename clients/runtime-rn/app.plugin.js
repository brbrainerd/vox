// Expo config-plugin convention entry: `plugins: ["@vox/runtime-rn"]`
// resolves to this file. It is ESM (package root is `"type": "module"`), so
// it relies on Node's require(esm) support in the Expo CLI (Node >= 20.17).
// If your toolchain predates that, reference the CommonJS implementation
// directly instead: `plugins: [["@vox/runtime-rn/plugin", { ... }]]`.
import withVoxRuntime from "./plugin/index.js";
export default withVoxRuntime;
