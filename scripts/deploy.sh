#!/bin/bash
set -e

# Configuration
SERVER="${SERVER:-user@your-server.com}"
SSH_PORT="${SSH_PORT:-22}"
APP_DIR="${APP_DIR:-/opt/happy-remote}"
DATA_DIR="${DATA_DIR:-/var/lib/happy-remote/data}"
BINARY_NAME="${BINARY_NAME:-happy-server}"
DOMAIN="${DOMAIN:-happy.your-domain.com}"

# Default Ports
HAPPY_SERVER_PORT="${HAPPY_SERVER_PORT:-16789}"
HAPPY_DAEMON_PORT="${HAPPY_DAEMON_PORT:-16790}"

# Argument Parsing
DEPLOY_FRONTEND=false
DEPLOY_SERVER=false
SHOW_HELP=false

usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -f, --frontend   Deploy Frontend (WASM) only"
    echo "  -s, --server     Deploy Happy Server only"
    echo "  -a, --all        Deploy both Frontend and Server (default)"
    echo "  -p, --port       Set server port (default: 16789)"
    echo "  -d, --daemon-port Set daemon port (default: 16790)"
    echo "  -h, --help       Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Deploy both frontend and server"
    echo "  $0 -f                 # Deploy only frontend"
    echo "  $0 -s -p 3000         # Deploy server on port 3000"
    echo ""
    exit 1
}

# Parse arguments
if [ $# -eq 0 ]; then
    DEPLOY_FRONTEND=true
    DEPLOY_SERVER=true
else
    while [[ $# -gt 0 ]]; do
        case $1 in
            -f|--frontend)
                DEPLOY_FRONTEND=true
                shift
                ;;
            -s|--server)
                DEPLOY_SERVER=true
                shift
                ;;
            -a|--all)
                DEPLOY_FRONTEND=true
                DEPLOY_SERVER=true
                shift
                ;;
            -p|--port)
                HAPPY_SERVER_PORT="$2"
                shift 2
                ;;
            -d|--daemon-port)
                HAPPY_DAEMON_PORT="$2"
                shift 2
                ;;
            -h|--help)
                usage
                ;;
            *)
                echo "Unknown option: $1"
                usage
                ;;
        esac
    done
fi

echo "========================================"
echo "  Happy Remote Deployment"
echo "========================================"
echo "Server: $SERVER (Port: $SSH_PORT)"
echo "App Dir: $APP_DIR"
echo ""

if [ "$DEPLOY_FRONTEND" = true ]; then
    echo ">>> Target: Frontend (WASM)"
fi
if [ "$DEPLOY_SERVER" = true ]; then
    echo ">>> Target: Happy Server (Backend)"
    echo ">>> Server Port: $HAPPY_SERVER_PORT"
    echo ">>> Daemon Port: $HAPPY_DAEMON_PORT"
fi
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color


