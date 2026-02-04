# Happy Coding 🚀

Universal toolkit for AI coding environments - manage Claude, Codex, and Antigravity from a single CLI.

## ✨ Features

- **Multi-platform support** - Build skills/workflows for Claude, Codex, Antigravity
- **Environment management** - Switch between multiple API providers seamlessly
- **Configuration sync** - Keep local and system configs in sync
- **File watching** - Auto-rebuild on file changes in dev mode
- **Cross-platform** - Works on macOS, Linux, and Windows

## 📦 Installation

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

## 🛠️ Usage

### Build Commands
```bash
happy init [name]        # Initialize a new project
happy build [-t target]  # Build for all/specific platforms
happy dev [-t target]    # Development mode with file watching
happy install [-g]       # Install built artifacts
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
├── claude_settings.json    # Local config backup
├── docs/                   # Research & design documents
│   ├── RESEARCH_SUMMARY.md
│   ├── claude_code_research_report.md
│   ├── openai_codex_research_report.md
│   ├── antigravity_research_report.md
│   └── unified_devkit_architecture.md
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
  antigravity:
    enabled: false

skills:
  - name: code-review
    description: Review code for best practices
    prompt: |
      Review the following code and provide feedback...

workflows:
  - name: release
    description: Prepare a release
    steps:
      - skill: code-review
      - command: cargo test
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
