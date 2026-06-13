"""Deterministic platform-compatibility scan over the Vox crates.

Surfaces patterns that threaten Vox's Mac/Linux/Windows portability. Findings are
graded by confidence. Occurrences INSIDE a matching `#[cfg(...)]`-gated function are
de-prioritized (gated code is the *correct* way to be OS-specific); the smell is
UN-gated OS assumptions.

Usage: python os_compat.py --repo-root <repo> --out <report.md>
"""
import argparse
import re
from collections import defaultdict
from pathlib import Path

# (category, confidence, compiled regex, note)
RULES = [
    ("abs-unix-path", "high", re.compile(r'"(/(?:tmp|usr|etc|var|home|opt|bin|proc|sys|dev|root)\b[^"]*)"'),
     "Hardcoded absolute Unix path literal — breaks on Windows. Use std::env::temp_dir()/dirs/Path."),
    ("home-tilde", "high", re.compile(r'"(~/[^"]*)"'),
     "Literal ~ home path — not expanded on Windows. Use the `dirs`/`home` crate."),
    ("win-drive-path", "high", re.compile(r'"([A-Za-z]:\\\\[^"]*)"'),
     "Hardcoded Windows drive path — breaks on Unix."),
    ("shell-command", "high", re.compile(r'Command::new\(\s*"(sh|bash|cmd|cmd\.exe|powershell|pwsh|zsh|/bin/[a-z]+)"'),
     "OS-specific shell invocation — pick per-OS or avoid the shell."),
    ("dynlib-ext", "high", re.compile(r'"\.(so|dylib|dll)"|\.(so|dylib|dll)"'),
     "Hardcoded dynamic-lib extension — differs per OS (.so/.dylib/.dll)."),
    ("env-home-asym", "high", re.compile(r'(?:env::var|var|getenv)\(\s*"HOME"'),
     "Reads HOME (Unix) — Windows uses USERPROFILE. Use the `dirs` crate."),
    ("path-sep-env", "high", re.compile(r"\.split\(\s*'[:;]'\s*\)|split\(\"[:;]\"\)"),
     "Splitting on ':' or ';' — PATH separator differs per OS. Use std::env::split_paths."),
    ("path-join-fmt", "medium", re.compile(r'format!\(\s*"[^"]*\{\}/\{\}'),
     "Building a path with `/` in format! — use Path::join / PathBuf for portability."),
    ("crlf-literal", "medium", re.compile(r'"\\r\\n"'),
     "Hardcoded CRLF literal — line-ending assumption."),
    ("os-unix-api", "medium", re.compile(r"std::os::unix|PermissionsExt|from_mode|\.mode\(0o"),
     "Unix-only OS API (permissions/mode). Needs a Windows path or cfg gate."),
    ("os-windows-api", "medium", re.compile(r"std::os::windows|CREATE_NO_WINDOW|winapi|windows_sys"),
     "Windows-only OS API. Needs a Unix path or cfg gate."),
    ("unix-symlink", "medium", re.compile(r"std::os::unix::fs::symlink|unix::fs::symlink"),
     "Unix symlink API — Windows uses symlink_file/symlink_dir."),
]

CFG_OS = re.compile(r"#\[cfg\([^)]*(windows|unix|target_os\s*=\s*\"(?:linux|macos|windows|ios|android)\")[^)]*\)\]")


