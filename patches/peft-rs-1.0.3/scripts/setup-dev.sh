#!/usr/bin/env bash
# Quick setup script for peft-rs development environment

set -e

echo "Setting up peft-rs development environment..."
echo ""

# Install required cargo tools
echo "Installing cargo tools..."
if ! command -v cargo-audit &> /dev/null; then
    echo "  Installing cargo-audit..."
    cargo install cargo-audit
fi

if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "  Installing cargo-tarpaulin (for coverage)..."
    cargo install cargo-tarpaulin
fi

if ! command -v cargo-outdated &> /dev/null; then
    echo "  Installing cargo-outdated..."
    cargo install cargo-outdated
fi

echo ""
echo "Setting up git hooks..."
if [ -f scripts/pre-commit.sh ]; then
    cp scripts/pre-commit.sh .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit
    echo "  ✅ Pre-commit hook installed"
else
    echo "  ⚠️  Pre-commit script not found"
fi

echo ""
echo "Making scripts executable..."
chmod +x scripts/*.sh

echo ""
echo "Running initial quality check..."
if bash scripts/quality-check.sh; then
    echo ""
    echo "✅ Development environment setup complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Review docs/DEVELOPMENT_PLAN.md for project roadmap"
    echo "  2. Check docs/TASK_TRACKER.md for current tasks"
    echo "  3. Create a working branch: git checkout -b working/your-feature"
    echo "  4. Run 'bash scripts/quality-check.sh' before creating PRs"
else
    echo ""
    echo "⚠️  Initial quality check found issues"
    echo "    This is expected if you're starting fresh"
    echo "    Review the output above and fix as needed"
fi
