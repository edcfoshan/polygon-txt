# UI 响应式适配（窗口拉伸跟随）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Tauri 应用窗口拖拽边缘/最大化时，UI 整体等比拉伸跟随窗口尺寸，三栏保持 260:260:360 视觉比例。

**Architecture:** 纯 CSS 改动。把写死的 `.app{width:880px;height:600px}` 改为填满视口；把 `.main` 从 flex 改成 grid，用 `grid-template-columns:minmax(下限, fr比例)` 让三栏等比变宽且拖到最小尺寸不挤字段。

**Tech Stack:** 原生 CSS（无预处理器）、Tauri v2 WebView2、单文件 HTML（index.html 内联 style）。

## Global Constraints

- **仅改 `index.html` 的 `<style>` 段**——零 JS、零 Rust、零配置改动
- **保留视觉比例**：面→TXT 三栏 260:260:360；TXT→面 两栏 300:flex
- **minmax 下限对齐 tauri 窗口配置**：`minWidth:800`（240+240+320=800），`minHeight:540`
- **不动**：titlebar 38px 高度、字段密度、字体、按钮尺寸、亮/暗主题 token、所有 JS 逻辑
- **`decorations:false`**（无边框窗口）→ `.app` 不再需要外圆角/外边框/阴影
- **TDD 形式说明**：本任务无单元测试框架（前端是单文件 HTML 内联 JS，无 vitest/jest）。验证方式为构建后人工 + Playwright 自动化截图对比，每个任务结束前必须 `npm run build` 通过且页面无 console 错误

---

## 文件结构

| 文件 | 职责 | 改动类型 |
|------|------|----------|
| `index.html` | 单文件应用（CSS/JS/HTML 全内联） | 修改 `<style>` 段 5 处 |

无新文件创建。无其他文件改动。

---

## Task 1: body 取消固定卡片居中

**Files:**
- Modify: `index.html:59-67`（`body` 选择器）

**Interfaces:**
- Consumes: 无
- Produces: `body` 改为 `display:block`，让 `.app` 能占满视口（供 Task 3 的 `.app{width:100%;height:100vh}` 生效）

**背景：** 原 body 用 `display:flex;justify-content:center` 居中一张 880×600 的卡片。Task 3 让卡片占满后，flex 居中失效且会让 `100vh` 高度计算异常（flex 容器内子项的 `height:100vh` 在某些 WebView 版本下会塌陷），所以先改 body。

- [ ] **Step 1: 修改 body 的 display 与对齐**

打开 `index.html`，定位到第 59-67 行的 `body{...}` 选择器。将第 62 行：

```css
  display:flex;justify-content:center;align-items:flex-start;
```

改为：

```css
  display:block;
```

修改后 `body` 选择器应为：

```css
body{
  font-family:"Inter","Noto Sans SC","Microsoft YaHei","PingFang SC",system-ui,sans-serif;
  font-size:13px;color:var(--tx);
  display:block;
  min-height:100vh;
  background:
    radial-gradient(ellipse at 50% 25%, var(--srf2) 0%, var(--bg) 60%);
  transition:background .35s ease, color .35s ease;
}
```

- [ ] **Step 2: 构建验证**