def gated_lines(text: str) -> set:
    """Approximate set of line numbers inside a #[cfg(os...)]-attributed item/block.

    When a cfg(os) attribute precedes an item, mark the following brace-balanced block
    (or single statement up to ';') as gated.
    """
    gated = set()
    lines = text.split("\n")
    for idx, line in enumerate(lines):
        if CFG_OS.search(line):
            # find next non-attribute line, then balance braces from there
            j = idx + 1
            while j < len(lines) and lines[j].strip().startswith("#["):
                j += 1
            # scan forward balancing braces; cap span to avoid runaway
            depth = 0
            started = False
            k = j
            while k < len(lines) and k < j + 400:
                gated.add(k + 1)
                opens = lines[k].count("{")
                closes = lines[k].count("}")
                depth += opens - closes
                if opens:
                    started = True
                if started and depth <= 0:
                    break
                if not started and lines[k].rstrip().endswith(";"):
                    break
                k += 1
    return gated


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo-root", default=".")
    ap.add_argument("--out", default="graphify-out/OS_COMPATIBILITY.md")
    args = ap.parse_args()
    repo = Path(args.repo_root)

    findings = defaultdict(list)   # (category, confidence) -> [(file,line,gated,snippet)]
    cfg_counts = defaultdict(lambda: [0, 0])  # file -> [win, unix]
    nfiles = 0
    for rs in (repo / "crates").rglob("*.rs"):
        s = str(rs).replace("\\", "/")
        if "/target/" in s:
            continue
        try:
            text = rs.read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        nfiles += 1
        rel = str(rs.relative_to(repo)).replace("\\", "/")
        # vox-code-audit detector sources are deliberately full of example bad-code
        # strings (they ARE the fixtures); exclude from string-literal path rules.
        is_detector_fixture = "vox-code-audit/src/detectors/" in rel
        gated = gated_lines(text)
        lines = text.split("\n")
        for i, line in enumerate(lines, 1):
            # skip URL/markup contexts that look like absolute paths but aren't
            line_is_url = ("href=" in line) or ("://" in line) or ('src="/' in line)
            if "cfg(" in line:
                if re.search(r"windows", line):
                    cfg_counts[rel][0] += 1
                if re.search(r"\bunix\b|target_os\s*=\s*\"(?:linux|macos)\"", line):
                    cfg_counts[rel][1] += 1
            for cat, conf, rx, _note in RULES:
                if cat in ("abs-unix-path", "win-drive-path", "dynlib-ext") and is_detector_fixture:
                    continue
                if cat in ("abs-unix-path", "win-drive-path") and line_is_url:
                    continue
                if rx.search(line):
                    findings[(cat, conf)].append((rel, i, i in gated, line.strip()[:120]))

    notes = {cat: note for cat, _c, _rx, note in RULES}
    order = {"high": 0, "medium": 1}
    cats = sorted(findings.keys(), key=lambda k: (order[k[1]], -len(findings[k])))

    # asymmetric cfg: file handles one OS but not the other
    asym = [(f, w, u) for f, (w, u) in cfg_counts.items() if (w > 0) != (u > 0)]
    asym.sort(key=lambda x: -(x[1] + x[2]))

    out = ["# OS / Platform Compatibility — deterministic scan\n",
           f"Scanned {nfiles} Rust files across crates/. Goal: maintain Mac/Linux/Windows parity.\n",
           "Findings inside a matching `#[cfg(os)]` block are marked `[gated]` (expected); "
           "**un-gated** findings are the real portability smells.\n"]
    total_ungated = 0
    out.append("\n## Summary by category\n")
    for cat, conf in cats:
        items = findings[(cat, conf)]
        ung = sum(1 for *_, g, _ in items if not g)
        total_ungated += ung
        out.append(f"- **{cat}** ({conf}): {len(items)} hits, **{ung} un-gated**")
    out.append(f"\n**Total un-gated portability findings: {total_ungated}**")
    out.append(f"\nAsymmetric cfg files (handle one OS, not the other): {len(asym)}\n")

    for cat, conf in cats:
        items = findings[(cat, conf)]
        ungated = [x for x in items if not x[2]]
        if not ungated:
            continue
        out.append(f"\n## {cat} — {conf}  ({len(ungated)} un-gated)\n")
        out.append(f"_{notes[cat]}_\n")
        for f, l, _g, snip in ungated[:25]:
            out.append(f"- `{f}:{l}` — `{snip}`")
        if len(ungated) > 25:
            out.append(f"- … +{len(ungated) - 25} more")

    out.append("\n## Asymmetric cfg files (top 30)\n")
    out.append("_One OS handled, the other absent — likely a missing platform branch._\n")
    for f, w, u in asym[:30]:
        which = "windows-only" if w > 0 else "unix-only"
        out.append(f"- `{f}` — {which} (win={w}, unix={u})")

    Path(args.out).write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    print(f"ungated={total_ungated} asym={len(asym)} categories={len(cats)}")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
