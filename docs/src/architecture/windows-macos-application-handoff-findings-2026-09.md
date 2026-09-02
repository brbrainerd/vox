---
title: "Windows-to-macOS application handoff"
description: "A September 2026 inventory of this Windows workstation and practical macOS migration choices."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---
# Windows-to-macOS Application Handoff — September 2026
## Purpose and evidence
This handoff documents the workstation software inventory captured on 2026-09-02 and recommends a macOS migration path. It does not claim that every installed package is needed or that every Steam title runs on macOS.

The inventory combined:

- `winget list --accept-source-agreements` for installed packages, classic uninstall entries, and Store registrations.
- `C:\Program Files`, `C:\Program Files (x86)`, and `%LOCALAPPDATA%\Programs` for desktop applications with incomplete package-manager identities.
- `Get-AppxPackage` for Microsoft Store/Appx packages. This is the reliable source for protected `C:\Program Files\WindowsApps`; its contents should not be copied as application files.

Several packages are duplicated or side-by-side. On Windows, reassess the intentional version of Cursor, CrystalDiskMark, ImageMagick, Subtitle Edit, and the CUDA toolkits before retaining them. macOS does not use CUDA.
## Migration order
1. Install cross-platform development, communication, and cloud-sync applications first.
2. Move repositories, documents, browser profiles, exports, and passwords before configuring replacements.
3. Rebuild Windows-only workflows from the mapping below.
4. Do not migrate drivers, Windows runtimes, Appx frameworks, Microsoft Store infrastructure, or vendor control panels. macOS provides platform-specific replacements.
5. For games, install Steam and verify every title's macOS build and cloud-save status before transferring data.
## Keep the same application on macOS
| Area | Applications found | macOS action |
| --- | --- | --- |
| AI and editors | Claude, Claude Code, Cursor, Visual Studio Code, Antigravity, LM Studio | Install current macOS releases when offered. For Antigravity, verify the vendor build; Cursor or VS Code is the fallback. |
| Development | Git, GitHub CLI, GitHub Desktop, jj, Node.js, Deno, Python, Rustup, LLVM, LuaJIT, uv, PowerShell, DBeaver | Install through the vendor or Homebrew. macOS provides a Unix environment, so WSL is unnecessary. |
| Browsers | Chrome, Edge, Firefox, Thorium, Zen Browser | Install matching browser builds where currently supported; Safari is also available. |
| Collaboration | Discord, Slack, Microsoft Teams, Zoom Workplace, WhatsApp, Chrome Remote Desktop | Use their macOS clients. |
| Remote/networking | Tailscale, Parsec, Moonlight, GeForce NOW, Google Drive | Use macOS clients. The Windows-only Parsec virtual display and USB drivers do not transfer. |
| Media | Audacity, Blender, HandBrake, ImageMagick, Krita, MediaInfo, OBS Studio, Topaz Video AI, VLC, XnConvert, yt-dlp, FFmpeg, Jellyfin Media Player, Spotify | Install the same macOS applications. Homebrew suits FFmpeg, ImageMagick, and yt-dlp. |
| Productivity | Evernote, LibreOffice, Qalculate!, Xmind, Zotero, Outlook, OneDrive, Microsoft To Do | Use the same macOS client where available or supported web access. |
| Hardware/smart devices | Philips Hue Sync, Logi Options+ | Use macOS releases. Brother, Lenovo, Intel, NVIDIA, ELAN, Synaptics, Realtek, and Thunderbolt packages are Windows hardware support and should not transfer. |
| General | Docker Desktop, OpenTTD, Mudlet, Old School RuneScape, Warp | Install the macOS release. Reassess Docker Desktop if a lighter container stack is preferred. |
## Windows-only replacement matrix
| Windows application or workflow | Best macOS choice | Handoff note |
| --- | --- | --- |
| 7-Zip | Keka; `7zz` with Homebrew for CLI use | Keka is the practical native archive UI. |
| AltSnap | BetterTouchTool or Rectangle | BetterTouchTool is best for advanced input/window automation; Rectangle is simpler. |
| AutoHotkey | Keyboard Maestro | Rebuild significant hotkeys and macros; scripts do not run natively. |
| Autoruns | KnockKnock plus Login Items | Use KnockKnock for persistence checks and System Settings for normal management. |
| BCUninstaller | AppCleaner | Removes app bundles and associated support files. |
| Bulk Rename Utility | Name Mangler | Finder batch rename is sufficient for simple jobs. |
| Cheat Engine | No direct universal replacement | Evaluate memory tooling per game; do not assume Windows trainers work on macOS. |
| CPU-Z, GPU-Z, Intel Graphics Command Center, NVIDIA Control Panel | Stats and System Information; iStat Menus optional | Stats is the free menu-bar default. Apple Silicon does not expose the NVIDIA/CUDA model. |
| CrystalDiskMark | Blackmagic Disk Speed Test | Appropriate for macOS storage throughput tests. |
| Ditto | Maccy or Raycast Clipboard History | Maccy is focused; Raycast is broader. |
| EasyJoin | LocalSend | Cross-platform local file transfer. |
| AudioRelay | Audio MIDI Setup with BlackHole; Airfoil optional | Use the free native routing path first. Buy Airfoil only when its device-streaming workflow is required. |
| Equalizer APO and Peace | eqMac | Recreate profiles manually. |
| ExplorerPatcher, MSEdgeRedirect, Command Palette | Native settings, Raycast, or Alfred | Windows shell modifications have no migration value. |
| Borderless Gaming | Native fullscreen/window controls or BetterDisplay | Most macOS games need no borderless-window utility; BetterDisplay is useful for display control. |
| FastStone Image Viewer, IrfanView, ImageGlass | XnView MP or Pixea | XnView MP is feature-rich; Pixea is lightweight. |
| FileSeek | Spotlight or Raycast; HoudahSpot optional | Start with free indexed search; buy HoudahSpot only for advanced query features. |
| f.lux | Night Shift | Native color-temperature scheduling. |
| Hot Alarm Clock, Windows Clock | Clock and Calendar | Use native alarms, timers, and notifications. |
| K-Lite Codec Pack and Windows codec extensions | IINA | Broad playback support with a native macOS interface. |
| Link Shell Extension | Finder aliases or `ln -s` | Use aliases for users, symbolic links for developer workflows. |
| LockHunter, PowerToys File Locksmith | Activity Monitor, `lsof`, or `fuser` | Identify the process holding the file. |
| Macrium Reflect Home | Time Machine; Carbon Copy Cloner optional | Time Machine is the free recovery default. Add Carbon Copy Cloner only for a tested clone workflow. |
| MacroRecorder, VoiceBot | Hammerspoon and macOS Voice Control; Keyboard Maestro optional | Hammerspoon is the free automation default. Buy Keyboard Maestro only for a workflow it cannot meet. |
| Notepad++ | CotEditor | Use VS Code/Cursor for source code. |
| OnTopReplica | Helium or Picture in Picture | Helium provides a floating browser window. |
| PDF-XChange Editor | Preview; PDF Expert optional | Preview is the free default for reading and annotation. Buy PDF Expert only for advanced editing needs. |
| Process Explorer, Process Lasso | Activity Monitor and App Tamer | Inspect processes and manage background resource use. |
| Process Monitor | Instruments or `fs_usage` | Developer profiling and file-system activity inspection. |
| Power Automate Desktop | Shortcuts | Extend with Keyboard Maestro where necessary. |
| Recuva | TestDisk/PhotoRec; Disk Drill Pro optional | Use free recovery tools first; use Disk Drill only after its scan/preview proves it can recover needed files. Always recover to another disk. |
| Subtitle Edit | Aegisub or a retained Windows VM/remote session | Use Aegisub for subtitle authoring; retain Subtitle Edit only when its specific workflows are essential. |
| Screenshot Captor, Snipping Tool | Shottr and Screenshot | Use native capture for routine work; Shottr for scrolling/annotation. |
| SyncTrayzor | Syncthing-macOS | Retains the Syncthing protocol with a macOS front end. |
| TeraCopy | Finder; ForkLift optional | Finder is the free default. Buy ForkLift only for queueing or dual-pane workflows. |
| TrafficMonitor | Stats; iStat Menus optional | Stats is the free menu-bar default for performance and network statistics. |
| UniGetUI, Winget, Chocolatey | Homebrew plus Cork | Homebrew is the package manager; Cork is an optional GUI. |
| WinDbg | LLDB and Instruments | Xcode supplies the native debugging stack. |
| WinDirStat | GrandPerspective; DaisyDisk optional | GrandPerspective is the free default. Buy DaisyDisk only for its preferred workflow. |
| WinHTTrack | SiteSucker | Use only where site downloading is authorized. |
| WinThumbsPreloader | Quick Look | Native replacement for thumbnail-preload workflows. |
| xplorer2 | Finder; ForkLift or Path Finder optional | Finder is the free default. Add a paid dual-pane manager only after a demonstrated need. |
| Windows Calculator | Calculator | Native application. |
| Windows Camera, Sound Recorder | Photo Booth/Continuity Camera; Voice Memos | Native capture alternatives. |
| Windows Maps, Weather, News, Feedback Hub, Get Help | Apple Maps, Weather, News, and support.apple.com | Windows service apps should not be migrated. The Apple Support app is not a macOS replacement. |
| Windows Mail and Calendar, Outlook, People | Apple Mail/Calendar/Contacts or Outlook for Mac | Choose based on iCloud versus Microsoft 365 needs. |
| Windows Notepad | TextEdit or CotEditor | TextEdit for basic text, CotEditor for advanced text work. |
| Windows Photos, Paint, Paint 3D, 3D Viewer | Photos, Preview, Pixelmator Pro, Blender | Photos/Preview for viewing; Pixelmator Pro for general editing. |
| Windows Terminal and WSL | Terminal or iTerm2 | macOS includes a POSIX environment. Use a Linux VM/container only for Linux-specific needs. |
| Game Bar and Xbox services | OBS Studio and Steam | Use OBS for recording; Xbox overlays are Windows-only. |
| Windows Security, Defender, PC Health Check, Update Health | Native security and Software Update | Do not install Windows security components on macOS. |
| Visual Studio Community/Build Tools, Windows SDK | Xcode and Command Line Tools | Retain VS Code/Cursor and language toolchains for cross-platform work. |
| CUDA 12.6/12.8/12.9/13.0/13.3 and NVIDIA Nsight | Metal, MLX, PyTorch MPS, Instruments | CUDA does not run on Apple Silicon. Retain a Windows/Linux NVIDIA host for CUDA workloads. |
| Voicy | macOS Dictation and Voice Memos | Reassess the original product function; use native capture/dictation for the common case. |
| Marvin | Verify the vendor current macOS build | The package name alone is insufficient to select a reliable substitute. Preserve exported data before replacing it. |
## Non-migratable Windows dependencies
Do not transfer these as applications:

