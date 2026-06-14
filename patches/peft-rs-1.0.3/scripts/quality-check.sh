#!/usr/bin/env bash
# Comprehensive quality check script for peft-rs
# Run before creating PRs

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

FAILED=0

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         PEFT-RS Quality Check Suite                      ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# 1. Format check
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "1. Code Formatting Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if cargo fmt -- --check; then
    echo -e "${GREEN}✅ Format check passed${NC}"
else
    echo -e "${RED}❌ Format check failed${NC}"
    echo "   Fix with: cargo fmt"
    FAILED=1
fi
echo ""

# 2. Clippy check
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "2. Clippy Lint Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/clippy-output.txt; then
    echo -e "${GREEN}✅ Clippy check passed${NC}"
else
    echo -e "${RED}❌ Clippy check failed${NC}"
    CLIPPY_WARNINGS=$(grep -c "error:" /tmp/clippy-output.txt || echo "0")
    echo "   Found ${CLIPPY_WARNINGS} issues"
    FAILED=1
fi
echo ""

# 3. Build check
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "3. Build Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if cargo build --all-features; then
    echo -e "${GREEN}✅ Build check passed${NC}"
else
    echo -e "${RED}❌ Build check failed${NC}"
    FAILED=1
fi
echo ""

# 4. Test suite
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "4. Test Suite"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if cargo test --all-features 2>&1 | tee /tmp/test-output.txt; then
    TESTS_PASSED=$(grep -oP '\d+(?= passed)' /tmp/test-output.txt | tail -1)
    echo -e "${GREEN}✅ All ${TESTS_PASSED} tests passed${NC}"
else
    echo -e "${RED}❌ Tests failed${NC}"
    FAILED=1
fi
echo ""

# 5. Documentation build
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "5. Documentation Build"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if cargo doc --no-deps --all-features --document-private-items 2>&1 | grep -i "error" > /dev/null; then
    echo -e "${RED}❌ Documentation build failed${NC}"
    FAILED=1
else
    echo -e "${GREEN}✅ Documentation build passed${NC}"
fi
echo ""

# 6. Security audit
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "6. Security Audit"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if command -v cargo-audit &> /dev/null; then
    if cargo audit 2>&1 | tee /tmp/audit-output.txt; then
        echo -e "${GREEN}✅ Security audit passed${NC}"
    else
        echo -e "${YELLOW}⚠️  Security audit found issues${NC}"
        echo "   Review output above"
        # Don't fail build for audit warnings
    fi
else
    echo -e "${YELLOW}⚠️  cargo-audit not installed${NC}"
    echo "   Install with: cargo install cargo-audit"
fi
echo ""

# 7. Dependency check
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "7. Dependency Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if cargo tree --duplicates | grep -v "^$" > /dev/null; then
    echo -e "${YELLOW}⚠️  Duplicate dependencies found${NC}"
    cargo tree --duplicates
else
    echo -e "${GREEN}✅ No duplicate dependencies${NC}"
fi
echo ""

# Summary
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                       SUMMARY                             ║"
echo "╚═══════════════════════════════════════════════════════════╝"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✅ All quality checks passed!${NC}"
    echo ""
    echo "Ready to commit and push."
    exit 0
else
    echo -e "${RED}❌ Some quality checks failed${NC}"
    echo ""
    echo "Fix the issues above before committing."
    exit 1
fi
