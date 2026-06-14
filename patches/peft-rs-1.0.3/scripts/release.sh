#!/usr/bin/env bash
# Release script for peft-rs
# Usage: ./scripts/release.sh [version]
# Example: ./scripts/release.sh 0.4.0

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

VERSION=$1

if [ -z "$VERSION" ]; then
    echo -e "${RED}Error: Version required${NC}"
    echo "Usage: $0 <version>"
    echo "Example: $0 0.4.0"
    exit 1
fi

# Remove 'v' prefix if provided
VERSION=${VERSION#v}

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         peft-rs Release Process v${VERSION}              "
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Verify we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${RED}❌ Must be on main branch to release${NC}"
    echo "   Current branch: $CURRENT_BRANCH"
    exit 1
fi

# Verify working directory is clean
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${RED}❌ Working directory not clean${NC}"
    echo "   Commit or stash changes first"
    git status --short
    exit 1
fi

echo -e "${GREEN}✅ On main branch with clean working directory${NC}"
echo ""

# Step 1: Quality checks
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 1: Running quality checks"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo -n "Running tests... "
if cargo test --all-features --quiet; then
    echo -e "${GREEN}✅${NC}"
else
    echo -e "${RED}❌ Tests failed${NC}"
    exit 1
fi

echo -n "Running clippy... "
if cargo clippy --all-targets --quiet -- -D warnings; then
    echo -e "${GREEN}✅${NC}"
else
    echo -e "${RED}❌ Clippy errors found${NC}"
    exit 1
fi

echo -n "Checking format... "
if cargo fmt --check; then
    echo -e "${GREEN}✅${NC}"
else
    echo -e "${RED}❌ Format check failed${NC}"
    exit 1
fi

echo -n "Running security audit... "
if cargo audit &> /dev/null; then
    echo -e "${GREEN}✅${NC}"
else
    echo -e "${YELLOW}⚠️  Audit warnings (review above)${NC}"
fi

echo -n "Building documentation... "
if cargo doc --no-deps --quiet; then
    echo -e "${GREEN}✅${NC}"
else
    echo -e "${RED}❌ Doc build failed${NC}"
    exit 1
fi

echo ""

# Step 2: Verify GPG setup
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 2: Verifying GPG setup"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if bash scripts/verify-gpg.sh > /dev/null 2>&1; then
    echo -e "${GREEN}✅ GPG configuration valid${NC}"
else
    echo -e "${RED}❌ GPG configuration invalid${NC}"
    echo "   Run: bash scripts/verify-gpg.sh"
    exit 1
fi

echo ""

# Step 3: Verify version matches
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 3: Version verification"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
if [ "$CARGO_VERSION" != "$VERSION" ]; then
    echo -e "${RED}❌ Version mismatch${NC}"
    echo "   Cargo.toml: $CARGO_VERSION"
    echo "   Requested:  $VERSION"
    exit 1
fi

echo -e "${GREEN}✅ Version matches: ${VERSION}${NC}"
echo ""

# Step 4: Create signed tag
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 4: Creating signed git tag"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

TAG_NAME="v${VERSION}"

if git tag | grep -q "^${TAG_NAME}$"; then
    echo -e "${YELLOW}⚠️  Tag ${TAG_NAME} already exists${NC}"
    read -p "Delete and recreate? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git tag -d "$TAG_NAME"
        git push origin ":refs/tags/${TAG_NAME}" || true
    else
        echo "Aborting"
        exit 1
    fi
fi

echo "Creating signed tag: ${TAG_NAME}"
git tag -s "$TAG_NAME" -m "Release v${VERSION}"

echo -n "Verifying tag signature... "
if git tag -v "$TAG_NAME" > /dev/null 2>&1; then
    echo -e "${GREEN}✅${NC}"
else
    echo -e "${RED}❌ Tag signature verification failed${NC}"
    exit 1
fi

echo ""

# Step 5: Dry run publish
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 5: Cargo publish dry run"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if cargo publish --dry-run; then
    echo -e "${GREEN}✅ Dry run successful${NC}"
else
    echo -e "${RED}❌ Dry run failed${NC}"
    exit 1
fi

echo ""

# Step 6: Confirm publication
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                   READY TO PUBLISH                        ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""
echo "Version: ${VERSION}"
echo "Tag:     ${TAG_NAME} (signed)"
echo "Branch:  main"
echo ""
echo -e "${YELLOW}This will:${NC}"
echo "  1. Push tag to GitHub"
echo "  2. Publish to crates.io"
echo ""
read -p "Continue? (y/N): " -n 1 -r
echo

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Release cancelled"
    echo "To remove the tag: git tag -d ${TAG_NAME}"
    exit 0
fi

# Step 7: Push tag
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 7: Pushing tag to GitHub"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

git push origin "$TAG_NAME"
echo -e "${GREEN}✅ Tag pushed to GitHub${NC}"
echo ""

# Step 8: Publish to crates.io
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Step 8: Publishing to crates.io"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cargo publish
echo -e "${GREEN}✅ Published to crates.io${NC}"
echo ""

# Success summary
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                  RELEASE SUCCESSFUL                       ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""
echo -e "${GREEN}✅ Version ${VERSION} released!${NC}"
echo ""
echo "Next steps:"
echo "  • View on crates.io: https://crates.io/crates/peft-rs"
echo "  • Create GitHub release: gh release create ${TAG_NAME}"
echo "  • Update README badges if needed"
echo ""