- Microsoft .NET Native Framework/Runtime entries, Visual C++ redistributables, Windows App Runtime, `Microsoft.UI.Xaml`, Windows SDK add-ons, Application Verifier, MSBuild, reference assemblies, and Visual Studio installer components.
- Microsoft Store, App Installer, Windows Package Manager sources, Store Experience Host, Widgets/Web Experience runtimes, and Microsoft Advertising SDK.
- AV1, AVC, HEVC, MPEG-2, VP9, WebP, HEIF, and Raw image extensions. macOS uses its own media framework; add IINA only when needed.
- ELAN/TrackPoint/Synaptics/SmartAudio/Realtek/Intel/Lenovo/Thunderbolt/NVIDIA drivers and support applications, Hyper-V, WSL, Windows update tooling, Defender, and Windows shell hosts.
- Dolby Access and the Dolby Digital Plus decoder. Confirm requirements against the new Mac/audio hardware before choosing a replacement.
## Desktop applications found in installation directories
The folder review corroborated the Winget inventory. Generic OS, framework, and shared-library directories are excluded from this actionable list.

- `C:\Program Files`: 7-Zip, Audacity, AutoHotkey, BCUninstaller, Blender, Bulk Rename Utility, Cheat Engine, CrystalDiskMark 8/9, Cursor, Ditto, Docker, DownloadHelper CoApp, Equalizer APO, ExplorerPatcher, Git, GitHub CLI, HandBrake, Hue Sync, ImageGlass, ImageMagick 7.1.1/7.1.2, IrfanView, Jellyfin, Krita, LibreOffice, Link Shell Extension, LLVM, LockHunter, Logi Options+, Macrium, MediaInfo, Moonlight, Firefox, MSEdgeRedirect, Node.js, Notepad++, OBS Studio, OpenTTD, Parsec, PDF-XChange, Process Lasso, PuTTY, Qalculate!, Recuva, Subtitle Edit, SyncTrayzor, Tailscale, TeraCopy, Topaz Video AI, VLC, VoiceBot, WinDirStat, WinHTTrack, WinThumbsPreloader, WSL, XnConvert, xplorer2, Zen Browser, Zoom, and Zotero.
- `C:\Program Files (x86)`: AudioRelay, Borderless Gaming, EasyJoin, FastStone Image Viewer, FileSeek, Geekbench 6, GPU-Z, Hot Alarm Clock, K-Lite Codec Pack, MacroRecorder, OnTopReplica, Screenshot Captor, Steam, and Windows Installation Assistant.
- `%LOCALAPPDATA%\Programs`: LuaJIT, Marvin, Python, UniGetUI, Voicy, and Warp.