# 1. Build Frontend (WASM)
if [ "$DEPLOY_FRONTEND" = true ]; then
    echo -e "${YELLOW}>>> Building Frontend (WASM)...${NC}"

    # Ensure wasm target is installed
    if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
        echo ">>> Installing wasm32-unknown-unknown target..."
        rustup target add wasm32-unknown-unknown
    fi

    # Install wasm-bindgen-cli if not present
    if ! command -v wasm-bindgen &> /dev/null; then
        echo ">>> Installing wasm-bindgen-cli..."
        cargo install wasm-bindgen-cli
    fi

    # Build WASM package
    cd crates/happy-web
    cargo build --release --target wasm32-unknown-unknown

    # Generate JS bindings
    wasm-bindgen --out-dir pkg --target web --no-typescript \
        ../../target/wasm32-unknown-unknown/release/happy-web.wasm

    # Copy static assets
    mkdir -p dist/pkg
    cp pkg/* dist/pkg/
    cp index.html dist/
    cp style.css dist/pkg/ 2>/dev/null || true
    mkdir -p dist/assets
    cp -r assets/* dist/assets/ 2>/dev/null || true

    cd ../..
    echo -e "${GREEN}>>> Frontend build complete${NC}"
fi

# 2. Build Backend (Native Cross-compile for Linux x86_64)
if [ "$DEPLOY_SERVER" = true ]; then
    echo -e "${YELLOW}>>> Building Happy Server (Linux x86_64)...${NC}"

    # Ensure target is installed
    if ! rustup target list | grep -q "x86_64-unknown-linux-musl (installed)"; then
        echo ">>> Installing x86_64-unknown-linux-musl target..."
        rustup target add x86_64-unknown-linux-musl
    fi

    cargo build -p happy-server --release --target x86_64-unknown-linux-musl

    echo -e "${GREEN}>>> Server build complete${NC}"
fi

# 3. Prepare Remote Directories
echo -e "${YELLOW}>>> Preparing remote directories...${NC}"
ssh -p $SSH_PORT $SERVER "mkdir -p $APP_DIR $DATA_DIR"

# 4. Stop remote service before uploading
if [ "$DEPLOY_SERVER" = true ]; then
    echo -e "${YELLOW}>>> Stopping remote service...${NC}"
    ssh -p $SSH_PORT $SERVER "systemctl stop happy-remote || true"
fi

# 5. Upload Frontend
if [ "$DEPLOY_FRONTEND" = true ]; then
    echo -e "${YELLOW}>>> Compressing Frontend...${NC}"
    tar -czf frontend.tar.gz -C crates/happy-web/dist .

    echo -e "${YELLOW}>>> Uploading Frontend...${NC}"
    ssh -p $SSH_PORT $SERVER "rm -rf $APP_DIR/frontend"
    scp -O -P $SSH_PORT frontend.tar.gz $SERVER:$APP_DIR/

    echo -e "${YELLOW}>>> Extracting Frontend...${NC}"
    ssh -p $SSH_PORT $SERVER "mkdir -p $APP_DIR/frontend && tar -xzf $APP_DIR/frontend.tar.gz -C $APP_DIR/frontend && rm $APP_DIR/frontend.tar.gz"

    rm frontend.tar.gz
    echo -e "${GREEN}>>> Frontend deployed${NC}"
fi

# 6. Upload Server Binary and Configure
if [ "$DEPLOY_SERVER" = true ]; then
    echo -e "${YELLOW}>>> Uploading Server Binary...${NC}"
    scp -O -P $SSH_PORT target/x86_64-unknown-linux-musl/release/happy-server $SERVER:$APP_DIR/

    # Retrieve existing JWT_SECRET if available to prevent session invalidation
    echo -e "${YELLOW}>>> Checking for existing configuration...${NC}"
    EXISTING_SECRET=$(ssh -p $SSH_PORT $SERVER "grep JWT_SECRET $APP_DIR/happy-remote.env 2>/dev/null | cut -d= -f2" || true)
    
    if [ -n "$EXISTING_SECRET" ]; then
        echo ">>> Found existing JWT_SECRET, preserving it."
        JWT_SECRET="$EXISTING_SECRET"
    else
        echo ">>> No existing JWT_SECRET found, generating new one."
        JWT_SECRET=$(openssl rand -base64 32)
    fi

    # Generate Configuration File
    echo -e "${YELLOW}>>> Generating Configuration...${NC}"
    cat <<EOF > happy-remote.env
# Happy Remote Server Configuration
BIND_ADDRESS=0.0.0.0:$HAPPY_SERVER_PORT
DATABASE_PATH=$DATA_DIR/happy_remote.db
DATA_DIR=$DATA_DIR
JWT_SECRET=$JWT_SECRET
RUST_LOG=info

# Static files directory
STATIC_DIR=$APP_DIR/frontend
EOF

    scp -O -P $SSH_PORT happy-remote.env $SERVER:$APP_DIR/happy-remote.env
    rm happy-remote.env

    # Upload Systemd Service
    echo -e "${YELLOW}>>> Configuring Systemd Service...${NC}"
    cat <<EOF > happy-remote.service
[Unit]
Description=Happy Remote Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=$APP_DIR
ExecStart=$APP_DIR/happy-server
Restart=always
RestartSec=5
EnvironmentFile=$APP_DIR/happy-remote.env

[Install]
WantedBy=multi-user.target
EOF

    scp -O -P $SSH_PORT happy-remote.service $SERVER:/etc/systemd/system/happy-remote.service
    rm happy-remote.service

    # Reload and restart service
    echo -e "${YELLOW}>>> Starting Service...${NC}"
    ssh -p $SSH_PORT $SERVER "systemctl daemon-reload && systemctl enable happy-remote && systemctl restart happy-remote"

    echo -e "${GREEN}>>> Server deployed and started${NC}"
fi

# 7. Deployment Summary
echo ""
echo "========================================"
echo -e "${GREEN}  Deployment Complete!${NC}"
echo "========================================"
echo ""
echo "Access Information:"
echo "  Web Dashboard: https://$DOMAIN"
echo "  API Endpoint:  https://$DOMAIN/api/v1"
echo "  WebSocket:     wss://$DOMAIN/ws"
if [ "$DEPLOY_SERVER" = true ]; then
    echo ""
    echo "Port Information:"
    echo "  - Server Port: $HAPPY_SERVER_PORT (HTTP/WebSocket)"
    echo "  - Configure your router to forward port 80/443 to $HAPPY_SERVER_PORT"
fi
echo ""
echo "CLI Configuration:"
echo "  Run the following on your development machine:"
echo ""
echo "    happy config set-server https://$DOMAIN"
echo ""
echo "Service Management (on server):"
echo "  systemctl status happy-remote  # Check status"
echo "  systemctl stop happy-remote    # Stop service"
echo "  systemctl start happy-remote   # Start service"
echo "  journalctl -u happy-remote -f  # View logs"
echo ""