Run:
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转" && npm run build
```
Expected: 构建成功，`dist/index.html` 生成，无错误。

- [ ] **Step 3: Commit**

```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转"
git add index.html
git commit -m "refactor(ui): body 取消 flex 居中，为 app 占满视口铺路"
```

---

## Task 2: `.app` 填满视口（去固定宽高/圆角/边框/阴影）

**Files:**
- Modify: `index.html:84-95`（`.app` 选择器）

**Interfaces:**
- Consumes: Task 1 的 `body{display:block}`
- Produces: `.app` 占满 100% 宽 × 100vh 高，无边框装饰（供 Task 3 的 grid 布局有充足空间铺开）

**背景：** `.app` 原 `width:880px;height:600px` 是不跟随缩放的根因。同时 `decorations:false` 表示这是自定义无边框窗口，原本的 `border`/`border-radius`/`box-shadow` 是模拟"卡片悬浮"视觉，填满视口后不再适用，需一并移除。

- [ ] **Step 1: 修改 .app 选择器**

定位到 `index.html` 第 84-95 行的 `.app{...}`。将：

```css
.app{
  width:880px;height:600px;
  background:var(--srf);
  border:1px solid var(--brd);
  border-radius:8px;
  overflow:hidden;
  display:flex;flex-direction:column;
  box-shadow:var(--shadow-lg), var(--ring);
  position:relative;
  transition:background .3s ease, border-color .3s ease, box-shadow .3s ease;
}
```

改为：

```css
.app{
  width:100%;height:100vh;
  background:var(--srf);
  overflow:hidden;
  display:flex;flex-direction:column;
  position:relative;
  transition:background .3s ease;
}
```

变更点（逐一对账）：
- `width:880px` → `width:100%`
- `height:600px` → `height:100vh`
- 删除 `border:1px solid var(--brd);`
- 删除 `border-radius:8px;`
- 删除 `box-shadow:var(--shadow-lg), var(--ring);`
- `transition` 中删除 `border-color .3s ease, box-shadow .3s ease`（这些属性已不存在）

- [ ] **Step 2: 构建验证**

Run:
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转" && npm run build
```
Expected: 构建成功，无错误。

- [ ] **Step 3: 视觉冒烟（人工或 Playwright）**

启动 dev：
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转" && npm run tauri dev
```
窗口打开后，肉眼确认：
- app 卡片填满整个窗口（无 body 背景露出）
- 三栏目前会因 `.pnl-l/m{width:260px}` 仍是固定值，**这是预期**——grid 改造在 Task 3
- 此刻拖动窗口边缘，app 跟着变化（但内部栏宽不变）

> 注：Task 2 单独不完美，三栏还固定。Task 3 完成后才有完整效果。此步骤仅确认根容器已跟随。

- [ ] **Step 4: Commit**

```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转"
git add index.html
git commit -m "feat(ui): .app 占满视口，移除固定 880x600 与卡片装饰