Other Winget applications without dedicated program directories include Handy, Geekbench 6, DownloadHelper CoApp, and the Video DownloadHelper companion. Use a current macOS/browser vendor build where offered; otherwise use `yt-dlp` for authorized downloads. Reassess Handy through its vendor browser/mobile control surface rather than treating it as a Windows-only dependency.
## Microsoft Store and Appx applications
User-facing Store packages include 3D Viewer, Brother Print Support, Command Palette, Copilot, Dolby Access, Feedback Hub, Game Bar, Get Help, Intel Graphics Command Center, Jigsaw/Minesweeper/Solitaire/Ultimate Word Games, Microsoft To Do, Mixed Reality Portal, Movies & TV, Outlook, Paint/Paint 3D, Phone Link, Photos, Power Automate, PowerToys components, Quick Assist, Snipping Tool, Weather, News, Windows Calculator/Camera/Clock/Maps/Media Player/Notepad/Sound Recorder/Terminal, NVIDIA Control Panel, OneDrive, WhatsApp, and Xbox support packages.

The Appx inventory also contains servicing/framework identities and shell hosts. They are summarized above as Windows dependencies rather than counted as portable applications.
## Games and game-adjacent applications
Install Steam for macOS, sign in, and inspect each game's Store page/library compatibility marker and cloud-save status. The Windows inventory contains:

