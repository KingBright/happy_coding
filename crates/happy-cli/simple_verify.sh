#!/bin/bash
set -e

../../target/debug/happy daemon start || true
sleep 1

TAG="tag-sys-$RANDOM"
echo "Starting background session with tag: $TAG"
# Use a command that exits or just run it and kill it immediately
# The goal is to get the state file created
../../target/debug/happy run --tag $TAG --local > /dev/null 2>&1 &
RUN_PID=$!
sleep 2
kill -9 $RUN_PID || true
sleep 1

echo "Verifying WebSocket attach by tag: $TAG"
python3 -u - <<PYTHONEOF
import json
import asyncio
import websockets
import sys

async def test_attach():
    uri = "ws://127.0.0.1:16790"
    try:
        async with websockets.connect(uri) as websocket:
            # 1. Connected
            await asyncio.wait_for(websocket.recv(), timeout=5.0)
            
            # 2. Attach
            attach_msg = {"type": "attach_session", "session_id": "$TAG"}
            print(f"Sending attach for tag: $TAG")
            await websocket.send(json.dumps(attach_msg))
            
            # 3. Response
            response = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            print(f"Server Response: {response}")
            
            if "attached" in response.lower() or "session_attached" in response.lower():
                print("SUCCESS")
                return True
            else:
                print("FAILURE")
                return False
    except Exception as e:
        print(f"Error: {e}")
        return False

success = asyncio.run(test_attach())
sys.exit(0 if success else 1)
PYTHONEOF
