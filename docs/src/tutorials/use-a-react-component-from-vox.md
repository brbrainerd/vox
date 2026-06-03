---
title: "Use a React component from Vox"
description: "Import an external React (or React Native) component or hook into a .vox component and render it — supported import forms, what the compiler emits, auto-injected CSS, provider guidance, and current limitations."
category: "Tutorials"
---

# Use a React component from Vox

Vox compiles `component` declarations to TypeScript/React. You can import an
external React component or hook from any npm package or local `.tsx` file and
render it inside a Vox `view:` — it compiles to an ordinary ES import plus a JSX
tag, with no FFI or marshaling.

## Import forms

```vox
// vox:skip — illustrative import surface (paths are project-specific)
import react Button from "@acme/ui/Button"                         // default
import react { Dialog, DialogContent } from "@radix-ui/react-dialog" // named
import react { Button as AcmeButton } from "@acme/ui"               // aliased
import react * as Dialog from "@radix-ui/react-dialog"              // namespace
```

The module specifier is emitted verbatim — Vox does no path rewriting; your
bundler's `package.json` `exports` resolution does the work.

## Rendering

Reference an imported component in a `view:` using call syntax. It lowers to a
JSX tag:

```vox
import react { Button } from "@acme/ui"

component Page() {
  view: column() {
    Button()
  }
}
```

emits (web target):

```tsx
// vox:skip — emitted output, not Vox source
import { Button } from "@acme/ui";

export function Page(): React.ReactElement {
  return (
    <div>
      <Button />
    </div>
  );
}
```

The compiler:

- emits the ES import once (grouped per module for named imports),
- renders `Button()` → `<Button/>` (it is *not* mistaken for an HTML element and
  does **not** get a spurious sibling `./Button` import),
- passes props straight through — props are a plain JS object.

The same works on the **React Native** target: the import is emitted and the
component renders as a tag (the RN target maps Vox primitives like `column`/`text`
to `View`/`Text`, but external components pass through unchanged).

## Required CSS and providers are handled for known libraries

Vox keeps a table of common component libraries
([`external_libs.rs`](../../../crates/vox-codegen/src/codegen_ts/external_libs.rs)).
When you import one, the emitter:

- **auto-injects required CSS files.** Importing `@mantine/core` emits
  `import "@mantine/core/styles.css";` for you (Mantine ships real CSS, not
  runtime CSS-in-JS).
- **emits provider setup guidance** for libraries that mandate a top-level
  provider. Importing `@mantine/core`, `@chakra-ui/react`, `react-native-paper`,
  or `tamagui` emits a comment reminding you to mount `<MantineProvider>` /
  `<ChakraProvider>` / `<PaperProvider>` / `<TamaguiProvider>` at your app root.

Headless libraries (Radix, Headless UI, React Aria) need neither, and the
emitter correctly adds nothing for them.

The scaffolded `vite.config.ts` also sets `resolve.dedupe: ["react","react-dom"]`
so the app and the imported library share a single React copy (duplicate copies
cause React's "Invalid hook call" error).

## Type checking

Vox treats an imported component as opaque: it does not type-check the props you
pass against the library's `.d.ts`. That check is performed by the downstream
`tsc` against the genuine type definitions — exactly as a plain `.js` import
would behave. A misspelled prop surfaces when you build the emitted TypeScript.

## Current limitations

- **Namespace member tags are not rendered as elements.** `import react * as
  Dialog from "@radix-ui/react-dialog"` emits the namespace import, but using a
  member as a tag (`Dialog.Root()`) lowers to a call expression, not
  `<Dialog.Root/>`. For Radix-style component sets, **use the named form**
  (`import react { Dialog, DialogContent } from "@radix-ui/react-dialog"`), which
  renders as tags.
- **Dependencies are not added to `package.json` for you.** In app mode the
  `package.json` is yours to own; install the package and its peers (the
  provider-guidance comments name the relevant peers).

## See also

- [Phase 5 Sub-Spec: Native React / React Native Component Interop](../architecture/external-frontend-interop-phase5-component-interop-subspec-2026.md)
- [External Frontend Interop Plan (2026)](../architecture/external-frontend-interop-plan-2026.md)
