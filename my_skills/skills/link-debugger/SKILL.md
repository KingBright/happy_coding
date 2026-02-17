---
name: link-debugger
description: Systematic troubleshooting for data flow and integration issues (Trace Detective)
---

# Link Debugger (Trace Detective) Skill

Systematic troubleshooting methodology for technical link/pipeline issues. Focuses on data flow tracing, key node logging, and context-aware debugging.

## 🧠 Context Awareness (CRITICAL)

**DO NOT ASK** the user to explain the system context if it can be derived.
1.  **Check Architecture**: Read `docs/*.md` (especially `architecture`, `design` docs).
2.  **Use `code-analyst`**: If documentation is missing, use the `code-analyst` skill to map the relevant code paths.
3.  **Check History**: Use `lessons-learner` to check for similar past issues.

Only ask the user for context if it is **specific to the current runtime environment** or strictly undetectable from code.

## 🕵️‍♂️ Troubleshooting Methodology

### Phase 1: Map the Link (Mental Model)
Before touching code, identify the Data Link:
1.  **Source**: Where does the data originate? (UI input, API request, Database, File)
2.  **Transform**: What processes modify the data? (Parsers, Business Logic, Network Transmission)
3.  **Sink**: Where should the data arrive? (UI display, DB write, File output)

### Phase 2: Trace Detective (Binary Search Logging)
If the link is broken (Data doesn't arrive or arrives wrong), use **Binary Search Logging**:

1.  **Identify Midpoint**: Pick a critical node in the middle of the flow.
2.  **Inject Log**: Add a high-visibility log (e.g., `LOG.info(">>> [TRACE] Midpoint Data: {}", data)`)
3.  **Verify**:
    - If log appears + data correct -> Issue is in **second half**.
    - If log missing / data wrong -> Issue is in **first half**.
4.  **Repeat**: Split the remaining scope in half and repeat.

**Rules for Logging:**
- **Keyword**: Use a consistent prefix like `[TRACE]` or `[DEBUG_LINK]` to easily grep logs.
- **Content**: Log critical IDs, timestamps, and payload sizes.
- **Cleanup**: Mark these logs as `// TODO: REMOVE` to ensure cleanup later.

### Phase 3: verification
1.  **Reproduce**: Trigger the flow.
2.  **Analyze**: Check logs in `docker logs`, `tail -f`, or file output.
3.  **Fix**: Once the broken link is found, fix it.
4.  **Cleanup**: Remove the temporary trace logs.

## 🛠️ Tool Usage Guide
- `grep_search`: Find "Source" and "Sink" code locations.
- `write_to_file` / `replace_file_content`: Inject logs.
- `run_command`: Run tests or trigger flows.
- `notify_user`: Report findings with "Evidence" (logs).



