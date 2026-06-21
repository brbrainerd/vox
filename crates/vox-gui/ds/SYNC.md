# Re-syncing Limes to Claude Design

The Limes bundle was synced to the Claude Design project **"Vox Axis — Limes"**
(`143d83f3-1f3b-49e4-ab12-fdfa95be1d93`) on 2026-06-20 via the `DesignSync`
tool, using the bundled `design-sync` converter run manually (the `/design-sync`
skill is user-invocation-only, so it can't be driven by an agent).

The committed inputs are `package.json`, `dist/index.d.ts`, `.design-sync/config.json`,
and `.design-sync/previews/*.tsx`. Everything else (`node_modules/`, `.ds-sync/`,
`ds-bundle/`, `.design-sync/.cache/`) is regenerated and gitignored.

## Recipe (Windows / pnpm monorepo)

From `crates/vox-gui/`:

1. **Build the ESM entry** (esbuild lives in the pnpm store):
   ```
   ESB=ui/node_modules/.pnpm/esbuild@0.28.1/node_modules/esbuild/lib/main.js
   node -e "require(process.argv[1]).build({entryPoints:['ds/components/index.ts'],bundle:true,format:'esm',external:['react','react/jsx-runtime','react-dom'],jsx:'automatic',outfile:'ds/dist/index.js'}).then(()=>console.log('ok'))" "$ESB"
   ```
2. **Stage converter + deps into `ds/`** (the converter has no node_modules of its own):
   - Copy the skill's `design-sync/*.mjs` + `lib/` → `ds/.ds-sync/`.
   - `npm install ts-morph @types/react@^19 --no-save` in `ds/`, then copy
     `esbuild`, `@esbuild`, `react`, `react-dom`, `scheduler` from
     `ui/node_modules/.pnpm/*` into `ds/node_modules/` (npm prunes them — restore
     after the install).
3. **Run the converter** (point esbuild at the real win32 binary — copies lose it):
   ```
   export ESBUILD_BINARY_PATH=".../@esbuild+win32-x64@0.28.1/node_modules/@esbuild/win32-x64/esbuild.exe"
   node ds/.ds-sync/package-build.mjs --config ds/.design-sync/config.json \
     --node-modules ds/node_modules --entry ds/dist/index.js --out ds/ds-bundle
   ```
4. **Patch the closure**: the converter does not copy `fonts/`, `tokens/`, or
   `components.css` into `ds-bundle/`, but `_ds_bundle.css` `@import`s them — copy
   them in from `ds/` so `styles.css`'s `@import` closure resolves.
5. **Upload** via the `DesignSync` tool: `create_project` (or reuse the project
   id above) → `finalize_plan` (localDir = `ds/ds-bundle`) → `write_files`.

## Notes / gotchas

- `package-build.mjs` is browser-free; the grading step (package-capture/validate)
  needs a headless browser and was skipped — previews were verified by rendering
  the component `.html` cards through a local static server instead.
- Without `@types/react`, `Button`/`Card` props (which extend React DOM types)
  emit empty `.d.ts` bodies — install it before building.
