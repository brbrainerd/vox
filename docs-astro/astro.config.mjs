// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLlmsTxt from 'starlight-llms-txt';
import { voxGrammar } from './src/plugins/vox-grammar.mjs';
import { getSidebar } from './src/utils/sidebar.mjs';

export default defineConfig({
  site: 'https://voxlang.org/',
  integrations: [
    starlight({
      title: 'Vox: The AI-Native Programming Language',
      description: 'Official documentation for Vox, the AI-native full-stack programming language.',
      routeMiddleware: './src/routeData.ts',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/vox-foundation/vox' }
      ],
      editLink: {
        baseUrl: 'https://github.com/vox-foundation/vox/edit/main/docs/src/',
      },
      // Sidebar is dynamically generated from SUMMARY.md to maintain SSOT
      sidebar: getSidebar(),
      expressiveCode: {
        shiki: {
          langs: [voxGrammar],
        },
      },
      plugins: [
        starlightLlmsTxt({
          projectName: 'Vox',
          description: 'Vox is an AI-native full-stack programming language. It compiles a single .vox file into a database schema, type-safe server, and live browser application. Designed first as a target for large language models.',
          llmsFullTxt: true,
        }),
      ],
      pagefind: true,
    }),
  ],
  vite: {
    resolve: {
      alias: {
        '@docs-src': new URL('../docs/src', import.meta.url).pathname,
      },
    },
  },
  // Content lives in docs/src/ (symlinked or copied in Phase 3).
  srcDir: './src',
  outDir: './dist',
  publicDir: './public',
});
