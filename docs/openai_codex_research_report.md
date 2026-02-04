# OpenAI Codex (CLI & Desktop) 功能与扩展能力调研报告

## 调研日期：2025年

---

## 一、Codex CLI 功能调研

### 1.1 是否支持自定义 skill/prompt 模板？

**答案：是 ✅**

Codex CLI 支持多种方式自定义 prompt 和 skill：

#### A. Agent Skills 系统（推荐方式）
- **官方文档**：https://developers.openai.com/codex/skills/
- **标准规范**：https://agentskills.io
- **启用方式**：`codex --enable skills`

**Skill 结构**：
```
my-skill/
├── SKILL.md          # 必需：指令 + 元数据（YAML front matter）
├── scripts/          # 可选：可执行代码
├── references/       # 可选：文档
├── assets/           # 可选：模板、资源
└── agents/
    └── openai.yaml   # 可选：外观和依赖配置
```

**SKILL.md 示例**：
```yaml
---
name: draft-commit-message
description: Draft a conventional commit message when the user asks for help writing a commit message.
---

Draft a conventional commit message that matches the change summary provided by the user.

Requirements:
- Use the Conventional Commits format: `type(scope): summary`
- Use the imperative mood in the summary
- Keep the summary under 72 characters
```

**Skill 存储位置**（按优先级）：
| 范围 | 位置 | 用途 |
|------|------|------|
| REPO | `$CWD/.codex/skills/` | 当前工作目录 |
| REPO above | `../.codex/skills/` | 嵌套项目 |
| REPO root | Git 仓库根目录 | 整个仓库 |
| USER | `~/.codex/skills/` | 个人所有项目 |
| ADMIN | `/etc/codex/skills/` | 系统范围 |
| SYSTEM | 内置 | Codex 自带 |

**调用方式**：
- 显式调用：`/skills` 命令 或 `$skill-name`
- 隐式调用：Codex 根据任务描述自动选择

#### B. AGENTS.md 自定义指令
- **官方文档**：https://developers.openai.com/codex/guides/agents-md/

**搜索顺序**（按优先级）：
1. `~/.codex/AGENTS.override.md` 或 `~/.codex/AGENTS.md`（全局）
2. 项目根目录到当前工作目录的 `AGENTS.override.md` 或 `AGENTS.md`
3. 合并时后面的文件覆盖前面的

**示例 AGENTS.md**：
```markdown
# Working agreements

- Always run `npm test` after modifying JavaScript files
- Prefer `pnpm` when installing dependencies
- Ask for confirmation before adding new production dependencies
```

#### C. 系统指令覆盖（实验性）
- 配置项：`experimental_instructions_file`（已弃用，新名称 `model_instructions_file`）
- 注意：GPT-5/GPT-5-Codex 模型对此有限制，可能返回 "Instructions are not valid" 错误

---

### 1.2 是否支持自定义 workflow？

**答案：部分支持 ⚠️**

#### 通过 Skills 实现工作流
- Skills 可以封装特定的工作流程
- 使用 `$skill-creator` skill 可以通过自然语言描述创建工作流
- 使用 `$create-plan` skill（实验性）可以创建复杂功能的计划

#### 通过 Slash 命令控制会话
- `/init` - 生成 AGENTS.md 脚手架
- `/review` - 请求 Codex 审查工作树
- `/compact` - 压缩对话历史
- `/diff` - 显示 Git diff
- `/fork` - 分叉当前对话
- `/resume` - 恢复保存的对话

#### 限制
- 没有原生的"工作流引擎"或"自动化脚本"系统
- 不能像某些 AI 工具那样定义多步骤的自动化工作流

---

### 1.3 是否支持自定义命令/快捷指令？

**答案：部分支持 ⚠️**

#### 内置 Slash 命令
Codex CLI 提供以下内置命令（无法自定义新的 slash 命令）：

