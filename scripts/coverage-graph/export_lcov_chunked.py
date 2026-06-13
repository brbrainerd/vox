"""Export an lcov file from an existing llvm-cov `.profdata` by CHUNKING the object
list — a Windows workaround for `cargo llvm-cov ... export` overflowing the command
line when the workspace has hundreds of test binaries (os error 206).

llvm-cov export with a subset of `-object` binaries + the full profdata reports coverage
for just those binaries' functions; the union across chunks is the full picture. The
downstream ingester ORs `reached` across records, so concatenating the chunk lcovs is
correct. (CI on Linux has no arg limit and exports lcov directly — this is local-only.)

Usage:
  python export_lcov_chunked.py --profdata target/llvm-cov-target/vox.profdata \
      --bin-dir target/llvm-cov-target/debug --out target/llvm-cov-lcov.info [--chunk 60]
"""
import argparse
import subprocess
import sys
from pathlib import Path

LLVM_COV = Path(
    "C:/Users/Owner/.rustup/toolchains/1.96.0-x86_64-pc-windows-msvc"
    "/lib/rustlib/x86_64-pc-windows-msvc/bin/llvm-cov.exe"
)
NO_WINDOW = 0x08000000  # CREATE_NO_WINDOW — no flashing consoles on Windows


def export_chunk(profdata: Path, objs, out_path: Path) -> bool:
    cmd = [str(LLVM_COV), "export", "-format=lcov", f"-instr-profile={profdata}",
           "--ignore-filename-regex", r"(\\.cargo\\|/\.cargo/|rustc|/rustlib/|target[\\/]llvm-cov)"]
    cmd.append(str(objs[0]))
    for o in objs[1:]:
        cmd += ["-object", str(o)]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, creationflags=NO_WINDOW)
    except Exception as e:  # noqa
        print(f"  chunk error: {e}", file=sys.stderr)
        return False
    if r.returncode != 0:
        print(f"  chunk failed (rc={r.returncode}): {r.stderr.strip().splitlines()[-1:]}", file=sys.stderr)
        return False
    out_path.write_text(r.stdout, encoding="utf-8")
    return True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--profdata", required=True)
    ap.add_argument("--bin-dir", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--chunk", type=int, default=60)
    args = ap.parse_args()

    profdata = Path(args.profdata)
    bin_dir = Path(args.bin_dir)
    objs = sorted(set(list(bin_dir.glob("*.exe")) + list((bin_dir / "deps").glob("*.exe"))))
    print(f"{len(objs)} instrumented binaries; chunk={args.chunk}")

    tmp = Path(args.out).with_suffix(".chunks")
    tmp.mkdir(exist_ok=True)
    parts, ok, fail = [], 0, 0
    chunks = [objs[i:i + args.chunk] for i in range(0, len(objs), args.chunk)]
    for ci, chunk in enumerate(chunks):
        cp = tmp / f"chunk_{ci:03d}.lcov"
        if export_chunk(profdata, chunk, cp):
            parts.append(cp)
            ok += 1
        else:
            # retry this chunk one object at a time so one bad binary doesn't lose 60
            for j, o in enumerate(chunk):
                cp1 = tmp / f"chunk_{ci:03d}_{j:03d}.lcov"
                if export_chunk(profdata, [o], cp1):
                    parts.append(cp1)
            fail += 1
        print(f"  chunk {ci + 1}/{len(chunks)} done")

    with Path(args.out).open("w", encoding="utf-8") as fh:
        for p in parts:
            fh.write(p.read_text(encoding="utf-8", errors="replace"))
    print(f"chunks ok={ok} fallback={fail}; wrote {args.out} ({Path(args.out).stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
