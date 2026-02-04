# Google IDX (Firebase Studio) 调研报告

## 概述

**Google IDX** 现已更名为 **Firebase Studio**，是 Google 推出的基于浏览器的云端 AI 驱动开发环境。它基于 Code OSS (VS Code 的开源基础) 构建，运行在 Google Cloud 基础设施上。

**官方文档**: https://firebase.google.com/docs/studio

---

## 1. LLM Provider 可配置性

### 1.1 是否可以修改/切换 LLM 服务提供商？

**答案：否** ❌

Firebase Studio 的 AI 功能**仅支持 Google Gemini 模型**，无法切换到其他 LLM 提供商（如 OpenAI、Anthropic 等）。

### 1.2 支持哪些模型？Gemini 是唯一的吗？

**答案：是的，Gemini 是唯一支持的模型** 

根据官方文档和最新信息：
- Firebase Studio 内置模型为 **Gemini 2.5 Pro**（截至 2025 年 9 月）
- 用户可以在不同 Gemini 模型之间选择，如：
  - Gemini 2.5 Pro
  - Gemini 2.5 Flash
  - Gemini 2.0 Flash
  - 其他 Gemini 系列模型

### 1.3 是否可以接入自定义的 OpenAI API、Anthropic API 或其他？

**答案：否** ❌

- 无法直接配置 OpenAI API、Anthropic API 或其他第三方 LLM 提供商
- 内置 AI 功能（代码补全、AI 聊天、代码生成）**仅使用 Gemini 模型**
- 如果需要在项目中使用其他 LLM，可以通过代码调用相应的 API，但这与 IDE 内置 AI 功能无关

### 1.4 配置方式

**模型选择方式**：
1. 在 Gemini Chat 面板中手动选择模型
2. 通过 `.gemini/settings.json` 配置文件调整设置
3. 通过环境变量 `GEMINI_MODEL` 指定模型（用于 Gemini CLI）

**配置示例**（`.gemini/settings.json`）：
```json
{
  "theme": "dark",
  "enableUsageStatistics": false
}
```

**环境变量配置**（`.env`）：
```
GEMINI_API_KEY=your_api_key
GEMINI_MODEL=gemini-2.5-pro
GOOGLE_CLOUD_PROJECT=your_project_id
```

---

## 2. 扩展系统

### 2.1 .idx/dev.nix 配置文件详细说明

**文件位置**: `.idx/dev.nix`

**核心配置选项**：

| 配置项 | 说明 | 类型 |
|--------|------|------|
| `channel` | nixpkgs 渠道版本 | string |
| `packages` | 要安装的软件包列表 | list |
| `env` | 环境变量设置 | object |
| `idx.extensions` | VS Code 扩展程序列表 | list |
| `idx.previews` | 预览配置 | object |
| `idx.workspace.onCreate` | 创建工作区时执行的命令 | object |
| `idx.workspace.onStart` | 启动工作区时执行的命令 | object |
| `services` | 启用的服务（Docker、Postgres 等）| object |
| `imports` | 导入其他 nix 文件 | list |

**完整配置示例**：

```nix
{ pkgs, ... }: {
  # nixpkgs 渠道版本
  channel = "stable-24.05"; # 或 "unstable"

  # 安装的软件包
  packages = [
    pkgs.nodejs_20
    pkgs.python3
    pkgs.go
  ];

  # 环境变量
  env = {
    HELLO = "world";
    PATH = [ "/some/path/bin" ];
  };

  # IDE 扩展（从 Open VSX Registry 获取）
  idx.extensions = [
    "angular.ng-template"
    "bradlc.vscode-tailwindcss"
  ];

  # 预览配置
  idx.previews = {
    enable = true;
    previews = {
      web = {
        command = [ "npm" "run" "start" "--" "--port" "$PORT" "--host" "0.0.0.0" ];
        manager = "web";
        cwd = "app/client"; # 可选：指定工作目录
      };
    };
  };

  # 工作区生命周期钩子
  idx.workspace = {
    onCreate = {
      npm-install = "npm install --no-audit --prefer-offline";
    };
    onStart = {
      npm-watch = "npm run watch";
      # 默认打开的文件
      default.openFiles = [ "src/index.ts" ];
    };
  };

  # 启用服务
  services.docker.enable = true;
  services.postgres = {
    enable = true;
    extensions = [ "pgvector" ];
  };
  services.redis.enable = true;
  services.mysql.enable = true;
}
```

### 2.2 支持的服务（Services）

| 服务 | 配置选项 | 说明 |
|------|----------|------|
| Docker | `services.docker.enable` | 启用 Rootless Docker |
| PostgreSQL | `services.postgres.enable`, `extensions`, `package` | 支持多种扩展（pgvector, postgis 等） |
| MySQL | `services.mysql.enable`, `package` | 可指定 MySQL 版本 |
| Redis | `services.redis.enable` | 内存数据存储 |
| Pub/Sub | `services.pubsub.enable`, `port`, `project-id` | Google Pub/Sub 模拟器 |

### 2.3 是否支持插件/扩展系统？

**答案：是** ✅

- 支持 **VS Code 扩展**（通过 Open VSX Registry）
- 不支持 Microsoft VS Code Marketplace 直接安装
- 可以通过 `.vsix` 文件手动安装扩展

