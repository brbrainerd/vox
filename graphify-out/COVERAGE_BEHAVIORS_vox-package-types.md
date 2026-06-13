# Semantic Behavior Map — `vox-package-types`

Deterministically synthesized from 24 distinct proven-behavior claims (of 24 extracted) across 16 symbols. 0 symbols have an explicit error-path proof; **12 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `VersionReq::parse`  (edge, happy; EXTRACTED)
- [happy] Parses caret requirement "^1.2.0" and matches 1.2.x and 1.x versions but not 2.0.0 or 1.1.0  (crates/vox-package-types/src/resolver/tests.rs)
- [edge] Caret requirement with major=0 ("^0.2.0") restricts matches to minor version 0.2.x  (crates/vox-package-types/src/resolver/tests.rs)
- [happy] Parses tilde requirement "~1.2.0" and matches 1.2.x but not 1.3.0  (crates/vox-package-types/src/resolver/tests.rs)
- [happy] Parses compound requirement ">=1.0.0, <2.0.0" and matches versions within the range  (crates/vox-package-types/src/resolver/tests.rs)
- [happy] Parses wildcard requirement "*" and matches any version  (crates/vox-package-types/src/resolver/tests.rs)

### `SemVer::parse`  (happy; EXTRACTED)
- [happy] Parses "1.2.3" into major=1, minor=2, patch=3 with no prerelease  (crates/vox-package-types/src/resolver/tests.rs)
- [happy] Extracts prerelease suffix from version strings like "1.0.0-beta.1"  (crates/vox-package-types/src/resolver/tests.rs)
- [happy] Accepts short form "2" and normalizes to (major=2, minor=0, patch=0, pre=None)  (crates/vox-package-types/src/resolver/tests.rs)

### `SemVer`  (invariant; EXTRACTED)
- [invariant] Implements correct ordering with patch < minor < major progression  (crates/vox-package-types/src/resolver/tests.rs)
- [invariant] Prerelease versions sort strictly before their corresponding release versions  (crates/vox-package-types/src/resolver/tests.rs)

### `VoxManifest::from_str`  (happy; EXTRACTED)
- [happy] from_str parses a minimal manifest with only name, defaulting version to 0.1.0 and kind to library  (crates/vox-package-types/src/manifest.rs)
- [happy] from_str parses all manifest fields including dependencies with multiple specs (simple, detailed with features, path-based) and features map  (crates/vox-package-types/src/manifest.rs)

### `Lockfile::get_locked_version`  (happy; EXTRACTED)
- [happy] get_locked_version returns the exact SemVer that was added for a package name, or None if not found  (crates/vox-package-types/src/lockfile.rs)

### `Lockfile::is_locked`  (happy; EXTRACTED)
- [happy] is_locked returns true for a package and version that were added, false for wrong version or package name  (crates/vox-package-types/src/lockfile.rs)

### `Lockfile::remove`  (happy; EXTRACTED)
- [happy] remove returns true when removing an existing package, false on subsequent calls, and clears the packages field  (crates/vox-package-types/src/lockfile.rs)

### `Lockfile::to_toml_string`  (happy; EXTRACTED)
- [happy] to_toml_string serializes package names, dependencies, and content hashes into TOML format  (crates/vox-package-types/src/lockfile.rs)

### `PackageKind`  (happy; EXTRACTED)
- [happy] Agent variant serializes to JSON string "agent" and deserializes back to Agent  (crates/vox-package-types/src/package_kind.rs)

### `PackageKind::Display`  (happy; EXTRACTED)
- [happy] Display trait converts PackageKind variants to lowercase string representation (library, agent, etc.)  (crates/vox-package-types/src/package_kind.rs)

### `PackageKind::from_str_loose`  (happy; EXTRACTED)
- [happy] from_str_loose parses case-insensitive and abbreviated variants (library/lib, application/app, skill, agent, workflow, snippet, component) and returns None for unknown strings  (crates/vox-package-types/src/package_kind.rs)

### `PackageKind::is_dependency_eligible`  (happy; EXTRACTED)
- [happy] Library and Skill kinds are dependency-eligible; Application and Snippet kinds are not  (crates/vox-package-types/src/package_kind.rs)

### `PackageKind::namespace`  (happy; EXTRACTED)
- [happy] namespace method returns plural namespace strings for package kinds (skills for Skill, workflows for Workflow)  (crates/vox-package-types/src/package_kind.rs)

### `PackageSource::Path`  (invariant; EXTRACTED)
- [invariant] PackageSource::Path round-trips through Lockfile serialization and deserialization preserving the path string  (crates/vox-package-types/src/lockfile.rs)

### `VoxManifest`  (invariant; EXTRACTED)
- [invariant] VoxManifest round-trips through to_toml_string and from_str preserving package name and kind fields  (crates/vox-package-types/src/manifest.rs)

### `VoxManifest::scaffold`  (happy; EXTRACTED)
- [happy] scaffold creates a manifest with specified name and kind, includes Apache-2.0 license, and can serialize to TOML  (crates/vox-package-types/src/manifest.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Lockfile::get_locked_version`** — only: _get_locked_version returns the exact SemVer that was added for a package name, or None if not found_
- **`Lockfile::is_locked`** — only: _is_locked returns true for a package and version that were added, false for wrong version or package name_
- **`Lockfile::remove`** — only: _remove returns true when removing an existing package, false on subsequent calls, and clears the packages field_
- **`Lockfile::to_toml_string`** — only: _to_toml_string serializes package names, dependencies, and content hashes into TOML format_
- **`PackageKind`** — only: _Agent variant serializes to JSON string "agent" and deserializes back to Agent_
- **`PackageKind::Display`** — only: _Display trait converts PackageKind variants to lowercase string representation (library, agent, etc.)_
- **`PackageKind::from_str_loose`** — only: _from_str_loose parses case-insensitive and abbreviated variants (library/lib, application/app, skill, agent, workflow, snippet, component) and returns None for unknown strings_
- **`PackageKind::is_dependency_eligible`** — only: _Library and Skill kinds are dependency-eligible; Application and Snippet kinds are not_
- **`PackageKind::namespace`** — only: _namespace method returns plural namespace strings for package kinds (skills for Skill, workflows for Workflow)_
- **`SemVer::parse`** — only: _Parses "1.2.3" into major=1, minor=2, patch=3 with no prerelease_
- **`VoxManifest::from_str`** — only: _from_str parses a minimal manifest with only name, defaulting version to 0.1.0 and kind to library_
- **`VoxManifest::scaffold`** — only: _scaffold creates a manifest with specified name and kind, includes Apache-2.0 license, and can serialize to TOML_
