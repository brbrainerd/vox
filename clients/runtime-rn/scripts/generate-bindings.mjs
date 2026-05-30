#!/usr/bin/env node
//! Regenerate the uniffi TypeScript + C++ TurboModule bindings from the
//! `vox-runtime-rn` Rust crate.
//!
//! Invoked via `npm run generate-bindings` from `clients/runtime-rn/`.
//! Compiles the Rust crate first (so the host-arch dynamic library is fresh),
//! then runs `uniffi-bindgen-react-native generate jsi bindings` against it.
//!
//! Cross-host: `.dll` on Windows, `.so` on Linux, `.dylib` on macOS.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { platform } from "node:os";

const SELF_DIR = fileURLToPath(new URL(".", import.meta.url));
const PKG_ROOT = resolve(SELF_DIR, "..");
const WORKSPACE_ROOT = resolve(PKG_ROOT, "..", "..");

function libName() {
  switch (platform()) {
    case "darwin":
      return "libvox_runtime_rn.dylib";
    case "linux":
      return "libvox_runtime_rn.so";
    case "win32":
      return "vox_runtime_rn.dll";
    default:
      throw new Error(`unsupported host platform: ${platform()}`);
  }
}

function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, { stdio: "inherit", shell: process.platform === "win32", ...opts });
  if (r.status !== 0) {
    console.error(`Command failed with exit code ${r.status}`);
    process.exit(r.status ?? 1);
  }
}

// 1. Cargo build the Rust crate to refresh the host-arch dynamic library.
run("cargo", ["build", "-p", "vox-runtime-rn"], { cwd: WORKSPACE_ROOT });

// 2. Locate the produced library.
const libPath = join(WORKSPACE_ROOT, "target", "debug", libName());
if (!existsSync(libPath)) {
  console.error(`expected library at ${libPath} after cargo build; not found`);
  process.exit(1);
}

// 3. Ensure output dirs exist.
const tsDir = join(PKG_ROOT, "src", "__generated__");
const cppDir = join(WORKSPACE_ROOT, "target", "uniffi-bindgen-cpp-tmp");
mkdirSync(tsDir, { recursive: true });
mkdirSync(cppDir, { recursive: true });

// 4. Run uniffi-bindgen-react-native.
run("npx", [
  "-y",
  "uniffi-bindgen-react-native@latest",
  "generate",
  "jsi",
  "bindings",
  "--library",
  "--no-format",
  "--ts-dir",
  tsDir,
  "--cpp-dir",
  cppDir,
  libPath,
]);

console.log(`\nBindings written to ${tsDir}`);
console.log(`C++ TurboModule glue written to ${cppDir}`);
