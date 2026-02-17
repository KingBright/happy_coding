# Happy Coding 🚀

Universal toolkit for AI coding environments - manage Claude, Codex, and Antigravity from a single CLI.

## ✨ Core Features

### 1. Skill Management 📦
Define, build, and distribute your own AI skills.
- **One-click Install**: Compile and install skills to multiple agents (Claude, Codex, Antigravity) with a single command.
- **Idempotent Builds**: Smart build system that prevents duplication and ensures consistency.

### 2. Custom API Management 🔌
Manage your own custom API gateways and providers.
- **Multi-provider Support**: Switch seamlessly between Claude, OpenAI, and custom endpoints.
- **Environment Isolation**: extensive support for distinct development, testing, and production API environments.

### 3. Configuration Management ⚙️
Take control of your AI environment settings.
- **Claude Config Sync**: Keep your `claude_settings.json` in sync between your project and the system.
- **Version Controlled Configs**: Manage agent behavior instructions and settings as code.

### 4. Remote Web Interface 🌐
1-to-Many remote programming via a centralized web dashboard.
- **Remote Control**: Access and manage multiple coding sessions from a browser.
- **Unified Dashboard**: Monitor and interact with distributed agent instances.

## 🧰 Core Skills Library

The toolkit includes a set of high-quality, pre-built skills for professional AI-assisted development:

| Skill | Description |
|-------|-------------|
| **`tdd-workflow`** | Enforce Test-Driven Development loops with strict coverage requirements. |
| **`verification-loop`** | Comprehensive pre-commit check (Build, Type, Lint, Test, Security). |
| **`project-architect`** | "Day 0" skill for structural design, interface definitions, and roadmap planning. |
| **`task-orchestrator`** | "Project Manager" for breaking down complex requirements into tracked tasks. |
| **`code-analyst`** | Deep project analysis generating architecture reports and competitive comparisons. |
| **`lessons-learner`** | Long-term memory system for recording/retrieving mistakes (Category-based knowledge base). |
| **`security-review`** | Security audit checklist and patterns. |
| **`mcp-builder`** | Guide for building Model Context Protocol (MCP) servers. |
| **`skill-creator`** | Meta-skill for creating and refining new skills. |
| **`agent-framework`** | Agent architecture patterns, tool execution, and MCP integration. |
| **`link-debugger`** | Systematic troubleshooting for data flow and integration issues. |
| **`secure-execution`** | Security hooks for bash command validation and sandboxed execution. |

## �📦 Installation

### From source
```bash
git clone https://github.com/your-repo/happy_coding.git
cd happy_coding
cargo install --path crates/happy-cli
```

### Verify installation
```bash
happy --version
happy --help
```

### Install Core Skills (Optional)
To use the core skills (like `code-analyst` or `project-architect`) in your AI sessions, build and install them globally:

```bash
cd my_skills
happy build
happy install --global
```

## 🛠️ Usage

### Build Commands
```bash
happy init [name]        # Initialize a new skill project
happy build [-t target]  # Build for all/specific platforms
happy dev [-t target]    # Development mode with file watching
happy install --global   # Install built artifacts to global environment (~/.claude, etc.)
happy validate           # Validate configuration
happy doctor             # Diagnose environment setup
```

### Environment Management
Manage multiple Claude API environments (useful for switching between providers):

```bash
happy env list           # List all configured environments
happy env add <name>     # Add a new environment (interactive)
happy env use <name>     # Switch to an environment
happy env delete <name>  # Delete an environment
happy env run [name]     # Run Claude with specific environment
```

### Remote Programming 📡
Enable 1-to-many remote coding sessions via the web interface:

```bash
# On your remote server (e.g., cloud VM)
happy daemon start       # Start the background daemon
happy auth login         # Authenticate with your account
happy run                # Start a new coding session

# On your local machine (optional, for management)
happy notify --msg "Job done"
```

Access the **Web Dashboard** at `https://happy.your-domain.com` to manage your sessions.

#### Server Deployment
You can easily deploy the server component to a Linux VM using the provided script:

```bash
# Configure your deployment
export SERVER="root@your-ip"
export DOMAIN="happy.your-domain.com"

# specific deployment
./scripts/deploy.sh --server
```

**Environment Variables** (`happy-remote.env`):
- `BIND_ADDRESS`: IP/Port to bind (default: `0.0.0.0:16789`)
- `JWT_SECRET`: Secret for authentication security
- `DATA_DIR`: Path to store session data

### Configuration Sync
Sync Claude settings between local project and system:

```bash
happy config push        # Push local config → ~/.claude/
happy config pull        # Pull ~/.claude/ → local config
happy config diff        # Show diff between local and system
```

## 🎯 Supported Platforms

| Platform | Output Directory | Generated Files |
|----------|------------------|-----------------|
| Claude Code | `.claude/` | `skills/*/SKILL.md`, `settings.json`, `mcp.json` |
| OpenAI Codex | `.codex/` | `skills/*/SKILL.md`, `AGENTS.md`, `config.toml` |
| Antigravity | `.agent/` | `skills/`, `workflows/`, `rules/` |

## 📁 Project Structure

```
happy_coding/
├── Cargo.toml              # Workspace configuration
├── Cargo.lock
├── README.md
├── .gitignore
├── my_skills/              # Example User Project (Reference)
│   ├── happy.config.yaml
│   └── skills/
└── crates/
    ├── happy-core/         # Core types, config, builder, watcher
    ├── happy-adapters/     # Platform-specific adapters
    └── happy-cli/          # CLI application (binary: happy)
```

## 📝 Configuration Example

Create `happy.config.yaml` in your project:

```yaml
name: my-project
version: "1.0.0"
description: My AI-powered development project

targets:
  claude:
    enabled: true
  codex:
    enabled: true

skills:
  # Option 1: Path-based (Recommended)
  - name: project-architect
    path: skills/project-architect
    description: "Start new projects with structured technical foundation"

  # Option 2: Inline Prompt
  - name: quick-fix
    description: "Apply quick fixes"
    prompt: |
      Analyze the error and suggest 3 potential fixes.
```

## 🔧 Development

```bash
# Run in development
cargo run --bin happy -- --help

# Run tests
cargo test

# Build release
cargo build --release
```

## 📄 License

MIT
