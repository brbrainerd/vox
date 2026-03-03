const esbuild = require('esbuild');

esbuild.build({
  entryPoints: ['webview-ui/src/index.tsx'],
  outfile: 'out/webview.js',
  bundle: true,
  minify: true,
  format: 'iife',
  sourcemap: true,
  external: ['vscode'],
}).catch(() => process.exit(1));