| 命令 | 用途 |
|------|------|
| `/permissions` | 设置 Codex 的权限 |
| `/apps` | 浏览应用连接器 |
| `/compact` | 压缩对话 |
| `/diff` | 显示 Git diff |
| `/exit` 或 `/quit` | 退出 CLI |
| `/feedback` | 发送日志 |
| `/init` | 生成 AGENTS.md |
| `/logout` | 登出 |
| `/mcp` | 列出 MCP 工具 |
| `/mention` | 附加文件 |
| `/model` | 切换模型 |
| `/ps` | 显示后台终端 |
| `/fork` | 分叉对话 |
| `/resume` | 恢复对话 |
| `/new` | 开始新对话 |
| `/review` | 审查工作树 |
| `/status` | 显示会话配置 |
| `/skills` | 列出可用 skills |

#### 通过 Skills 实现类似功能
虽然不能直接创建新的 slash 命令，但可以通过 Skills 实现类似效果：
- 使用 `$skill-name` 语法调用特定 skill
- 在 AGENTS.md 中定义指令模式

---

### 1.4 配置方式是什么？配置文件在哪里？

**答案：TOML 配置文件 ✅**

#### 配置文件位置
| 级别 | 位置 | 用途 |
|------|------|------|
| 用户级 | `~/.codex/config.toml` | 个人默认配置 |
| 项目级 | `.codex/config.toml` | 项目特定配置（需信任项目） |
| 系统级 | `/etc/codex/config.toml` | 系统范围配置（Unix） |

#### 配置优先级（从高到低）
1. CLI flags 和 `--config` 覆盖
2. Profile 值（`--profile <name>`）
3. 项目配置文件（从根目录到当前目录，最近的优先）
4. 用户配置 `~/.codex/config.toml`
5. 系统配置 `/etc/codex/config.toml`
6. 内置默认值

#### 配置示例
```toml
#:schema https://developers.openai.com/codex/config-schema.json

# 核心模型选择
model = "gpt-5.2-codex"
model_provider = "openai"

# 推理设置
model_reasoning_effort = "medium"  # minimal | low | medium | high | xhigh
model_reasoning_summary = "auto"   # auto | concise | detailed | none

# 审批策略
approval_policy = "on-request"  # untrusted | on-failure | on-request | never

# 沙盒模式
sandbox_mode = "workspace-write"

# MCP 服务器配置
[mcp_servers.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp@latest"]

[mcp_servers.openaiDeveloperDocs]
url = "https://developers.openai.com/mcp"

# 自定义模型提供商
[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
http_headers = { "Authorization" = "Bearer YOUR_API_KEY" }
wire_api = "chat"

# Profile 配置
[profiles.openrouter]
model = "gpt-5"
model_reasoning_effort = "high"
approval_policy = "on-request"
model_provider = "openrouter"

# Skill 配置
[[skills.config]]
path = "/path/to/skill"
enabled = false
```

#### 常用配置项
| 配置项 | 说明 |
|--------|------|
| `model` | 默认模型 |
| `model_provider` | 模型提供商 |
| `approval_policy` | 审批策略 |
| `sandbox_mode` | 沙盒模式 |
| `model_reasoning_effort` | 推理努力程度 |
| `developer_instructions` | 额外的开发者指令 |

---

## 二、Codex 桌面版（Codex Desktop App）功能调研

### 2.1 是否支持自定义 skill/prompt 模板？

**答案：是 ✅**

桌面版支持与 CLI 相同的自定义能力：

#### Agent Skills
- 可用在桌面版中（官方文档确认）
- 通过相同的 `~/.codex/skills/` 和 `.codex/skills/` 目录加载
- 使用 `/skills` 命令或 `$skill-name` 调用

#### AGENTS.md
- 完全支持
- 在 **Settings > Personalization** 中可以编辑自定义指令
- 编辑后会更新 `AGENTS.md` 文件

