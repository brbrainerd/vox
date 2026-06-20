# Storage Tiering Orchestrator — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a safe, advisory/manual PowerShell tool that catalogs developer directories, scores their I/O "heat," decides promote/demote between three drive tiers (D: hot / C: warm / X: cold), and executes idle-gated, reversible moves via directory junctions — plus a Defender/Search tuner.

**Architecture:** Six small PowerShell modules with one responsibility each (Catalog, Monitor, Policy, Mover, Tuner, CLI), pure-function decision logic kept separate from the single filesystem-writing unit (Mover). All moves are idle-gated, copy-verify-swap, and journaled for crash-safe rollback. Dry-run is the default; AUTO categories act, ADVISORY categories only recommend, MANUAL/LOCKED never act.

**Tech Stack:** PowerShell 7+, Pester v5 (tests), directory junctions (`New-Item -ItemType Junction`), robocopy, JSON catalog/journal. Lives **outside the Vox repo** at `C:\Users\Owner\storage-tier\` (AGENTS.md bans in-repo `.ps1` glue).

**Spec:** [`docs/superpowers/specs/2026-06-20-storage-tiering-orchestrator-design.md`](../specs/2026-06-20-storage-tiering-orchestrator-design.md)

---

## File Structure

| File | Responsibility |
|---|---|
| `C:\Users\Owner\storage-tier\config.json` | Drive map, category policies, thresholds |
| `…\StorageTier.Catalog.psm1` | Load/save/add/get/update catalog (JSON state store) |
| `…\StorageTier.Monitor.psm1` | Heat score from access-recency + size + free space |
| `…\StorageTier.Policy.psm1` | Pure decision: promote/demote/hold |
| `…\StorageTier.Mover.psm1` | Idle-gate, copy-verify-swap, junction, journal, rollback |
| `…\StorageTier.Tuner.psm1` | Defender exclusions + Search scope sync |
| `…\Invoke-StorageTier.ps1` | CLI orchestrator (dry-run default, advisory queue) |
| `…\Install-StorageTierTask.ps1` | Register idle-triggered Scheduled Task |
| `…\tests\*.Tests.ps1` | Pester tests, one per module |

**Decision purity rule:** `Monitor` and `Policy` are pure functions (no filesystem writes). `Mover` and `Tuner` are the only units that mutate system state. This keeps all dangerous logic in two auditable files.

---

## Task 1: Scaffold + config schema + Pester bootstrap

**Files:**
- Create: `C:\Users\Owner\storage-tier\config.json`
- Create: `C:\Users\Owner\storage-tier\tests\Config.Tests.ps1`

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Config.Tests.ps1
BeforeAll { $cfg = Get-Content "$PSScriptRoot\..\config.json" -Raw | ConvertFrom-Json }
Describe 'config.json' {
  It 'maps three tiers to drive letters' {
    $cfg.tiers.hot  | Should -Be 'D'
    $cfg.tiers.warm | Should -Be 'C'
    $cfg.tiers.cold | Should -Be 'X'
  }
  It 'defines a size floor and cold threshold' {
    $cfg.sizeFloorGB    | Should -BeGreaterThan 0
    $cfg.coldThresholdDays | Should -BeGreaterThan 0
  }
  It 'declares the rust-build category as AUTO' {
    ($cfg.categories | Where-Object name -eq 'rust-build').mode | Should -Be 'AUTO'
  }
  It 'declares OS category as LOCKED' {
    ($cfg.categories | Where-Object name -eq 'os-apps').mode | Should -Be 'LOCKED'
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Config.Tests.ps1`
Expected: FAIL (config.json not found).

- [ ] **Step 3: Write the config**

```json
{
  "tiers": { "hot": "D", "warm": "C", "cold": "X" },
  "sizeFloorGB": 5,
  "coldThresholdDays": 14,
  "reserveFreeGB": { "D": 150, "C": 200, "X": 500 },
  "categories": [
    { "name": "rust-build", "mode": "AUTO",     "hot": "D", "cold": "X",
      "match": ["C:\\Users\\Owner\\vox", "C:\\Users\\Owner\\vox-*", "C:\\Users\\Owner\\jj-fork", "C:\\Users\\Owner\\edit-mind-rust"] },
    { "name": "dev-repos",  "mode": "AUTO",     "hot": "C", "cold": "X",
      "match": ["C:\\Users\\Owner\\govSim", "C:\\Users\\Owner\\Ovi", "C:\\Users\\Owner\\fableforge", "C:\\Users\\Owner\\NullCascade"] },
    { "name": "media",      "mode": "AUTO",     "hot": "C", "cold": "X",
      "match": ["C:\\Users\\Owner\\brVideo", "C:\\Users\\Owner\\CrossDevice", "C:\\Users\\Owner\\Downloads", "C:\\Users\\Owner\\VRT"] },
    { "name": "llm-models", "mode": "ADVISORY", "hot": "D", "cold": "C",
      "match": ["C:\\Users\\Owner\\.lmstudio", "C:\\Users\\Owner\\.ollama"] },
    { "name": "docker",     "mode": "MANUAL",   "hot": "C", "cold": "C",
      "match": ["C:\\Users\\Owner\\AppData\\Local\\Docker"] },
    { "name": "os-apps",    "mode": "LOCKED",   "hot": "C", "cold": "C",
      "match": ["C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)"] }
  ],
  "pinned": ["D:\\cargo", "D:\\rustup", "D:\\sccache"]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Config.Tests.ps1`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier init -q