根因修复：窗口拖拽时主容器现在跟随变化。
三栏等比布局待 Task 3 grid 改造。"
```

---

## Task 3: `.main` 改 grid + 三栏 minmax/fr 等比布局

**Files:**
- Modify: `index.html:421`（`.main` 选择器，新增 grid 规则）
- Modify: `index.html:428`（`.app[data-mode="t"] .pnl-l` 移除固定宽度）
- Modify: `index.html:449-450`（`.pnl-l`、`.pnl-m` 移除固定宽度与 flex-shrink）

**Interfaces:**
- Consumes: Task 2 的 `.app{width:100%;height:100vh}`
- Produces: 三栏等比响应式布局，面→TXT 模式三栏 260:260:360，TXT→面 模式两栏 300:flex

**关键设计：**
- `minmax(下限, fr比例)` —— 下限保证拖到 `minWidth:800` 时字段不溢出，`fr` 保证拉大时按比例同步变宽
- 下限之和 240+240+320=800，正好等于 `tauri.conf.json` 的 `minWidth:800`
- TXT 模式下限 280，避免单栏过窄挤压输出设置字段

- [ ] **Step 1: 移除 .pnl-l / .pnl-m 的固定宽度**

定位到 `index.html` 第 449-450 行：

```css
.pnl-l{width:260px;border-right:1px solid var(--brd);flex-shrink:0}
.pnl-m{width:260px;border-right:1px solid var(--brd);flex-shrink:0}
```

改为（移除 `width` 和 `flex-shrink:0`，grid 列宽接管）：

```css
.pnl-l{border-right:1px solid var(--brd)}
.pnl-m{border-right:1px solid var(--brd)}
```

> 为何移除 `flex-shrink:0`：`.main` 已从 flex 改 grid（见 Step 3），`flex-shrink` 对 grid 项无效，移除保持清洁。`.pnl-l/m` 的其他属性（border、后续的 `.pnl-m .pnl-bd` 等子规则）不受影响。

- [ ] **Step 2: 移除 TXT 模式下 .pnl-l 的固定宽度**

定位到 `index.html` 第 428 行：

```css
.app[data-mode="t"] .pnl-l{width:300px}
```

改为（grid 列宽接管，删除整行 width 声明；为最小改动，将该行替换为空操作注释或直接删除 width）：

直接删除这一行的 `width:300px`，保留选择器以承载后续可能的样式覆盖会引入空规则。最干净的做法是**整行删除**第 428 行（包括选择器和声明）。

删除前上下文（第 427-429 行）：
```css
.app[data-mode="t"] .pnl-m{display:none}
.app[data-mode="t"] .pnl-l{width:300px}
.app[data-mode="t"] .ctr{flex:1}
```

删除后：
```css
.app[data-mode="t"] .pnl-m{display:none}
.app[data-mode="t"] .ctr{flex:1}
```

> 注意：`.app[data-mode="t"] .ctr{flex:1}` **保留不动**。虽然 `.main` 改 grid 后 `.ctr` 作为 grid 项不再受 flex 影响，但 `.ctr` 内部仍是 flex 容器（`.ctr-inner{display:flex;flex-direction:column;flex:1}` 第 425 行），`flex:1` 在 `.ctr` 自身的 flex 上下文中无意义但无害，且未来若回退布局需用到，保留以缩小改动面。

- [ ] **Step 3: 新增 .main 的 grid 规则**

定位到 `index.html` 第 421 行：

```css
.main{flex:1;display:flex;overflow:hidden}
```

改为（保留 `flex:1` 让 `.main` 在 `.app` 这个 flex 容器中占满标题栏与页脚之间的纵向空间；`display:flex` 改为 `display:grid` 由后续模式规则覆盖）：

```css
.main{flex:1;overflow:hidden}
/* Responsive grid — columns scale with window, keep 260:260:360 visual ratio */
.app[data-mode="s"] .main{
  display:grid;
  grid-template-columns:minmax(240px,260fr) minmax(240px,260fr) minmax(320px,360fr);
}
.app[data-mode="t"] .main{
  display:grid;
  grid-template-columns:minmax(280px,300fr) 1fr;
}
```

**逐项说明：**
- `.main` 移除 `display:flex`，因为下面的模式规则会用 `display:grid` 覆盖。保留 `flex:1`（`.main` 在 `.app{display:flex;flex-direction:column}` 中作为子项，需 flex:1 占满纵向）和 `overflow:hidden`。
- `data-mode="s"`（面→TXT）：三栏，minmax 下限 240/240/320，fr 比例 260/260/360（即原始视觉比例）。
- `data-mode="t"`（TXT→面）：两栏，minmax 下限 280，fr 比例 300，剩余 `1fr` 给主区。中栏 `.pnl-m` 在 t 模式下 `display:none`（第 427 行规则保留），grid 自动只布局可见的 2 个子项。

- [ ] **Step 4: 构建验证**

Run:
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转" && npm run build
```
Expected: 构建成功，无错误。

- [ ] **Step 5: 多尺寸人工/Playwright 验证**

