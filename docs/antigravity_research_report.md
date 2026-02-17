# Antigravity AI 编程工具调研报告

## 调研日期：2026年1月

---

## 1. 工具识别

### 1.1 Antigravity 是否是一个已知的 AI IDE？

**是的，Antigravity 是 Google 推出的 AI 编程工具。**

Google Antigravity 是 Google 于 2025 年 11 月 18 日发布的 AI-first 集成开发环境(IDE)，基于 VS Code 代码库构建，但集成了丰富的 AI 增强功能。它是 Google 对 Cursor、Windsurf 等 AI 编程工具的回应。

### 1.2 官方信息

- **官方网站**: https://antigravity.google/
- **发布日期**: 2025年11月18日
- **当前状态**: 免费公开预览版
- **支持平台**: macOS、Windows、Linux
- **核心 AI 模型**: Gemini 3 Pro（默认），同时支持 Claude Sonnet 4.5 和 GPT-OSS

### 1.3 核心定位

Antigravity 定位为 **"Agent-First"（代理优先）IDE**，与传统 AI 编程工具的主要区别在于：
- 传统工具（如 GitHub Copilot）：AI 作为助手，提供代码建议
- Antigravity：AI 作为代理，可以自主规划、执行和验证任务

---

## 2. Antigravity 功能与自定义能力

### 2.1 是否支持自定义 Skill/Prompt 模板？

**是的，支持。**

#### Skills 系统
- **存储位置**: `~/.gemini/antigravity/skills/`
- **文件格式**: 每个 skill 是一个文件夹，包含 `SKILL.md` 文件
- **Skill 结构**:
  ```
  skills/
  ├── python-clean-code/
  │   ├── SKILL.md
  │   └── scripts/
  │       └── lint.py
  └── my-custom-skill/
      └── SKILL.md
  ```
- **渐进式披露**: Agent 首先只看到 skill 名称和描述，当请求匹配描述时才加载完整指令

#### 跨平台兼容性
Antigravity 的 skill 系统基于开放标准，可与其他 AI 工具兼容：
- Claude Code (`~/.claude/`)
- Codex CLI (`~/.codex/`)
- Gemini CLI (`~/.gemini/`)
- Qwen Code (`~/.qwen/`)
- Windsurf (`~/.codeium/windsurf/`)
- Trae (`~/.trae/`)

### 2.2 是否支持自定义 Workflow？

**是的，支持。**

#### Workflows 系统
- **存储位置**: `.agent/workflows/` 目录
- **文件格式**: Markdown 文件
- **调用方式**: 使用 `/[workflow-name]` 斜杠命令
- **功能**: 定义一系列步骤来指导 Agent 执行重复性任务

#### Workflow 示例
```markdown
# 工作流名称

## Step 1
执行第一个任务...

## Step 2
执行第二个任务...
```

#### 预置 Workflow 类型
- `/address-pr-comments` - 处理 PR 评论
- `/git-workflows` - Git 工作流
- `/security-scan` - 安全扫描

### 2.3 是否支持自定义命令？

**部分支持。**

- **Turbo Mode**: 使用 `// turbo` 注释让 Agent 自动执行命令而无需确认
- **斜杠命令**: 通过 workflows 创建自定义斜杠命令
- **MCP 集成**: 支持 Model Context Protocol，可连接外部工具和服务

### 2.4 配置系统

#### 规则层次结构 (Hierarchy of Rules)

1. **System Rules（系统规则）** - 不可修改
   - 来自 Google DeepMind 的核心指令
   - 定义 Agent 的基本行为（如"编码前总是先规划"）

2. **User Rules（用户规则）** - 可自定义
   - **Global Rules**: 适用于所有项目
   - **Workspace Rules**: 仅适用于当前项目
   - 设置路径: Agent Manager → 右上角 "..." → Customizations → Rules

#### 可自定义的内容

| 类别 | 说明 | 示例 |
|------|------|------|
| Tech Stack & Libraries | 指定技术栈 | "使用 Next.js 14 + Tailwind CSS" |
| Coding Style | 编码风格 | "优先使用函数组件" |
| Behavior | 交互行为 | "发现潜在 bug 时先询问" |
| Design Philosophy | 设计理念 | 可覆盖默认的"Premium"设计风格 |

#### Artifacts 机制

Agent 输出不仅包括代码，还包括：
- `task.md` - 动态任务清单
- `implementation_plan.md` - 技术蓝图
- `walkthrough.md` - 工作证明

用户可以对这些 Artifacts 添加评论，Agent 会考虑这些反馈。

### 2.5 开发模式选择

安装时可选择四种模式：

| 模式 | 说明 |
|------|------|
| **Secure Mode** | 增强安全控制，限制 Agent 访问外部资源 |
| **Review-driven development** | 执行关键任务前询问权限（推荐） |
| **Agent-driven development** | Agent 自动执行无需确认 |
| **Custom** | 自定义策略配置 |

### 2.6 双视图界面

| 视图 | 功能 |
|------|------|
| **Editor View（编辑器视图）** | 类似 VS Code 的传统 IDE 体验，Agent 在侧边栏 |
| **Manager View（管理器视图）** | 任务控制中心，可同时管理多个 Agent |

### 2.7 Agent 模式

| 模式 | 说明 |
|------|------|
| **Plan Mode（规划模式）** | 先生成详细计划再执行，适合复杂任务 |
| **Fast Mode（快速模式）** | 立即执行，适合快速修复 |

