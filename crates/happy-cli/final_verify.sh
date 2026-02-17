#!/bin/bash
set -e

TAG="tag-sys-$(date +%s)"
# 2. Start a background session
echo "Starting background session with tag: $TAG"
# Use --local to avoid cloud registration requirement for this local test
/Users/jinliang/workspace/happy_coding/target/debug/happy run claude --tag $TAG --yes --local > run.log 2>&1 &
RUN_PID=$!
sleep 5

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
            await asyncio.wait_for(websocket.recv(), timeout=5.0)
            attach_msg = {"type": "attach_session", "session_id": "$TAG"}
            print(f"Sending attach for tag: $TAG")
            await websocket.send(json.dumps(attach_msg))
            
            response_raw = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            response = json.loads(response_raw)
            print(f"Server Response: {response}")
            
            if response.get("type") == "session_attached":
                print("SUCCESS: Tag attachment verified!")
                return True
            else:
                return False
                    
    except Exception as e:
        print(f"Error: {e}")
        return False

success = asyncio.run(test_attach())
sys.exit(0 if success else 1)
PYTHONEOF

kill -9 $RUN_PID || true