- 3D Ultra Minigolf Adventures, A Game About Feeding a Black Hole, Angel Clicker, Arx Fatalis, Astro Colony, Balatro, Blasphemous, Blight of the Immortals, Brotato, Castle Crashers, Caves of Qud, Coal LLC, Command & Conquer: Red Alert, Contraption Maker, Core Keeper, Crawl, Cruelty Squad, Darkest Dungeon II, Duck Detective: The Secret Salami, Dwarf Fortress, Factorio, Fallen Aces, Frog Fractions, FTL: Faster Than Light, Garden Galaxy Demo, Gloomwood, Ground Zero Hero Demo, How Many Dudes?, How to Fish, Idle Biceps, Idle Gumball Machine, IdleOn, Idling Gears, If On A Winter's Night, Four Travelers, Into the Breach, Ion Fury, Iron Commando, Keep on Mining! - Worlds, My Fire Is Bigger Than Yours, Necesse, No one lives under the lighthouse, Nocthra Demo, OCTOPATH TRAVELER, Oxygen Not Included, Ostranauts, Pocket Gecko, Project Zomboid, Pupple Pop, Puzzle Pirates, Quasimorph, RimWorld, Rock Bottom, Root, Ropuka's Idle Island, Scritchy Scratchy Demo, Selaco, Sephiria, Shadows of Doubt, shapez Demo, shatAAAAp! Playtest, Sheltered, SHENZHEN I/O, Slab, Sorcery! Parts 1 & 2, Space Warlord Organ Trading Simulator, Spirits of Xanadu, Stardew Valley, Terminus: Zombie Survivors, The Necromancer's Tale, Timber Rush, Trap Them - Sniper Edition, Undertale, Unforeseen Incidents, Untitled Goose Game, Valheim, West of Loathing, and Yet Another Zombie Survivors.

For titles without a current macOS build, first consider GeForce NOW, then Moonlight/Parsec streaming from the retained Windows workstation. Use CrossOver only after confirming the individual game's current compatibility, anti-cheat behavior, and save handling. Do not copy the Windows Steam installation into macOS as the migration strategy.
## Data handoff checklist

- Prefer browser/password-manager sync. If a password CSV is unavoidable, import it locally, then remove the plaintext export immediately; never place it in a repository, shared cloud folder, or transferable backup.
- Create a source-data inventory and independent Windows/external backup before transfer. Record data classes, file counts or hashes where practical, and the application-specific restore method.
- Confirm Git remotes and push every repository before cloning on macOS. Choose HTTPS through `gh auth login` or SSH for Git transport; configure commit signing separately only where policy requires it.
- Rebuild FFmpeg, ImageMagick, and yt-dlp scripts with macOS paths and Homebrew binaries.
- Back up OBS scenes, profiles, plugins, recordings, and media sources; device names will change.
- Export/sync Evernote, Xmind, Zotero, LibreOffice, and PDF annotations through supported formats.
- Verify Steam cloud saves and back up local saves before retiring the Windows machine.
- Retain Windows or a remote Windows/Linux NVIDIA host for CUDA, Windows SDK/Nsight, and games without a viable macOS path. Before relying on it, test the selected remote protocol from the intended network, verify allowed-user access, power/restart recovery, firewall reachability, and a physical recovery path.

## Source-data acceptance gate

Do not erase, reset, sell, or repurpose the Windows source until all required data classes have been independently backed up, transferred, restored/opened on macOS, and checked against the source inventory. Preserve it for a defined acceptance period that covers normal work, a reboot, one backup cycle, and at least one remote-NVIDIA access test where that host remains required.

For Vox publish/review workflows only, store the workflow-specific `GITHUB_TOKEN` in Clavis through the stdin-only Secrets CLI and validate the selected workflow with `vox secrets doctor`; GitHub CLI login alone does not provision this Vox secret.
## Installation-scope correction
The application inventory above is a migration catalog, not an automatic-install list. Use the audited AI-development baseline below. Docker Desktop, Steam, personal media applications, cloud-sync clients, collaboration apps, and duplicate GUI applications are opt-in only after a specific workflow justifies them.

Reconfirm licensing, account entitlements, game ports, data locations, and vendor macOS availability at migration time.

## Audited AI-development bootstrap (Apple Silicon, September 2026)
### Correction to the original handoff

The original inventory correctly separated Windows-only software from portable applications, but it was too broad to be a factory-install plan. An installer should not recreate every historical Windows record: Program Files entries and Appx registrations do not prove active use, and copying every application creates redundant runtimes, subscription cost, update burden, and attack surface.

This revision uses a free-first declarative baseline. It installs development, local-AI, container, database, browser-test, and migration utilities appropriate for an Apple Silicon Mac; personal collaboration, cloud-sync, media, and game applications remain opt-in. It deliberately excludes CUDA, Windows SDKs, Microsoft framework packages, games, Windows shell modifications, and optional commercial software from automatic installation.

### Audited decisions

