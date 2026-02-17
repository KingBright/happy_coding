#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}>>> Building happy-cli...${NC}"
cargo install --path crates/happy-cli --locked

# Copy to /usr/local/bin/happy if happy resolves there
BIN_PATH=$(which happy)
if [ "$BIN_PATH" == "/usr/local/bin/happy" ]; then
    echo -e "${YELLOW}>>> Updating /usr/local/bin/happy...${NC}"
    cp ~/.cargo/bin/happy /usr/local/bin/happy
fi

echo -e "${YELLOW}>>> Stopping daemon...${NC}"
happy daemon stop 2>/dev/null || true

sleep 1

echo -e "${YELLOW}>>> Starting daemon...${NC}"
happy daemon start

sleep 1

echo -e "${YELLOW}>>> Checking daemon status...${NC}"
happy daemon status

echo ""
echo -e "${GREEN}>>> Local upgrade complete!${NC}"
echo ""
echo "You can now run: happy run claude --remote"
echo "View logs with: tail -f ~/.happy/daemon.log"
