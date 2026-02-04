# Claude Code 功能与扩展能力调研报告

## 概述

本报告调研了 Anthropic 的 Claude Code CLI 工具及其桌面版本的功能和扩展能力，包括自定义技能、工作流、命令、配置系统以及扩展机制。

**调研日期**: 2025年1月

---

## 1. Claude Code CLI

### 1.1 是否支持自定义 skill/prompt 模板？

**✅ 是**

Claude Code CLI 完全支持自定义 Skills（技能），这是其核心的扩展机制之一。

**实现方式**:
- Skills 通过 `SKILL.md` 文件定义，使用 YAML frontmatter + Markdown 内容格式
- 支持两种作用域：
  - **个人技能**: `~/.claude/skills/<skill-name>/SKILL.md` - 跨所有项目可用
  - **项目技能**: `.claude/skills/<skill-name>/SKILL.md` - 仅在当前项目可用

**Skill 文件结构示例**:
```markdown
---
name: explain-code
description: Explains code with visual diagrams and analogies. Use when explaining how code works...
---

When explaining code, always include:
1. A visual diagram using ASCII art or Mermaid
2. A real-world analogy
3. A step-by-step breakdown
```

**重要更新**: 自定义 slash commands 已合并到 skills 系统中。原有的 `.claude/commands/` 目录仍然兼容，但新推荐使用 skills 格式。

**官方文档**: https://code.claude.com/docs/en/skills

---

### 1.2 是否支持自定义 workflow？

**✅ 是**

Claude Code 通过 **Hooks（钩子）** 系统支持自定义工作流自动化。

**实现方式**:
- Hooks 是在 Claude Code 生命周期特定点自动执行的 shell 命令
- 通过 `settings.json` 配置或交互式 `/hooks` 命令设置

**支持的 Hook 事件**:
| 事件 | 触发时机 | 常见用途 |
|------|---------|---------|
| `PreToolUse` | 工具执行前 | 验证命令、阻止危险操作 |
| `PostToolUse` | 工具执行后 | 自动格式化代码、运行测试 |
| `Notification` | Claude 需要输入时 | 发送桌面通知 |
| `Stop` | 主代理完成任务时 | 创建总结、发送通知 |
| `UserPromptSubmit` | 用户提交提示前 | 添加额外上下文 |
| `SessionStart` | 新会话开始时 | 设置环境、加载上下文 |
| `SessionEnd` | 会话结束时 | 清理、生成报告 |

**配置示例** (自动格式化代码):
```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write(*.py)",
        "hooks": [
          {
            "type": "command",
            "command": "python -m black $file"
          }
        ]
      }
    ]
  }
}
```

**官方文档**: https://code.claude.com/docs/en/hooks-guide

---

### 1.3 是否支持自定义命令/快捷指令？

**✅ 是**

Claude Code 支持多种方式创建自定义命令：

**方式一: Skills（推荐）**
- 创建 `SKILL.md` 文件，其中的 `name` 字段自动成为 `/command-name`
- 支持参数传递和动态上下文注入

**方式二: Slash Commands（传统方式，仍兼容）**
- 在 `.claude/commands/` 或 `~/.claude/commands/` 目录下创建 `.md` 文件
- 文件名即命令名，文件内容为提示模板

**命令文件示例**:
```markdown
# Deploy to Development Server
Run this command to build the project and sync files to the dev environment.

```bash
npm run build
scp -r ./dist user@dev-machine:/path/to/deploy
```
```

**命名空间支持**:
- 子目录中的命令会自动获得命名空间前缀
- 例如 `.claude/commands/deploy/dev.md` 可通过 `/deploy:dev` 调用

**官方文档**: https://code.claude.com/docs/en/slash-commands

---

### 1.4 配置方式是什么？配置文件在哪里？

Claude Code 使用 **分层配置系统**，支持多个作用域：

| 作用域 | 位置 | 影响范围 | 是否共享给团队 |
|--------|------|---------|--------------|
| **Managed** | `managed-settings.json` | 机器上所有用户 | 是（IT部署） |
| **User** | `~/.claude/` 目录 | 所有项目 | 否 |
| **Project** | `.claude/` 目录（仓库内） | 当前仓库所有协作者 | 是（提交到git） |
| **Local** | `.claude/*.local.*` 文件 | 仅当前仓库的个人设置 | 否（gitignored） |

**主要配置文件**:

1. **settings.json** - 核心配置文件
   - 用户级: `~/.claude/settings.json`
   - 项目级: `.claude/settings.json`
   - 本地级: `.claude/settings.local.json`

2. **CLAUDE.md** - 项目上下文说明文件
   - 全局: `~/.claude/CLAUDE.md`
   - 项目级: `./CLAUDE.md`
   - 子目录: 组件特定说明

