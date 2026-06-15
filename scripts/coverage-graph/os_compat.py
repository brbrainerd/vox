"""Deterministic platform-compatibility scan over the Vox crates and scripts.

Surfaces patterns that threaten Vox's Mac/Linux/Windows portability. Findings are
graded by confidence. Occurrences INSIDE a matching `#[cfg(...)]`-gated function are
de-prioritized (gated code is the *correct* way to be OS-specific); the smell is
UN-gated OS assumptions.

Usage:
  python os_compat.py [--repo-root <repo>] [--out <report.md>] [--baseline <prev.md>]

--baseline: path to a previous run's output file for trend tracking.
"""
import argparse
import re
import sys
from collections import defaultdict
from pathlib import Path

# ---------------------------------------------------------------------------
# Hardcoded fallback RULES — used when os_compat_rules.toml is not found.
# (category, confidence, compiled regex, note, detector_fixture_exempt, url_exempt)
# ---------------------------------------------------------------------------
RULES_FALLBACK = [
    ("abs-unix-path", "high",
     re.compile(r'"(/(?:tmp|usr|etc|var|home|opt|bin|proc|sys|dev|root)\b[^"]*)"'),
     "Hardcoded absolute Unix path literal — breaks on Windows. Use std::env::temp_dir()/dirs/Path.",
     True, True),
    ("home-tilde", "high",
     re.compile(r'"(~/[^"]*)"'),
     "Literal ~ home path — not expanded on Windows. Use the `dirs`/`home` crate.",
     False, False),
    ("win-drive-path", "high",
     re.compile(r'"([A-Za-z]:\\\\[^"]*)"'),
     "Hardcoded Windows drive path — breaks on Unix.",
     False, False),
    ("shell-command", "high",
     re.compile(r'Command::new\(\s*"(sh|bash|cmd|cmd\.exe|powershell|pwsh|zsh|/bin/[a-z]+)"'),
     "OS-specific shell invocation — pick per-OS or avoid the shell.",
     False, False),
    ("dynlib-ext", "high",
     re.compile(r'"\.(so|dylib|dll)"|\.(so|dylib|dll)"'),
     "Hardcoded dynamic-lib extension — differs per OS (.so/.dylib/.dll).",
     True, False),
    ("env-home-asym", "high",
     re.compile(r'(?:env::var|var|getenv)\(\s*"HOME"'),
     "Reads HOME (Unix) — Windows uses USERPROFILE. Use the `dirs` crate.",
     False, False),
    ("path-sep-env", "high",
     re.compile(r"\.split\(\s*'[:;]'\s*\)|split\(\"[:;]\"\)"),
     "Splitting on ':' or ';' — PATH separator differs per OS. Use std::env::split_paths.",
     False, False),
    ("path-join-fmt", "medium",
     re.compile(r'format!\(\s*"[^"]*\{\}/\{\}'),
     "Building a path with `/` in format! — use Path::join / PathBuf for portability.",
     False, False),
    ("crlf-literal", "medium",
     re.compile(r'"\\r\\n"'),
     "Hardcoded CRLF literal — line-ending assumption.",
     False, False),
    ("os-unix-api", "medium",
     re.compile(r"std::os::unix|PermissionsExt|from_mode|\.mode\(0o"),
     "Unix-only OS API (permissions/mode). Needs a Windows path or cfg gate.",
     False, False),
    ("os-windows-api", "medium",
     re.compile(r"std::os::windows|CREATE_NO_WINDOW|winapi|windows_sys"),
     "Windows-only OS API. Needs a Unix path or cfg gate.",
     False, False),
    ("unix-symlink", "medium",
     re.compile(r"std::os::unix::fs::symlink|unix::fs::symlink"),
     "Unix symlink API — Windows uses symlink_file/symlink_dir.",
     False, False),
]


def _load_toml_rules(toml_path: Path):
    """Load rules from os_compat_rules.toml.

    Returns a list of (name, confidence, compiled_re, note, det_exempt, url_exempt)
    tuples, or None if loading fails (caller falls back to RULES_FALLBACK).
    """
    if not toml_path.exists():
        return None

    # Try tomllib (stdlib Python >= 3.11), then tomli (third-party), then fail.
    tomllib = None
    if sys.version_info >= (3, 11):
        import tomllib as _tl
        tomllib = _tl
    else:
        try:
            import tomli as _tl  # type: ignore[import]
            tomllib = _tl
        except ImportError:
            return None  # no TOML parser available — use fallback

    try:
        with toml_path.open("rb") as fh:
            data = tomllib.load(fh)
    except Exception as exc:
        print(f"[os_compat] warning: failed to parse {toml_path}: {exc}", file=sys.stderr)
        return None

    rules = []
    for entry in data.get("rule", []):
        try:
            rx = re.compile(entry["pattern"])
        except re.error as exc:
            print(f"[os_compat] warning: bad regex in rule {entry.get('name')!r}: {exc}",
                  file=sys.stderr)
            continue
        rules.append((
            entry["name"],
            entry.get("confidence", "medium"),
            rx,
            entry.get("note", ""),
            bool(entry.get("detector_fixture_exempt", False)),
            bool(entry.get("url_exempt", False)),
        ))
    return rules or None


