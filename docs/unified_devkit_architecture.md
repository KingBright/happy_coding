# 跨平台通用 AI 编程工具开发套件架构设计

## 文档信息
- **版本**: v1.0.0
- **日期**: 2025年
- **状态**: 设计阶段

---

## 1. 可行性分析结论

### 1.1 总体可行性: ✅ **可行，但有条件**

基于对 Google IDX、Claude Code、OpenAI Codex 和 Antigravity 的调研分析，制定统一的跨平台开发套件是**可行的**，但需要处理以下关键差异和限制。

### 1.2 核心结论

| 维度 | 可行性 | 限制条件 |
|------|--------|---------|
| **Skill 标准** | ✅ 高可行 | 3/4 工具支持 SKILL.md，Google IDX 需降级方案 |
| **Workflow 标准** | ⚠️ 中等可行 | 2/4 原生支持，需模拟层 |
| **Command 标准** | ⚠️ 中等可行 | 差异较大，需适配器转换 |
| **配置统一** | ✅ 高可行 | 可通过抽象层统一 |
| **MCP 支持** | ✅ 高可行 | 3/4 工具原生支持 |

### 1.3 关键限制

1. **Google IDX 限制**（最大障碍）
   - 不支持自定义 Skill/Workflow
   - 仅支持有限的 `.idx/airules.md`
   - 解决方案：降级为规则注入模式

2. **OpenAI Codex 限制**
   - 不支持自定义 Slash 命令
   - Workflow 只能通过 Skill 间接实现
   - 解决方案：Skill 内嵌工作流逻辑

3. **Antigravity 限制**
   - 文档较少，API 不稳定
   - 解决方案：保持适配器灵活性

---

## 2. 技术架构设计

### 2.1 架构概览（分层架构）

```
┌─────────────────────────────────────────────────────────────────┐
│                      CLI 工具层 (CLI Layer)                      │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│  │  init   │ │  dev    │ │  build  │ │ install │ │ publish │   │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                    核心引擎层 (Core Engine)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ Config      │  │ Project     │  │ Template Engine         │  │
│  │ Manager     │  │ Scanner     │  │ (Skill/Workflow/Command)│  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ Validator   │  │ Dependency  │  │ Registry Client         │  │
│  │ (Schema)    │  │ Resolver    │  │                         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                   适配器层 (Adapter Layer)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Claude Code │  │ OpenAI Codex│  │ Antigravity │              │
│  │   Adapter   │  │   Adapter   │  │   Adapter   │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Google IDX  │  │   Generic   │  │   Custom    │              │
│  │   Adapter   │  │   Adapter   │  │   Adapter   │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                   目标平台层 (Target Platforms)                  │
│     Claude Code    OpenAI Codex    Antigravity    Google IDX    │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 核心设计原则

1. **统一抽象，适配实现**
   - 顶层使用统一的配置格式和 API
   - 底层通过适配器转换为各平台特定格式

2. **渐进增强，优雅降级**
   - 高级功能在支持的平台使用原生实现
   - 不支持的平台使用模拟/降级方案

3. **声明式配置，命令式扩展**
   - 基础配置使用声明式 YAML/JSON
   - 复杂逻辑使用 JavaScript/TypeScript 扩展

4. **本地优先，云端可选**
   - 核心功能在本地完成
   - 可选的云端注册表和协作功能

---

## 3. 核心模块设计

### 3.1 模块关系图

```
                    ┌─────────────────┐
                    │   CLI Entry     │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
    ┌─────────────────┐ ┌─────────┐ ┌─────────────────┐
    │ Command Parser  │ │ Config  │ │ Plugin Manager  │
    │   (yargs)       │ │ Loader  │ │                 │
    └────────┬────────┘ └────┬────┘ └────────┬────────┘
             │               │               │
             └───────────────┼───────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  Core Engine    │
                    │  Orchestrator   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ Skill Builder │   │Workflow Engine│   │Command Router │
└───────┬───────┘   └───────┬───────┘   └───────┬───────┘
        │                   │                   │
        └───────────────────┼───────────────────┘
                            │
                            ▼
                   ┌─────────────────┐
                   │ Adapter Factory │
                   └────────┬────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ Claude Adapter│   │ Codex Adapter │   │  IDX Adapter  │