#### 个性化设置
- **Settings > Personalization** 提供：
  - 选择 **Friendly** 或 **Pragmatic** 人格
  - 添加自定义指令

---

### 2.2 是否支持自定义 workflow？

**答案：部分支持 ⚠️**

桌面版提供以下工作流相关功能：

#### Automations（自动化）
- 官方文档：https://developers.openai.com/codex/app/automations/
- 可以设置自动化任务

#### Git 集成
- **Settings > Git**：
  - 标准化分支命名
  - 选择是否使用 force push
  - 设置生成 commit message 的 prompt
  - 设置生成 PR description 的 prompt

#### 限制
- 没有完整的可视化工作流编辑器
- 工作流主要通过 Skills 和 AGENTS.md 实现

---

### 2.3 配置方式是什么？

**答案：图形界面 + TOML 配置文件 ✅**

#### 图形界面设置
- **Settings**（`Cmd + ,`）提供以下配置类别：
  - **General**：文件打开位置、命令输出显示、多行提示设置
  - **Appearance**：主题、窗口透明度、UI/代码字体
  - **Notifications**：通知设置
  - **Agent configuration**：代理配置（与 CLI/IDE 共享）
  - **Git**：分支命名、force push、commit/PR prompt
  - **Integrations & MCP**：MCP 服务器配置
  - **Personalization**：人格选择、自定义指令
  - **Archived threads**：归档对话管理

#### 配置文件
- 桌面版与 CLI/IDE 扩展共享相同的 `config.toml` 配置
- 高级选项需要直接编辑 `~/.codex/config.toml`

#### MCP 配置
- **Settings > Integrations & MCP**：
  - 启用推荐的 MCP 服务器
  - 添加自定义 MCP 服务器
  - OAuth 认证流程支持
- 配置存储在 `config.toml` 中，与 CLI 共享

---

## 三、扩展机制调研

### 3.1 是否有插件系统？

**答案：没有传统插件系统，但有 Skills 和 MCP ⚠️**

#### Skills（Agent Skills）
- 基于 Markdown 的轻量级扩展机制
- 无需运行独立服务器
- 专为 Codex 生态系统设计
- 易于创建和分享

#### 与 MCP 的对比
| 特性 | Skills | MCP |
|------|--------|-----|
| 协议类型 | Codex 专用 | 通用协议 |
| 通信方式 | 本地文件 | JSON-RPC / HTTP |
| 基础设施 | 无需服务器 | 需要服务器 |
| 生态系统 | OpenAI 专属 | 多平台支持 |
| 实时通信 | 不支持 | 支持 |
| 复杂集成 | 有限 | 灵活 |

---

### 3.2 是否支持 MCP (Model Context Protocol)？

**答案：是 ✅**

#### MCP 支持详情
- **官方文档**：https://developers.openai.com/codex/mcp/
- **支持平台**：CLI、IDE 扩展、桌面版
- **配置共享**：三个平台共享相同的 MCP 配置

#### MCP 服务器类型
| 类型 | 传输方式 | 用途 |
|------|----------|------|
| STDIO | 本地进程 | 本地工具、文件系统访问 |
| Streamable HTTP | HTTP URL | 云服务、第三方集成 |

#### MCP 配置示例
```toml
# STDIO 服务器
[mcp_servers.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp@latest"]

# Streamable HTTP 服务器
[mcp_servers.openaiDeveloperDocs]
url = "https://developers.openai.com/mcp"

# 需要 OAuth 的服务器
[mcp_servers.some_oauth_server]
url = "https://example.com/mcp"
```

#### CLI 命令
```bash
# 添加 MCP 服务器
codex mcp add <server-name> -- <stdio server-command>
codex mcp add context7 -- npx -y @upstash/context7-mcp

# 添加 HTTP 服务器
codex mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp

# 列出 MCP 服务器
codex mcp list

# 登录 OAuth 服务器
codex mcp login <server-name>

# TUI 中查看
codex
/mcp
```

