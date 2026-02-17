---
name: secure-execution
description: Security hooks for bash command validation and sandboxed execution
---

# Secure Execution Skill

Security hooks for bash command validation and execution sandboxing.

## Overview

This skill provides patterns for implementing secure command execution in AI agents. It uses an allowlist approach where only explicitly permitted commands can run, with additional validation for sensitive operations.

## When to Activate

- Building an AI agent that executes shell commands
- Implementing pre-tool-use security hooks
- Creating sandboxed development environments
- Adding command validation to autonomous coding systems

## Core Concepts

### 1. Allowlist-Based Security Model

Instead of trying to block all dangerous commands (blacklist), explicitly allow only safe commands:

```python
# Allowed commands for development tasks
ALLOWED_COMMANDS = {
    # File inspection
    "ls", "cat", "head", "tail", "wc", "grep",
    # File operations
    "cp", "mkdir", "chmod",  # chmod needs extra validation
    # Directory
    "pwd",
    # Node.js development
    "npm", "node",
    # Version control
    "git",
    # Process management
    "ps", "lsof", "sleep", "pkill",  # pkill needs extra validation
}

# Commands requiring additional validation
COMMANDS_NEEDING_EXTRA_VALIDATION = {"pkill", "chmod", "init.sh"}
```

### 2. Command Parsing

Handle complex shell syntax (pipes, chaining, subshells):

```python
import shlex
import re

def extract_commands(command_string: str) -> list[str]:
    """Extract command names from shell input."""
    commands = []

    # Split on semicolons (simple heuristic)
    segments = re.split(r'(?<!["\'])\s*;\s*(?!["\'])', command_string)

    for segment in segments:
        segment = segment.strip()
        if not segment:
            continue

        try:
            tokens = shlex.split(segment)
        except ValueError:
            # Malformed command - fail safe
            return []

        expect_command = True
        for token in tokens:
            # Shell operators indicate new command
            if token in ("|", "||", "&&", "&"):
                expect_command = True
                continue

            # Skip shell keywords
            if token in ("if", "then", "else", "fi", "for", "while", "do", "done"):
                continue

            # Skip flags and variable assignments
            if token.startswith("-") or "=" in token:
                continue

            if expect_command:
                cmd = os.path.basename(token)
                commands.append(cmd)
                expect_command = False

    return commands
```

### 3. Security Hook Interface

```python
async def bash_security_hook(input_data, tool_use_id=None, context=None):
    """
    Pre-tool-use hook for bash command validation.

    Returns:
        Empty dict to allow execution
        {"decision": "block", "reason": "..."} to block
    """
    if input_data.get("tool_name") != "Bash":
        return {}

    command = input_data.get("tool_input", {}).get("command", "")
    if not command:
        return {}

    commands = extract_commands(command)

    if not commands:
        return {
            "decision": "block",
            "reason": f"Could not parse command: {command}"
        }

    # Check allowlist
    for cmd in commands:
        if cmd not in ALLOWED_COMMANDS:
            return {
                "decision": "block",
                "reason": f"Command '{cmd}' not in allowlist"
            }

        # Extra validation for sensitive commands
        if cmd in COMMANDS_NEEDING_EXTRA_VALIDATION:
            allowed, reason = validate_command(cmd, command)
            if not allowed:
                return {"decision": "block", "reason": reason}

    return {}
```

### 4. Command-Specific Validation

**pkill validation** - Only allow killing dev processes:

```python
def validate_pkill_command(command_string: str) -> tuple[bool, str]:
    allowed_processes = {"node", "npm", "npx", "vite", "next"}

    tokens = shlex.split(command_string)
    args = [t for t in tokens[1:] if not t.startswith("-")]

    if not args:
        return False, "pkill requires a process name"

    target = args[-1].split()[0]  # Handle "pkill -f 'node server.js'"

    if target in allowed_processes:
        return True, ""
    return False, f"pkill only allowed for: {allowed_processes}"
```

**chmod validation** - Only allow making files executable:

```python
def validate_chmod_command(command_string: str) -> tuple[bool, str]:
    tokens = shlex.split(command_string)
    mode = None

    for token in tokens[1:]:
        if token.startswith("-"):
            return False, "chmod flags not allowed"
        elif mode is None:
            mode = token

    # Only allow +x variants
    if not re.match(r"^[ugoa]*\+x$", mode):
        return False, f"chmod only allowed with +x mode, got: {mode}"

    return True, ""
```

## Integration with Agent SDK

### TypeScript/Node.js

```typescript
import { Agent } from '@anthropic-ai/agents';

const agent = new Agent({
  name: "coding-agent",
  systemPrompt: "...",
  hooks: {
    preToolUse: async (toolUse) => {
      if (toolUse.toolName === 'Bash') {
        const result = await validateBashCommand(toolUse.input);
        if (result.decision === 'block') {
          throw new Error(`Security: ${result.reason}`);
        }
      }
      return toolUse;
    }
  }
});
```

### Python

```python
from agents import Agent

async def security_hook(tool_use_id, tool_name, tool_input):
    if tool_name == "Bash":
        result = await bash_security_hook({
            "tool_name": tool_name,
            "tool_input": tool_input
        })
        if result.get("decision") == "block":
            raise SecurityError(result["reason"])

agent = Agent(
    name="coding-agent",
    system="...",
    hooks={"pre_tool_use": security_hook}
)
```

## Best Practices

1. **Fail Secure**: If parsing fails, block the command
2. **Use shlex**: Don't use regex for command parsing when possible
3. **Validate Early**: Check before any command execution
4. **Log Blocked Commands**: For debugging and auditing
5. **Test Thoroughly**: Include edge cases like nested commands

## Example: Complete Validation

```python
import shlex
import re
import os

ALLOWED_COMMANDS = {
    "ls", "cat", "pwd", "git", "npm", "node"
}

async def validate_bash_command(command: str) -> dict:
    """Complete validation example."""

    # Parse command
    try:
        tokens = shlex.split(command)
    except ValueError:
        return {"decision": "block", "reason": "Malformed command"}

    if not tokens:
        return {"decision": "block", "reason": "Empty command"}

    # Extract base command
    base_cmd = os.path.basename(tokens[0])

    # Check allowlist
    if base_cmd not in ALLOWED_COMMANDS:
        return {
            "decision": "block",
            "reason": f"'{base_cmd}' not in allowed commands"
        }

    # Allow
    return {}
```


