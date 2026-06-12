# voxup

Hermetic toolchain multiplexer and installer for Vox (rustup-style bootstrap).

## Install

From a release channel (when published):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

Windows (PowerShell):

```powershell
Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1
.\voxup.ps1
```

From this repository (development):

```bash
cargo install --path crates/voxup
voxup install default
```

`voxup install` downloads pinned toolchains into `~/.vox/toolchains/` and links `~/.vox/bin/vox` as a PATH proxy. See [voxup omnibus installer spec](../../docs/src/architecture/voxup-omnibus-installer-spec-2026.md).

## Commands

| Command | Purpose |
| --- | --- |
| `voxup install [profile]` | Install or update the default Vox CLI + mandatory toolchains |
| `voxup proxy -- <vox args…>` | Run `vox` with hermetic `PATH` prepended |

## Related

- `vox doctor` — environment health after install
- `crates/_public.toml` — crates eligible for `cargo publish` (CR-K2)
