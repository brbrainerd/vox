#!/bin/bash
# TOESTUB Self-Apply Automation for Vox

set -e

echo "🚀 Running TOESTUB Architectural Scan..."

# 1. Build the engine
echo "🛠️  Building TOESTUB engine..."
cargo build -p vox-toestub --release

# 2. Run scan
echo "🔍 Scanning codebase for anti-patterns..."
cargo run -q -p vox-toestub --bin toestub

echo "✅ TOESTUB: Scan complete."