**扩展配置方式**：
```nix
idx.extensions = [
  "publisher.extension-name"
];
```

**查找扩展**: https://open-vsx.org/

### 2.4 是否有类似 VS Code 的扩展市场？

**答案：部分支持** ⚠️

- 使用 **Open VSX Registry** 而非 Microsoft Marketplace
- 部分流行的 VS Code 扩展可能不可用或版本较旧
- 可以通过下载 `.vsix` 文件手动安装 Microsoft Marketplace 的扩展（可能有许可限制）

---

## 3. 自定义 Skill/Workflow/Command

### 3.1 是否支持自定义命令？

**答案：是** ✅

**通过 dev.nix 配置自定义命令**：

```nix
idx.workspace = {
  onCreate = {
    # 创建工作区时执行
    setup = "npm install && npm run build";
  };
  onStart = {
    # 启动工作区时执行
    start-dev = "npm run dev";
    start-test = "npm run test:watch";
  };
};
```

### 3.2 是否支持自定义 AI 技能/工作流？

**答案：有限支持** ⚠️

**可用的 AI 自定义选项**：

1. **AI 规则文件**（`.idx/airules.md` 或 `GEMINI.md`）
   - 为 Gemini 提供项目特定的指令
   - 可包含编码风格指南、项目背景信息等

2. **代码库索引配置**
   - 通过 `.aiexclude` 文件排除敏感代码不被索引
   - 在 `.vscode/settings.json` 中启用/禁用代码库索引

3. **Slash 命令**
   - 内置命令如 `/fixError`, `/explain`, `/addComments`
   - 无法创建完全自定义的 AI 技能

**AI 规则文件示例**（`GEMINI.md`）：
```markdown
# Project Instructions

## Coding Style
- Use TypeScript for all new code
- Follow functional programming patterns
- Use async/await instead of callbacks

## Project Structure
- /src - Source code
- /tests - Test files
- /docs - Documentation
```

### 3.3 配置方式总结

| 功能 | 配置方式 | 支持程度 |
|------|----------|----------|
| 自定义启动命令 | `dev.nix` 中的 `onCreate`/`onStart` | ✅ 完全支持 |
| 环境变量 | `dev.nix` 中的 `env` | ✅ 完全支持 |
| 软件包安装 | `dev.nix` 中的 `packages` | ✅ 完全支持 |
| IDE 扩展 | `dev.nix` 中的 `idx.extensions` | ✅ 完全支持 |
| AI 规则 | `.idx/airules.md` 或 `GEMINI.md` | ⚠️ 有限支持 |
| 自定义 AI 命令 | 不支持 | ❌ 不支持 |
| 第三方 LLM | 不支持 | ❌ 不支持 |

---

## 4. 限制和注意事项

### 4.1 LLM 相关限制

1. **无法更换 LLM 提供商** - 只能使用 Google Gemini
2. **无法接入 OpenAI/Anthropic API** 作为内置 AI 功能
3. **模型选择受限** - 只能从 Gemini 系列中选择
4. **需要 Google 账号** - 使用 Gemini 需要美区通道

### 4.2 扩展相关限制

1. **Open VSX 扩展数量有限** - 不如 Microsoft Marketplace 丰富
2. **部分扩展版本较旧** - 更新可能不及时
3. **手动安装 .vsix 可能有许可风险**

### 4.3 其他限制

1. **需要稳定的互联网连接** - 离线功能有限
2. **工作区数量有限制** - 免费用户 2-3 个，Google Developer Program 成员 10 个
3. **休眠机制** - 闲置一段时间后会进入休眠状态

---

## 5. 官方文档链接

| 主题 | 链接 |
|------|------|
| Firebase Studio 概览 | https://firebase.google.com/docs/studio |
| dev.nix 参考文档 | https://firebase.google.com/docs/studio/devnix-reference |
| 自定义工作区 | https://firebase.google.com/docs/studio/customize-workspace |
| 创建自定义模板 | https://firebase.google.com/docs/studio/custom-templates |
| IDX 成为 Firebase Studio | https://firebase.google.com/docs/studio/idx-is-firebase-studio |
| Nix 语言教程 | https://nix.dev/tutorials/nix-language |
| Open VSX Registry | https://open-vsx.org/ |
| Nix 包搜索 | https://search.nixos.org/packages |

---

## 6. 总结

| 功能 | 是否支持 | 说明 |
|------|----------|------|
| 修改 LLM Provider | ❌ 否 | 仅支持 Gemini |
| 接入 OpenAI API | ❌ 否 | 无法作为内置 AI |
| 接入 Anthropic API | ❌ 否 | 无法作为内置 AI |
| 切换 Gemini 模型 | ✅ 是 | 可在多个 Gemini 模型中选择 |
| 自定义 dev.nix | ✅ 是 | 完整的 Nix 配置系统 |
| 安装 VS Code 扩展 | ✅ 是 | 通过 Open VSX |
| 自定义启动命令 | ✅ 是 | onCreate/onStart 钩子 |
| 自定义 AI 技能 | ⚠️ 有限 | 仅支持 AI 规则文件 |
| 创建自定义模板 | ✅ 是 | 支持模板创建和分享 |

---

*报告生成时间: 2025年*
*基于 Firebase Studio / Google IDX 官方文档和社区资源*