def _load_rules(script_dir: Path):
    """Return the active rule list, preferring TOML over the hardcoded fallback."""
    toml_path = script_dir / "os_compat_rules.toml"
    rules = _load_toml_rules(toml_path)
    if rules is not None:
        print(f"[os_compat] loaded {len(rules)} rules from {toml_path.name}", file=sys.stderr)
        return rules
    print(f"[os_compat] using {len(RULES_FALLBACK)} hardcoded fallback rules", file=sys.stderr)
    return RULES_FALLBACK


CFG_OS = re.compile(
    r"#\[cfg\([^)]*(windows|unix|target_os\s*=\s*\"(?:linux|macos|windows|ios|android)\")[^)]*\)\]"
)


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


def _scan_files(file_iter, rules, findings, cfg_counts, repo):
    """Scan an iterable of Path objects using the given rules.

    findings: defaultdict(list) keyed by (category, confidence)
    cfg_counts: defaultdict keyed by rel-path -> [win_count, unix_count]
    """
    nfiles = 0
    for path in file_iter:
        s = str(path).replace("\\", "/")
        if "/target/" in s:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        nfiles += 1
        rel = str(path.relative_to(repo)).replace("\\", "/")
        is_detector_fixture = "vox-code-audit/src/detectors/" in rel
        gated = gated_lines(text)
        lines = text.split("\n")
        for i, line in enumerate(lines, 1):
            line_is_url = ("href=" in line) or ("://" in line) or ('src="/' in line)
            if "cfg(" in line:
                if re.search(r"windows", line):
                    cfg_counts[rel][0] += 1
                if re.search(r"\bunix\b|target_os\s*=\s*\"(?:linux|macos)\"", line):
                    cfg_counts[rel][1] += 1
            for cat, conf, rx, _note, det_exempt, url_exempt in rules:
                if det_exempt and is_detector_fixture:
                    continue
                if url_exempt and line_is_url:
                    continue
                if rx.search(line):
                    findings[(cat, conf)].append((rel, i, i in gated, line.strip()[:120]))
    return nfiles


def _parse_baseline(baseline_path: Path):
    """Parse a previous run's markdown report for trend comparison.

    Returns (total_ungated, {category: ungated_count}) or (None, {}) on failure.
    """
    try:
        text = baseline_path.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return None, {}

    total = None
    m = re.search(r"\*\*Total un-gated portability findings:\s*(\d+)\*\*", text)
    if m:
        total = int(m.group(1))

    # Parse summary lines like: - **cat-name** (conf): N hits, **M un-gated**
    cat_counts = {}
    for m in re.finditer(r"-\s+\*\*([^*]+)\*\*\s+\([^)]+\):\s+\d+\s+hits,\s+\*\*(\d+)\s+un-gated\*\*", text):
        cat_counts[m.group(1).strip()] = int(m.group(2))

    return total, cat_counts


