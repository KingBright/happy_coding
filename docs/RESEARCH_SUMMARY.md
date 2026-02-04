# AI 编程工具调研报告

## 调研日期
2026-02-04

## 调研目标
1. Google IDX 的 LLM Provider 可配置性
2. 跨平台 Skill/Workflow/Command 统一开发的可行性
3. 开发一套通用开发套件

---

## 1. Google IDX (Firebase Studio)

### LLM Provider 可配置性

| 问题 | 答案 |
|------|------|
| 是否可以修改/切换 LLM 提供商？ | **❌ 否** |
| Gemini 是唯一的吗？ | **✅ 是，仅支持 Gemini 系列模型** |
| 可以接入 OpenAI/Anthropic API 吗？ | **❌ 否** |
| 支持的模型 | Gemini 2.5 Pro、Gemini 2.5 Flash、Gemini 2.0 Flash 等 |

**配置方式：**
- 在 Gemini Chat 面板中手动选择模型
- 通过 `.gemini/settings.json` 配置
- 通过 `GEMINI_MODEL` 环境变量指定

### 扩展系统

- **Nix 配置**: `.idx/dev.nix` 支持完整的开发环境配置
- **VS Code 扩展**: 支持 Open VSX Registry 的扩展
- **服务**: 支持 Docker、PostgreSQL 等

### 自定义能力

| 功能 | 支持 | 配置方式 |
|------|------|----------|
| 自定义启动命令 | ✅ | `dev.nix` 中的 `onCreate`/`onStart` |
| 自定义 AI 规则 | ⚠️ 有限 | `.idx/airules.md` 或 `GEMINI.md` |
| 自定义 AI 技能 | ❌ | 不支持 |
| 自定义工作流 | ❌ | 不支持 |

---

## 2. Claude Code (CLI + Desktop)

### 自定义能力

| 功能 | 支持情况 | 实现方式 |
|------|---------|---------|
| **自定义 Skill** | ✅ **是** | `SKILL.md` 文件，支持 `~/.claude/skills/` (个人) 和 `.claude/skills/` (项目) |
| **自定义 Workflow** | ✅ **是** | **Hooks 系统** - 支持 `PreToolUse`, `PostToolUse`, `Notification`, `SessionStart` 等事件 |
| **自定义命令** | ✅ **是** | Skills 自动创建 `/command-name`，兼容 `.claude/commands/` 目录 |
| **配置文件** | ✅ | `~/.claude/settings.json` (用户), `.claude/settings.json` (项目) |

### 扩展机制

| 机制 | 支持情况 | 说明 |
|------|---------|------|
| **插件系统** | ✅ **是** (Public Beta) | `/plugin` 命令安装，支持 skills + agents + hooks + MCP |
| **MCP 支持** | ✅ **是** | 一流支持，可连接数据库、GitHub、API 等外部工具 |
| **扩展方式** | 多种 | Skills → Hooks → MCP → Subagents → Plugins |

### 官方文档
- 主文档: https://code.claude.com/docs/en/overview
- Skills: https://code.claude.com/docs/en/skills
- Hooks: https://code.claude.com/docs/en/hooks-guide
- MCP: https://code.claude.com/docs/en/mcp
- Plugins: https://code.claude.com/docs/en/plugins

---

## 3. OpenAI Codex (CLI + Desktop)

### 自定义能力

| 功能 | 支持情况 | 说明 |
|------|----------|------|
| **自定义 Skills** | ✅ **是** | 通过 `SKILL.md` 文件，支持显式(`$skill`)和隐式调用 |
| **自定义 Prompts** | ✅ **是** | 通过 `AGENTS.md` 文件，支持全局和项目级配置 |
| **自定义 Workflow** | ⚠️ **部分** | 通过 Skills 间接实现，无原生工作流引擎 |
| **自定义 Slash 命令** | ❌ **否** | 仅支持内置命令（如 `/model`, `/review` 等） |
| **配置方式** | ✅ **TOML** | `~/.codex/config.toml` |

### 扩展机制

| 功能 | 支持情况 | 说明 |
|------|----------|------|
| **插件系统** | ❌ **否** | 无传统插件系统 |
| **MCP 支持** | ✅ **是** | 支持 STDIO 和 Streamable HTTP 两种类型 |
| **功能扩展方式** | ✅ **多种** | Skills + MCP + AGENTS.md + 自定义 Provider |

### 官方文档
- Codex CLI: https://developers.openai.com/codex/cli/
- 桌面版: https://developers.openai.com/codex/app/
- 配置文档: https://developers.openai.com/codex/config-basic/
- Skills 文档: https://developers.openai.com/codex/skills/
- MCP 文档: https://developers.openai.com/codex/mcp/
- AGENTS.md: https://developers.openai.com/codex/guides/agents-md/

---

## 4. Antigravity (Google)

### 自定义能力

