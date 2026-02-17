#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔹 Installing Happy Coding CLI locally...${NC}"

# Ensure we are in the project root
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

cd "$PROJECT_ROOT"

# Check dependencies
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi

echo -e "${BLUE}📦 Building and installing 'happy'...${NC}"
cargo install --path crates/happy-cli --force

echo -e "${GREEN}✅ Successfully installed 'happy'!${NC}"
echo "Try running: happy --help"
