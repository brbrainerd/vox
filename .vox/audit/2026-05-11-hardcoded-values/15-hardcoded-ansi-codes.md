# 15 — hardcoded ansi codes

**Severity**: warning  
**Itemized**: 5

### hv-1090 — `contracts/reports/scaling-audit/findings-scaling-latest.json:1639`

**Substring**

```text
\x1b[
```

**Why it matters**: Raw ANSI escapes break non-TTY consumers and theming.

**Fix** (use-theme-aware-styles): Prefer owo-colors, anstyle, or terminal abstraction — avoid raw ESC bytes.

**Verify**: `rg -nF "\\x1b[" "contracts/reports/scaling-audit/findings-scaling-latest.json"`

**Confidence**: medium

---

### hv-1091 — `crates/vox-cli/src/commands/status.rs:120`

**Substring**

```text
\x1b[
```

**Why it matters**: Raw ANSI escapes break non-TTY consumers and theming.

**Fix** (use-theme-aware-styles): Prefer owo-colors, anstyle, or terminal abstraction — avoid raw ESC bytes.

**Verify**: `rg -nF "\\x1b[" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: medium

---

### hv-1092 — `crates/vox-cli/src/commands/status.rs:122`

**Substring**

```text
\x1b[
```

**Why it matters**: Raw ANSI escapes break non-TTY consumers and theming.

**Fix** (use-theme-aware-styles): Prefer owo-colors, anstyle, or terminal abstraction — avoid raw ESC bytes.

**Verify**: `rg -nF "\\x1b[" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: medium

---

### hv-1093 — `crates/vox-cli/src/commands/status.rs:124`

**Substring**

```text
\x1b[
```

**Why it matters**: Raw ANSI escapes break non-TTY consumers and theming.

**Fix** (use-theme-aware-styles): Prefer owo-colors, anstyle, or terminal abstraction — avoid raw ESC bytes.

**Verify**: `rg -nF "\\x1b[" "crates/vox-cli/src/commands/status.rs"`

**Confidence**: medium

---

### hv-1094 — `crates/vox-ml-cli/src/commands/mens/status.rs:410`

**Substring**

```text
\x1b[
```

**Why it matters**: Raw ANSI escapes break non-TTY consumers and theming.

**Fix** (use-theme-aware-styles): Prefer owo-colors, anstyle, or terminal abstraction — avoid raw ESC bytes.

**Verify**: `rg -nF "\\x1b[" "crates/vox-ml-cli/src/commands/mens/status.rs"`

**Confidence**: medium

---