└───────────────┘   └───────────────┘   └───────────────┘
```

### 3.2 核心模块详解

#### 3.2.1 Config Manager（配置管理器）

**职责**: 加载、验证、合并配置

```typescript
interface ConfigManager {
  // 加载配置
  loadGlobalConfig(): UnifiedConfig;
  loadProjectConfig(projectPath: string): UnifiedConfig;
  
  // 合并配置（优先级：项目 > 全局 > 默认）
  mergeConfigs(...configs: UnifiedConfig[]): UnifiedConfig;
  
  // 验证配置
  validate(config: unknown): ValidationResult;
  
  // 转换为目标平台格式
  toPlatformConfig(platform: Platform): PlatformSpecificConfig;
}
```

#### 3.2.2 Project Scanner（项目扫描器）

**职责**: 发现项目中的 Skill、Workflow、Command

```typescript
interface ProjectScanner {
  // 扫描项目结构
  scan(projectPath: string): ProjectStructure;
  
  // 发现 Skills
  discoverSkills(path: string): SkillDefinition[];
  
  // 发现 Workflows
  discoverWorkflows(path: string): WorkflowDefinition[];
  
  // 发现 Commands
  discoverCommands(path: string): CommandDefinition[];
  
  // 检测目标平台
  detectPlatforms(projectPath: string): Platform[];
}
```

#### 3.2.3 Adapter Factory（适配器工厂）

**职责**: 创建和管理平台适配器

```typescript
interface AdapterFactory {
  // 获取适配器
  getAdapter(platform: Platform): PlatformAdapter;
  
  // 注册自定义适配器
  registerAdapter(platform: Platform, adapter: PlatformAdapter): void;
  
  // 检查平台支持的功能
  getCapabilities(platform: Platform): PlatformCapabilities;
}

interface PlatformCapabilities {
  skills: boolean;
  workflows: boolean;
  customCommands: boolean;
  hooks: boolean;
  mcp: boolean;
}
```

#### 3.2.4 Skill Builder（技能构建器）

**职责**: 构建和打包 Skill

```typescript
interface SkillBuilder {
  // 从配置构建 Skill
  build(skillConfig: SkillConfig): BuildResult;
  
  // 生成 SKILL.md
  generateSkillMarkdown(skill: SkillDefinition): string;
  
  // 打包 Skill
  pack(skillPath: string, outputPath: string): void;
  
  // 验证 Skill
  validate(skillPath: string): ValidationResult;
}
```

#### 3.2.5 Workflow Engine（工作流引擎）

**职责**: 管理和执行工作流

```typescript
interface WorkflowEngine {
  // 注册工作流
  registerWorkflow(workflow: WorkflowDefinition): void;
  
  // 触发工作流
  trigger(event: WorkflowEvent, context: Context): Promise<void>;
  
  // 获取可用触发器
  getTriggers(): TriggerDefinition[];
  
  // 模拟不支持的平台
  simulateWorkflow(workflow: WorkflowDefinition, platform: Platform): SkillDefinition;
}
```

---

## 4. 统一配置格式设计

### 4.1 项目配置文件: `aicode.config.yaml`

```yaml
# 统一 AI 编程工具配置格式 v1.0
# 支持多平台输出

# 元信息
name: my-ai-project
version: 1.0.0
description: "AI-powered development project"
author: "Your Name"

# 目标平台配置（可选，自动检测）
targets:
  - claude
  - codex
  - antigravity
  - idx

# Skill 配置
skills:
  # 方式1: 内联定义
  - name: code-review
    description: "Review code for quality and best practices"
    triggers:
      - type: command
        name: /review
      - type: file
        pattern: "*.ts"
    instructions: |
      Review the provided code for:
      1. Type safety
      2. Performance issues
      3. Best practices
    
  # 方式2: 引用外部文件
  - name: test-generator
    source: ./skills/test-generator/SKILL.md
    
  # 方式3: 引用目录（自动发现）
  - source: ./skills/documentation/

# Workflow 配置
workflows:
  - name: pre-commit
    description: "Run before each commit"
    triggers:
      - type: hook
        event: pre-commit
    steps:
      - name: lint
        action: run-command
        command: "npm run lint"
      - name: test
        action: run-command
        command: "npm test"
      - name: ai-review
        action: invoke-skill
        skill: code-review
        
  - name: onboarding
    description: "New developer onboarding"
    triggers:
      - type: command
        name: /onboard
    steps:
      - name: setup
        action: run-script
        script: ./scripts/onboarding.js