def _trend_section(baseline_path: Path, current_total: int, current_cats: dict) -> list:
    """Build the ## Trend since baseline markdown section lines."""
    prev_total, prev_cats = _parse_baseline(baseline_path)
    lines = ["\n## Trend since baseline\n",
             f"_Baseline: `{baseline_path}`_\n"]

    if prev_total is None:
        lines.append("_Could not parse baseline file — trend unavailable._")
        return lines

    # Per-category diff
    all_cats = sorted(set(list(prev_cats.keys()) + list(current_cats.keys())))
    for cat in all_cats:
        prev_n = prev_cats.get(cat, 0)
        curr_n = current_cats.get(cat, 0)
        delta = curr_n - prev_n
        if delta < 0:
            lines.append(f"- {cat}: {prev_n} → {curr_n} ({delta} ✓)")
        elif delta > 0:
            lines.append(f"- {cat}: {prev_n} → {curr_n} (+{delta} ✗)")
        else:
            lines.append(f"- {cat}: {prev_n} → {curr_n} (no change)")

    net = current_total - prev_total
    if net < 0:
        verdict = "improving"
        net_str = str(net)
    elif net > 0:
        verdict = "regressing"
        net_str = f"+{net}"
    else:
        verdict = "stable"
        net_str = "0"

    lines.append(f"\n**Net change: {net_str} ({verdict})**")
    return lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo-root", default=".")
    ap.add_argument("--out", default="graphify-out/OS_COMPATIBILITY.md")
    ap.add_argument(
        "--baseline",
        default=None,
        help="Path to a previous run's output file for trend comparison.",
    )
    args = ap.parse_args()
    repo = Path(args.repo_root).resolve()

    script_dir = Path(__file__).parent.resolve()
    rules = _load_rules(script_dir)

    findings: defaultdict = defaultdict(list)    # (category, confidence) -> [(file,line,gated,snippet)]
    cfg_counts: defaultdict = defaultdict(lambda: [0, 0])  # rel -> [win, unix]

    # ── Rust crates scan ────────────────────────────────────────────────────
    rust_files = (repo / "crates").rglob("*.rs")
    nfiles_rs = _scan_files(rust_files, rules, findings, cfg_counts, repo)

    # ── Vox script scan (scripts/**/*.vox) ──────────────────────────────────
    vox_findings: defaultdict = defaultdict(list)
    vox_cfg: defaultdict = defaultdict(lambda: [0, 0])
    scripts_dir = repo / "scripts"
    nfiles_vox = 0
    if scripts_dir.exists():
        vox_files = scripts_dir.rglob("*.vox")
        nfiles_vox = _scan_files(vox_files, rules, vox_findings, vox_cfg, repo)

    notes = {cat: note for cat, _c, _rx, note, *_ in rules}
    order = {"high": 0, "medium": 1, "low": 2}
    cats = sorted(findings.keys(), key=lambda k: (order.get(k[1], 9), -len(findings[k])))
    vox_cats = sorted(vox_findings.keys(), key=lambda k: (order.get(k[1], 9), -len(vox_findings[k])))

    # asymmetric cfg: file handles one OS but not the other
    all_cfg = dict(cfg_counts)
    all_cfg.update({k: vox_cfg[k] for k in vox_cfg})
    asym = [(f, w, u) for f, (w, u) in all_cfg.items() if (w > 0) != (u > 0)]
    asym.sort(key=lambda x: -(x[1] + x[2]))

    total_ungated = 0
    # Build per-category ungated counts for trend tracking
    current_cat_counts: dict = {}

    out = [
        "# OS / Platform Compatibility — deterministic scan\n",
        f"Scanned **{nfiles_rs}** Rust files (crates/) + **{nfiles_vox}** Vox script files (scripts/)."
        " Goal: maintain Mac/Linux/Windows parity.\n",
        "Findings inside a matching `#[cfg(os)]` block are marked `[gated]` (expected); "
        "**un-gated** findings are the real portability smells.\n",
    ]

    out.append("\n## Summary by category\n")
    for cat, conf in cats:
        items = findings[(cat, conf)]
        ung = sum(1 for *_, g, _ in items if not g)
        total_ungated += ung
        current_cat_counts[cat] = current_cat_counts.get(cat, 0) + ung
        out.append(f"- **{cat}** ({conf}): {len(items)} hits, **{ung} un-gated**")

    for cat, conf in vox_cats:
        items = vox_findings[(cat, conf)]
        ung = sum(1 for *_, g, _ in items if not g)
        total_ungated += ung
        current_cat_counts[f"[vox]{cat}"] = current_cat_counts.get(f"[vox]{cat}", 0) + ung
        out.append(f"- **[vox]{cat}** ({conf}): {len(items)} hits, **{ung} un-gated** (Vox scripts)")

    out.append(f"\n**Total un-gated portability findings: {total_ungated}**")
    out.append(f"\nAsymmetric cfg files (handle one OS, not the other): {len(asym)}\n")

    # ── Rust detail sections ─────────────────────────────────────────────────
    for cat, conf in cats:
        items = findings[(cat, conf)]
        ungated = [x for x in items if not x[2]]
        if not ungated:
            continue
        out.append(f"\n## {cat} — {conf}  ({len(ungated)} un-gated)\n")
        out.append(f"_{notes.get(cat, '')}_\n")
        for f, l, _g, snip in ungated[:25]:
            out.append(f"- `{f}:{l}` — `{snip}`")
        if len(ungated) > 25:
            out.append(f"- … +{len(ungated) - 25} more")

    # ── Vox script detail sections ───────────────────────────────────────────
    for cat, conf in vox_cats:
        items = vox_findings[(cat, conf)]
        ungated = [x for x in items if not x[2]]
        if not ungated:
            continue
        out.append(f"\n## [vox]{cat} — {conf}  ({len(ungated)} un-gated, Vox scripts)\n")
        out.append(f"_{notes.get(cat, '')}_\n")
        for f, l, _g, snip in ungated[:25]:
            out.append(f"- `{f}:{l}` — `{snip}`")
        if len(ungated) > 25:
            out.append(f"- … +{len(ungated) - 25} more")

    out.append("\n## Asymmetric cfg files (top 30)\n")
    out.append("_One OS handled, the other absent — likely a missing platform branch._\n")
    for f, w, u in asym[:30]:
        which = "windows-only" if w > 0 else "unix-only"
        out.append(f"- `{f}` — {which} (win={w}, unix={u})")

    # ── Trend section (optional) ─────────────────────────────────────────────
    if args.baseline:
        baseline_path = Path(args.baseline)
        out.extend(_trend_section(baseline_path, total_ungated, current_cat_counts))

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    print(f"ungated={total_ungated} asym={len(asym)} categories={len(cats) + len(vox_cats)}")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
