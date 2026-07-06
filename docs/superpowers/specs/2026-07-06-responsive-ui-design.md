# 设计文档：UI 响应式适配（窗口拉伸跟随）

**日期：** 2026-07-06
**作者：** 陈得冠
**状态：** 已确认，待实施

## 一、问题陈述

当前应用窗口支持缩放（`tauri.conf.json` 中 `resizable:true`，`minWidth:800`，`minHeight:540`），但拖拽窗口边缘或最大化时，UI **完全不跟随变化**——主容器永远保持 880×600 像素，窗口多出来的空间只显示 body 背景。

**根因：** `index.html` 第 86 行 `.app{width:880px;height:600px}` 写死固定像素，且 `.pnl-l`、`.pnl-m` 也写死 `width:260px`。窗口尺寸变化时容器没有任何响应。

## 二、目标

- 拖拽窗口边缘、最大化、还原时，UI 整体等比拉伸跟随窗口尺寸
- 保持当前视觉布局比例不变（三栏 260:260:360）
- 零 JS、零 Rust、零配置改动，仅改 CSS
- 不破坏现有亮/暗主题、模式切换（面→TXT / TXT→面）逻辑

## 三、非目标

- 不引入可拖拽分隔条（splitter）——留待未来按需追加
- 不做移动端/平板适配——这是桌面 Tauri 应用
- 不改字段密度、字体大小、按钮尺寸——只动布局容器尺寸

## 四、用户决策记录

| 决策点 | 选择 |
|--------|------|
| 拉伸策略 | 整体等比拉伸 |
| 三栏拉伸方式 | 三栏按比例同时变宽（保持 260:260:360 视觉比例） |
| 实现路径 | 路径 A：CSS grid + fr 比例单位 |

## 五、技术方案

### 5.1 根容器填满视口

```css
.app{
  width:100%;            /* 原 width:880px */
  height:100vh;          /* 原 height:600px */
  border-radius:0;       /* decorations:false 无边框窗口无需圆角 */
  border:none;           /* 无边框窗口无需外边框 */
}
```

理由：`decorations:false` 表示这是自定义无边框窗口，原本的圆角和外边框是模拟"卡片悬浮"视觉，在填满视口后不再适用。

### 5.2 body 取消居中

```css
body{
  display:block;                    /* 原 display:flex; justify-content:center */
  align-items:normal;               /* 原 align-items:flex-start */
  min-height:100vh;
}
```

理由：原本 body 用 flex 居中一个 880×600 的卡片，现在 app 要占满，居中逻辑失效且会引入多余的计算。

### 5.3 三栏 grid 等比布局

**面→TXT 模式（`data-mode="s"`，三栏）：**
```css
.app[data-mode="s"] .main{
  display:grid;
  grid-template-columns: minmax(240px, 260fr) minmax(240px, 260fr) minmax(320px, 360fr);
}
```

**TXT→面 模式（`data-mode="t"`，两栏）：**
```css
.app[data-mode="t"] .main{
  display:grid;
  grid-template-columns: minmax(280px, 300fr) 1fr;
}
```

**关键设计：`minmax(下限, 比例)`**
- 下限保证窗口拖到 `minWidth:800` 时三栏不被挤到字段溢出（240+240+320=800 正好等于 minWidth）
- `fr` 比例保证窗口拉大时三栏同步等比变宽，视觉比例保持 260:260:360 不变
- TXT 模式下限 280 对应原 300px，避免单栏过窄

### 5.4 移除原有固定宽度声明

需要移除/覆盖的选择器：
- `.pnl-l{width:260px;...}` → 移除 `width:260px`（由 grid 列宽接管）
- `.pnl-m{width:260px;...}` → 移除 `width:260px`
- `.app[data-mode="t"] .pnl-l{width:300px}` → 移除（由 grid 列宽接管）
- `.pnl-l{flex-shrink:0}` 和 `.pnl-m{flex-shrink:0}` → 移除（grid 项不受 flex-shrink 影响，但保留无害；为清洁起见移除）

`.main` 原有 `display:flex` 会被 `display:grid` 覆盖，原 `overflow:hidden`、`flex:1` 需保留（`.main` 在 `.app` 这个 flex 容器里仍需要 `flex:1` 来占满标题栏与页脚之间的纵向空间）。

### 5.5 不变的部分

| 项 | 现状 | 处理 |
|----|------|------|
| titlebar 高度 38px | flex-shrink:0 | 不动 |
| 页脚/状态栏 | flex-shrink:0 | 不动 |
| 字段、字体、按钮尺寸 | 全部 px | 不动 |
| 亮/暗主题 token | CSS 变量 | 不动 |
| `tauri.conf.json` 窗口配置 | minWidth:800, minHeight:540 | 不动（与 grid minmax 下限对齐） |
| 所有 JS 逻辑 | main.js | 不动 |

## 六、改动范围

仅 `index.html` 的 `<style>` 段，约 5 处修改、15 行 CSS：

1. `body` 选择器：`display/align-items` 调整
2. `.app` 选择器：`width/height/border/border-radius`
3. `.pnl-l`、`.pnl-m`：移除 `width` 与 `flex-shrink:0`
4. `.app[data-mode="t"] .pnl-l`：移除 `width:300px`
5. 新增 `.app[data-mode] .main` 两条 grid 规则

## 七、验证清单

实施后必须逐一验证：

- [ ] **800×540（最小）**：三栏不挤、字段不溢出、按钮可点
- [ ] **880×600（默认）**：与改造前视觉一致（关键回归点）
- [ ] **1200×800**：三栏等比变宽，主区可见更多行坐标
- [ ] **1920×1080（最大化）**：填满屏幕，无空白边距，布局协调
- [ ] **横向拉宽**：右栏扩展，左/中按比例扩展
- [ ] **纵向拉高**：主区列表/预览纵向延伸，标题栏/页脚不动
- [ ] **面→TXT 模式**：三栏 grid 正常
- [ ] **TXT→面 模式**：两栏 grid 正常，中栏隐藏
- [ ] **模式切换**：s↔t 切换后布局正确（grid 列数动态变化）
- [ ] **亮/暗主题**：两种主题下拉伸均正常
- [ ] **圆角/边框**：app 填满后无残留视觉边框

## 八、风险与回滚

**风险：** 极低。纯 CSS 改动，无逻辑变更，最坏情况是某些极端尺寸下视觉不协调。

**回滚：** `git revert` 单次提交即可。