# Command 配置
commands:
  - name: deploy
    description: "Deploy to production"
    trigger: /deploy
    workflow: deploy-workflow
    
  - name: docs
    description: "Generate documentation"
    trigger: /docs
    skill: documentation

# MCP 服务器配置
mcp:
  servers:
    - name: filesystem
      transport: stdio
      command: "npx"
      args: ["-y", "@modelcontextprotocol/server-filesystem", "."]
      
    - name: github
      transport: streamable-http
      url: "https://api.github.com/mcp"
      headers:
        Authorization: "Bearer ${GITHUB_TOKEN}"

# 规则配置（注入到 AI 上下文）
rules:
  global: |
    Always follow these principles:
    1. Write clean, maintainable code
    2. Include comprehensive tests
    3. Document public APIs
    
  perFile:
    "*.ts": |
      Use strict TypeScript mode
      Prefer interfaces over types
      
    "*.test.ts": |
      Use descriptive test names
      Follow AAA pattern (Arrange-Act-Assert)

# 平台特定覆盖
platformOverrides:
  claude:
    skillsDir: .claude/skills
    settingsFile: .claude/settings.json
    
  codex:
    agentsFile: AGENTS.md
    configFile: .codex/config.toml
    
  idx:
    rulesFile: .idx/airules.md
    devNixFile: .idx/dev.nix
```

### 4.2 Skill 定义格式: `SKILL.md`

```markdown
---
name: code-review
version: 1.0.0
description: "Review code for quality issues"
author: "AI DevKit"
tags: ["quality", "review", "typescript"]
triggers:
  - type: command
    name: /review
  - type: file
    pattern: "*.ts"
---

# Code Review Skill

## Purpose
Review TypeScript code for common issues and best practices.

## When to Use
- Before committing code
- During code review
- When refactoring

## Instructions

### Step 1: Analyze Code Structure
Review the code for:
- Proper TypeScript types
- Function complexity
- Error handling

### Step 2: Check Best Practices
- Use of async/await
- Proper error boundaries
- Memory leak potential

### Step 3: Provide Recommendations
Give specific, actionable feedback with code examples.

## Examples

### Input
```typescript
function process(data: any) {
  return data.map(x => x.value);
}
```

### Output
```typescript
interface DataItem {
  value: string;
}

function process(data: DataItem[]): string[] {
  if (!Array.isArray(data)) {
    throw new Error('Data must be an array');
  }
  return data.map(x => x.value);
}
```

## Output Format
Provide feedback in this structure:
1. Summary
2. Critical Issues
3. Warnings
4. Suggestions
```

### 4.3 Workflow 定义格式: `workflow.yaml`

```yaml
name: ci-pipeline
description: "Continuous integration pipeline"
version: 1.0.0

triggers:
  - type: hook
    event: pre-commit
  - type: schedule
    cron: "0 9 * * 1"  # Weekly on Monday 9am
  - type: command
    name: /ci

variables:
  NODE_VERSION: "18"
  TEST_TIMEOUT: "30000"

steps:
  - id: setup
    name: "Setup Environment"
    action: run-command
    command: "npm ci"
    
  - id: lint
    name: "Run Linter"
    action: run-command
    command: "npm run lint"
    continueOnError: false
    
  - id: typecheck
    name: "Type Check"
    action: run-command
    command: "npm run typecheck"
    
  - id: test
    name: "Run Tests"
    action: run-command
    command: "npm test"
    env:
      CI: "true"
      
  - id: ai-review
    name: "AI Code Review"
    action: invoke-skill
    skill: code-review
    condition: "${{ github.event_name == 'pull_request' }}"
    
  - id: notify
    name: "Send Notification"
    action: call-mcp
    server: slack
    tool: send-message
    args:
      channel: "#dev"
      message: "CI completed for ${{ github.sha }}"

onError:
  action: invoke-skill
  skill: error-handler
  args:
    step: "${{ failedStep }}"
    error: "${{ errorMessage }}"
```

### 4.4 Command 定义格式: `command.yaml`

```yaml
name: deploy
description: "Deploy application to production"
version: 1.0.0
trigger: /deploy

