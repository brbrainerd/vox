// ESM shim over the CommonJS plugin implementation in plugin/index.js.
//
// The package root is `"type": "module"`, so this `.js` file is ESM; the real
// plugin lives in `plugin/` (its own `"type": "commonjs"` scope) because
// Expo's plugin resolver loads plugins with `require()`. Prefer referencing
// the plugin as `"@vox/runtime-rn/plugin"` in app.json — that path is plain
// CommonJS and works on every Node version Expo supports.
import withVoxRuntime from "./plugin/index.js";
export default withVoxRuntime;
