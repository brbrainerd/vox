# Release Process for peft-rs

## Publication Information

**Crates.io**: Ready for publication as early alpha/beta  
**Maintainer**: Tyler Zervas (tzervas)  
**Contact**: tz-dev@vectorweight.com  
**GPG Key**: Required for all releases

## Pre-Release Checklist

### Code Quality
- [ ] All clippy warnings resolved (`cargo clippy -- -D warnings`)
- [ ] All tests passing (`cargo test --all-features`)
- [ ] Code formatted (`cargo fmt --check`)
- [ ] No security vulnerabilities (`cargo audit`)
- [ ] Documentation complete (`cargo doc --no-deps`)

### Version Management
- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated with all changes
- [ ] Git tag created: `v0.X.Y`
- [ ] All changes committed to appropriate branch

### Branch Workflow
- [ ] Changes merged to `dev`
- [ ] Integrated and tested in `testing` branch
- [ ] Final validation in `testing`
- [ ] Merged to `main` for release

## GPG Signing Configuration

### Required GPG Key Details
```
Name: Tyler Zervas
Email: tz-dev@vectorweight.com
Username: tzervas
```

### Setup GPG Signing

1. **Verify GPG key exists**:
   ```bash
   gpg --list-secret-keys tz-dev@vectorweight.com
   ```

2. **Configure Git to use GPG**:
   ```bash
   git config --global user.signingkey <KEY_ID>
   git config --global commit.gpgsign true
   git config --global tag.gpgsign true
   ```

3. **Verify signing works**:
   ```bash
   echo "test" | gpg --clearsign
   ```

### Creating Signed Tags

```bash
# Create signed release tag
git tag -s v0.4.0 -m "Release v0.4.0: Quality & Tooling Phase"

# Verify tag signature
git tag -v v0.4.0

# Push tag to remote
git push origin v0.4.0
```

## Publishing to Crates.io

### Authentication
Already configured via `cargo login` with API token.

### Pre-Publication Verification

```bash
# Dry run (doesn't actually publish)
cargo publish --dry-run

# Check what will be included
cargo package --list

# Verify the package
cargo package
tar -tzf target/package/peft-rs-0.4.0.crate
```

### Publication Commands

#### Alpha/Beta Releases (Pre-1.0)

```bash
# All versions before 1.0.0 are considered unstable
# Current: 0.4.0 = alpha quality

# Publish to crates.io
cargo publish

# Verify publication
cargo search peft-rs
```

#### Version Naming Convention
- **0.1.x - 0.3.x**: Early development
- **0.4.x**: Quality improvements, production-ready code structure
- **0.5.x**: Quantization support
- **0.6.x**: Enhanced inference
- **0.7.x**: Test optimization
- **0.8.x**: Performance optimization
- **0.9.x**: Pre-1.0 stabilization
- **1.0.0**: Stable API (requires explicit authorization)

### Post-Publication

1. **Verify on crates.io**:
   - Visit: https://crates.io/crates/peft-rs
   - Check documentation builds
   - Verify metadata

2. **Create GitHub Release**:
   ```bash
   # Using GitHub CLI
   gh release create v0.4.0 \
     --title "v0.4.0 - Quality & Tooling" \
     --notes-file RELEASE_NOTES.md
   ```

3. **Update Documentation**:
   - Link to crates.io in README.md
   - Update installation instructions
   - Add badges (version, downloads, docs)

## Release Types

### Alpha Releases (0.1.x - 0.4.x)
- Early testing
- API may change
- Limited documentation
- Core features implemented

**Current Status**: 0.4.0 = **Ready for alpha publication**

### Beta Releases (0.5.x - 0.8.x)
- Feature complete
- API mostly stable
- Comprehensive documentation
- Production testing

### Release Candidates (0.9.x)
- API frozen
- Only bug fixes
- Final testing
- Documentation complete

### Stable Release (1.0.0+)
- API stability guarantee
- Full documentation
- Production validated
- Requires explicit authorization

## Emergency Procedures

### Yanking a Release
```bash
# If critical bug found
cargo yank --vers 0.4.0

# To un-yank (if fixed quickly)
cargo yank --vers 0.4.0 --undo
```

### Publishing Patches
```bash
# For critical fixes
# Version: 0.4.0 → 0.4.1
cargo publish
```

## Current Publication Status

### Version 0.4.0
**Status**: ✅ Ready for alpha publication to crates.io

**Checklist**:
- ✅ Code quality: All clippy warnings fixed
- ✅ Tests: 124 tests passing
- ✅ Documentation: Comprehensive docs added
- ✅ Security: No known vulnerabilities
- ✅ Authentication: Cargo login configured
- ⏳ Pending: PR merge to dev → testing → main
- ⏳ Pending: GPG signed tag creation

**Publication Steps**:
1. Merge `working/quality-clippy-fixes` PR to `dev`
2. Merge `dev` to `testing` for validation
3. Merge `testing` to `main`
4. Create GPG signed tag: `v0.4.0`
5. Push tag to GitHub
6. Publish to crates.io: `cargo publish`
7. Create GitHub release with notes
8. Update README badges

## Automation Considerations

### Future CI/CD Integration
```yaml
# .github/workflows/release.yml (future)
on:
  push:
    tags:
      - 'v*'

jobs:
  publish:
    - verify GPG signature
    - run quality checks
    - cargo publish (with API token secret)
    - create GitHub release
```

## Version History

| Version | Date | Type | Published | Notes |
|---------|------|------|-----------|-------|
| 0.1.0 | 2026-01-06 | Dev | No | Initial adapters |
| 0.2.0 | 2026-01-07 | Dev | No | Additional adapters |
| 0.3.0 | 2026-01-08 | Dev | No | Infrastructure complete |
| 0.4.0 | 2026-01-09 | Alpha | Pending | Quality & tooling |

## References

- [Cargo Book - Publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Crates.io](https://crates.io)
- [Semantic Versioning](https://semver.org)
- [GPG Signing Guide](https://git-scm.com/book/en/v2/Git-Tools-Signing-Your-Work)

---

**Note**: This is an early alpha library. APIs may change before 1.0.0.  
Production use is at your own risk until stable release.