# 参数定义
arguments:
  - name: environment
    description: "Target environment"
    type: choice
    choices: ["staging", "production"]
    default: "staging"
    required: true
    
  - name: version
    description: "Version to deploy"
    type: string
    required: false
    
  - name: skip-tests
    description: "Skip test execution"
    type: boolean
    default: false

# 执行逻辑
execution:
  # 方式1: 引用 workflow
  workflow: deploy-workflow
  
  # 方式2: 内联步骤
  steps:
    - name: "Validate Environment"
      action: run-script
      script: |
        if (args.environment === 'production') {
          // Require approval
        }
        
    - name: "Build Application"
      action: run-command
      command: "npm run build"
      
    - name: "Deploy"
      action: run-command
      command: "npm run deploy:${{ args.environment }}"

# 输出处理
output:
  format: markdown
  template: |
    ## Deployment Result
    
    **Environment**: ${{ args.environment }}
    **Version**: ${{ result.version }}
    **Status**: ${{ result.success ? '✅ Success' : '❌ Failed' }}
    
    ${{ result.output }}
```

---

## 5. CLI 命令设计

### 5.1 命令结构

```
aicode [command] [options]

Commands:
  init [name]          初始化新项目
  dev                  开发模式（监听变化）
  build                构建项目
  install [package]    安装 skill/workflow/command
  publish              发布到注册表
  validate             验证配置
  doctor               诊断环境问题
  config               管理配置
  
Options:
  -t, --target         指定目标平台
  -c, --config         指定配置文件
  -v, --verbose        详细输出
  --dry-run            模拟运行
```

### 5.2 详细命令说明

#### `aicode init [name]`

初始化新的 AI 编程工具项目。

```bash
# 交互式初始化
aicode init my-project

# 使用模板
aicode init my-project --template typescript

# 指定目标平台
aicode init my-project --targets claude,codex

# 跳过交互
aicode init my-project --yes
```

**功能**:
- 创建项目结构
- 生成 `aicode.config.yaml`
- 创建示例 Skill
- 初始化 Git 仓库

**输出结构**:
```
my-project/
├── aicode.config.yaml      # 主配置文件
├── .aicode/                # 工具内部文件
│   └── cache/
├── skills/                 # Skill 目录
│   └── example/
│       └── SKILL.md
├── workflows/              # Workflow 目录
│   └── example.yaml
├── commands/               # Command 目录
│   └── example.yaml
└── rules/                  # 规则文件
    └── global.md
```

#### `aicode dev`

开发模式，监听文件变化并自动同步。

```bash
# 基础开发模式
aicode dev

# 指定目标平台
aicode dev --target claude

# 热重载
aicode dev --hot-reload

# 只监听特定目录
aicode dev --watch skills/
```

**功能**:
- 监听配置文件变化
- 自动同步到目标平台
- 实时验证
- 错误提示

#### `aicode build`

构建项目，生成目标平台特定文件。

```bash
# 构建所有目标
aicode build

# 构建特定目标
aicode build --target claude

# 输出到指定目录
aicode build --output ./dist

# 生产模式（优化）
aicode build --production

# 只构建特定类型
aicode build --type skill
```

**输出示例**:
```
dist/
├── claude/
│   ├── .claude/
│   │   ├── skills/
│   │   └── settings.json
│   └── SKILL.md
├── codex/
│   ├── AGENTS.md
│   └── .codex/
│       └── config.toml
├── idx/
│   └── .idx/
│       ├── airules.md
│       └── dev.nix
└── antigravity/
    └── .gemini/
        └── antigravity/
            └── skills/
```

#### `aicode install [package]`

从注册表安装 Skill/Workflow/Command。

```bash
# 安装 skill
aicode install @aicode/skill-code-review

# 安装特定版本
aicode install @aicode/skill-code-review@1.2.0

# 从 GitHub 安装
aicode install github:user/repo

# 从本地安装
aicode install ./local-skill

# 全局安装
aicode install @aicode/skill-code-review --global

# 安装到项目
aicode install @aicode/skill-code-review --save
```

#### `aicode publish`

发布到注册表。

```bash
# 发布当前项目
aicode publish

# 发布特定类型
aicode publish --type skill

# 指定注册表
aicode publish --registry https://registry.aicode.dev

# 预发布版本
aicode publish --tag beta

# 干运行
aicode publish --dry-run
```

#### `aicode validate`

验证配置正确性。

```bash
# 验证项目配置
aicode validate

