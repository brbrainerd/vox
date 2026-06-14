#!/usr/bin/env bash
# GPG setup and verification script for peft-rs releases

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         GPG Signing Setup Verification                   ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Required details (configurable via environment variables)
REQUIRED_NAME="${GPG_REQUIRED_NAME:-Tyler Zervas}"
REQUIRED_EMAIL="${GPG_REQUIRED_EMAIL:-tz-dev@vectorweight.com}"
REQUIRED_USER="${GPG_REQUIRED_USER:-tzervas}"

echo "Checking GPG configuration for releases..."
echo ""

# 1. Check if GPG is installed
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "1. GPG Installation Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if command -v gpg &> /dev/null; then
    GPG_VERSION=$(gpg --version | head -n 1)
    echo -e "${GREEN}✅ GPG installed: ${GPG_VERSION}${NC}"
else
    echo -e "${RED}❌ GPG not installed${NC}"
    echo "   Install with: sudo apt-get install gnupg (Debian/Ubuntu)"
    exit 1
fi
echo ""

# 2. Check for GPG key with required email
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "2. GPG Key Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if gpg --list-secret-keys "${REQUIRED_EMAIL}" &> /dev/null; then
    echo -e "${GREEN}✅ GPG key found for ${REQUIRED_EMAIL}${NC}"
    
    # Show key details
    KEY_ID=$(gpg --list-secret-keys --keyid-format LONG "${REQUIRED_EMAIL}" | grep sec | awk '{print $2}' | cut -d'/' -f2)
    echo "   Key ID: ${KEY_ID}"
    
    # Verify name
    if gpg --list-secret-keys "${REQUIRED_EMAIL}" | grep -q "${REQUIRED_NAME}"; then
        echo -e "${GREEN}✅ Name matches: ${REQUIRED_NAME}${NC}"
    else
        echo -e "${YELLOW}⚠️  Name does not match${NC}"
        echo "   Expected: ${REQUIRED_NAME}"
    fi
else
    echo -e "${RED}❌ No GPG key found for ${REQUIRED_EMAIL}${NC}"
    echo ""
    echo "To create a GPG key:"
    echo "  gpg --full-generate-key"
    echo "  - Choose RSA and RSA"
    echo "  - Key size: 4096 bits"
    echo "  - Validity: 0 (does not expire) or set expiration"
    echo "  - Real name: ${REQUIRED_NAME}"
    echo "  - Email: ${REQUIRED_EMAIL}"
    exit 1
fi
echo ""

# 3. Check Git GPG configuration
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "3. Git GPG Configuration"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

GIT_SIGNING_KEY=$(git config --global user.signingkey || echo "")
GIT_COMMIT_SIGN=$(git config --global commit.gpgsign || echo "false")
GIT_TAG_SIGN=$(git config --global tag.gpgsign || echo "false")

if [ -n "$GIT_SIGNING_KEY" ]; then
    echo -e "${GREEN}✅ Git signing key configured: ${GIT_SIGNING_KEY}${NC}"
else
    echo -e "${YELLOW}⚠️  Git signing key not configured${NC}"
    echo "   Set with: git config --global user.signingkey ${KEY_ID}"
fi

if [ "$GIT_COMMIT_SIGN" = "true" ] || [ "$GIT_COMMIT_SIGN" = "1" ] || [ "$GIT_COMMIT_SIGN" = "yes" ]; then
    echo -e "${GREEN}✅ Commit signing enabled${NC}"
else
    echo -e "${YELLOW}⚠️  Commit signing not enabled${NC}"
    echo "   Enable with: git config --global commit.gpgsign true"
fi

if [ "$GIT_TAG_SIGN" = "true" ] || [ "$GIT_TAG_SIGN" = "1" ] || [ "$GIT_TAG_SIGN" = "yes" ]; then
    echo -e "${GREEN}✅ Tag signing enabled${NC}"
else
    echo -e "${YELLOW}⚠️  Tag signing not enabled${NC}"
    echo "   Enable with: git config --global tag.gpgsign true"
fi
echo ""

# 4. Test GPG signing
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "4. GPG Signing Test"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if echo "test" | gpg --clearsign &> /dev/null; then
    echo -e "${GREEN}✅ GPG signing works${NC}"
else
    echo -e "${RED}❌ GPG signing failed${NC}"
    echo "   Check GPG agent and passphrase"
    exit 1
fi
echo ""

# 5. Check cargo login status
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "5. Cargo Authentication"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ -f ~/.cargo/credentials.toml ] || [ -f ~/.cargo/credentials ]; then
    echo -e "${GREEN}✅ Cargo credentials configured${NC}"
else
    echo -e "${YELLOW}⚠️  Cargo credentials not found${NC}"
    echo "   Already logged in via 'cargo login' per user"
fi
echo ""

# Summary
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                       SUMMARY                             ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

if [ -n "$KEY_ID" ] && { [ "$GIT_TAG_SIGN" = "true" ] || [ "$GIT_TAG_SIGN" = "1" ] || [ "$GIT_TAG_SIGN" = "yes" ]; } && { [ "$GIT_COMMIT_SIGN" = "true" ] || [ "$GIT_COMMIT_SIGN" = "1" ] || [ "$GIT_COMMIT_SIGN" = "yes" ]; }; then
    echo -e "${GREEN}✅ All GPG signing requirements met!${NC}"
    echo ""
    echo "Ready to create signed releases:"
    echo "  git tag -s vX.Y.Z -m 'Release vX.Y.Z'"
    echo "  git tag -v vX.Y.Z"
    echo "  git push origin vX.Y.Z"
    echo ""
    exit 0
else
    echo -e "${YELLOW}⚠️  Some configuration needed${NC}"
    echo ""
    echo "Run these commands to complete setup:"
    if [ -z "$GIT_SIGNING_KEY" ]; then
        echo "  git config --global user.signingkey ${KEY_ID}"
    fi
    if [ "$GIT_COMMIT_SIGN" != "true" ] && [ "$GIT_COMMIT_SIGN" != "1" ] && [ "$GIT_COMMIT_SIGN" != "yes" ]; then
        echo "  git config --global commit.gpgsign true"
    fi
    if [ "$GIT_TAG_SIGN" != "true" ] && [ "$GIT_TAG_SIGN" != "1" ] && [ "$GIT_TAG_SIGN" != "yes" ]; then
        echo "  git config --global tag.gpgsign true"
    fi
    echo ""
    exit 1
fi
