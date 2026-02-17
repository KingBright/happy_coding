# Git UI 优化设计方案

## 问题分析

### 1. Git 按钮与 Session 捆绑
**当前问题：**
- "变更"按钮放在顶部 header，虽然只在有 session 时显示，但位置不够直观
- 用户切换 session 时，按钮状态可能会混乱
- 应该让每个 session 有自己的变更入口

**改进方案：**
- 将 Git 按钮从顶部移到每个 session 卡片内（右下角悬浮小按钮）
- 或者在终端标题栏显示当前 session 的 Git 状态
- 切换 session 时自动重置 Git panel 状态

### 2. Diff 显示优化
**当前问题：**
- 使用 `<pre>` 简单展示原始 diff 文本，可读性差
- 没有语法高亮
- 新增/删除行没有明显视觉区分

**改进方案：**
- 解析 diff 格式，按行类型渲染：
  - 文件头（文件名、模式变更）- 蓝色背景
  - 新增行（+）- 绿色背景 + 行号
  - 删除行（-）- 红色背景 + 行号
  - 上下文行 - 正常背景
  - 区块头（@@ -x,x +x,x @@）- 灰色背景
- 添加行号显示
- 代码语法高亮（可选，使用 highlight.js 或 Prism）

### 3. 移动端布局优化
**当前问题：**
- 文件列表和 diff 区域在移动端上下堆叠，但比例可能不太合理
- 小屏幕上的 touch 目标太小
- 缺少手势支持

**改进方案：**
- 移动端采用全屏 drawer/sheet 样式
- 文件列表可折叠
- 更大的 touch 目标（最小 44px）
- 支持左右滑动切换文件

---

## 具体实现方案

### 方案 A：Session 卡片内悬浮按钮

```
┌─────────────────────────────────┐
│ session-1                       │
│ ┌─────────────────────────────┐ │
│ │ 终端内容                    │ │
│ │                             │ │
│ │                             │ │
│ │                     [📝 3]  │ │  ← 悬浮按钮显示变更数量
│ └─────────────────────────────┘ │
└─────────────────────────────────┘
```

**优点：**
- 每个 session 有独立的 Git 入口
- 直观显示变更数量
- 点击打开该 session 的 Git panel

**缺点：**
- 终端区域可能被遮挡
- 需要处理多个 session 同时打开 Git panel 的情况

### 方案 B：终端标题栏集成

```
┌─────────────────────────────────┐
│ session-1                [📝] │  ← 标题栏右侧显示 Git 按钮
├─────────────────────────────────┤
│ 终端内容                        │
│                                 │
└─────────────────────────────────┘
```

**优点：**
- 不会遮挡终端内容
- 与 session 明确关联

**缺点：**
- 标题栏空间可能不够

### 推荐：方案 B + 改进的 Diff 显示

---

## Diff 渲染实现

### Diff 解析函数

```rust
#[derive(Clone, PartialEq)]
enum DiffLine {
    Header(String),           // diff --git, index, ---, +++
    ChunkHeader(String),      // @@ -x,x +x,x @@
    Context(String),          // 上下文行（空格开头）
    Addition(String),         // 新增行（+开头）
    Deletion(String),         // 删除行（-开头）
    Empty,                    // 空行
}

fn parse_diff(diff_text: &str) -> Vec<DiffLine> {
    // 解析 diff 文本为结构化数据
}
```

### 渲染组件

```rust
fn render_diff_line(line: &DiffLine) -> Html {
    match line {
        DiffLine::Header(text) => html! {
            <div class="diff-line header">{ text }</div>
        },
        DiffLine::ChunkHeader(text) => html! {
            <div class="diff-line chunk-header">{ text }</div>
        },
        DiffLine::Context(text) => html! {
            <div class="diff-line context">
                <span class="diff-marker">{" "}</span>
                <span class="diff-content">{ text }</span>
            </div>
        },
        DiffLine::Addition(text) => html! {
            <div class="diff-line addition">
                <span class="diff-marker">{"+"}</span>
                <span class="diff-content">{ text }</span>
            </div>
        },
        DiffLine::Deletion(text) => html! {
            <div class="diff-line deletion">
                <span class="diff-marker">{"-"}</span>
                <span class="diff-content">{ text }</span>
            </div>
        },
        DiffLine::Empty => html! {
            <div class="diff-line empty"></div>
        },
    }
}
```

### CSS 样式

```css
.diff-line {
    display: flex;
    font-family: "SF Mono", Monaco, monospace;
    font-size: 13px;
    line-height: 1.6;
    padding: 1px 0;
}

.diff-line.header {
    background: rgba(88, 166, 255, 0.1);
    color: var(--accent-primary);
    font-weight: 500;
    padding: 4px 8px;
    margin: 4px 0;
    border-radius: 4px;
}

.diff-line.chunk-header {
    background: rgba(139, 148, 158, 0.2);
    color: var(--text-secondary);
    padding: 2px 8px;
}

.diff-line.addition {
    background: rgba(35, 134, 54, 0.15);
}

.diff-line.addition .diff-marker {
    color: var(--accent-success);
    font-weight: bold;
    width: 20px;
    text-align: center;
}

.diff-line.deletion {
    background: rgba(248, 81, 73, 0.15);
}

.diff-line.deletion .diff-marker {
    color: var(--accent-error);
    font-weight: bold;
    width: 20px;
    text-align: center;
}

.diff-line.context {
    background: transparent;
}

.diff-line.context .diff-marker {
    color: var(--text-muted);
    width: 20px;
    text-align: center;
}

.diff-content {
    flex: 1;
    white-space: pre-wrap;
    word-break: break-all;
    padding-left: 8px;
}
```

---

## 移动端优化

### Git Panel 改为全屏 Drawer

```css
@media (max-width: 768px) {
    .git-panel {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        z-index: 1000;
        background: var(--bg-primary);
        border-radius: 12px 12px 0 0;
        transform: translateY(100%);
        transition: transform 0.3s ease;
    }

    .git-panel.open {
        transform: translateY(0);
    }

    .git-panel-header {
        position: sticky;
        top: 0;
        background: var(--bg-secondary);
        padding: 16px;
        border-bottom: 1px solid var(--border-color);
    }

    .git-content {
        flex-direction: column;
        height: calc(100% - 60px);
    }

    .git-file-list {
        width: 100%;
        max-height: 40%;
        border-right: none;
        border-bottom: 1px solid var(--border-color);
    }

    .git-diff-viewer {
        flex: 1;
        overflow: auto;
    }
}
```

---

## 实施步骤

### Phase 1: Diff 语法高亮（高优先级）
1. 添加 diff 解析函数
2. 创建新的 diff 渲染组件
3. 更新 CSS 样式
4. 测试各种 diff 格式

### Phase 2: Git 按钮位置优化（中优先级）
1. 从顶部 header 移除 Git 按钮
2. 在终端标题栏添加 Git 按钮
3. 确保切换 session 时状态正确

### Phase 3: 移动端优化（中优先级）
1. 实现全屏 drawer 样式
2. 添加滑动手势支持
3. 优化 touch 目标大小

---

## 参考设计

### GitHub 风格的 Diff 显示
- 新增行：浅绿色背景 + 绿色左边框
- 删除行：浅红色背景 + 红色左边框
- 行号显示在左侧
- 文件头突出显示

### VS Code 风格的 Diff
- 侧边栏显示迷你地图
- 双栏对比视图（可选）
- 内联/分栏切换

### 建议采用
GitHub 风格的简洁 diff 显示，适合 Web 界面。