# 验证特定文件
aicode validate ./skills/my-skill/SKILL.md

# 验证并修复
aicode validate --fix

# 严格模式
aicode validate --strict
```

#### `aicode doctor`

诊断环境问题。

```bash
# 运行诊断
aicode doctor

# 诊断特定平台
aicode doctor --platform claude

# 输出报告
aicode doctor --output report.json
```

**诊断内容**:
- 目标平台 CLI 安装状态
- 配置文件位置
- 权限检查
- 网络连接
- MCP 服务器状态

---

## 6. 适配器实现策略

### 6.1 适配器架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Adapter Interface                        │
├─────────────────────────────────────────────────────────────┤
│  + initialize(): void                                       │
│  + supports(feature: Feature): boolean                      │
│  + convertSkill(skill: UnifiedSkill): PlatformSkill         │
│  + convertWorkflow(workflow: UnifiedWorkflow): PlatformWF   │
│  + convertCommand(command: UnifiedCommand): PlatformCmd     │
│  + install(path: string): void                              │
│  + uninstall(name: string): void                            │
│  + validate(config: unknown): ValidationResult              │
└─────────────────────────────────────────────────────────────┘
                              △
          ┌───────────────────┼───────────────────┐
          │                   │                   │
          ▼                   ▼                   ▼
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
│  ClaudeAdapter  │   │  CodexAdapter   │   │   IDXAdapter    │
└─────────────────┘   └─────────────────┘   └─────────────────┘
```

### 6.2 各平台适配器策略

#### 6.2.1 Claude Code 适配器

**支持功能**: ✅ 完整支持

```typescript
class ClaudeAdapter implements PlatformAdapter {
  name = 'claude';
  
  capabilities = {
    skills: true,
    workflows: true,
    customCommands: true,
    hooks: true,
    mcp: true
  };
  
  convertSkill(skill: UnifiedSkill): ClaudeSkill {
    return {
      name: skill.name,
      description: skill.description,
      content: this.generateSkillMarkdown(skill),
      location: `.claude/skills/${skill.name}/SKILL.md`
    };
  }
  
  convertWorkflow(workflow: UnifiedWorkflow): ClaudeWorkflow {
    // 转换为 Claude Hooks 配置
    return {
      hooks: workflow.triggers.map(t => ({
        event: this.mapTriggerToHook(t),
        command: workflow.name
      }))
    };
  }
  
  convertCommand(command: UnifiedCommand): ClaudeCommand {
    // Claude 命令通过 Skill 自动创建
    return {
      name: command.trigger,
      skill: command.name
    };
  }
  
  install(projectPath: string): void {
    // 创建 .claude 目录结构
    // 写入 settings.json
    // 复制 skills
  }
}
```

#### 6.2.2 OpenAI Codex 适配器

**支持功能**: ⚠️ 部分支持（无自定义命令）

```typescript
class CodexAdapter implements PlatformAdapter {
  name = 'codex';
  
  capabilities = {
    skills: true,
    workflows: false,  // 通过 Skill 模拟
    customCommands: false,
    hooks: false,
    mcp: true
  };
  
  convertSkill(skill: UnifiedSkill): CodexSkill {
    // Codex 使用 AGENTS.md 格式
    return {
      name: skill.name,
      content: this.generateAgentsMarkdown(skill),
      location: 'AGENTS.md'
    };
  }
  
  convertWorkflow(workflow: UnifiedWorkflow): CodexSkill {
    // 工作流转换为 Skill（降级方案）
    return {
      name: workflow.name,
      description: workflow.description,
      content: this.workflowToSkill(workflow),
      location: `AGENTS.md#${workflow.name}`
    };
  }
  
  convertCommand(command: UnifiedCommand): null {
    // Codex 不支持自定义命令
    // 转换为 Skill 中的说明
    console.warn(`Codex does not support custom commands: ${command.name}`);
    return null;
  }
  
  private workflowToSkill(workflow: UnifiedWorkflow): string {
    // 将工作流步骤转换为 Skill 指令
    return `
## ${workflow.name}

When user wants to run "${workflow.name}" workflow:
${workflow.steps.map(s => `- ${s.name}: ${s.action}`).join('\\n')}

Execute these steps in order...
`;
  }
}
```

#### 6.2.3 Antigravity 适配器

**支持功能**: ✅ 基本支持

```typescript
class AntigravityAdapter implements PlatformAdapter {
  name = 'antigravity';
  
