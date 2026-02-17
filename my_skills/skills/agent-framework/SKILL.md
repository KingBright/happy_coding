---
name: agent-framework
description: Agent architecture patterns, tool execution, and MCP integration
---

# Agent Framework Skill

Core patterns for building AI agents with Claude API, tool execution, and MCP integration.

## Overview

This skill provides foundational patterns for agent architecture including the agent loop, message history management, tool definitions, and MCP server integration.

## When to Activate

- Building a custom AI agent from scratch
- Implementing tool use with Claude API
- Managing conversation context and token budgets
- Integrating MCP (Model Context Protocol) servers
- Designing async tool execution workflows

## Core Architecture

### 1. Agent Loop Pattern

The fundamental agent execution cycle:

```python
class Agent:
    async def _agent_loop(self, user_input: str):
        # 1. Add user message to history
        await self.history.add_message("user", user_input, None)

        tool_dict = {tool.name: tool for tool in self.tools}

        while True:
            # 2. Truncate if context window exceeded
            self.history.truncate()

            # 3. Call Claude API
            params = self._prepare_message_params()
            response = self.client.messages.create(**params)

            # 4. Extract tool calls
            tool_calls = [
                block for block in response.content
                if block.type == "tool_use"
            ]

            # 5. Add assistant response to history
            await self.history.add_message(
                "assistant", response.content, response.usage
            )

            if tool_calls:
                # 6. Execute tools and add results
                tool_results = await execute_tools(
                    tool_calls, tool_dict, parallel=True
                )
                await self.history.add_message("user", tool_results)
            else:
                # 7. No tool calls - return final response
                return response
```

### 2. Tool Definition Pattern

Define tools using dataclasses with schema:

```python
from dataclasses import dataclass
from typing import Any

@dataclass
class Tool:
    """Base class for all agent tools."""
    name: str
    description: str
    input_schema: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        """Convert to Claude API format."""
        return {
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema,
        }

    async def execute(self, **kwargs) -> str:
        """Execute the tool - must be implemented by subclasses."""
        raise NotImplementedError


# Example implementation
@dataclass
class FileReadTool(Tool):
    def __init__(self):
        super().__init__(
            name="file_read",
            description="Read contents of a file",
            input_schema={
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to read"
                    }
                },
                "required": ["path"]
            }
        )

    async def execute(self, path: str) -> str:
        with open(path, 'r') as f:
            return f.read()
```

### 3. Message History with Token Management

```python
class MessageHistory:
    def __init__(
        self,
        model: str,
        system: str,
        context_window_tokens: int,
        client: Any,
        enable_caching: bool = True,
    ):
        self.model = model
        self.system = system
        self.context_window_tokens = context_window_tokens
        self.messages: list[dict] = []
        self.total_tokens = 0
        self.enable_caching = enable_caching
        self.message_tokens: list[tuple[int, int]] = []
        self.client = client

        # Count system prompt tokens
        try:
            system_token = (
                self.client.messages.count_tokens(
                    model=self.model,
                    system=self.system,
                    messages=[{"role": "user", "content": "test"}],
                ).input_tokens - 1
            )
        except Exception:
            system_token = len(self.system) / 4  # Fallback estimate

        self.total_tokens = system_token

    async def add_message(
        self,
        role: str,
        content: str | list[dict],
        usage: Any | None = None,
    ):
        """Add message and track token usage."""
        if isinstance(content, str):
            content = [{"type": "text", "text": content}]

        message = {"role": role, "content": content}
        self.messages.append(message)

        if role == "assistant" and usage:
            # Include cache tokens in total
            total_input = (
                usage.input_tokens
                + getattr(usage, "cache_read_input_tokens", 0)
                + getattr(usage, "cache_creation_input_tokens", 0)
            )
            output_tokens = usage.output_tokens

            current_turn_input = total_input - self.total_tokens
            self.message_tokens.append((current_turn_input, output_tokens))
            self.total_tokens += current_turn_input + output_tokens

    def truncate(self) -> None:
        """Remove oldest messages when context limit exceeded."""
        if self.total_tokens <= self.context_window_tokens:
            return

        TRUNCATION_NOTICE_TOKENS = 25
        TRUNCATION_MESSAGE = {
            "role": "user",
            "content": [{
                "type": "text",
                "text": "[Earlier history has been truncated.]",
            }],
        }

        def remove_message_pair():
            """Remove a user-assistant message pair."""
            self.messages.pop(0)
            self.messages.pop(0)
            if self.message_tokens:
                input_toks, output_toks = self.message_tokens.pop(0)
                self.total_tokens -= input_toks + output_toks

        while (
            self.message_tokens
            and len(self.messages) >= 2
            and self.total_tokens > self.context_window_tokens
        ):
            remove_message_pair()

            if self.messages and self.message_tokens:
                original_input, original_output = self.message_tokens[0]
                self.messages[0] = TRUNCATION_MESSAGE
                self.message_tokens[0] = (
                    TRUNCATION_NOTICE_TOKENS,
                    original_output,
                )
                self.total_tokens += (
                    TRUNCATION_NOTICE_TOKENS - original_input
                )

    def format_for_api(self) -> list[dict]:
        """Format messages for API with optional caching."""
        result = [
            {"role": m["role"], "content": m["content"]}
            for m in self.messages
        ]

        # Add prompt caching to the last message
        if self.enable_caching and self.messages:
            result[-1]["content"] = [
                {**block, "cache_control": {"type": "ephemeral"}}
                for block in self.messages[-1]["content"]
            ]
        return result
```

### 4. Parallel Tool Execution

