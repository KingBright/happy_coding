import json
import asyncio
import websockets
import sys

async def test_attach():
    uri = "ws://127.0.0.1:16790"
    print(f"Connecting to {uri}...")
    try:
        async with websockets.connect(uri) as websocket:
            msg = await asyncio.wait_for(websocket.recv(), timeout=2.0)
            print(f"Connected Event: {msg}")
            
            attach_msg = {
                "type": "attach_session",
                "session_id": "verification-test"
            }
            print(f"Sending: {attach_msg}")
            await websocket.send(json.dumps(attach_msg))
            
            response = await asyncio.wait_for(websocket.recv(), timeout=2.0)
            print(f"Server Response: {response}")
            
    except Exception as e:
        print(f"Error: {e}")

asyncio.run(test_attach())