#### 限制
- Codex 原生只支持 **stdio-based MCP servers**（本地运行）
- 远程 HTTP MCP 服务器需要额外工具（如 `codex-mcp-http-bridge`）

---

### 3.3 如何扩展其功能？

**答案：多种方式 ✅**

#### 1. Agent Skills（推荐）
- 创建 `SKILL.md` 文件定义新能力
- 可以包含脚本、模板、资源
- 支持显式和隐式调用

#### 2. MCP 服务器
- 使用社区 MCP 服务器（如 Context7、Playwright、Firecrawl）
- 开发自定义 MCP 服务器
- 连接外部工具和服务

#### 3. 自定义模型提供商
```toml
[model_providers.custom]
name = "Custom Provider"
base_url = "https://api.custom.com/v1"
http_headers = { "Authorization" = "Bearer TOKEN" }
wire_api = "chat"
```

#### 4. AGENTS.md 指令
- 定义项目规范和工作流程
- 设置代码风格指南
- 配置构建和测试命令

#### 5. 配置文件扩展
- Profiles：针对不同场景的配置组合
- Feature flags：启用实验性功能
- Rules：安全规则和约束

---

## 四、官方资源链接

### 文档
- **Codex 总览**：https://developers.openai.com/codex/
- **CLI 文档**：https://developers.openai.com/codex/cli/
- **桌面版文档**：https://developers.openai.com/codex/app/
- **配置基础**：https://developers.openai.com/codex/config-basic/
- **配置参考**：https://developers.openai.com/codex/config-reference/
- **配置示例**：https://developers.openai.com/codex/config-sample/
- **AGENTS.md 指南**：https://developers.openai.com/codex/guides/agents-md/
- **Skills 文档**：https://developers.openai.com/codex/skills/
- **创建 Skill**：https://developers.openai.com/codex/skills/create-skill/
- **MCP 文档**：https://developers.openai.com/codex/mcp/
- **Slash 命令**：https://developers.openai.com/codex/cli/slash-commands/

### GitHub
- **Codex CLI 源码**：https://github.com/openai/codex
- **OpenAI Skills 仓库**：https://github.com/openai/skills
- **Agent Skills 标准**：https://agentskills.io

### 社区资源
- **Codex Settings 示例**：https://github.com/feiskyer/codex-settings
- **codex-mcp-http-bridge**：https://github.com/scottweiss/codex-mcp-bridge

---

## 五、总结

| 功能 | CLI | 桌面版 | 说明 |
|------|-----|--------|------|
| 自定义 Skills | ✅ | ✅ | 通过 SKILL.md 实现 |
| 自定义 Prompts | ✅ | ✅ | 通过 AGENTS.md 实现 |
| 自定义 Workflow | ⚠️ | ⚠️ | 通过 Skills 间接实现 |
| 自定义 Slash 命令 | ❌ | ❌ | 仅支持内置命令 |
| MCP 支持 | ✅ | ✅ | STDIO + Streamable HTTP |
| 插件系统 | ❌ | ❌ | 使用 Skills/MCP 替代 |
| TOML 配置 | ✅ | ✅ | `~/.codex/config.toml` |
| 图形界面配置 | ❌ | ✅ | 桌面版提供 |
| 多 Profile | ✅ | ✅ | 支持 |
| 自定义模型提供商 | ✅ | ✅ | 支持 |

### 关键限制
1. **没有传统插件系统**：不能安装第三方开发的插件
2. **不能创建自定义 Slash 命令**：只能使用内置命令
3. **MCP 限制**：原生只支持本地 stdio MCP，远程 HTTP 需要桥接工具
4. **GPT-5 模型指令限制**：`experimental_instructions_file` 对 GPT-5 模型有限制

### 推荐扩展方式
1. **首选 Skills**：最简单，无需基础设施
2. **次选 MCP**：需要与外部服务集成时使用
3. **配合 AGENTS.md**：定义项目规范和上下文