| Prior choice | Decision | Reason |
| --- | --- | --- |
| Docker Desktop | Replace in the baseline with Colima plus Docker CLI | Colima is MIT-licensed, supports Apple Silicon, Docker, and Docker Compose workflows. Docker Desktop remains optional when its GUI or organization-managed features are specifically required. |
| LM Studio and Ollama | Install Ollama automatically; make LM Studio optional | Both work on Apple Silicon. One automatic local runtime avoids duplicate model files and background services; LM Studio is useful when a model-discovery GUI is desired. |
| Cursor and VS Code | Install VS Code in the baseline; make Cursor opt-in | VS Code is the free editor baseline. Install Cursor only when its account-backed AI workflow is deliberately selected. |
| BetterTouchTool, Keyboard Maestro, DaisyDisk, PDF Expert, HoudahSpot | Prefer Rectangle, Hammerspoon, GrandPerspective, Preview, Spotlight/Raycast | The free alternatives meet the ordinary window management, automation, disk inspection, PDF annotation, and search needs. Buy the paid tool only after identifying a missing capability. |
| Docker/AI GPU configuration | Do not port NVIDIA or CUDA packages | Metal is the platform compute API; MLX is an Apple-Silicon ML framework; PyTorch MPS is PyTorch's Metal backend. Choose the framework/runtime per workload. CUDA/Nsight workflows require an NVIDIA host. |
| All game/media applications | Do not install by default | They are personal workloads, not AI-development dependencies. Install individual apps after confirming use, license, and macOS support. |

### Verified availability and limits

- Homebrew supports a declarative `Brewfile` through `brew bundle`; its default Apple Silicon prefix is `/opt/homebrew`.
- Current Homebrew metadata includes arm64 macOS packages for Warp, Cursor, Visual Studio Code, Claude, LM Studio, DBeaver Community, Raycast, Colima, Docker, Docker Compose, Docker Buildx, uv, and the selected utility casks below.
- Ollama supports Apple M-series hardware on macOS 14 or later. LM Studio supports Apple Silicon and offers an MLX runtime; it is intentionally optional because its model store is separate from Ollama's.
- Local models consume unified memory and potentially tens of GB of disk. 128 GB RAM is excellent capacity, not a reason to download several large models automatically. Verify free disk capacity before pulling one.
- Product installation does not transfer accounts, subscriptions, licenses, private repositories, OAuth sessions, or secrets. Do not put credentials in a Brewfile, shell history, or bootstrap script.

### AI-development gaps addressed

| Gap on a Windows-oriented inventory | Baseline response |
| --- | --- |
| Apple build toolchain | Install Xcode Command Line Tools before Homebrew; add full Xcode for Apple-platform builds, Metal/MLX source development, or profiling. |
| Declarative package baseline | Store the generated Brewfile under `~/.config/ai-dev-bootstrap/` and rerun `brew bundle check`. It is repeatable desired-state configuration, not a version-locked build. |
| Local inference on Apple Silicon | Use the free Ollama runtime. Evaluate one model at a time after disk, context, resident-memory, and concurrency checks; choose Ollama MLX, MLX, or PyTorch MPS by workload rather than treating them as interchangeable. |
| Linux containers without Docker Desktop licensing assumptions | Use Colima with Docker, Compose, and Buildx clients. |
| Source control, search, linting, and file tools | Install Git, Git LFS, GitHub CLI, jj, uv, Rustup, Node, Deno, ripgrep, fd, jq, yq, shellcheck, shfmt, actionlint, pre-commit, and Lefthook. |
| API/database testing | Install Bruno and DBeaver Community. |
| Browser compatibility checks | Install Chrome and Firefox; Safari is already available. |
| Secrets and identity | Use macOS Keychain plus the project-required Clavis workflow. Select a password manager deliberately; do not install or seed one automatically. |

### Paste into Warp on the new Mac

Copy this entire block into a Warp terminal **only after signing in to macOS with an administrator-capable account**. It fails closed unless the Mac is Apple Silicon and runs macOS 14 or later, creates only user-owned workspace/configuration folders, installs named packages from official Homebrew taps, and records a declarative baseline in a Brewfile. The Command Line Tools dialog is intentionally interactive; rerun the block after that installation completes.

