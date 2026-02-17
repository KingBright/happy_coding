#!/bin/bash
set -e

# Configuration
SERVER="root@hackerlife.fun"
SSH_PORT="222"
APP_DIR="/opt/happy-remote"
DATA_DIR="/volume1/docker/happy-remote/data"
BINARY_NAME="happy-server"
DOMAIN="happy.hackerlife.fun"

# Default Ports
HAPPY_SERVER_PORT="16789"
HAPPY_DAEMON_PORT="16790"

# Delegate to the main deploy script with these overrides
# We source the main script but we need to pass arguments to it.
# However, the main script is designed to run directly. 
# Instead, we will call the main script but pass these variables as environment variables 
# or modified arguments. 
#
# Actually, the simplest way since the main script hardcodes these variables at the top
# is to NOT use the main script directly if it doesn't support overrides easily,
# OR we can modify the main script to accept environment variable overrides.
#
# Let's check deploy.sh content again. It sets SERVER="..." etc. at the top.
# If I export these variables, they won't automatically override the assignments in deploy.sh 
# unless deploy.sh uses `SERVER="${SERVER:-root@...}"`.
#
# Since I just sanitized deploy.sh, I know it uses direct assignment.
#
# Strategy: I will copy the content of deploy.sh but with my config. 
# OR somewhat better: I will modify deploy.sh to allow env var overrides.
#
# Let's try to modify deploy.sh first to be uniform.
#
# Wait, the user asked for a "self_deploy script". 
# 
# Usage: ./scripts/self_deploy.sh [options]

# Export variables so they might be used if we tweak deploy.sh, 
# but for now let's just copy the logic or use sed to inject? No that's messy.
#
# Correct approach: Update scripts/deploy.sh to use default values like:
# SERVER="${SERVER:-user@your-server.com}"
#
# Then self_deploy.sh can just export the private values and call deploy.sh.

export SERVER
export SSH_PORT
export APP_DIR
export DATA_DIR
export BINARY_NAME
export DOMAIN
export HAPPY_SERVER_PORT
export HAPPY_DAEMON_PORT

# Get the absolute path to the deploy script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
DEPLOY_SCRIPT="$SCRIPT_DIR/deploy.sh"

# Execute the deploy script with passed arguments
bash "$DEPLOY_SCRIPT" "$@"