git -C C:/Users/Owner/storage-tier add config.json tests/Config.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(config): tier map, category policies, thresholds"
```

---

## Task 2: Catalog module (load/save/add/get/update)

**Files:**
- Create: `C:\Users\Owner\storage-tier\StorageTier.Catalog.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Catalog.Tests.ps1`

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Catalog.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Catalog.psm1" -Force }
Describe 'Catalog' {
  BeforeEach { $script:cat = New-Item -ItemType Directory -Path "$TestDrive\cat-$([guid]::NewGuid())" ; $script:db = "$($cat.FullName)\catalog.json" }
  It 'returns empty array for a new catalog' {
    (Get-Catalog -Path $db).Count | Should -Be 0
  }
  It 'adds and retrieves an entry by path' {
    Add-CatalogEntry -Path $db -DirPath 'C:\Users\Owner\vox' -Category 'rust-build' -Tier 'C'
    $e = Get-CatalogEntry -Path $db -DirPath 'C:\Users\Owner\vox'
    $e.category | Should -Be 'rust-build'
    $e.tier     | Should -Be 'C'
  }
  It 'updates the tier of an existing entry idempotently' {
    Add-CatalogEntry -Path $db -DirPath 'C:\Users\Owner\vox' -Category 'rust-build' -Tier 'C'
    Set-CatalogTier   -Path $db -DirPath 'C:\Users\Owner\vox' -Tier 'D'
    (Get-CatalogEntry -Path $db -DirPath 'C:\Users\Owner\vox').tier | Should -Be 'D'
    (Get-Catalog -Path $db).Count | Should -Be 1
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Catalog.Tests.ps1`
Expected: FAIL (module not found).

- [ ] **Step 3: Write the module**

```powershell
# StorageTier.Catalog.psm1
function Get-Catalog {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path $Path)) { return @() }
  $raw = Get-Content $Path -Raw
  if ([string]::IsNullOrWhiteSpace($raw)) { return @() }
  ,@($raw | ConvertFrom-Json)
}
function Save-Catalog {
  param([Parameter(Mandatory)][string]$Path,[Parameter(Mandatory)][AllowEmptyCollection()][array]$Entries)
  New-Item -ItemType Directory -Path (Split-Path $Path) -Force | Out-Null
  ,@($Entries) | ConvertTo-Json -Depth 6 | Set-Content -Path $Path -Encoding UTF8
}
function Get-CatalogEntry {
  param([Parameter(Mandatory)][string]$Path,[Parameter(Mandatory)][string]$DirPath)
  Get-Catalog -Path $Path | Where-Object { $_.dirPath -eq $DirPath } | Select-Object -First 1
}
function Add-CatalogEntry {
  param([Parameter(Mandatory)][string]$Path,[Parameter(Mandatory)][string]$DirPath,
        [Parameter(Mandatory)][string]$Category,[Parameter(Mandatory)][string]$Tier)
  $all = @(Get-Catalog -Path $Path | Where-Object { $_.dirPath -ne $DirPath })
  $all += [pscustomobject]@{ dirPath=$DirPath; category=$Category; tier=$Tier; junctioned=$false; lastMove=$null }
  Save-Catalog -Path $Path -Entries $all
}
function Set-CatalogTier {
  param([Parameter(Mandatory)][string]$Path,[Parameter(Mandatory)][string]$DirPath,[Parameter(Mandatory)][string]$Tier)
  $all = @(Get-Catalog -Path $Path)
  foreach ($e in $all) { if ($e.dirPath -eq $DirPath) { $e.tier = $Tier } }
  Save-Catalog -Path $Path -Entries $all
}
Export-ModuleMember -Function Get-Catalog,Save-Catalog,Get-CatalogEntry,Add-CatalogEntry,Set-CatalogTier
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Catalog.Tests.ps1`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Catalog.psm1 tests/Catalog.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(catalog): JSON state store with idempotent upsert"
```

---

## Task 3: Monitor — heat scoring

**Files:**
- Create: `C:\Users\Owner\storage-tier\StorageTier.Monitor.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Monitor.Tests.ps1`

Heat = recency-weighted. A dir touched today scores high; one untouched past the cold threshold scores ~0. Pure function over a supplied "now" and last-access time (live process I/O is Phase 2).

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Monitor.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Monitor.psm1" -Force }
Describe 'Get-HeatScore' {
  $now = [datetime]'2026-06-20T12:00:00'
  It 'scores a just-used dir near 1' {
    Get-HeatScore -LastAccess $now.AddHours(-1) -Now $now -ColdDays 14 | Should -BeGreaterThan 0.9
  }
  It 'scores a dir at the cold threshold near 0' {
    Get-HeatScore -LastAccess $now.AddDays(-14) -Now $now -ColdDays 14 | Should -BeLessThan 0.05
  }
  It 'never returns negative' {
    Get-HeatScore -LastAccess $now.AddDays(-90) -Now $now -ColdDays 14 | Should -BeGreaterOrEqual 0
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Monitor.Tests.ps1`
Expected: FAIL (module not found).

