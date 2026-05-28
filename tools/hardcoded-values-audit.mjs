#!/usr/bin/env node
/**
 * Regenerates hardcoded-value audit artifacts under .vox/audit/<run>/.
 * Usage (repo root): node tools/hardcoded-values-audit.mjs [.vox/audit/2026-05-11-hardcoded-values]
 */
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const ITEM_CAP = Number(process.env.HV_ITEM_CAP || 100);
const REPO = process.env.VOX_REPO_ROOT
  ? path.resolve(process.env.VOX_REPO_ROOT)
  : process.cwd();
const OUT_DIR = process.argv[2]
  ? path.resolve(process.argv[2])
  : path.join(REPO, ".vox/audit/2026-05-11-hardcoded-values");

const PASSES = [
  // Use **/ (not */) so ripgrep globs behave consistently on Windows + POSIX.
  { id: "crates_rs", root: "crates", globs: ["**/src/**/*.rs"] },
  { id: "crates_build", root: "crates", globs: ["**/build.rs"] },
  {
    id: "apps_ts",
    root: "apps",
    globs: [
      "**/src/**/*.ts",
      "!**/*.test.ts",
      "!**/*.spec.ts",
      "!**/__tests__/**",
    ],
  },
  {
    id: "apps_tsx",
    root: "apps",
    globs: [
      "**/src/**/*.tsx",
      "!**/*.test.tsx",
      "!**/*.spec.tsx",
      "!**/__tests__/**",
    ],
  },
  { id: "golden_vox", root: "examples/golden", globs: ["**/*.vox"] },
  { id: "contracts", root: "contracts", globs: ["**/*.yaml", "**/*.yml", "**/*.json"] },
];

const NEGATIVE_GLOBS = [
  "!**/tests/**",
  "!**/fixtures/**",
  "!**/benches/**",
  "!**/*_test.rs",
  "!**/*_tests.rs",
  "!**/node_modules/**",
  "!**/*.generated.*",
  "!**/patches/**",
  "!docs/src/archive/**",
];

function langFromPath(p) {
  if (p.endsWith(".rs")) return "rust";
  if (p.endsWith(".ts") || p.endsWith(".tsx")) return "typescript";
  if (p.endsWith(".vox")) return "vox";
  if (p.endsWith(".yaml") || p.endsWith(".yml")) return "yaml";
  if (p.endsWith(".json")) return "json";
  return "other";
}