启动 dev：
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转" && npm run tauri dev
```

逐一验证（按验证清单）：
- [ ] **800×540（最小）**：拖到最小尺寸，三栏不挤、字段不溢出、按钮可点
- [ ] **880×600（默认）**：与改造前视觉一致（关键回归点——三栏宽度应仍约为 260/260/360）
- [ ] **1200×800**：三栏等比变宽，主区可见更多行坐标
- [ ] **1920×1080（最大化）**：填满屏幕，无空白边距
- [ ] **面→TXT 模式**：三栏 grid 正常
- [ ] **TXT→面 模式**：切到 TXT→面（点模式切换），两栏 grid 正常，中栏隐藏
- [ ] **模式来回切换**：s→t→s，布局每次都正确重排
- [ ] **亮/暗主题**：切主题，两种下拉伸均正常
- [ ] **无 console 错误**：F12 看 console，无报错

> 若 800×540 下某栏字段溢出：把对应 minmax 下限调高（如 240→260），但需保证三下限之和 ≤ 800。若调高后总和 >800，需同步提高 `tauri.conf.json` 的 `minWidth`（但这属于配置改动，需谨慎）。

- [ ] **Step 6: Commit**

```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转"
git add index.html
git commit -m "feat(ui): .main 改 grid，三栏 minmax/fr 等比响应式

面→TXT: minmax(240,260fr) x3，保持 260:260:360 视觉比例
TXT→面: minmax(280,300fr) + 1fr，单栏不挤
拖拽窗口/最大化时三栏同步变宽。"
```

---

## Task 4: 最终回归验证 + 文档同步

**Files:**
- Modify: `CLAUDE.md` / `AGENTS.md`（若其中描述了固定布局，需同步）—— **先检查，可能无需改动**

**Interfaces:**
- Consumes: Task 1-3 全部完成
- Produces: 验证通过的可发布状态

- [ ] **Step 1: 检查 AGENTS.md 是否提及固定布局**

Run:
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转"
grep -n "880\|600px\|260px\|width:260\|flex" AGENTS.md CLAUDE.md 2>/dev/null
```

- 若输出包含对固定布局的描述（如"3 columns: 260+260+360"），更新为响应式描述
- 若仅是历史说明或无匹配，跳过 Step 2

> AGENTS.md 第 11 行（Dual-Mode Layout 段）提到 "3 columns: 260+260+360" 和 "2 columns: 300+flex"——这描述的是**视觉比例**，响应式改造后仍然成立（grid fr 比例就是 260:260:360）。建议在该段补一句说明已改为响应式，但不强制。

- [ ] **Step 2（条件性）: 更新 AGENTS.md 布局描述**

若 Step 1 决定更新，定位 AGENTS.md 的 `### Dual-Mode Layout` 段，在末尾追加：

```markdown
布局为响应式：`.main` 用 CSS grid + `minmax(下限, fr比例)`，窗口拉伸时三栏按 260:260:360 等比变宽，拖到 minWidth:800 时下限保证字段不溢出。
```

- [ ] **Step 3: 完整构建冒烟**

Run:
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转" && npm run build
```
Expected: 构建成功。

- [ ] **Step 4: Rust 集成测试回归（确保未破坏后端）**

Run:
```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转\src-tauri" && cargo test --test integration_test
```
Expected: 17 个集成测试全部通过（前端 CSS 改动不应影响后端，但跑一遍确认无构建副作用）。

- [ ] **Step 5: Commit（若有文档改动）**

```bash
cd "C:\Users\Administrator\Documents\txt与gdb互转"
git add AGENTS.md CLAUDE.md
git commit -m "docs: 同步响应式布局说明"
```

若 Step 1 判定无需改文档，跳过本步骤。

---

## 验收标准

全部 Task 完成后，必须满足：

1. ✅ `npm run build` 成功
2. ✅ `cargo test --test integration_test` 17 个测试通过
3. ✅ 窗口拖拽边缘时 UI 跟随变化（核心目标）
4. ✅ 最大化/还原时 UI 正确重排
5. ✅ 默认 880×600 下与改造前视觉一致（回归）
6. ✅ 最小 800×540 下字段不溢出
7. ✅ 面→TXT / TXT→面 双模式布局均正确
8. ✅ 亮/暗主题切换后布局正常
9. ✅ console 无错误