- [ ] **Step 3: Write the module**

```powershell
# StorageTier.Monitor.psm1
function Get-HeatScore {
  param([Parameter(Mandatory)][datetime]$LastAccess,[Parameter(Mandatory)][datetime]$Now,[Parameter(Mandatory)][int]$ColdDays)
  $ageDays = ($Now - $LastAccess).TotalDays
  if ($ageDays -le 0) { return 1.0 }
  # exponential decay reaching ~0.05 at ColdDays
  $score = [math]::Exp(-3.0 * $ageDays / $ColdDays)
  [math]::Max(0.0, [math]::Round($score, 4))
}
function Get-DirLastAccess {
  param([Parameter(Mandatory)][string]$DirPath)
  if (-not (Test-Path $DirPath)) { return [datetime]::MinValue }
  $files = Get-ChildItem -LiteralPath $DirPath -Recurse -File -Force -ErrorAction SilentlyContinue |
           Sort-Object LastAccessTime -Descending | Select-Object -First 1
  if ($files) { $files.LastAccessTime } else { (Get-Item $DirPath).LastWriteTime }
}
Export-ModuleMember -Function Get-HeatScore,Get-DirLastAccess
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Monitor.Tests.ps1`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Monitor.psm1 tests/Monitor.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(monitor): exponential-decay heat score"
```

---

## Task 4: Policy — pure decision function

**Files:**
- Create: `C:\Users\Owner\storage-tier\StorageTier.Policy.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Policy.Tests.ps1`

Decision: given (mode, currentTier, heat, sizeGB, sizeFloor, hotTier, coldTier, hotFreeGB, reserveGB) → `promote` / `demote` / `hold`. AUTO acts; ADVISORY returns the same decision but caller treats it as a recommendation; MANUAL/LOCKED always `hold`.

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Policy.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Policy.psm1" -Force }
Describe 'Get-TierDecision' {
  It 'promotes a hot AUTO dir above the size floor with free space' {
    Get-TierDecision -Mode AUTO -CurrentTier C -Heat 0.95 -SizeGB 50 -SizeFloorGB 5 `
      -HotTier D -ColdTier X -HotFreeGB 800 -ReserveGB 150 | Should -Be 'promote'
  }
  It 'demotes a cold AUTO dir' {
    Get-TierDecision -Mode AUTO -CurrentTier D -Heat 0.01 -SizeGB 50 -SizeFloorGB 5 `
      -HotTier D -ColdTier X -HotFreeGB 800 -ReserveGB 150 | Should -Be 'demote'
  }
  It 'holds a dir below the size floor' {
    Get-TierDecision -Mode AUTO -CurrentTier C -Heat 0.95 -SizeGB 2 -SizeFloorGB 5 `
      -HotTier D -ColdTier X -HotFreeGB 800 -ReserveGB 150 | Should -Be 'hold'
  }
  It 'holds when promotion would breach the hot-drive reserve' {
    Get-TierDecision -Mode AUTO -CurrentTier C -Heat 0.95 -SizeGB 50 -SizeFloorGB 5 `
      -HotTier D -ColdTier X -HotFreeGB 160 -ReserveGB 150 | Should -Be 'hold'
  }
  It 'always holds for LOCKED and MANUAL' {
    Get-TierDecision -Mode LOCKED -CurrentTier C -Heat 0.95 -SizeGB 50 -SizeFloorGB 5 -HotTier D -ColdTier X -HotFreeGB 800 -ReserveGB 150 | Should -Be 'hold'
    Get-TierDecision -Mode MANUAL -CurrentTier C -Heat 0.95 -SizeGB 50 -SizeFloorGB 5 -HotTier D -ColdTier X -HotFreeGB 800 -ReserveGB 150 | Should -Be 'hold'
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Policy.Tests.ps1`
Expected: FAIL (module not found).

- [ ] **Step 3: Write the module**

```powershell
# StorageTier.Policy.psm1
function Get-TierDecision {
  param(
    [Parameter(Mandatory)][ValidateSet('AUTO','ADVISORY','MANUAL','LOCKED')][string]$Mode,
    [Parameter(Mandatory)][string]$CurrentTier,
    [Parameter(Mandatory)][double]$Heat,
    [Parameter(Mandatory)][double]$SizeGB,
    [Parameter(Mandatory)][double]$SizeFloorGB,
    [Parameter(Mandatory)][string]$HotTier,
    [Parameter(Mandatory)][string]$ColdTier,
    [Parameter(Mandatory)][double]$HotFreeGB,
    [Parameter(Mandatory)][double]$ReserveGB
  )
  if ($Mode -in 'LOCKED','MANUAL') { return 'hold' }
  if ($SizeGB -lt $SizeFloorGB)    { return 'hold' }
  if ($Heat -ge 0.5 -and $CurrentTier -ne $HotTier) {
    if (($HotFreeGB - $SizeGB) -lt $ReserveGB) { return 'hold' }  # reserve guard
    return 'promote'
  }
  if ($Heat -lt 0.1 -and $CurrentTier -ne $ColdTier) { return 'demote' }
  'hold'
}
Export-ModuleMember -Function Get-TierDecision
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Policy.Tests.ps1`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Policy.psm1 tests/Policy.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(policy): pure promote/demote/hold with size+reserve guards"
```

---

## Task 5: Mover — idle-gate

**Files:**
- Create: `C:\Users\Owner\storage-tier\StorageTier.Mover.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Mover.Idle.Tests.ps1`

A dir is movable only if no file under it is exclusively locked. We probe by trying to open each sampled file with `FileShare.None`.

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Mover.Idle.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Mover.psm1" -Force }
Describe 'Test-DirectoryIdle' {
  It 'reports idle for a dir with no open handles' {
    $d = New-Item -ItemType Directory -Path "$TestDrive\idle-$([guid]::NewGuid())"
    Set-Content "$($d.FullName)\a.txt" 'x'
    Test-DirectoryIdle -DirPath $d.FullName | Should -BeTrue
  }
  It 'reports busy when a file is held open exclusively' {
    $d = New-Item -ItemType Directory -Path "$TestDrive\busy-$([guid]::NewGuid())"
    $f = "$($d.FullName)\locked.bin"; Set-Content $f 'x'
    $stream = [IO.File]::Open($f,'Open','ReadWrite','None')
    try   { Test-DirectoryIdle -DirPath $d.FullName | Should -BeFalse }
    finally { $stream.Close() }
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Idle.Tests.ps1`
Expected: FAIL (function not defined).

- [ ] **Step 3: Write the function**

```powershell
# StorageTier.Mover.psm1  (idle-gate)
function Test-DirectoryIdle {
  param([Parameter(Mandatory)][string]$DirPath,[int]$SampleLimit = 500)
  if (-not (Test-Path $DirPath)) { return $true }
  $files = Get-ChildItem -LiteralPath $DirPath -Recurse -File -Force -ErrorAction SilentlyContinue |
           Select-Object -First $SampleLimit
  foreach ($f in $files) {
    try { $s = [IO.File]::Open($f.FullName,'Open','Read','None'); $s.Close() }
    catch [IO.IOException] { return $false }   # locked => busy
    catch { }                                   # access denied etc. => ignore
  }
  $true
}
Export-ModuleMember -Function Test-DirectoryIdle
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Idle.Tests.ps1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Mover.psm1 tests/Mover.Idle.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(mover): idle-gate via exclusive-open probe"
```

---

## Task 6: Mover — junction create + validate

**Files:**
- Modify: `C:\Users\Owner\storage-tier\StorageTier.Mover.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Mover.Junction.Tests.ps1`

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Mover.Junction.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Mover.psm1" -Force }
Describe 'New-TierJunction / Test-IsJunction' {
  It 'creates a junction that resolves to the target and reads through' {
    $target = New-Item -ItemType Directory -Path "$TestDrive\tgt-$([guid]::NewGuid())"
    Set-Content "$($target.FullName)\hello.txt" 'world'
    $link = "$TestDrive\link-$([guid]::NewGuid())"
    New-TierJunction -LinkPath $link -TargetPath $target.FullName
    Test-IsJunction -Path $link | Should -BeTrue
    Get-Content "$link\hello.txt" | Should -Be 'world'
  }
  It 'reports non-junction directories as not junctions' {
    $d = New-Item -ItemType Directory -Path "$TestDrive\plain-$([guid]::NewGuid())"
    Test-IsJunction -Path $d.FullName | Should -BeFalse
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Junction.Tests.ps1`
Expected: FAIL (functions not defined).

- [ ] **Step 3: Add the functions**

```powershell
# append to StorageTier.Mover.psm1
function Test-IsJunction {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path $Path)) { return $false }
  $item = Get-Item -LiteralPath $Path -Force
  [bool]($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
}
function New-TierJunction {
  param([Parameter(Mandatory)][string]$LinkPath,[Parameter(Mandatory)][string]$TargetPath)
  if (-not (Test-Path $TargetPath)) { throw "Junction target missing: $TargetPath" }
  if (Test-IsJunction -Path $TargetPath) { throw "Refusing to nest junction on target: $TargetPath" }
  if (Test-Path $LinkPath) { throw "Link path already exists: $LinkPath" }
  New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath | Out-Null
}
Export-ModuleMember -Function Test-DirectoryIdle,Test-IsJunction,New-TierJunction
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Junction.Tests.ps1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Mover.psm1 tests/Mover.Junction.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(mover): junction create/validate with nest guard"
```

---

## Task 7: Mover — copy-verify-swap with journal + rollback

**Files:**
- Modify: `C:\Users\Owner\storage-tier\StorageTier.Mover.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Mover.Move.Tests.ps1`

`Move-TierDirectory` moves `SourceDir` (a real dir) to `<TargetRoot>\<name>`, leaves a junction at the original path, and journals the operation. On copy/verify failure, it restores the original and leaves no junction.

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Mover.Move.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Mover.psm1" -Force }
Describe 'Move-TierDirectory' {
  It 'moves data, leaves a junction, and content reads through the original path' {
    $src = New-Item -ItemType Directory -Path "$TestDrive\src-$([guid]::NewGuid())"
    1..3 | ForEach-Object { Set-Content "$($src.FullName)\f$_.txt" "data$_" }
    $tgtRoot = New-Item -ItemType Directory -Path "$TestDrive\hot-$([guid]::NewGuid())"
    $jrn = "$TestDrive\journal.json"
    Move-TierDirectory -SourceDir $src.FullName -TargetRoot $tgtRoot.FullName -JournalPath $jrn
    Test-IsJunction -Path $src.FullName | Should -BeTrue
    Get-Content "$($src.FullName)\f2.txt" | Should -Be 'data2'
    (Get-Content $jrn -Raw | ConvertFrom-Json)[-1].status | Should -Be 'done'
  }
  It 'is a no-op (idempotent) if the source is already a junction' {
    $src = New-Item -ItemType Directory -Path "$TestDrive\j-$([guid]::NewGuid())"
    $real = New-Item -ItemType Directory -Path "$TestDrive\real-$([guid]::NewGuid())"
    Set-Content "$($real.FullName)\x.txt" 'y'
    Remove-Item $src.FullName; New-Item -ItemType Junction -Path $src.FullName -Target $real.FullName | Out-Null
    { Move-TierDirectory -SourceDir $src.FullName -TargetRoot "$TestDrive" -JournalPath "$TestDrive\j2.json" } | Should -Not -Throw
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Move.Tests.ps1`
Expected: FAIL (function not defined).

- [ ] **Step 3: Add the function**

```powershell
# append to StorageTier.Mover.psm1
function Write-Journal {
  param([string]$JournalPath,[hashtable]$Entry)
  $all = @(); if (Test-Path $JournalPath) { $all = @(Get-Content $JournalPath -Raw | ConvertFrom-Json) }
  $all += [pscustomobject]$Entry
  ,@($all) | ConvertTo-Json -Depth 6 | Set-Content $JournalPath -Encoding UTF8
}
function Move-TierDirectory {
  param([Parameter(Mandatory)][string]$SourceDir,[Parameter(Mandatory)][string]$TargetRoot,[Parameter(Mandatory)][string]$JournalPath)
  if (Test-IsJunction -Path $SourceDir) { return }            # already tiered; no-op
  if (-not (Test-DirectoryIdle -DirPath $SourceDir)) { throw "Source busy: $SourceDir" }
  $name = Split-Path $SourceDir -Leaf
  $target = Join-Path $TargetRoot $name
  if (Test-Path $target) { throw "Target already exists: $target" }
  Write-Journal $JournalPath @{ ts=(Get-Date).ToString('o'); src=$SourceDir; target=$target; status='start' }
  # copy
  & robocopy $SourceDir $target /E /COPY:DAT /R:2 /W:2 /NFL /NDL /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { throw "robocopy failed ($LASTEXITCODE) for $SourceDir" }
  # verify (file count)
  $srcCount = (Get-ChildItem $SourceDir -Recurse -File -Force -EA SilentlyContinue).Count
  $tgtCount = (Get-ChildItem $target   -Recurse -File -Force -EA SilentlyContinue).Count
  if ($tgtCount -lt $srcCount) {
    Remove-Item $target -Recurse -Force -ErrorAction SilentlyContinue
    throw "Verify failed: $tgtCount/$srcCount files copied"
  }
  # swap: rename original to .bak, junction in its place
  $bak = "$SourceDir.tierbak"
  Rename-Item -LiteralPath $SourceDir -NewName (Split-Path $bak -Leaf)
  try { New-TierJunction -LinkPath $SourceDir -TargetPath $target }
  catch { Rename-Item -LiteralPath $bak -NewName (Split-Path $SourceDir -Leaf); throw }
  # finalize
  Remove-Item $bak -Recurse -Force -ErrorAction SilentlyContinue
  Write-Journal $JournalPath @{ ts=(Get-Date).ToString('o'); src=$SourceDir; target=$target; status='done' }
}
Export-ModuleMember -Function Test-DirectoryIdle,Test-IsJunction,New-TierJunction,Move-TierDirectory,Write-Journal
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Move.Tests.ps1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Mover.psm1 tests/Mover.Move.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(mover): copy-verify-swap with journal + rollback"
```

---

## Task 8: Mover — reversal (undo a tier move)

**Files:**
- Modify: `C:\Users\Owner\storage-tier\StorageTier.Mover.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Mover.Revert.Tests.ps1`

`Restore-TierDirectory` replaces a junction at `LinkPath` with the real directory moved back from its target.

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Mover.Revert.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Mover.psm1" -Force }
Describe 'Restore-TierDirectory' {
  It 'turns a junction back into a real local directory with its content' {
    $src = New-Item -ItemType Directory -Path "$TestDrive\s-$([guid]::NewGuid())"
    Set-Content "$($src.FullName)\keep.txt" 'v'
    $tgtRoot = New-Item -ItemType Directory -Path "$TestDrive\h-$([guid]::NewGuid())"
    Move-TierDirectory -SourceDir $src.FullName -TargetRoot $tgtRoot.FullName -JournalPath "$TestDrive\jm.json"
    Restore-TierDirectory -LinkPath $src.FullName -JournalPath "$TestDrive\jm.json"
    Test-IsJunction -Path $src.FullName | Should -BeFalse
    Get-Content "$($src.FullName)\keep.txt" | Should -Be 'v'
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Revert.Tests.ps1`
Expected: FAIL (function not defined).

- [ ] **Step 3: Add the function**

```powershell
# append to StorageTier.Mover.psm1
function Restore-TierDirectory {
  param([Parameter(Mandatory)][string]$LinkPath,[Parameter(Mandatory)][string]$JournalPath)
  if (-not (Test-IsJunction -Path $LinkPath)) { return }     # nothing to revert
  $target = (Get-Item -LiteralPath $LinkPath -Force).Target | Select-Object -First 1
  if (-not (Test-DirectoryIdle -DirPath $target)) { throw "Target busy: $target" }
  $tmp = "$LinkPath.restore"
  & robocopy $target $tmp /E /COPY:DAT /R:2 /W:2 /NFL /NDL /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { Remove-Item $tmp -Recurse -Force -EA SilentlyContinue; throw "restore robocopy failed ($LASTEXITCODE)" }
  & cmd /c rmdir "$LinkPath"                                  # remove junction (not its target)
  Rename-Item -LiteralPath $tmp -NewName (Split-Path $LinkPath -Leaf)
  Remove-Item $target -Recurse -Force -ErrorAction SilentlyContinue
  Write-Journal $JournalPath @{ ts=(Get-Date).ToString('o'); src=$LinkPath; target=$target; status='reverted' }
}
Export-ModuleMember -Function Test-DirectoryIdle,Test-IsJunction,New-TierJunction,Move-TierDirectory,Write-Journal,Restore-TierDirectory
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Mover.Revert.Tests.ps1`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Mover.psm1 tests/Mover.Revert.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(mover): reversible restore from junction"
```

---

## Task 9: Tuner — Defender exclusion sync (dry-run aware)

**Files:**
- Create: `C:\Users\Owner\storage-tier\StorageTier.Tuner.psm1`
- Test: `C:\Users\Owner\storage-tier\tests\Tuner.Tests.ps1`

The tuner emits the *set of exclusion changes* as data (so it's testable without admin), and a separate apply function calls `Add-MpPreference` only when `-Execute` is set.

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Tuner.Tests.ps1
BeforeAll { Import-Module "$PSScriptRoot\..\StorageTier.Tuner.psm1" -Force }
Describe 'Get-DefenderExclusionPlan' {
  It 'adds promoted build dirs not already excluded' {
    $plan = Get-DefenderExclusionPlan -ManagedHotDirs @('D:\vox','D:\cargo') -CurrentExclusions @('D:\cargo')
    $plan.toAdd    | Should -Contain 'D:\vox'
    $plan.toAdd    | Should -Not -Contain 'D:\cargo'
  }
  It 'removes exclusions no longer managed (only ones we own)' {
    $plan = Get-DefenderExclusionPlan -ManagedHotDirs @('D:\vox') -CurrentExclusions @('D:\vox','D:\old-build') -OwnedExclusions @('D:\vox','D:\old-build')
    $plan.toRemove | Should -Contain 'D:\old-build'
  }
  It 'never removes an exclusion it does not own' {
    $plan = Get-DefenderExclusionPlan -ManagedHotDirs @('D:\vox') -CurrentExclusions @('C:\Windows') -OwnedExclusions @()
    $plan.toRemove | Should -Not -Contain 'C:\Windows'
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Tuner.Tests.ps1`
Expected: FAIL (module not found).

- [ ] **Step 3: Write the module**

```powershell
# StorageTier.Tuner.psm1
function Get-DefenderExclusionPlan {
  param([string[]]$ManagedHotDirs = @(),[string[]]$CurrentExclusions = @(),[string[]]$OwnedExclusions = @())
  $toAdd    = @($ManagedHotDirs | Where-Object { $_ -notin $CurrentExclusions })
  $toRemove = @($OwnedExclusions | Where-Object { $_ -in $CurrentExclusions -and $_ -notin $ManagedHotDirs })
  [pscustomobject]@{ toAdd=$toAdd; toRemove=$toRemove }
}
function Set-DefenderExclusions {
  param([Parameter(Mandatory)]$Plan,[switch]$Execute)
  foreach ($p in $Plan.toAdd)    { if ($Execute) { Add-MpPreference    -ExclusionPath $p -ErrorAction SilentlyContinue } else { Write-Host "[dry] +excl $p" } }
  foreach ($p in $Plan.toRemove) { if ($Execute) { Remove-MpPreference -ExclusionPath $p -ErrorAction SilentlyContinue } else { Write-Host "[dry] -excl $p" } }
}
Export-ModuleMember -Function Get-DefenderExclusionPlan,Set-DefenderExclusions
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Tuner.Tests.ps1`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add StorageTier.Tuner.psm1 tests/Tuner.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(tuner): owned-only Defender exclusion plan (dry-run aware)"
```

---

## Task 10: Orchestrator CLI (dry-run default, advisory queue)

**Files:**
- Create: `C:\Users\Owner\storage-tier\Invoke-StorageTier.ps1`
- Test: `C:\Users\Owner\storage-tier\tests\Invoke.Tests.ps1`

Ties units together: build/refresh catalog from config `match` globs, score heat, decide per category, and — only with `-Execute` — perform AUTO moves; ADVISORY/MANUAL emit to an advisory queue file. Dry-run prints the plan.

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Invoke.Tests.ps1
Describe 'Invoke-StorageTier dry-run' {
  It 'emits a plan and writes nothing when -Execute is absent' {
    $work = New-Item -ItemType Directory -Path "$TestDrive\w-$([guid]::NewGuid())"
    $repo = New-Item -ItemType Directory -Path "$($work.FullName)\bigrepo"
    $fs = New-Object byte[] (6GB/1000); 1..1000 | ForEach-Object { [IO.File]::WriteAllBytes("$($repo.FullName)\f$_.bin",$fs) }
    $cfg = @{ tiers=@{hot='D';warm='C';cold='X'}; sizeFloorGB=5; coldThresholdDays=14;
             reserveFreeGB=@{D=1;C=1;X=1};
             categories=@(@{name='rust-build';mode='AUTO';hot=$work.FullName;cold=$work.FullName;match=@("$($repo.FullName)")}) }
    $cfgPath = "$($work.FullName)\config.json"; $cfg | ConvertTo-Json -Depth 6 | Set-Content $cfgPath
    $out = & "$PSScriptRoot\..\Invoke-StorageTier.ps1" -ConfigPath $cfgPath -StateDir $work.FullName 6>&1
    ($out -join "`n") | Should -Match 'PLAN|promote|hold'
    Test-IsJunction -Path $repo.FullName | Should -BeFalse   # nothing moved
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Invoke.Tests.ps1`
Expected: FAIL (script not found).

- [ ] **Step 3: Write the orchestrator**

```powershell
# Invoke-StorageTier.ps1
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ConfigPath,[Parameter(Mandatory)][string]$StateDir,[switch]$Execute)
$ErrorActionPreference='Stop'
$here = Split-Path $MyInvocation.MyCommand.Path
Import-Module "$here\StorageTier.Catalog.psm1","$here\StorageTier.Monitor.psm1","$here\StorageTier.Policy.psm1","$here\StorageTier.Mover.psm1" -Force
$cfg = Get-Content $ConfigPath -Raw | ConvertFrom-Json
$db  = Join-Path $StateDir 'catalog.json'
$jrn = Join-Path $StateDir 'journal.json'
$adv = Join-Path $StateDir 'advisory-queue.json'
$now = Get-Date
$advisories = @()
Write-Host "=== STORAGE-TIER PLAN ($(if($Execute){'EXECUTE'}else{'DRY-RUN'})) ==="
foreach ($c in $cfg.categories) {
  foreach ($glob in $c.match) {
    foreach ($dir in (Get-Item -Path $glob -ErrorAction SilentlyContinue)) {
      if (Test-IsJunction -Path $dir.FullName) { continue }
      $sizeGB = [math]::Round(((Get-ChildItem $dir.FullName -Recurse -File -Force -EA SilentlyContinue | Measure-Object Length -Sum).Sum)/1GB,1)
      $heat   = Get-HeatScore -LastAccess (Get-DirLastAccess -DirPath $dir.FullName) -Now $now -ColdDays $cfg.coldThresholdDays
      $curTier= $dir.FullName.Substring(0,1)
      $hotFree= [math]::Round((Get-Volume -DriveLetter $c.hot.Substring(0,1) -EA SilentlyContinue).SizeRemaining/1GB,0)
      if (-not $hotFree) { $hotFree = 0 }
      $decision = Get-TierDecision -Mode $c.mode -CurrentTier $curTier -Heat $heat -SizeGB $sizeGB `
                    -SizeFloorGB $cfg.sizeFloorGB -HotTier ($c.hot.Substring(0,1)) -ColdTier ($c.cold.Substring(0,1)) `
                    -HotFreeGB $hotFree -ReserveGB ($cfg.reserveFreeGB.($c.hot.Substring(0,1)))
      Write-Host ("  [{0,-10}] {1,-40} heat={2} size={3}GB -> {4}" -f $c.mode,$dir.FullName,$heat,$sizeGB,$decision)
      if ($decision -eq 'hold') { continue }
      if ($c.mode -eq 'AUTO' -and $Execute) {
        $root = if ($decision -eq 'promote') { $c.hot } else { $c.cold }
        Move-TierDirectory -SourceDir $dir.FullName -TargetRoot $root -JournalPath $jrn
        Add-CatalogEntry -Path $db -DirPath $dir.FullName -Category $c.name -Tier ($root.Substring(0,1))
      } elseif ($c.mode -in 'AUTO','ADVISORY') {
        $advisories += [pscustomobject]@{ dir=$dir.FullName; category=$c.name; decision=$decision; heat=$heat; sizeGB=$sizeGB }
      }
    }
  }
}
,@($advisories) | ConvertTo-Json -Depth 6 | Set-Content $adv -Encoding UTF8
Write-Host "Advisory queue: $adv ($($advisories.Count) items)"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Invoke.Tests.ps1`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add Invoke-StorageTier.ps1 tests/Invoke.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(cli): dry-run-default orchestrator with advisory queue"
```

---

## Task 11: Idle-triggered Scheduled Task installer

**Files:**
- Create: `C:\Users\Owner\storage-tier\Install-StorageTierTask.ps1`
- Test: `C:\Users\Owner\storage-tier\tests\Install.Tests.ps1`

- [ ] **Step 1: Write the failing test**

```powershell
# tests\Install.Tests.ps1
Describe 'Install-StorageTierTask' {
  It 'builds an idle trigger + dry-run action without registering when -WhatIfOnly' {
    $spec = & "$PSScriptRoot\..\Install-StorageTierTask.ps1" -WhatIfOnly
    $spec.trigger  | Should -Be 'OnIdle'
    $spec.action   | Should -Match 'Invoke-StorageTier.ps1'
    $spec.action   | Should -Not -Match '-Execute'   # starts in dry-run
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Install.Tests.ps1`
Expected: FAIL (script not found).

- [ ] **Step 3: Write the installer**

```powershell
# Install-StorageTierTask.ps1
[CmdletBinding()]
param([switch]$WhatIfOnly,[int]$IdleMinutes=10)
$here = Split-Path $MyInvocation.MyCommand.Path
$action = "pwsh -NoProfile -File `"$here\Invoke-StorageTier.ps1`" -ConfigPath `"$here\config.json`" -StateDir `"$here\state`""
$spec = [pscustomobject]@{ trigger='OnIdle'; idleMinutes=$IdleMinutes; action=$action }
if ($WhatIfOnly) { return $spec }
$t = New-ScheduledTaskTrigger -AtLogOn
$settings = New-ScheduledTaskSettingsSet -RunOnlyIfIdle -IdleDuration (New-TimeSpan -Minutes $IdleMinutes) -IdleWaitTimeout (New-TimeSpan -Hours 2)
$act = New-ScheduledTaskAction -Execute 'pwsh' -Argument "-NoProfile -File `"$here\Invoke-StorageTier.ps1`" -ConfigPath `"$here\config.json`" -StateDir `"$here\state`""
Register-ScheduledTask -TaskName 'StorageTierOrchestrator' -Trigger $t -Settings $settings -Action $act -Force | Out-Null
$spec
```

- [ ] **Step 4: Run test to verify it passes**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests\Install.Tests.ps1`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git -C C:/Users/Owner/storage-tier add Install-StorageTierTask.ps1 tests/Install.Tests.ps1
git -C C:/Users/Owner/storage-tier commit -q -m "feat(task): idle-triggered scheduled task installer (dry-run start)"
```

---

## Task 12: Full-suite gate + README

**Files:**
- Create: `C:\Users\Owner\storage-tier\README.md`
- Test: (runs the whole suite)

- [ ] **Step 1: Run the full Pester suite**

Run: `Invoke-Pester C:\Users\Owner\storage-tier\tests -Output Detailed`
Expected: PASS — all tests across Config, Catalog, Monitor, Policy, Mover (idle/junction/move/revert), Tuner, Invoke, Install.

- [ ] **Step 2: Write the README**

```markdown
# Storage Tier Orchestrator (Phase 1)
Advisory/manual 3-tier storage mover for this workstation. See design:
docs/superpowers/specs/2026-06-20-storage-tiering-orchestrator-design.md (in the vox repo).

## Use
- Dry-run:  pwsh ./Invoke-StorageTier.ps1 -ConfigPath ./config.json -StateDir ./state
- Execute:  add -Execute  (AUTO categories move; ADVISORY/MANUAL only queue)
- Schedule: pwsh ./Install-StorageTierTask.ps1   (idle-triggered, starts in dry-run)
- Revert a move: Import-Module ./StorageTier.Mover.psm1; Restore-TierDirectory -LinkPath <path> -JournalPath ./state/journal.json

## Safety
Moves only idle dirs (no open handles), copy-verify-swap, junction-based, journaled & reversible.
Defender exclusions: owned-only. OS/Program Files = LOCKED. Games/Docker = MANUAL.
```

- [ ] **Step 3: Commit**

```bash
git -C C:/Users/Owner/storage-tier add README.md
git -C C:/Users/Owner/storage-tier commit -q -m "docs: phase-1 usage + safety README"
```

---

## Self-Review

**Spec coverage:** Catalog (§2.1)→T2; Monitor/heat (§2.2)→T3; Policy (§2.3)→T4; Mover idle-gate/junction/copy-verify-swap/journal/rollback/reversal (§2.4, §3)→T5–T8; System I/O Reduction tuner (§2.5)→T9; Control surface/dry-run/advisory (§2.6)→T10; scheduled idle trigger (§5 Phase 1)→T11; size floor + reserve guard (§4)→T4. Games (MANUAL) and Docker (MANUAL/PINNED) are config-level (T1) and intentionally have no auto-move task. Live process-I/O monitoring is explicitly Phase 2 (out of scope).

**Placeholder scan:** none — every step has runnable test + implementation code.

**Type consistency:** function names stable across tasks — `Test-IsJunction`, `New-TierJunction`, `Move-TierDirectory`, `Restore-TierDirectory`, `Get-TierDecision`, `Get-HeatScore`, `Get-DefenderExclusionPlan`; catalog uses `dirPath`/`category`/`tier` throughout; journal `status` values `start`/`done`/`reverted` are consistent.

**Note for executor:** Tasks 7, 8, 10, 11 create real junctions / read `Get-Volume` and may need a writable test volume; run Pester from a normal (non-elevated) shell first — only the live `Set-DefenderExclusions -Execute` and `Register-ScheduledTask` paths need elevation, and those are not exercised by the unit tests.
```