```python
import asyncio

async def _execute_single_tool(
    call: Any, tool_dict: dict[str, Tool]
) -> dict[str, Any]:
    """Execute a single tool and handle errors."""
    response = {
        "type": "tool_result",
        "tool_use_id": call.id
    }

    try:
        result = await tool_dict[call.name].execute(**call.input)
        response["content"] = str(result)
    except KeyError:
        response["content"] = f"Tool '{call.name}' not found"
        response["is_error"] = True
    except Exception as e:
        response["content"] = f"Error: {str(e)}"
        response["is_error"] = True

    return response


async def execute_tools(
    tool_calls: list[Any],
    tool_dict: dict[str, Tool],
    parallel: bool = True
) -> list[dict[str, Any]]:
    """Execute multiple tools in parallel or sequentially."""

    if parallel:
        return await asyncio.gather(*[
            _execute_single_tool(call, tool_dict)
            for call in tool_calls
        ])
    else:
        results = []
        for call in tool_calls:
            results.append(
                await _execute_single_tool(call, tool_dict)
            )
        return results
```

### 5. MCP Server Integration

```python
from contextlib import AsyncExitStack

async def setup_mcp_connections(
    mcp_servers: list[dict],
    stack: AsyncExitStack
) -> list[Tool]:
    """Connect to MCP servers and return their tools."""
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client

    tools = []

    for server in mcp_servers:
        server_params = StdioServerParameters(
            command=server["command"],
            args=server.get("args", []),
            env={**os.environ, **server.get("env", {})}
        )

        transport = await stack.enter_async_context(
            stdio_client(server_params)
        )
        read, write = transport

        session = await stack.enter_async_context(
            ClientSession(read, write)
        )
        await session.initialize()

        # Convert MCP tools to agent tools
        mcp_tools = await session.list_tools()
        for tool in mcp_tools.tools:
            tools.append(MCPToolAdapter(session, tool))

    return tools


class MCPToolAdapter(Tool):
    """Adapts MCP tools to agent Tool interface."""

    def __init__(self, session, mcp_tool):
        self.session = session
        self.mcp_tool = mcp_tool
        super().__init__(
            name=mcp_tool.name,
            description=mcp_tool.description,
            input_schema=mcp_tool.inputSchema
        )

    async def execute(self, **kwargs) -> str:
        result = await self.session.call_tool(
            self.name,
            arguments=kwargs
        )
        return str(result.content)
```

### 6. Complete Agent Class

```python
from dataclasses import dataclass
from typing import Any
import os

@dataclass
class ModelConfig:
    """Configuration for Claude model parameters."""
    model: str = "claude-sonnet-4-20250514"
    max_tokens: int = 4096
    temperature: float = 1.0
    context_window_tokens: int = 180000


class Agent:
    """Claude-powered agent with tool use capabilities."""

    def __init__(
        self,
        name: str,
        system: str,
        tools: list[Tool] | None = None,
        mcp_servers: list[dict] | None = None,
        config: ModelConfig | None = None,
        verbose: bool = False,
    ):
        self.name = name
        self.system = system
        self.verbose = verbose
        self.tools = list(tools or [])
        self.config = config or ModelConfig()
        self.mcp_servers = mcp_servers or []

        from anthropic import Anthropic
        self.client = Anthropic(
            api_key=os.environ.get("ANTHROPIC_API_KEY", "")
        )

        self.history = MessageHistory(
            model=self.config.model,
            system=self.system,
            context_window_tokens=self.config.context_window_tokens,
            client=self.client,
        )

    def _prepare_message_params(self) -> dict[str, Any]:
        """Prepare parameters for API call."""
        return {
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "system": self.system,
            "messages": self.history.format_for_api(),
            "tools": [tool.to_dict() for tool in self.tools],
        }

    async def run_async(self, user_input: str) -> Any:
        """Run agent with MCP tools."""
        async with AsyncExitStack() as stack:
            original_tools = list(self.tools)

            try:
                mcp_tools = await setup_mcp_connections(
                    self.mcp_servers, stack
                )
                self.tools.extend(mcp_tools)
                return await self._agent_loop(user_input)
            finally:
                self.tools = original_tools

    def run(self, user_input: str) -> Any:
        """Run agent synchronously."""
        import asyncio
        return asyncio.run(self.run_async(user_input))
```

## Configuration Patterns

### Model Configurations

```python
# Fast responses
FAST_CONFIG = ModelConfig(
    model="claude-haiku-4-5-20251001",
    max_tokens=1024,
    temperature=0.5,
)

# Complex reasoning
REASONING_CONFIG = ModelConfig(
    model="claude-opus-4-20250514",
    max_tokens=8192,
    temperature=1.0,
)

# Balanced
DEFAULT_CONFIG = ModelConfig(
    model="claude-sonnet-4-20250514",
    max_tokens=4096,
    temperature=1.0,
)
```

## Best Practices

1. **Token Management**: Always track token usage to avoid context window overflow
2. **Error Handling**: Wrap tool execution to prevent agent crashes
3. **Parallel Execution**: Use `asyncio.gather` for independent tool calls
4. **MCP Cleanup**: Use `AsyncExitStack` for proper resource cleanup
5. **Prompt Caching**: Enable caching on the last message for cost savings
6. **Truncation**: Implement graceful degradation when context is exceeded

## Example Usage

```python
# Create tools
tools = [
    FileReadTool(),
    FileWriteTool(),
    WebSearchTool(),
]

# Configure MCP servers
mcp_servers = [
    {
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-puppeteer"]
    }
]

# Initialize agent
agent = Agent(
    name="coding-assistant",
    system="You are a helpful coding assistant...",
    tools=tools,
    mcp_servers=mcp_servers,
    config=ModelConfig(),
    verbose=True
)

# Run
response = agent.run("Read the README and summarize the project")
```