| 功能 | 支持情况 | 说明 |
|------|----------|------|
| **自定义 Skill** | ✅ 支持 | `~/.gemini/antigravity/skills/` 目录，SKILL.md 格式 |
| **自定义 Workflow** | ✅ 支持 | `.agent/workflows/` 目录，斜杠命令调用 |
| **自定义命令** | ⚠️ 部分支持 | Turbo Mode + MCP 集成 |
| **配置系统** | ✅ 多层次 | System Rules → Global Rules → Workspace Rules |

### 与其他工具对比

| 工具 | Skill | Workflow | 多Agent | 浏览器集成 |
|------|-------|----------|---------|------------|
| **Antigravity** | ✅ | ✅ | ✅ | ✅ 原生 |
| **Cursor** | ⚠️ | ⚠️ | ❌ | ❌ 需插件 |
| **Windsurf** | ✅ | ✅ | ⚠️ | ❌ |
| **GitHub Copilot** | ✅(预览) | ✅ | ❌ | ❌ |

### 官方链接
- 官网: https://antigravity.google/
- 文档: https://antigravity.google/docs/rules-workflows
- 社区规则库: https://antigravity.codes/blog/user-rules

---

## 5. 统一开发可行性分析

### 可行性结论

| 维度 | 可行性 | 关键限制 |
|------|--------|---------|
| **Skill 标准** | ✅ 高可行 | 3/4 工具支持 SKILL.md，Google IDX 需降级为规则注入 |
| **Workflow 标准** | ⚠️ 中等可行 | 2/4 原生支持，Codex/IDX 需模拟/降级 |
| **Command 标准** | ⚠️ 中等可行 | 差异较大，Codex 不支持自定义命令 |
| **配置统一** | ✅ 高可行 | 可通过抽象层统一 |
| **MCP 支持** | ✅ 高可行 | 3/4 工具原生支持 |

### 功能降级矩阵

| 功能 | Claude | Codex | Antigravity | IDX |
|------|--------|-------|-------------|-----|
| Skill | ✅ 原生 | ✅ 原生 | ✅ 原生 | ⚠️ 规则降级 |
| Workflow | ✅ 原生 | ⚠️ Skill模拟 | ✅ 原生 | ⚠️ 钩子降级 |
| Custom Command | ✅ 原生 | ❌ 不支持 | ⚠️ Workflow降级 | ❌ 不支持 |
| Hooks | ✅ 原生 | ❌ 不支持 | ❌ 不支持 | ⚠️ 有限支持 |
| MCP | ✅ 原生 | ✅ 原生 | ✅ 原生 | ❌ 不支持 |

---

## 6. 通用开发套件 (AI Code DevKit)

### 已实现的解决方案

我们开发了一套完整的通用开发套件 **AI Code DevKit**，支持：

#### 核心功能
- ✅ 统一的配置格式 (`aicode.config.yaml`)
- ✅ 跨平台构建系统
- ✅ 文件监听和自动重建
- ✅ 配置验证和诊断
- ✅ 一键安装到本地环境

#### CLI 命令
- `aicode init` - 初始化新项目
- `aicode build` - 构建所有目标平台
- `aicode dev` - 开发模式（监听变化）
- `aicode install` - 安装到本地环境
- `aicode validate` - 验证配置
- `aicode doctor` - 诊断环境

#### 适配器支持
- ✅ Claude Adapter (完整支持)
- ✅ Codex Adapter (完整支持，workflow 模拟)
- ✅ Antigravity Adapter (完整支持)
- ✅ IDX Adapter (降级支持)

#### 项目结构
```
aicode-devkit/
├── packages/
│   ├── cli/              # CLI 入口
│   ├── core/             # 核心引擎
│   └── adapters/         # 平台适配器
│       ├── claude/
│       ├── codex/
│       ├── antigravity/
│       └── idx/
├── templates/            # 项目模板
└── examples/             # 示例项目
```

---

## 7. 结论与建议

### Google IDX 的 LLM Provider 结论

**❌ Google IDX 不支持修改 LLM Provider**，仅支持 Gemini 系列模型。这是平台的硬性限制，无法绕过。

### 统一开发套件结论

**✅ 可以制定统一的 Skill/Workflow/Command 标准**，但需要注意：

1. **Skill**: 高可行性，3/4 平台原生支持 SKILL.md 格式
2. **Workflow**: 中等可行性，Claude 和 Antigravity 原生支持，Codex 需要模拟，IDX 降级为规则
3. **Command**: 中等可行性，Claude 原生支持，Codex 不支持需要降级

### 推荐方案

1. **使用 AI Code DevKit** 进行跨平台开发
2. **主要目标平台**: Claude Code 和 Antigravity（完整功能）
3. **次要目标平台**: Codex（功能降级但可用）
4. **降级平台**: IDX（仅作为 AI 规则参考）

---

## 参考文档

- 完整架构设计: `/mnt/okcomputer/output/unified_devkit_architecture.md`
- 开发套件代码: `/mnt/okcomputer/output/aicode-devkit/`
- 示例项目: `/mnt/okcomputer/output/aicode-devkit/examples/`