  capabilities = {
    skills: true,
    workflows: true,
    customCommands: true,  // 通过 workflows
    hooks: false,
    mcp: true
  };
  
  convertSkill(skill: UnifiedSkill): AntigravitySkill {
    return {
      name: skill.name,
      description: skill.description,
      content: skill.instructions,
      location: `.gemini/antigravity/skills/${skill.name}/SKILL.md`
    };
  }
  
  convertWorkflow(workflow: UnifiedWorkflow): AntigravityWorkflow {
    return {
      name: workflow.name,
      description: workflow.description,
      triggers: workflow.triggers.map(t => ({
        type: t.type,
        command: t.type === 'command' ? t.name : undefined
      })),
      location: `.agent/workflows/${workflow.name}.yaml`
    };
  }
  
  convertCommand(command: UnifiedCommand): AntigravityWorkflow {
    // 命令通过 workflow 实现
    return this.convertWorkflow({
      name: command.name,
      description: command.description,
      triggers: [{ type: 'command', name: command.trigger }],
      steps: command.execution.steps
    });
  }
}
```

#### 6.2.4 Google IDX 适配器

**支持功能**: ⚠️ 有限支持（降级方案）

```typescript
class IDXAdapter implements PlatformAdapter {
  name = 'idx';
  
  capabilities = {
    skills: false,       // 不支持
    workflows: false,    // 不支持
    customCommands: false, // 有限支持
    hooks: false,
    mcp: false
  };
  
  convertSkill(skill: UnifiedSkill): IDXRules {
    // 降级：将 Skill 转换为规则提示
    return {
      content: `
## ${skill.name}

${skill.description}

When working with code related to this skill:
${skill.instructions}
`,
      location: '.idx/airules.md'
    };
  }
  
  convertWorkflow(workflow: UnifiedWorkflow): IDXDevNix {
    // 降级：使用 onStart/onCreate 钩子
    const hooks = workflow.triggers
      .filter(t => t.type === 'hook')
      .map(t => ({
        event: t.event,
        command: workflow.steps[0]?.command
      }));
      
    return {
      onCreate: hooks.find(h => h.event === 'create')?.command,
      onStart: hooks.find(h => h.event === 'start')?.command,
      location: '.idx/dev.nix'
    };
  }
  
  convertCommand(command: UnifiedCommand): null {
    // IDX 不支持自定义命令
    console.warn(`IDX does not support custom commands: ${command.name}`);
    return null;
  }
  
  install(projectPath: string): void {
    // 创建 .idx 目录
    // 写入 dev.nix
    // 写入 airules.md（合并所有 skill 为规则）
  }
}
```

### 6.3 功能降级矩阵

| 功能 | Claude | Codex | Antigravity | IDX |
|------|--------|-------|-------------|-----|
| Skill | ✅ 原生 | ✅ 原生 | ✅ 原生 | ⚠️ 规则降级 |
| Workflow | ✅ 原生 | ⚠️ Skill模拟 | ✅ 原生 | ⚠️ 钩子降级 |
| Custom Command | ✅ 原生 | ❌ 不支持 | ⚠️ Workflow降级 | ❌ 不支持 |
| Hooks | ✅ 原生 | ❌ 不支持 | ❌ 不支持 | ⚠️ 有限支持 |
| MCP | ✅ 原生 | ✅ 原生 | ✅ 原生 | ❌ 不支持 |

---

## 7. 项目结构规范

### 7.1 标准项目结构

```
my-ai-project/
│
├── aicode.config.yaml          # 主配置文件（必需）
├── package.json                # NPM 配置（可选）
├── README.md                   # 项目文档
│
├── .aicode/                    # 工具内部目录
│   ├── cache/                  # 缓存文件
│   ├── build/                  # 构建输出
│   └── registry/               # 本地注册表缓存
│
├── skills/                     # Skill 目录
│   ├── code-review/
│   │   ├── SKILL.md           # Skill 定义
│   │   ├── examples/          # 示例文件
│   │   └── tests/             # 测试用例
│   └── documentation/
│       ├── SKILL.md
│       └── templates/
│
├── workflows/                  # Workflow 目录
│   ├── ci.yaml
│   ├── pre-commit.yaml
│   └── deploy.yaml
│
├── commands/                   # Command 目录
│   ├── deploy.yaml
│   └── docs.yaml
│
├── rules/                      # AI 规则目录
│   ├── global.md              # 全局规则
│   ├── typescript.md          # TypeScript 规则
│   └── testing.md             # 测试规则
│
├── mcp/                        # MCP 配置
│   ├── servers.yaml           # 服务器配置
│   └── tools/                 # 自定义工具
│
└── scripts/                    # 辅助脚本
    ├── build.js
    └── validate.js