```zsh
# Paste this block into Warp. It runs in a subshell, so validation exits do not close the terminal.
(
if [[ "$(uname)" != "Darwin" ]]; then
  print -u2 -- "[ERROR] This bootstrap must run on macOS."
  exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  print -u2 -- "[ERROR] This is an Apple Silicon bootstrap. Stop and choose an Intel-specific plan."
  exit 1
fi

macos_version="$(sw_vers -productVersion)"
macos_major="${macos_version%%.*}"
if [[ "$macos_major" -lt 14 ]]; then
  print -u2 -- "[ERROR] macOS 14 or later is required for this Homebrew and Ollama baseline; found $macos_version."
  exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
  print -- "[ACTION] Requesting Xcode Command Line Tools. Complete the macOS dialog, then rerun this bootstrap."
  xcode-select --install
  exit 0
fi

if [[ ! -x /opt/homebrew/bin/brew ]]; then
  print -- "[ACTION] Installing Homebrew from its official installer. Review its prompt before accepting."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)" || exit 1
fi
if [[ ! -x /opt/homebrew/bin/brew ]]; then
  print -u2 -- "[ERROR] Homebrew was not installed at /opt/homebrew/bin/brew."
  exit 1
fi
eval "$(/opt/homebrew/bin/brew shellenv)" || exit 1
if ! grep -Fqx 'eval "$(/opt/homebrew/bin/brew shellenv)"' "$HOME/.zprofile" 2>/dev/null; then
  print -- 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> "$HOME/.zprofile"
fi

bootstrap_root="$HOME/.config/ai-dev-bootstrap"
mkdir -p "$bootstrap_root" \
  "$HOME/Developer/src" \
  "$HOME/Developer/worktrees" \
  "$HOME/Developer/experiments" \
  "$HOME/AI/models" \
  "$HOME/AI/datasets" \
  "$HOME/AI/notebooks" \
  "$HOME/Media/raw" \
  "$HOME/Media/exports"

if [[ -e "$bootstrap_root/Brewfile" ]]; then
  cp "$bootstrap_root/Brewfile" "$bootstrap_root/Brewfile.before-bootstrap-$(date +%Y%m%d%H%M%S)" || exit 1
fi
cat > "$bootstrap_root/Brewfile" <<'BREWFILE'
# Developer core
brew "git"
brew "git-lfs"
brew "gh"
brew "jj"
brew "uv"
brew "python@3.14"
brew "node"
brew "pnpm"
brew "deno"
brew "rustup"
brew "cmake"
brew "ninja"
brew "llvm"
brew "pkgconf"
brew "sqlite"
brew "jq"
brew "yq"
brew "ripgrep"
brew "fd"
brew "fzf"
brew "bat"
brew "eza"
brew "zoxide"
brew "git-delta"
brew "direnv"
brew "shellcheck"
brew "shfmt"
brew "actionlint"
brew "pre-commit"
brew "lefthook"

# Local AI and containers
brew "ollama"
brew "colima"
brew "docker"
brew "docker-compose"
brew "docker-buildx"

# Day-one applications: development, browser testing, communications, data
cask "warp"
# Cursor is opt-in after its subscription-backed workflow is selected.
cask "visual-studio-code"
cask "claude"
cask "claude-code"
cask "dbeaver-community"
cask "bruno"
cask "google-chrome"
cask "firefox"
cask "tailscale-app"
cask "raycast"
cask "rectangle"
# Raycast supplies the default launcher/clipboard workflow; install Maccy only if needed.
cask "stats"
cask "appcleaner"
cask "grandperspective"
cask "localsend"
# Personal media, games, collaboration, cloud-sync, and secondary automation applications are opt-in after account and workflow review.
cask "font-jetbrains-mono-nerd-font"
BREWFILE

brew bundle install --file "$bootstrap_root/Brewfile" --no-upgrade || exit 1
rustup default stable || exit 1
corepack enable || exit 1
mkdir -p "$HOME/.docker/cli-plugins" || exit 1
ln -sfn "$(brew --prefix docker-compose)/bin/docker-compose" "$HOME/.docker/cli-plugins/docker-compose" || exit 1
ln -sfn "$(brew --prefix docker-buildx)/bin/docker-buildx" "$HOME/.docker/cli-plugins/docker-buildx" || exit 1

print -- "[NEXT] Inspect free storage with: df -h /"
print -- "[NEXT] Start containers after choosing resources: colima start --cpus 8 --memory 24 --disk 100 --vm-type vz --mount-type virtiofs"
print -- "[NEXT] Start local inference after storage review: brew services start ollama"
print -- "[NEXT] Verify the package state: brew bundle check --file $bootstrap_root/Brewfile"
)
```

### Required manual follow-up

1. Sign in interactively to GitHub, Tailscale, Google Drive, Claude, Cursor, Slack, Discord, Zoom, Steam, and any licensed media tools. Do not paste tokens into the bootstrap.
2. Start and verify the container runtime: `colima start --cpus 8 --memory 24 --disk 100 --vm-type vz --mount-type virtiofs`, then `docker run --rm hello-world`. Lower the allocation if local models, video tools, or other memory-heavy applications run simultaneously.
3. Start Ollama with `brew services start ollama`. After `df -h /` confirms sufficient free space, evaluate one model at a time. The current Apple-Silicon MLX option `ollama pull muse-glimmer:30b-mlx` is approximately 19 GB; do not pull multiple large models until a use case requires them.
4. Clone repositories into `~/Developer/src`. For this Vox workspace, follow the repository instructions after cloning, including its Clavis-based secret workflow and `vox run scripts/install-hooks.vox`; do not copy `.env` files or Windows credential stores.
5. Install full Xcode only when building or signing Apple-platform software. Command Line Tools are enough for the baseline.

### Optional, intentional additions

- `brew install --cask lm-studio` for a local-model GUI. Do not use it as a second always-running model service unless its separate model store is intentional.
- `brew install --cask betterdisplay` only if Rectangle and macOS display settings are insufficient.
- `brew install --cask docker-desktop` only if its GUI, extensions, or organization-managed features justify its license and resource cost; stop Colima before switching Docker contexts.
- `brew install --cask windows-app` if remote access to the retained Windows/NVIDIA host is required for CUDA, Nsight, or Windows-only games/tools.
- `brew install --cask crossover` only after a specific Windows application or Steam game has been tested; it is not a general migration substitute.
- Choose a password manager manually. macOS Keychain plus the project's Clavis workflow is the default secret boundary; Bitwarden or 1Password is a user-account decision, not a bootstrap default.