3. **MCP 配置**:
   - 用户级: `~/.claude/mcp.json`
   - 项目级: `.claude/mcp.json`

**settings.json 示例**:
```json
{
  "model": "claude-sonnet-4-20250514",
  "maxTokens": 4096,
  "permissions": {
    "allowedTools": ["Read", "Write", "Bash(git *)"],
    "deny": ["Read(./.env)", "Read(./.env.*)"]
  },
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write(*.py)",
        "hooks": [{"type": "command", "command": "python -m black $file"}]
      }
    ]
  },
  "attribution": {
    "commits": true,
    "pullRequests": true
  }
}
```

**官方文档**: https://code.claude.com/docs/en/settings

---

## 2. Claude Code Desktop 版

### 2.1 是否支持自定义 skill/prompt 模板？

**✅ 是**

Claude Desktop App 支持 Skills，但功能与 CLI 版本有一些差异：

- 支持通过 **Agent Skills** 功能创建自定义技能
- 可以在 Settings > Capabilities 中启用和上传 Skills
- Skills 需要以 ZIP 文件格式上传

**限制**:
- Desktop 版的 Skills 系统更侧重于对话增强，而非代码编辑自动化
- 需要通过 UI 界面手动上传，不像 CLI 版本可以直接使用文件系统

**官方帮助中心**: https://support.claude.com/en/articles/12512198-how-to-create-custom-skills

---

### 2.2 是否支持自定义 workflow？

**⚠️ 部分支持**

Claude Desktop App 的 **Code 标签页**（即 Claude Code 功能）支持：
- 通过 MCP 服务器连接外部工具
- 使用内置的 hooks 系统（与 CLI 共享配置）

但 Desktop 版的 **Chat 标签页**（普通对话）不支持 hooks 系统。

---

### 2.3 配置方式是什么？

Claude Desktop 使用不同的配置系统：

**MCP 服务器配置**:
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`

**配置示例** (MCP):
```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/username/Desktop"]
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "your-token"
      }
    }
  }
}
```

**与 CLI 共享配置**:
- Claude Desktop 的 Code 标签页可以读取 CLI 的 MCP 配置
- 可以通过 `/mcp` 命令在 CLI 中管理 MCP 服务器

---

### 2.4 Claude Desktop vs Claude Code CLI 对比

| 特性 | Claude Code CLI | Claude Desktop |
|------|----------------|----------------|
| 界面 | 终端/命令行 | 图形界面 |
| 新功能获取 | 最快 | 较稳定，可能滞后 |
| 并行会话 | 需手动管理 worktree | 内置支持 |
| 自定义 Skills | ✅ 完整支持 | ✅ 支持（通过上传） |
| 自定义 Hooks | ✅ 完整支持 | ⚠️ Code标签页支持 |
| MCP 支持 | ✅ 完整支持 | ✅ 完整支持 |
| 插件系统 | ✅ 支持 | ✅ 支持 |
| IDE 集成 | 通过扩展 | 原生界面 |

**官方文档**: https://code.claude.com/docs/en/desktop

---

## 3. 扩展机制

### 3.1 是否有插件系统？

**✅ 是**

Claude Code 有完整的 **插件系统（Plugins）**，目前处于 Public Beta 阶段。

**插件可以包含**:
- **Skills** - 自定义指令和提示模板
- **Subagents** - 专门用途的子代理
- **MCP servers** - 外部工具连接
- **Hooks** - 工作流自动化

**插件安装**:
```bash
# 使用 /plugin 命令交互式安装
/plugin

# 或使用 --plugin-dir 本地测试
claude --plugin-dir /path/to/plugin
```

**插件结构**:
```
my-plugin/
├── .claude-plugin/
│   └── plugin.json    # 插件清单
├── skills/            # 技能定义
├── agents/            # 子代理定义
├── hooks/             # 钩子配置
└── mcp-servers/       # MCP 服务器配置
```

**插件 vs 独立配置对比**:

| 特性 | 独立配置 (.claude/) | 插件 (.claude-plugin/) |
|------|--------------------|----------------------|
| 技能名称 | `/hello` | `/plugin-name:hello` |
| 适用场景 | 个人工作流、快速实验 | 团队共享、社区分发 |
| 版本控制 | 手动管理 | 支持版本化发布 |
| 命名空间 | 无 | 有（防止冲突） |

**官方文档**: 
- https://code.claude.com/docs/en/plugins
- https://www.anthropic.com/news/claude-code-plugins

---

### 3.2 是否支持 MCP (Model Context Protocol)？

**✅ 是 - 完整支持**

Claude Code 对 MCP 有一流的支持。MCP 是由 Anthropic 开发的开源标准，用于 AI 与外部工具和数据的集成。

**MCP 服务器可以**:
- 查询数据库（PostgreSQL, MySQL, SQLite）
- 访问文件系统
- 管理 GitHub 仓库
- 连接生产力工具（Notion, Slack, Jira）
- 调用任意 API

**添加 MCP 服务器的方式**:

```bash
# 方式1: 添加远程 HTTP 服务器
claude mcp add --transport http <name> <url>