```

### 7.2 Skill 包结构

```
skill-package/
├── SKILL.md                    # 主定义文件
├── package.json                # 包元信息
│
├── examples/                   # 示例
│   ├── input/
│   └── output/
│
├── tests/                      # 测试
│   ├── validate.test.js
│   └── fixtures/
│
└── resources/                  # 资源文件
    ├── templates/
    └── data/
```

---

## 8. 注册表设计

### 8.1 包格式

```json
{
  "name": "@aicode/skill-code-review",
  "version": "1.0.0",
  "description": "AI-powered code review skill",
  "type": "skill",
  "author": "AI DevKit Team",
  "license": "MIT",
  "keywords": ["code-review", "quality", "typescript"],
  "platforms": ["claude", "codex", "antigravity"],
  "main": "SKILL.md",
  "files": [
    "SKILL.md",
    "examples/",
    "resources/"
  ],
  "dependencies": {
    "@aicode/core": "^1.0.0"
  },
  "peerDependencies": {
    "aicode": ">=1.0.0"
  }
}
```

### 8.2 注册表 API

```
GET  /api/v1/packages              # 列出包
GET  /api/v1/packages/:name        # 获取包信息
GET  /api/v1/packages/:name/:ver   # 获取特定版本
POST /api/v1/packages              # 发布包
GET  /api/v1/search?q=keyword      # 搜索包
GET  /api/v1/platforms             # 支持的平台
```

---

## 9. 实现路线图

### Phase 1: MVP（4-6 周）
- [ ] 核心配置解析器
- [ ] Claude Code 适配器
- [ ] 基础 CLI 命令（init, build, install）
- [ ] Skill 格式标准化

### Phase 2: 扩展支持（4-6 周）
- [ ] OpenAI Codex 适配器
- [ ] Workflow 引擎
- [ ] 完整 CLI 命令集
- [ ] 本地注册表

### Phase 3: 完整生态（6-8 周）
- [ ] Antigravity 适配器
- [ ] Google IDX 适配器（降级）
- [ ] 云端注册表
- [ ] VS Code 扩展
- [ ] 文档和示例

### Phase 4: 高级功能（持续）
- [ ] MCP 集成
- [ ] 协作功能
- [ ] 性能优化
- [ ] 社区插件

---

## 10. 风险与缓解

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|---------|
| 平台 API 变化 | 高 | 高 | 适配器抽象层，快速响应 |
| Google IDX 限制 | 确定 | 中 | 降级方案，规则注入 |
| 用户采用率低 | 中 | 高 | 优秀文档，社区建设 |
| 性能问题 | 中 | 中 | 缓存，增量构建 |
| 安全漏洞 | 低 | 高 | 代码审查，依赖扫描 |

---

## 附录

### A. 配置文件 JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://aicode.dev/schema/config-v1.json",
  "title": "AI Code Config",
  "type": "object",
  "required": ["name", "version"],
  "properties": {
    "name": { "type": "string" },
    "version": { "type": "string" },
    "description": { "type": "string" },
    "targets": {
      "type": "array",
      "items": {
        "enum": ["claude", "codex", "antigravity", "idx"]
      }
    },
    "skills": {
      "type": "array",
      "items": {
        "$ref": "#/definitions/skill"
      }
    },
    "workflows": {
      "type": "array",
      "items": {
        "$ref": "#/definitions/workflow"
      }
    },
    "commands": {
      "type": "array",
      "items": {
        "$ref": "#/definitions/command"
      }
    },
    "mcp": {
      "$ref": "#/definitions/mcp"
    },
    "rules": {
      "$ref": "#/definitions/rules"
    }
  }
}
```

### B. 术语表

- **Skill**: AI 可以执行的任务定义
- **Workflow**: 自动化步骤序列
- **Command**: 用户触发的操作
- **Adapter**: 平台适配器
- **MCP**: Model Context Protocol

---

*文档结束*