---

## 3. 与其他 AI 编程工具对比

### 3.1 Cursor

| 特性 | Cursor | Antigravity |
|------|--------|-------------|
| 核心定位 | Editor-First | Agent-First |
| 自定义 Rules | ✅ `.cursorrules` | ✅ Global/Workspace Rules |
| 自定义 Workflows | ⚠️ 有限支持 | ✅ 完整支持 |
| 自定义 Skills | ⚠️ 有限支持 | ✅ 完整支持 |
| 多 Agent 支持 | ⚠️ 有限 | ✅ Manager View |
| 浏览器集成 | ❌ 需插件 | ✅ 原生支持 |
| 定价 | $20-200/月 | 当前免费 |

### 3.2 Windsurf

| 特性 | Windsurf | Antigravity |
|------|----------|-------------|
| 核心定位 | Agent-First | Agent-First |
| 自定义 Rules | ✅ `.windsurfrules` + `global_rules.md` | ✅ Global/Workspace Rules |
| 自定义 Workflows | ✅ `.windsurf/workflows/` | ✅ `.agent/workflows/` |
| 自定义 Skills | ✅ `SKILL.md` 格式 | ✅ `SKILL.md` 格式 |
| MCP 支持 | ✅ | ✅ |
| 浏览器集成 | ❌ | ✅ |
| 定价 | 免费 + $25/月 Pro | 当前免费 |

### 3.3 GitHub Copilot Chat/Workspace

| 特性 | GitHub Copilot | Antigravity |
|------|----------------|-------------|
| 自定义 Instructions | ✅ `.github/copilot-instructions.md` | ✅ Global/Workspace Rules |
| Agent Skills | ✅ `.github/skills/` (预览) | ✅ `~/.gemini/antigravity/skills/` |
| Custom Agents | ✅ (VS Code) | ❌ |
| Prompt Files | ✅ | ✅ Workflows |
| MCP 支持 | ✅ | ✅ |
| Agent 自主性 | 低 | 高 |

---

## 4. 关键发现与建议

### 4.1 Antigravity 的优势

1. **真正的 Agent-First 架构** - 不仅是代码建议，而是自主任务执行
2. **多 Agent 并行** - Manager View 可同时管理多个 Agent
3. **浏览器原生集成** - Agent 可以直接测试和验证 Web 应用
4. **开放标准** - Skills 和 Workflows 使用开放格式，可跨工具使用
5. **当前免费** - 公开预览期间完全免费

### 4.2 Antigravity 的局限

1. **稳定性问题** - 早期用户报告 Agent 经常因错误终止
2. **生态系统不成熟** - 相比 Cursor 和 VS Code，扩展和插件较少
3. **登录限制** - 必须使用 Google 账号
4. **代码隐私** - 代码发送到 Google 服务器处理
5. **闭源** - 不像 VS Code 那样开源

### 4.3 配置建议

对于希望使用 Antigravity 的团队：

1. **创建项目规则文件** - 在 `.agent/rules/` 中定义项目特定的编码标准
2. **建立技能库** - 为常见任务创建可复用的 skills
3. **设计工作流** - 为重复性任务创建 workflows
4. **使用 MCP 扩展** - 连接数据库、云服务等外部工具
5. **选择合适的开发模式** - 根据项目需求选择 Review-driven 或 Agent-driven

### 4.4 替代方案建议

| 使用场景 | 推荐工具 |
|----------|----------|
| 追求 Agent 自主性 | Antigravity |
| 需要稳定成熟 | Cursor |
| 已有 VS Code 生态 | Windsurf 或 GitHub Copilot |
| 数据隐私优先 | Void（开源） |
| 企业级需求 | JetBrains AI |

---

## 5. 参考链接

### Antigravity 官方资源
- 官网: https://antigravity.google/
- 文档: https://antigravity.google/docs/
- 下载: https://antigravity.google/download

### 社区资源
- Antigravity Rules 库: https://antigravity.codes/blog/user-rules
- Antigravity Workflows: https://antigravity.codes/blog/workflows
- Antigravity MCP 指南: https://antigravity.codes/blog/mcp

### 对比评测
- Cursor vs Antigravity: https://altalks.com/tech/cursor-vs-antigravity-vs-vs-code-which-code-editor-is-best-in-2026/
- 掘金对比文章: https://juejin.cn/post/7533512134003392562

### GitHub 项目
- Antigravity Setup: https://github.com/irahardianto/antigravity-setup
- Clean Code Skills: https://github.com/ertugrul-dmr/clean-code-skills
- Antigravity Panel 扩展: https://github.com/n2ns/antigravity-panel

---

## 6. 总结

**Antigravity 是一个真实存在的、由 Google 推出的 AI-first IDE**，它支持：

| 功能 | 支持情况 |
|------|----------|
| 自定义 Skill/Prompt 模板 | ✅ 是 |
| 自定义 Workflow | ✅ 是 |
| 自定义命令 | ⚠️ 部分支持 |
| 配置系统 | ✅ 多层次规则系统 |
| MCP 扩展 | ✅ 是 |

Antigravity 代表了 AI 编程工具向 "Agent-First" 方向的演进，虽然目前还存在稳定性等问题，但其开放的标准和强大的自定义能力使其成为值得关注的工具。

---

*报告生成时间: 2026年1月*
*数据来源: 网络搜索、官方文档、社区资源*