# 方式2: 添加远程 SSE 服务器
claude mcp add --transport sse <name> <url>

# 方式3: 添加本地 stdio 服务器
claude mcp add --transport stdio <name> <command> [args...]
```

**MCP 配置作用域**:
- **Local**: `.claude/mcp.json` - 仅当前项目
- **Project**: `.claude/mcp.json` - 团队共享
- **User**: `~/.claude/mcp.json` - 所有项目

**从 Claude Desktop 导入 MCP 配置**:
```bash
claude mcp import-from-claude-desktop
```

**Claude Code 也可作为 MCP 服务器**:
```bash
# 启动 Claude Code 作为 stdio MCP 服务器
claude mcp serve
```

**官方文档**: https://code.claude.com/docs/en/mcp

---

### 3.3 如何扩展其功能？

Claude Code 提供多种扩展方式：

| 扩展方式 | 复杂度 | 适用场景 | 说明 |
|---------|-------|---------|------|
| **Skills** | 低 | 自定义提示模板、指令 | 创建 SKILL.md 文件 |
| **Slash Commands** | 低 | 快捷指令 | 创建 .md 文件（已合并到 Skills） |
| **Hooks** | 中 | 工作流自动化 | 配置事件触发器 |
| **MCP Servers** | 中 | 连接外部工具/数据 | 使用现有或自建 MCP 服务器 |
| **Subagents** | 中 | 专门任务代理 | 创建隔离的执行上下文 |
| **Plugins** | 高 | 打包分享扩展 | 组合多种扩展方式 |

**扩展示例**:

1. **快速创建 Skill**:
```bash
mkdir -p ~/.claude/skills/my-skill
cat > ~/.claude/skills/my-skill/SKILL.md << 'EOF'
---
name: my-skill
description: Does something useful
---

When this skill is invoked, always:
1. Step one
2. Step two
3. Step three
EOF
```

2. **创建 Hook 自动格式化**:
```bash
# 使用交互式配置
claude
/hooks
# 选择 PostToolUse > Write > 添加命令
```

3. **安装 MCP 服务器**:
```bash
# 安装文件系统 MCP 服务器
claude mcp add --transport stdio filesystem npx -y @modelcontextprotocol/server-filesystem /Users/username/Desktop
```

---

## 4. 官方资源链接

### 主要文档
- **Claude Code 官方文档**: https://code.claude.com/docs/en/overview
- **Anthropic 官方文档**: https://docs.anthropic.com/en/docs/claude-code/overview
- **设置文档**: https://code.claude.com/docs/en/settings
- **Skills 文档**: https://code.claude.com/docs/en/skills
- **Hooks 指南**: https://code.claude.com/docs/en/hooks-guide
- **MCP 文档**: https://code.claude.com/docs/en/mcp
- **插件文档**: https://code.claude.com/docs/en/plugins

### 相关资源
- **MCP 协议规范**: https://modelcontextprotocol.io/introduction
- **Claude Desktop 支持**: https://support.claude.com/en/collections/16163169-claude-desktop
- **插件公告**: https://www.anthropic.com/news/claude-code-plugins
- **Agent Skills 标准**: https://agentskills.io

---

## 5. 总结

| 问题 | 答案 |
|------|------|
| Claude Code CLI 支持自定义 skill？ | ✅ **是** - 通过 SKILL.md 文件 |
| Claude Code CLI 支持自定义 workflow？ | ✅ **是** - 通过 Hooks 系统 |
| Claude Code CLI 支持自定义命令？ | ✅ **是** - Skills 自动创建 slash commands |
| Claude Desktop 支持自定义 skill？ | ✅ **是** - 通过 UI 上传 |
| Claude Desktop 支持自定义 workflow？ | ⚠️ **部分** - Code 标签页支持 Hooks |
| 有插件系统？ | ✅ **是** - Public Beta 阶段 |
| 支持 MCP？ | ✅ **是** - 完整支持 |

**核心优势**:
1. 分层配置系统灵活且强大
2. Skills 系统遵循开放标准 (agentskills.io)
3. MCP 支持连接几乎任何外部工具
4. Hooks 提供确定性的工作流自动化
5. 插件系统便于分享和复用

**注意事项**:
- 部分功能（如 nested slash commands）存在已知问题
- CLI 和 Desktop 版本在某些功能上有细微差异
- Skills 的 `allowed-tools` frontmatter 在 Agent SDK 中不被支持