### Live verification sources (checked 2026-09-02)

These are mutable vendor and package-manager references, not an immutable availability snapshot. Recheck package tokens and release requirements immediately before running the bootstrap.

- [Homebrew installation](https://docs.brew.sh/Installation), [Homebrew Bundle](https://docs.brew.sh/Brew-Bundle-and-Brewfile), and [Homebrew support tiers](https://docs.brew.sh/Support-Tiers)
- [Colima installation](https://colima.run/docs/installation/) and [runtime documentation](https://colima.run/docs/runtimes/)
- [Ollama macOS requirements](https://docs.ollama.com/macos) and [Ollama Apple Silicon MLX performance](https://ollama.com/blog/mlx-performance)
- [LM Studio system requirements](https://lmstudio.ai/docs/app/system-requirements)
- [Current Homebrew metadata for Warp](https://formulae.brew.sh/api/cask/warp.json), [Cursor](https://formulae.brew.sh/api/cask/cursor.json), [Claude](https://formulae.brew.sh/api/cask/claude.json), [LM Studio](https://formulae.brew.sh/api/cask/lm-studio.json), [DBeaver Community](https://formulae.brew.sh/api/cask/dbeaver-community.json), and [Colima](https://formulae.brew.sh/api/formula/colima.json)
## Final manual configuration sequence

Complete these steps after the bootstrap finishes. They intentionally require user decisions, account authentication, hardware selection, or macOS privacy consent and therefore do not belong in an unattended setup script.

1. **Preserve source data, then secure macOS.** Before import, make an independent Windows/external backup and create a source-data inventory. Apply Software Update, enable FileVault, and configure an encrypted Time Machine backup disk. Use a recovery method you control and verify it can unlock FileVault.
2. **Configure identity and source control.** Set the global Git author name and email deliberately, then choose HTTPS through `gh auth login` or SSH for Git transport. Configure commit signing only if required; GitHub SSH authentication and signing keys are distinct registrations even when generated from the same key material. Enable GitHub Desktop only if its GUI workflow is preferred; do not duplicate active Git credentials unnecessarily.
3. **Sign in and choose sync boundaries.** Authenticate each selected service one by one. For Google Drive, choose either a full My Drive mirror after a capacity review or streaming with named folders made available offline; do not move cloud files out of the Drive location while reorganizing. For every browser profile, record the account, enabled sync categories, and passphrase availability; verify required bookmarks, passwords, extensions, and tabs on the Mac before source cleanup.
4. **Grant macOS privacy permissions narrowly.** In System Settings → Privacy & Security, grant Accessibility only to Rectangle, Maccy, Hammerspoon, and other automation tools that actively need it. Grant Screen Recording, Microphone, and Camera access to OBS, conferencing tools, and AI applications only after opening and using each capability. Review the list afterwards and remove unused permissions.
5. **Select and configure the daily editor.** Use VS Code as the free default or select Cursor intentionally for its account-backed AI workflow, then enable profile sync only if the extensions and settings are trusted. Install repository-required extensions from the workspace recommendation rather than bulk-importing every Windows extension. Configure terminal integration to use Warp and verify that `git`, `gh`, `uv`, `node`, `pnpm`, `deno`, `rustup`, `cargo`, and `ollama` resolve from a new terminal.
6. **Start and test the container environment.** Inspect disk space with `df -h /`, start Colima with the documented resource limits, and run `docker run --rm hello-world`. Keep Docker Desktop stopped or uninstalled while Colima is the active Docker context. Install Kubernetes only when a project needs it.
7. **Configure local AI intentionally.** Start Ollama, select one model after reviewing its license, disk size, selected context, resident-memory use, and request concurrency, then verify a local response. Stop or unload inactive model and container services before memory-heavy work. Use Ollama MLX for its local inference workflow, MLX for MLX development, or PyTorch MPS for PyTorch workloads; route CUDA-only jobs to the retained NVIDIA host.
8. **Restore repositories and project configuration.** Clone each repository into `~/Developer/src`, confirm remotes and branch protections, then follow its own setup instructions. For Vox, build the CLI from the cloned source with `cargo install --path crates/vox-cli` before using `vox`; run `vox run scripts/install-hooks.vox` for Lefthook pre-commit setup and `cargo run -q -p vox-cli -- ci install-hooks` for the separate pre-push hook. Use Clavis for secrets. Do not import Windows `.env` files, application caches, WSL filesystems, or credential databases indiscriminately.
9. **Configure data and media workflows.** Import DBeaver connection definitions through its supported export/import mechanism, restore OBS scenes/profiles only after mapping the new camera and audio devices, and rescan media libraries in their native macOS applications. Keep source media, model weights, and generated output in the separate `~/Media`, `~/AI/models`, and project directories created by the bootstrap.
10. **Validate and record the baseline.** Run `brew bundle check --file ~/.config/ai-dev-bootstrap/Brewfile`, `brew doctor`, `git --version`, `docker context ls`, `ollama --version`, `gh auth status`, `vox --version`, and `vox ci pre-push --dry-run`. Save the Brewfile in a private configuration repository or encrypted backup after reviewing it; it is a declarative application baseline, not a version lockfile.

### Completion criteria

The migration is complete only when the source-data inventory, backup, file-count/hash checks where practical, and application restore/open tests have passed; the Windows source remains intact for a defined acceptance period; and the Mac has an encrypted backup, current OS updates, verified GitHub authentication, a single active Docker runtime, a tested local AI service, only intentional macOS permissions, project secrets restored through Clavis, and a passing Brewfile/Vox check. Install optional paid applications or Windows-compatibility software only after a specific workflow demonstrates that the free-first baseline is insufficient.
## Seven-track audit closure

This section is the controlling operational guidance for the migration. It supersedes an earlier statement in this document whenever they conflict.

### Confirmed corrections and preventive controls

1. **Bootstrap safety is mandatory.** The current bootstrap now rejects non-Apple-Silicon hardware and macOS versions below 14, verifies Homebrew before using it, stops on failed package/tool/plugin provisioning, and saves a timestamped copy of any previous Brewfile. A green-looking final message is meaningful only after those guarded steps complete.
2. **The Brewfile is a declarative baseline, not a lockfile.** `brew bundle --no-upgrade` avoids routine upgrades during installation but does not pin versions. Recheck package availability immediately before use and archive the resulting Brewfile after review. Do not treat live Homebrew/vendor links as immutable historical evidence.
3. **The automatic baseline is intentionally narrow.** It contains the developer toolchain, local AI runtime, container runtime, database/API tools, browser test pair, and low-cost workstation utilities. Cursor, personal media tools, collaboration clients, cloud-sync clients, Steam, Docker Desktop, and compatibility layers are opt-in; install only after confirming a concrete workflow, account entitlement, and license cost.
4. **Preserve source data before trusting the new Mac.** Make an independent Windows/external backup and a source-data inventory before transfer. For each data class, record its source, destination, file count or hash when practical, and a restore/open test. Do not erase, reset, sell, or repurpose the Windows host until the transfer has survived normal work, a reboot, a backup cycle, and any required remote-NVIDIA test.
5. **Use safe browser and cloud-sync migration paths.** Prefer browser or password-manager synchronization. If a password CSV is unavoidable, import it locally and remove the plaintext export immediately; never place it in a repository, shared cloud folder, or transferable backup. For Google Drive, choose either a full My Drive mirror after a capacity review or streaming with explicitly selected offline folders. Do not move cloud files out of the Drive location while reorganizing; verify each selected path completes sync without errors.
6. **Separate Git transport, signing, and Vox secrets.** Choose HTTPS through `gh auth login` or SSH for Git transport. Configure commit signing only when policy requires it; GitHub authentication and signing keys are separate registrations. GitHub CLI authentication does not supply Vox's workflow-specific `GITHUB_TOKEN`. If Vox review/publish is needed, set that secret through the stdin-only Clavis command surface and validate it with the applicable `vox secrets doctor` workflow; never put it in a Brewfile, `.env` copy, shell argument, or GitHub CLI configuration.
7. **Recover Vox from source in the correct order.** After cloning the repository, build the CLI with `cargo install --path crates/vox-cli` before running `vox` commands. Install Lefthook through Homebrew, run `vox run scripts/install-hooks.vox` for the pre-commit hook, and run `cargo run -q -p vox-cli -- ci install-hooks` for the separate pre-push delegate. Verify with `vox --version` and `vox ci pre-push --dry-run`; use the complete local gate before preparing a code change for push.
8. **Treat Apple-Silicon AI layers as distinct.** Metal is the platform compute API; MLX is an ML framework that uses Apple-Silicon acceleration; PyTorch MPS is PyTorch's Metal backend; Ollama's MLX path is a local inference runtime. Select the tool by workload. Do not describe MPS as a general CUDA replacement, and route CUDA-only/Nsight work to a tested NVIDIA host.
9. **Use a resource admission check for local models.** A model artifact size is storage information, not a resident-memory, context-window, concurrency, or performance guarantee. Before loading a model, check free disk, requested context, resident unified-memory use, active request count, and Colima allocation. Stop or unload non-active model/container services before running memory-heavy model, media, or build workloads. Install MLX/PyTorch in project-scoped `uv` environments only when their code is actually required; the bootstrap intentionally does not impose a global ML framework.
10. **Prove the retained Windows host is usable before depending on it.** Select one remote protocol, verify the authorized account, firewall/network reachability, sleep/reboot/power behavior, external-network access, and a physical recovery route. Do this before treating the host as the fallback for CUDA, Nsight, Windows SDKs, or unported games.

### Audit disposition

The seven reviews confirmed the bootstrap error-handling and macOS-version defects, package-scope contradiction, repository hook prerequisites, source-data preservation gap, Google Drive terminology error, browser-export credential risk, Git auth/signing conflation, Clavis publish-secret gap, and Apple-Silicon runtime conflation. They rejected invalid-package, broken-link, unsupported-Apple-Silicon, and automatic-model-download claims after checking current primary sources. No target-Mac throughput, model-quality, or capacity guarantee is asserted because no controlled measurement was performed on the destination Mac.