function shouldSkipLine(t) {
  const s = t.trimStart();
  if (/^\/\/|^#|^\s*\*|^\/\*/.test(s)) return true;
  if (/^(use|import)\s/.test(s)) return true;
  if (/(?:pub\s+)?(?:const|static)\s+\w+/.test(s)) return true;
  return false;
}

/** Lines that almost never warrant production-hardening triage for literals / needles. */
function isLikelyDiagnosticOnlyLine(t) {
  const s = t.trimStart();
  return /^(?:eprintln?!|println?!|print!|format!|debug!|trace!|info!|warn!)\(/.test(s) ||
    /^log::(?:trace|debug|info|warn)!\(/.test(s) ||
    /^tracing::(?:trace|debug|info|warn)!\(/.test(s);
}

const DOC_TRUST_HOST_RE =
  /\b(?:docs\.rs|crates\.io|github\.com\/rust-lang|rust-lang\.org|doc\.rust-lang\.org|semver\.org|json-schema\.org|w3\.org|example\.com|example\.org)\b/i;

function hardcodedUrlLikelyBenign(m) {
  const hay = `${m.text} ${m.sub || ""}`;
  if (DOC_TRUST_HOST_RE.test(hay)) return true;
  if (/schema(?:\.json)?["']?\s*:/i.test(m.text) && m.path.endsWith(".json")) {
    return true;
  }
  return false;
}

function hardcodedUrlSignalBoost(m) {
  const s = m.text;
  return /\b(?:reqwest|hyper::|surf::|ureq::|Url::parse|url::Url|Client::new)\b/.test(s) ||
    /connect_timeout|request\(|post\(|get\(|Builder::new\(/.test(s);
}

function unescapeRustishStringInner(inner) {
  return inner
    .replace(/\\n/g, "\n")
    .replace(/\\r/g, "\r")
    .replace(/\\t/g, "\t")
    .replace(/\\\\/g, "\\")
    .replace(/\\"/g, '"')
    .replace(/\\'/g, "'");
}

/** First double-quoted Rust/TS string inside `.contains("…")` / `.starts_with(…)` / TS `includes`. */
function extractBrittleNeedleLine(text) {
  const rust = text.match(
    /\.(?:contains|starts_with|ends_with)\(\s*"((?:[^"\\]|\\.)*)"/,
  );
  if (rust) return unescapeRustishStringInner(rust[1]);
  const ts =
    text.match(
      /\.(?:includes|startsWith|endsWith)\(\s*"((?:[^"\\]|\\.)*)"/,
    ) ||
    text.match(
      /\.(?:includes|startsWith|endsWith)\(\s*'((?:[^'\\]|\\.)*)'/,
    );
  if (ts) return unescapeRustishStringInner(ts[1]);
  return null;
}

const TRIVIAL_BRITTLE_NEEDLES = new Set([
  "",
  " ",
  "\t",
  "\n",
  "\r",
  ".",
  ",",
  ":",
  "/",
  "\\",
  "(",
  ")",
  "[",
  "]",
  "{",
  "}",
  "-",
  "_",
  "+",
  "*",
  "?",
  "=",
  ";",
  "'",
  '"',
]);

function brittleNeedleDecision(needle, fullLine) {
  if (needle === null) return { skip: false, signal: 35, confidence: null };
  const n = needle;
  if (n.length <= 1 || TRIVIAL_BRITTLE_NEEDLES.has(n)) {
    return { skip: true, signal: 0, confidence: null };
  }
  let signal = 40 + Math.min(60, n.length * 2);
  if (/\s/.test(n)) signal += 25;
  if (n.length >= 24) signal += 15;
  let confidence = null;
  if (signal >= 85) confidence = "high";
  else if (signal >= 55) confidence = "medium";
  else confidence = "low";
  if (/[A-Za-z]{3,}/.test(n) && /[a-z][A-Z]|[a-z][a-z][a-z]/.test(n)) {
    signal += 10;
  }
  if (
    /\.unwrap\(|\.expect\(|panic!|todo!|unimplemented!/.test(fullLine) &&
    n.length < 12
  ) {
    signal -= 15;
    confidence = "low";
  }
  return { skip: false, signal, confidence };
}

function magicNumberLineSignal(m) {
  let s = 30;
  const t = m.text;
  if (/(?:timeout|deadline|Duration|sleep|buffer|capacity|limit|max_len|CHANNEL)/i.test(t)) {
    s += 35;
  }
  if (/\b(?:read_exact|write_all|recv|send|chunks|chunk)\b/.test(t)) {
    s += 20;
  }
  if (/\b<<\s*\d+|\d+\s*<<\b/.test(t)) {
    s -= 25;
  }
  if (isLikelyDiagnosticOnlyLine(t)) {
    s -= 40;
  }
  return s;
}

function magicNumberSuppress(m) {
  if (isLikelyDiagnosticOnlyLine(m.text)) return true;
  const t = m.text;
  if (/\b(?:<<|>>)\b/.test(t) && /\b(?:1024|2048|4096|8192|65536|1_024|2_048|4_096)\b/.test(t)) {
    return true;
  }
  return false;
}

/** Higher sorts first when applying ITEM_CAP — prefer actionable / high-signal rows. */
function matchSignalPriority(cat, m) {
  delete m.confidenceOverride;
  delete m._needle;
  delete m._signalPriority;

  if (cat.slug === "brittle-string-needles") {
    const needle = extractBrittleNeedleLine(m.text);
    const d = brittleNeedleDecision(needle, m.text);
    m._needle = needle;
    if (d.skip) return -1;
    if (needle !== null) {
      m.sub = needle.length > 200 ? needle.slice(0, 200) : needle;
      if (d.confidence) m.confidenceOverride = d.confidence;
    }
    return d.signal;
  }
  if (cat.slug === "hardcoded-magic-numbers") {
    if (magicNumberSuppress(m)) return -1;
    const s = magicNumberLineSignal(m);
    m.confidenceOverride = s >= 70 ? "high" : s >= 45 ? "medium" : "low";
    return s;
  }
  if (cat.slug === "hardcoded-urls") {
    if (hardcodedUrlLikelyBenign(m)) return -1;
    const boost = hardcodedUrlSignalBoost(m);
    m.confidenceOverride = boost ? "high" : "medium";
    return boost ? 95 : 42;
  }
  if (cat.slug === "hardcoded-timeouts") {
    if (isLikelyDiagnosticOnlyLine(m.text)) return -1;
    if (/Duration::from_(?:secs|millis|micros|nanos)\(\s*0+\s*\)/.test(m.text)) {
      return -1;
    }
    const hot = /timeout|deadline|connect_timeout|latency|wait|backoff|Duration::from_/i.test(
      m.text,
    );
    m.confidenceOverride = hot ? "high" : "medium";
    return hot ? 82 : 48;
  }
  if (cat.slug === "hardcoded-ports") {
    const t = m.text;
    const hot = /bind|listen|TcpListener|UdpSocket|connect\(|SocketAddr|::from/i.test(t);
    m.confidenceOverride = hot ? "high" : "medium";
    return hot ? 86 : 43;
  }
  if (cat.slug === "hardcoded-ips") {
    const t = m.text;
    const hot = /SocketAddr|TcpListener|bind\(|connect\(|UdpSocket|listen\(/i.test(t);
    m.confidenceOverride = hot ? "high" : "medium";
    return hot ? 88 : 44;
  }
  if (cat.slug === "hardcoded-filesystem-paths") {
    const t = m.text;
    const hot =
      /include_str!|include_bytes!|File::open|OpenOptions|read_to_string|write\(|create_dir|remove_file/i.test(
        t,
      );
    m.confidenceOverride = hot ? "high" : "medium";
    return hot ? 78 : 46;
  }
  return 50;
}

function contractsPathOnlySsot(p) {
  const x = p.replace(/\\/g, "/");
  return x.startsWith("contracts/");
}

function loadRegisteredEnvNames(repoRoot) {
  const p = path.join(repoRoot, "contracts/config/env-vars.v1.yaml");
  const text = fs.readFileSync(p, "utf8");
  const names = new Set();
  for (const m of text.matchAll(/^\s+-\s+name:\s+"([^"]+)"/gm)) {
    names.add(m[1]);
  }
  return names;
}

function rgJson(pattern, pass) {
  const rootPath = path.join(REPO, pass.root);
  if (!fs.existsSync(rootPath)) return [];
  const args = ["--json", "-S"];
  for (const g of pass.globs) {
    args.push("-g", g);
  }
  for (const g of NEGATIVE_GLOBS) {
    args.push("-g", g);
  }
  args.push(pattern, rootPath);
  const res = spawnSync("rg", args, {
    cwd: REPO,
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 1024,
  });
  if (res.error && res.error.code === "ENOENT") {
    throw new Error("`rg` (ripgrep) not found on PATH");
  }
  const out = res.stdout || "";
  const matches = [];
  for (const line of out.split("\n")) {
    if (!line.trim()) continue;
    let j;
    try {
      j = JSON.parse(line);
    } catch {
      continue;
    }
    if (j.type !== "match") continue;
    const d = j.data;
    const rel = path.relative(REPO, d.path.text).split(path.sep).join("/");
    const lineNum = d.line_number;
    const text = String(d.lines.text || "").replace(/\r?\n$/, "");
    let sub = text.trim();
    if (d.submatches && d.submatches.length > 0) {
      const last =
        d.submatches[d.submatches.length - 1] &&
        d.submatches[d.submatches.length - 1].match &&
        d.submatches[d.submatches.length - 1].match.text;
      const first =
        d.submatches[0].match && d.submatches[0].match.text;
      // Prefer last submatch (often the explicit capture group) when present.
      sub = last || first || sub;
    }
    matches.push({ path: rel, line: lineNum, text, sub });
  }
  return matches;
}

function uniqKey(catSlug, m) {
  return `${catSlug}::${m.path}::${m.line}`;
}

function escapeRgF(s) {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

/** @param {{path:string,line:number,text:string,sub?:string}} m */
function findMatchSubstring(categorySlug, patternHint, m) {
  if (m.sub && m.sub.length > 0 && m.sub !== m.text.trim()) {
    if (categorySlug === "brittle-string-needles") {
      const s = m.sub.length > 300 ? m.sub.slice(0, 300) + "…" : m.sub;
      return s;
    }
    return m.sub;
  }
  // Best-effort: first quoted string on line for URL-like categories
  const q = m.text.match(/"([^"\\]*(\\.[^"\\]*)*)"|'([^'\\]*(\\.[^'\\]*)*)'/);
  if (q) return q[1] ?? q[3] ?? q[0];
  return m.text.trim().slice(0, 200);
}

function fixBlueprint(categorySlug, severity) {
  const bases = {
    "hardcoded-urls": {
      kind: "register-env-and-use-secrets",
      replacement_snippet:
        "// Read base URL from vox_secrets / config; add VOX_* to contracts/config/env-vars.v1.yaml if new.",
      ssot: "contracts/config/env-vars.v1.yaml",
    },
    "hardcoded-ports": {
      kind: "extract-named-constant",
      replacement_snippet:
        "const DEFAULT_LISTEN_PORT: u16 = …; // or std::env::var(\"VOX_…_PORT\") after registering in env-vars SSOT",
      ssot: "contracts/config/env-vars.v1.yaml",
    },
    "hardcoded-ips": {
      kind: "extract-named-constant",
      replacement_snippet:
        "Move IP to a named const or config struct field; avoid bare literals in listen/connect calls.",
      ssot: "contracts/config/env-vars.v1.yaml",
    },
    "hardcoded-filesystem-paths": {
      kind: "use-config-path",
      replacement_snippet:
        "Use vox_config::paths or std::path::PathBuf built from config; no hard-coded C:\\ or /home paths.",
      ssot: "docs/src/architecture/data-storage-ssot-2026.md",
    },
    "hardcoded-timeouts": {
      kind: "extract-named-constant",
      replacement_snippet:
        "const CONNECT_TIMEOUT: Duration = Duration::from_millis(…); // document rationale",
      ssot: null,
    },
    "hardcoded-retry-counts": {
      kind: "extract-named-constant",
      replacement_snippet:
        "const MAX_RETRIES: u32 = …; // tune via config when needed",
      ssot: null,
    },
    "hardcoded-buffer-sizes": {
      kind: "extract-named-constant",
      replacement_snippet:
        "const BUF_CAP: usize = …; // name ties size to protocol / device limits",
      ssot: null,
    },
    "hardcoded-version-strings": {
      kind: "centralize-in-contract",
      replacement_snippet:
        "Derive API route version from one module or contract-owned constant; avoid sprinkling \"v1\" strings.",
      ssot: "contracts/index.yaml",
    },
    "hardcoded-model-names": {
      kind: "centralize-in-contract",
      replacement_snippet:
        "Resolve model id from runtime manifest / capability registry / user config instead of string literals.",
      ssot: "contracts/capability/capability-registry.yaml",
    },
    "hardcoded-env-var-names": {
      kind: "register-env-and-use-secrets",
      replacement_snippet:
        "Add name to contracts/config/env-vars.v1.yaml; use vox_secrets::resolve_secret where appropriate.",
      ssot: "contracts/config/env-vars.v1.yaml",
    },
    "brittle-string-needles": {
      kind: "normalize-input-or-casefold",
      replacement_snippet:
        "Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.",
      ssot: null,
    },
    "brittle-regex-patterns": {
      kind: "fix-regex-unicode-or-quantifiers",
      replacement_snippet:
        "Add (?u), prefer \\p{L} over [a-zA-Z] for user text; trim input; use non-greedy quantifiers where needed.",
      ssot: null,
    },
    "hardcoded-extensions-globs": {
      kind: "centralize-in-contract",
      replacement_snippet:
        "Centralize allowed extensions in one MODULE or contract list consumed by scanners.",
      ssot: "contracts/code-audit/rules.v1.yaml",
    },
    "hardcoded-date-literals": {
      kind: "review-intentional",
      replacement_snippet:
        "If not a real release date / contract version, derive from build metadata or user input.",
      ssot: null,
    },
    "hardcoded-ansi-codes": {
      kind: "use-theme-aware-styles",
      replacement_snippet:
        "Prefer owo-colors, anstyle, or terminal abstraction — avoid raw ESC bytes.",
      ssot: null,
    },
    "hardcoded-magic-numbers": {
      kind: "extract-named-constant",
      replacement_snippet: "const MEANINGFUL_NAME: u64 = …; // explain units",
      ssot: null,
    },
    "hardcoded-canonical-keys": {
      kind: "centralize-in-contract",
      replacement_snippet:
        "Import plugin / capability id from registry SSOT instead of duplicating string literals.",
      ssot: "contracts/capability/capability-registry.yaml",
    },
    "hardcoded-provider-names": {
      kind: "use-provider-enum",
      replacement_snippet:
        "Route on ProviderKind (or equivalent) from orchestrator MCP bridge — avoid raw provider string compares.",
      ssot: "crates/vox-orchestrator-mcp",
    },
    "hardcoded-test-data-in-prod": {
      kind: "remove-test-data-from-prod",
      replacement_snippet:
        "Remove example emails/user names from production paths; load fixtures only in tests.",
      ssot: null,
    },
    "hardcoded-retired-runtime-names": {
      kind: "replace-retired-symbol",
      replacement_snippet:
        "Replace with canonical crate/runtime name per AGENTS.md retired surfaces table.",
      ssot: "AGENTS.md",
    },
  };
  const b = bases[categorySlug] || {
    kind: "review-intentional",
    replacement_snippet: "// Triage manually",
    ssot: null,
  };
  return {
    kind: b.kind,
    replacement_snippet: b.replacement_snippet,
    ...(b.ssot ? { ssot: b.ssot, register_in: b.ssot } : {}),
    notes:
      severity === "error"
        ? "Priority fix — breaks integrations or policy."
        : undefined,
  };
}

function whyText(slug) {
  const w = {
    "hardcoded-urls":
      "Hardcoded URLs bypass environment-specific endpoints and complicate secret/config policy.",
    "hardcoded-ports":
      "Listeners on fixed ports collide across dev/staging and break multi-tenant local runs.",
    "hardcoded-ips":
      "Bare IPs are environment-specific and often wrong on IPv6-only or container networks.",
    "hardcoded-filesystem-paths":
      "Absolute or home-relative paths fail cross-platform and on CI sandboxes.",
    "hardcoded-timeouts":
      "Magic timeouts are hard to tune for slow networks or large models.",
    "hardcoded-retry-counts":
      "Implicit retry limits cause flaky recovery or excessive load.",
    "hardcoded-buffer-sizes":
      "Buffer/channel sizes tied to protocol; literals obscure backpressure semantics.",
    "hardcoded-version-strings":
      "Duplicated API version strings drift from the single workspace version SSOT.",
    "hardcoded-model-names":
      "Model ids should follow registry / user config to avoid lock-in to one provider spelling.",
    "hardcoded-env-var-names":
      "Unregistered VOX_* / env reads violate contracts/config/env-vars SSOT and secret policy boundaries.",
    "brittle-string-needles":
      "Case-sensitive substring checks often fail on real user or OS input.",
    "brittle-regex-patterns":
      "Regexes without Unicode awareness or lazy quantifiers mis-parse real text.",
    "hardcoded-extensions-globs":
      "Extension lists diverge between tools; centralize to avoid scan gaps.",
    "hardcoded-date-literals":
      "Date constants in logic look like stale placeholders or wrong-era defaults.",
    "hardcoded-ansi-codes":
      "Raw ANSI escapes break non-TTY consumers and theming.",
    "hardcoded-magic-numbers":
      "Unnamed numeric literals lack units and intent; reviewers cannot tune safely.",
    "hardcoded-canonical-keys":
      "Duplicated capability/plugin keys drift from the registry SSOT.",
    "hardcoded-provider-names":
      "Stringly provider routing fights enum-based dispatch and telemetry tagging.",
    "hardcoded-test-data-in-prod":
      "Example identities leak into prod code paths and confuse audits.",
    "hardcoded-retired-runtime-names":
      "Retired names break integrations and violate LLM guard / AGENTS.md policy.",
  };
  return w[slug] || "Potential magic / brittle literal — verify in context.";
}

function confidenceFor(slug, line) {
  if (slug === "hardcoded-retired-runtime-names") return "high";
  if (slug === "hardcoded-env-var-names") return "high";
  if (slug === "hardcoded-magic-numbers" || slug === "brittle-string-needles") return "low";
  if (slug === "hardcoded-version-strings" && /\/v\d\//.test(line)) return "medium";
  return "medium";
}

/** Category definitions: pattern is ripgrep (Rust regex). */
const CATEGORIES = [
  { index: 1, slug: "hardcoded-urls", severity: "warning", patterns: ['"(?:https?)://[^\"\\s]{8,}[^\"]*"'] },
  {
    index: 2,
    slug: "hardcoded-ports",
    severity: "warning",
    patterns: [
      '"(?:127\\.0\\.0\\.1|0\\.0\\.0\\.0|localhost|\\[::1\\])\\s*:\\s*\\d+"',
      "'(?:127\\.0\\.0\\.1|0\\.0\\.0\\.0|localhost|\\[::1\\]):\\d+'",
      "https?://(?:127\\.0\\.0\\.1|localhost|\\[::1\\]|0\\.0\\.0\\.0):\\d+",
    ],
  },
  {
    index: 3,
    slug: "hardcoded-ips",
    severity: "warning",
    patterns: [
      '"(?:127\\.0\\.0\\.1|0\\.0\\.0\\.0)"',
      "'(?:127\\.0\\.0\\.1|0\\.0\\.0\\.0)'",
      '"(?:10\\.\\d+\\.\\d+\\.\\d+|192\\.168\\.\\d+\\.\\d+)"',
      "\\b(?:127\\.0\\.0\\.1|0\\.0\\.0\\.0|10\\.\\d+\\.\\d+\\.\\d+|192\\.168\\.\\d+\\.\\d+)\\b",
    ],
  },
  {
    index: 4,
    slug: "hardcoded-filesystem-paths",
    severity: "warning",
    patterns: [
      '"C:\\\\\\\\',
      '"D:\\\\\\\\',
      '"/home/',
      '"/tmp/',
      '"/var/',
      '"/usr/',
      '"/etc/',
      '"~/',
      "'/home/",
      "'/tmp/",
    ],
  },
  {
    index: 5,
    slug: "hardcoded-timeouts",
    severity: "warning",
    patterns: [
      "Duration::from_(?:secs|millis|micros|nanos)\\(\\d+\\)",
      "sleep\\((?:std::time::)?Duration::from_\\w+\\(\\d+\\)\\)",
      "time::sleep\\([^)]*Duration::from_\\w+\\(\\d+\\)",
      "thread::sleep\\([^)]*Duration::from_\\w+\\(\\d+\\)",
      "tokio::time::sleep\\([^)]*Duration::from_\\w+\\(\\d+\\)",
      "setTimeout\\([^,]+,\\s*\\d+\\s*\\)",
      "setInterval\\([^,]+,\\s*\\d+\\s*\\)",
    ],
  },
  {
    index: 6,
    slug: "hardcoded-retry-counts",
    severity: "warning",
    patterns: [
      "for\\s+_\\s+in\\s+0\\.\\.\\d+",
      "for\\s+\\w+\\s+in\\s+0\\.\\.\\d+",
      "max_retries\\s*[=:]\\s*\\d+",
      "MAX_RETRIES\\s*[=:]\\s*\\d+",
      "retry_count\\s*[=:]\\s*\\d+",
      "\\bretry\\(\\s*\\d+",
    ],
  },
  {
    index: 7,
    slug: "hardcoded-buffer-sizes",
    severity: "warning",
    patterns: [
      "with_capacity\\(\\d+\\)",
      "Vec::with_capacity\\(\\d+\\)",
      "String::with_capacity\\(\\d+\\)",
      "BufReader::with_capacity\\(\\d+",
      "bounded\\(\\d+\\)",
      "channel\\(\\d+\\)",
      "\\[0u8;\\s*\\d+\\]",
      "\\[[0-9]+u8;\\s*\\d+\\]",
    ],
  },
  {
    index: 8,
    slug: "hardcoded-version-strings",
    severity: "info",
    patterns: ['"/v\\d+/', '"v\\d+"', '"v\\d\\.\\d+"', '"\\d+\\.\\d+\\.\\d+"'],
  },
  {
    index: 9,
    slug: "hardcoded-model-names",
    severity: "warning",
    patterns: [
      '"gpt-[^\"]+"',
      "'gpt-[^']+'",
      '"claude[^\"]*"',
      '"whisper[^\"]*"',
      '"\\*[a-z-]*-whisper[^\"]*"',
      '"text-embedding[^\"]*"',
    ],
  },
  {
    index: 10,
    slug: "hardcoded-env-var-names",
    severity: "warning",
    patterns: [
      "(?:std::)?env::var\\(\\s*\"[^\"]+\"\\s*\\)",
      "option_env!\\(\\s*\"[^\"]+\"\\s*\\)",
      "var_os\\(\\s*\"[^\"]+\"\\s*\\)",
    ],
  },
  {
    index: 11,
    slug: "brittle-string-needles",
    severity: "warning",
    patterns: [
      '\\.contains\\(\\s*\\"',
      '\\.starts_with\\(\\s*\\"',
      '\\.ends_with\\(\\s*\\"',
      "\\.includes\\(\\s*\\'",
      "\\.startsWith\\(\\s*\\'",
      "\\.endsWith\\(\\s*\\'",
    ],
  },
  {
    index: 12,
    slug: "brittle-regex-patterns",
    severity: "info",
    patterns: ["Regex::new\\(", 'regex::Regex::new\\(\\s*r#"'],
  },
  {
    index: 13,
    slug: "hardcoded-extensions-globs",
    severity: "info",
    patterns: [
      '\\"\\.rs\\"',
      '\\"\\.ts\\"',
      '\\"\\.tsx\\"',
      '\\"\\.md\\"',
      '\\"\\.toml\\"',
      '\\"\\.json\\"',
      '\\*\\.toml',
      '\\*\\.rs',
    ],
  },
  {
    index: 14,
    slug: "hardcoded-date-literals",
    severity: "info",
    patterns: ["20\\d\\d-\\d\\d-\\d\\d", "from_ymd_opt\\(\\s*20\\d\\d"],
  },
  {
    index: 15,
    slug: "hardcoded-ansi-codes",
    severity: "warning",
    patterns: ["\\\\x1b\\[", "\\\\u\\{1b\\}", "\\u001b\\["],
  },
  {
    index: 16,
    slug: "hardcoded-magic-numbers",
    severity: "info",
    patterns: [
      "\\b1024\\b",
      "\\b2048\\b",
      "\\b4096\\b",
      "\\b8192\\b",
      "\\b16384\\b",
      "\\b65535\\b",
      "\\b65536\\b",
      "\\b1_024\\b",
      "\\b2_048\\b",
      "\\b4_096\\b",
      "\\b8_192\\b",
      "\\b16_384\\b",
      "\\b65_535\\b",
      "\\b65_536\\b",
      "\\b1_048_576\\b",
      "\\b1_000_000\\b",
    ],
  },
  {
    index: 17,
    slug: "hardcoded-canonical-keys",
    severity: "info",
    patterns: [
      '\\"oratio\\"',
      '\\"populi\\"',
      '\\"vox-orchestrator\\"',
      '\\"vox-cli\\"',
      '\\"mens\\"',
    ],
  },
  {
    index: 18,
    slug: "hardcoded-provider-names",
    severity: "info",
    patterns: [
      '\\"openai\\"',
      '\\"anthropic\\"',
      '\\"openrouter\\"',
      '\\"groq\\"',
      '\\"mistral\\"',
      '\\"cohere\\"',
    ],
  },
  {
    index: 19,
    slug: "hardcoded-test-data-in-prod",
    severity: "warning",
    patterns: [
      "test@",
      "@example\\.com",
      "@example\\.org",
      "user@example",
      '00000000-0000-0000-0000-000000000000',
    ],
  },
  {
    index: 20,
    slug: "hardcoded-retired-runtime-names",
    severity: "error",
    patterns: [
      "vox-dei",
      "vox-ars",
      "vox-ludus",
      "vox-lexer",
      "vox-parser",
      "vox-hir",
    ],
  },
];

function collectCategory(cat, registeredEnv) {
  const raw = [];
  const seen = new Set();
  for (const pat of cat.patterns) {
    for (const pass of PASSES) {
      try {
        const chunk = rgJson(pat, pass);
        for (const m of chunk) {
          if (shouldSkipLine(m.text)) continue;
          // Skip cat8 semver in package.json-like — heuristic: skip root package.json paths
          if (
            cat.slug === "hardcoded-version-strings" &&
            m.path.endsWith("package.json")
          ) {
            continue;
          }
          if (cat.slug === "hardcoded-version-strings" && m.path.endsWith(".schema.json")) {
            continue;
          }
          if (
            (cat.slug === "hardcoded-provider-names" ||
              cat.slug === "hardcoded-canonical-keys") &&
            contractsPathOnlySsot(m.path)
          ) {
            continue;
          }
          if (cat.slug === "hardcoded-env-var-names") {
            const mm =
              m.text.match(/(?:std::)?env::var\(\s*"([^"]+)"/) ||
              m.text.match(/option_env!\(\s*"([^"]+)"/) ||
              m.text.match(/var_os\(\s*"([^"]+)"/);
            const name = mm && mm[1];
            if (!name) continue;
            if (registeredEnv.has(name)) continue;
            m.sub = name;
          }
          if (cat.slug === "hardcoded-retired-runtime-names") {
            const p = m.path.replace(/\\/g, "/");
            if (p.startsWith("contracts/reports/")) continue;
            if (
              p.includes("retired-symbols") ||
              p.includes("retired-surfaces") ||
              p.includes("scaling-audit/")
            ) {
              continue;
            }
            const base = (p.split("/").pop() || "").toLowerCase();
            if (
              base === "docs_deprecated_command_guard.rs" ||
              base === "nomenclature_guard.rs" ||
              base === "retired_symbol_check.rs"
            ) {
              continue;
            }
          }
          const pri = matchSignalPriority(cat, m);
          if (pri < 0) continue;
          m._signalPriority = pri;
          const k = uniqKey(cat.slug, m);
          if (seen.has(k)) continue;
          seen.add(k);
          raw.push(m);
        }
      } catch {
        // skip pass
      }
    }
  }
  raw.sort((a, b) => {
    const pa = a._signalPriority ?? 50;
    const pb = b._signalPriority ?? 50;
    if (pb !== pa) return pb - pa;
    if (a.path === b.path) return a.line - b.line;
    return a.path.localeCompare(b.path);
  });
  return raw;
}

function toFinding(idNum, cat, m, registeredEnv) {
  const slug = cat.slug;
  const substr = findMatchSubstring(slug, "", m);
  const conf = m.confidenceOverride || confidenceFor(slug, m.text);
  const sev = cat.severity;
  const rel = m.path;
  const line = m.line;
  const lang = langFromPath(rel);
  const id = `hv-${String(idNum).padStart(4, "0")}`;
  const fx = fixBlueprint(slug, sev);
  const subEsc = escapeRgF(substr.slice(0, 160));
  let ver = `rg -nF "${subEsc}" "${rel}"`;
  if (substr.length > 160) {
    ver = `rg -nF "${escapeRgF(substr.slice(0, 80))}" "${rel}"`;
  }
  const finding = {
    id,
    category: slug,
    category_index: cat.index,
    file: rel,
    line,
    language: lang,
    severity: sev,
    confidence: conf,
    evidence_line: m.text.length > 400 ? m.text.slice(0, 400) + "…" : m.text,
    matched_substring: substr.length > 300 ? substr.slice(0, 300) + "…" : substr,
    why_it_matters: whyText(slug),
    suggested_fix: {
      kind: fx.kind,
      replacement_snippet: fx.replacement_snippet,
      ...(fx.register_in ? { register_in: fx.register_in } : {}),
      ...(fx.notes ? { notes: fx.notes } : {}),
    },
    verification_command: ver,
    ...(fx.register_in || fx.ssot
      ? { related_ssot: fx.register_in || fx.ssot }
      : {}),
  };
  if (slug === "hardcoded-env-var-names") {
    finding.suggested_fix.env_var_canonical = m.sub;
    finding.severity = m.sub.startsWith("VOX_") ? "error" : finding.severity;
  }
  return finding;
}

function writeMarkdown(cat, findingsChunk) {
  let md = `# ${String(cat.index).padStart(2, "0")} — ${cat.slug.replace(/-/g, " ")}\n\n`;
  md += `**Severity**: ${cat.severity}  \n`;
  md += `**Itemized**: ${findingsChunk.length}\n\n`;
  for (const f of findingsChunk) {
    md += `### ${f.id} — \`${f.file}:${f.line}\`\n\n`;
    md += `**Substring**\n\n\`\`\`text\n${f.matched_substring}\n\`\`\`\n\n`;
    md += `**Why it matters**: ${f.why_it_matters}\n\n`;
    md += `**Fix** (${f.suggested_fix.kind}): ${f.suggested_fix.replacement_snippet}\n\n`;
    if (f.related_ssot) md += `**SSOT**: \`${f.related_ssot}\`\n\n`;
    md += `**Verify**: \`${f.verification_command.replace(/`/g, "\\`")}\`\n\n`;
    md += `**Confidence**: ${f.confidence}\n\n---\n\n`;
  }
  const fn = path.join(OUT_DIR, `${String(cat.index).padStart(2, "0")}-${cat.slug}.md`);
  fs.writeFileSync(fn, md, "utf8");
}

function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const registeredEnv = loadRegisteredEnvNames(REPO);
  const allFindings = [];
  const categoryStats = [];
  let nextId = 1;

  for (const cat of CATEGORIES) {
    const raw = collectCategory(cat, registeredEnv);
    raw.sort((a, b) => (a.path === b.path ? a.line - b.line : a.path.localeCompare(b.path)));
    const capApplied = raw.length > ITEM_CAP;
    const chunk = raw.slice(0, ITEM_CAP).map((m) => toFinding(nextId++, cat, m, registeredEnv));
    allFindings.push(...chunk);
    categoryStats.push({
      category_index: cat.index,
      category_slug: cat.slug,
      raw_match_count: raw.length,
      itemized_count: chunk.length,
      cap_applied: capApplied,
    });
    writeMarkdown(cat, chunk);
  }

  const captured = new Date().toISOString();
  const doc = {
    schema_version: "1",
    captured_at_utc: captured,
    scope: {
      description: "Production-only surfaces (excludes tests/, fixtures/, benches/, * _test.rs)",
      include_globs: PASSES.flatMap((p) => p.globs.map((g) => `${p.root}/${g}`)),
      exclude_globs: NEGATIVE_GLOBS,
    },
    category_stats: categoryStats,
    findings: allFindings,
  };

  fs.writeFileSync(path.join(OUT_DIR, "findings.v1.json"), JSON.stringify(doc, null, 2), "utf8");

  const csv = ["category,file,line,severity,id", ...allFindings.map((f) =>
      `"${f.category}","${f.file}",${f.line},"${f.severity}","${f.id}"`)].join("\n");
  fs.writeFileSync(path.join(OUT_DIR, "summary.csv"), csv + "\n", "utf8");

  const totalRaw = categoryStats.reduce((s, c) => s + c.raw_match_count, 0);
  const totalItem = allFindings.length;
  let readme = `# Hardcoded values audit — production code\n\n`;
  readme += `- **Captured**: ${captured}\n`;
  readme += `- **Total itemized findings**: ${totalItem}\n`;
  readme += `- **Total raw matches (pre-cap)**: ${totalRaw}\n`;
  readme += `- **Cap per category**: ${ITEM_CAP}\n\n`;
  readme += `## For follow-up LLMs\n\n`;
  readme += `1. Open \`findings.v1.json\` — each entry is self-contained.\n`;
  readme += `2. Run \`verification_command\` from the repository root; expect at least one match.\n`;
  readme += `3. For \`confidence: "low"\`, confirm in IDE before editing.\n`;
  readme += `4. Regenerate: \`node tools/hardcoded-values-audit.mjs ${path.relative(REPO, OUT_DIR) || "."}\`\n\n`;
  readme += `## Category index\n\n`;
  for (const c of categoryStats) {
    readme += `- [${String(c.category_index).padStart(2, "0")}-${c.category_slug}.md](./${String(c.category_index).padStart(2, "0")}-${c.category_slug}.md) — raw: ${c.raw_match_count}, itemized: ${c.itemized_count}${c.cap_applied ? " (cap)" : ""}\n`;
  }
  readme += `\n## Schema\n\n- [findings.v1.schema.json](./findings.v1.schema.json)\n`;
  readme += `\n## Methodology\n\n- [methodology.md](./methodology.md)\n`;
  fs.writeFileSync(path.join(OUT_DIR, "README.md"), readme, "utf8");

  console.error(`Wrote ${totalItem} findings to ${OUT_DIR}`);

  // Schema enum sanity-check (full JSON Schema validation via ajv not required here).
  /*
  const fdef = JSON.parse(
    fs.readFileSync(path.join(REPO, "contracts/reports/scaling-audit/findings-array.v1.schema.json"), "utf8"),
  ).$defs.finding;
  const catEnum = new Set(fdef.properties.category.enum);
  const sevEnum = new Set(fdef.properties.severity.enum);
  const confEnum = new Set(fdef.properties.confidence.enum);
  const langEnum = new Set(fdef.properties.language.enum);
  const fixKinds = new Set(fdef.properties.suggested_fix.properties.kind.enum);
  let vErr = 0;
  for (const f of allFindings) {
    if (!catEnum.has(f.category)) vErr++;
    if (!sevEnum.has(f.severity)) vErr++;
    if (!confEnum.has(f.confidence)) vErr++;
    if (!langEnum.has(f.language)) vErr++;
    if (!f.suggested_fix || !fixKinds.has(f.suggested_fix.kind)) vErr++;
  }
  if (vErr) {
    console.error(`Schema enum validation failed (${vErr} issues)`);
    process.exit(1);
  }
  */

  const ciGateArgIndex = process.argv.indexOf("--ci-gate");
  if (ciGateArgIndex !== -1 && process.argv[ciGateArgIndex + 1]) {
    const baselineFile = process.argv[ciGateArgIndex + 1];
    let baselineCount = 1395;
    try {
      baselineCount = parseInt(fs.readFileSync(baselineFile, "utf8").trim(), 10);
    } catch (e) {
      console.warn(`Could not read baseline file ${baselineFile}, defaulting to ${baselineCount}`);
    }

    console.log(`Current hardcoded value findings: ${totalItem}`);
    console.log(`Maximum allowed baseline count: ${baselineCount}`);

    if (totalItem > baselineCount) {
      console.error(`ERROR: Hardcoded values increased from ${baselineCount} to ${totalItem}`);
      console.error(`Please remediate the new hardcoded values or update ${baselineFile} if strictly necessary.`);
      process.exit(1);
    } else {
      console.log(`Hardcoded values gate: clean ✓`);
    }
  }
}

main();
