# 动态投影 Modal — 设计稿

**状态**：草稿（待 spec 自审 + 用户审阅）
**日期**：2026-07-26
**作者**：Codex

## 1. 概述

把当前的 #og 复选框 + #oz 选择器，替换为点击按钮弹出 modal 的形式：

1. 先看到导入数据自动识别的结果（坐标系 / 形式 / 分带 / 带号）
2. 再手动选目标 CRS（坐标系 / 形式 / 分带 / 带号）
3. 应用后，TXT 头表自动同步到目标 CRS

约束：
- 单文件模式：loadedFiles.length === 1 时按钮才启用；多文件时按钮置灰并提示
- 同基准内互转：datum 锁定为输入基准，不做跨基准

## 2. 目标

- 让用户在转换前清楚看到我现在是什么 CRS、想变成什么 CRS
- 支持 5 种转换：大地→投影 3°/6°、投影→大地 (反算)、投影 3°↔6° (互转)
- 默认行为零风险（保持原样 / 不转换）
- 头表字段和实际坐标保持一致（不出现头表写大地但坐标是米的不一致）

## 3. 非目标

- 跨基准转换（西安80 → CGCS2000）— yagni
- 多文件批量投影
- 自定义 EPSG 代码选择
- 头表字段以外的自定义内容在 apply 时被覆盖

## 4. 用户场景

| 场景 | 输入 | 用户期望 | 转换模式 |
|------|------|----------|----------|
| A. 默认 | 任意 | 不做转换 | keep |
| B. 度→米 3° | CGCS2000 大地 3°带 38号 | 输出 GK 投影 3°带 38号 | A |
| C. 度→米 6° | CGCS2000 大地 3°带 38号 | 输出 GK 投影 6°带 | B |
| D. 米→度 | CGCS2000 投影 3°带 38号 | 输出大地（经纬度）| C |
| E. 米 3°→米 6° | CGCS2000 投影 3°带 38号 | 输出 GK 投影 6°带 20号 | F |
| F. 米 6°→米 3° | CGCS2000 投影 6°带 20号 | 输出 GK 投影 3°带 | G |
| G. .prj 缺失 | 任意（无 .prj）| 用质检反推兜底 | 任意 |

## 5. UI 设计

### 5.1 按钮替换

原来：
```
[ ] 输出公里网（经纬度→高斯-克吕格）    [3度带 ▼]
```

改成：
```
[ + 动态投影 ]                    <当前目标摘要>
```

### 5.2 Modal 结构（双段式）

```
┌─ 动态投影 ────────────────────────────┐ ×
│ 基于导入数据自动识别 · 同基准内互转  │
│ ───────────────────────────────────── │
│ 导入识别（只读）                       │
│   坐标系    CGCS2000                  │
│   形式      大地（度）                 │
│   分带      3°带                      │
│   带号      38  ✓                     │
│ ───────────────────────────────────── │
│ [ 不转换（按导入原样输出） ]          │
│ ───────────────────────────────────── │
│ 目标设置                               │
│   坐标系  [CGCS2000 ▼]   ← 锁源       │
│   形式    [大地（度） ▼]              │
│   ── type=投影时才显示 ──             │
│   分带    [3°带 ▼]                    │
│   带号    [38    ]                    │
│ ───────────────────────────────────── │
│ ☐ 无代号质检（.prj 缺失/冲突时显示）  │
│ ───────────────────────────────────── │
│ [取消]                       [应用]    │
└────────────────────────────────────────┘
```

### 5.3 Apply 后

按钮高亮 + 摘要 + toast：
```
[ ✓ 动态投影 ]   投影 3°带 / 38
```
toast 短暂显示：已应用：投影 3°带 / 带号 38

## 6. 状态模型

| 字段 | 类型 | 位置 | 含义 |
|------|------|------|------|
| projMode | string | window | 模式：keep / A / B / C / F / G |
| projZone | number|null | window | 用户在 modal 填的带号；空 = 自动反推 |
| projQc | bool | window | 无代号质检开关（仅 .prj 异常时显示并启用）|
| projQcResult | object|null | window | 上次 apply 质检结果 {ok, derivedZone, expected} |

持久化：写入 cfgs[cur].p.projMode / projZone / projQc，由现有 autoSave 机制落 localStorage。

projMode → 表单初始状态映射：

| projMode | type | band | zone |
|----------|------|------|------|
| keep | 沿用输入 | 沿用输入 | 沿用输入 |
| A | 投影 | 3° | 输入 z |
| B | 投影 | 6° | 输入 z |
| C | 大地 | (隐藏) | (隐藏) |
| F | 投影 | 6° | 反推 |
| G | 投影 | 3° | 反推 |

## 7. 行为规格

### 7.1 type 切换（决策 2）

切换 type 时，分带/带号沿用输入的当前值。

### 7.2 type=大地 的字段可见性（决策 3）

type=大地 时，分带和带号整行隐藏。

### 7.3 分带切换时的带号（决策 4 = A2）

- 带号空 → 自动从输入坐标反推新带的带号
- 带号非空 → 保留用户填的值

### 7.4 不转换 toggle（决策 5）

toggle 激活时：
- 整个目标设置块置灰
- Apply 后 projMode = keep

### 7.5 无代号质检可见性（决策 6）

- 默认隐藏
- 仅当 .prj 缺失 / 不全 / 冲突时显示
- 由 Rust detect_crs_completeness 提供

### 7.6 Apply 反馈（决策 7 = B）

1. 关闭 modal
2. toast 显示已应用：{摘要}
3. 按钮加 .on class
4. 按钮右侧 projModeLabel 显示目标 CRS 摘要

### 7.7 重新打开 modal 的初始值（决策 8 = B）

- projMode == keep → 显示输入 CRS
- projMode == A/B/C/F/G → 显示目标 CRS（按 6.1 映射表）

### 7.8 头表自动更新（决策 9 = A）

apply 时同步 6 个字段到目标 CRS，其他属性行不动：

| 字段 | 同步规则 |
|------|---------|
| 坐标系 | 强制 = 目标 datum |
| 形式 | 强制 = 大地（度）/ 投影（米） |
| 分带 | 强制 = 3°带 / 6°带 / — |
| 带号 | 强制 = 用户填的或反推的 |
| 投影类型 | 强制 = 高斯克吕格（type=投影）/ 不动（type=大地）|
| 计量单位 | 强制 = 米（type=投影）/ 不动（type=大地）|

### 7.9 取消按钮（决策 10 = A）

保留取消按钮，关闭 modal 丢弃所有未保存的 form 修改。

## 8. 头表自动更新实现细节

### 8.1 写入哪些 attr 行

通过 key 匹配定位 attr 行：
- 坐标系 → k = 坐标系
- 形式 → k = 形式（attr 自动生成的字段）
- 分带 → k = 分带
- 带号 → k = 带号
- 投影类型 → k = 投影类型
- 计量单位 → k = 计量单位

### 8.2 找不到行时怎么办

如果某个 key 的 attr 行不存在（如用户删了），不创建新行，缺失项记到 toast 警告。

### 8.3 与现有 og 复选框的关系

完全废弃：
- 删除 #og / #oz / #ogWarn HTML
- 删除 syncOgGate / refreshOgWarn JS
- 删除 og 相关 getOptions 字段

## 9. Rust 侧改动

### 9.1 projection.rs 新增

```rust
/// 根据投影后的 x 坐标反推带号
pub fn infer_zone_from_x(x: f64, band_width_deg: u8) -> Option<u32>

/// 投影 3°↔6°（同一基准内）
pub fn reband_projected(
    x: f64, y: f64,
    src_band: u8, src_zone: u32,
    dst_band: u8, dst_zone: u32,
    datum: Ellipsoid,
) -> Result<(f64, f64), ProjectionError>

/// 投影→大地（GK 反算）
pub fn gk_inverse(
    x: f64, y: f64,
    band_width_deg: u8, zone: u32,
    datum: Ellipsoid,
) -> Result<(f64, f64), ProjectionError>

/// 检测 .prj 完整性
pub fn detect_crs_completeness(crs_info: &CrsInfo) -> Completeness
```

### 9.2 lib.rs 新增 IPC

```rust
#[tauri::command]
async fn apply_dynamic_projection(
    features: Vec<Feature>,
    mode: String,
    target_band: Option<String>,
    target_zone: Option<u32>,
    do_qc: bool,
) -> Result<TransformResult, String>
```

### 9.3 convert.rs 集成点

在 SHP→TXT / GDB→TXT 流水线末尾：
1. 检测 cfg.projMode
2. 若非 keep，调用 apply_dynamic_projection
3. 同步更新头表 6 个字段
4. 若 do_qc=true，追加质检逻辑

## 10. 前端改动

### 10.1 index.html

删除：#og / #oz / #ogWarn（约 9 行）
新增：#btnProj 按钮、#projModal modal、CSS（约 60 行）

### 10.2 src/main.js

删除：
- syncOgGate / refreshOgWarn（约 20 行）
- getDemoCrInfo / applyDemoSeed（prototype only，约 20 行）
- URL hash demo seeding（约 10 行）

新增：
- openProjModal / closeProjModal / applyProjMode（约 60 行）
- renderProjDetection / renderProjTarget（约 40 行）
- updateProjButton（约 15 行）
- 事件绑定（约 15 行）
- 头表 attr 行同步 helper（约 20 行）

修改：
- getOptions 用 projMode 替代 og / oz
- getConfig 加入 projMode / projZone / projQc
- init 中 processImport 后调 updateProjButton

## 11. 验证计划

### 11.1 E2E（headless Chromium 17 场景）

扩展 verify-polygon-txt.js：

| 场景 | 期望 |
|------|------|
| 0 文件 | btnProj disabled |
| 2 文件 | btnProj disabled + tooltip |
| 1 文件 大地 | btnProj enabled，modal 显示 A/B |
| 1 文件 投影 3° | modal 显示 C + F |
| 1 文件 投影 6° | modal 显示 C + G |
| 应用 A | 按钮 .on + label=投影 3°带 |
| 重开 modal | form 显示目标 CRS |
| type 切换 | 分带/带号沿用 |
| type=大地 | 分带/带号行隐藏 |
| 分带切换 A2 | 带号空才反推 |
| 不转换 toggle | form 置灰，apply 后 projMode=keep |
| 取消按钮 | 改 form → 取消 → 不生效 |
| .prj 缺失 | QC 显示 |
| .prj 冲突 | QC 显示 |
| 头表同步 | apply 后 6 字段更新 |
| 保留其他 attr | 用户自定义行不动 |
| ArcGIS 对照 | 投影误差 <1mm |

### 11.2 现有 11 项测试

继续保留 autoSave + 恢复测试，确保不回归。

## 12. 渐进式实施步骤

1. 删除旧 UI + 加新 modal 骨架（不动 Rust）
2. 前端 mock 检测 + mock apply
3. 接 Rust detection (detect_crs_completeness)
4. 接 Rust projection (reband + inverse + 头表同步)
5. 接 Rust QC (反推带号 + 核对)
6. 删 prototype 代码（applyDemoSeed + URL hash）
7. 全套 E2E + 打包 release

每步可独立验证。

## 13. 风险 & 开放问题

### 13.1 风险

- GK 反算在 6°带的精度 <1mm 已验证；reband 是新算法需独立测试
- 跨基准 lock：UI 层锁住 datum，Rust 层需同样校验
- .prj 解析完整性：国内数据 WKT 写法多样，需充分测试样本

### 13.2 开放问题

- QC 反推算法（用 x 坐标反推 vs 用 feature 中心经度反推）— 实施时定
- 头表同步是否会冲掉用户手改值 — 决策 9 已定 强制覆盖 CRS 字段
- prototype 阶段的 applyDemoSeed / URL hash 必须删除

### 13.3 未来工作（不做）

- 跨基准转换（4 参数 / 7 参数 Bursa-Wolf）
- 自定义 EPSG 代码选择
- 多文件批量投影
- 投影椭球用户级选择

## 14. 验收（DoD）

- [ ] 17 个 E2E 场景全过
- [ ] ArcGIS Pro 对照误差 <1mm
- [ ] 现有 11 项 autoSave 测试不回归
- [ ] prototype 代码已删除
- [ ] #og / #oz / #ogWarn / syncOgGate / refreshOgWarn 全部移除
- [ ] verify-polygon-txt.js 扩展全过
- [ ] npm run tauri build 成功
- [ ] 产物替换 其他相关tbx放进去release/

## 15. 决策追溯

| # | 决策点 | 推荐 | 用户选 |
|---|--------|------|--------|
| 1 | 表单结构 | A (4 字段) | A |
| 2 | type 联动 | A (沿用) | A |
| 3 | type=大地 字段 | A (隐藏) | A |
| 4 | 分带→带号 | A2 | A2 |
| 5 | 不转换表达 | A (toggle) | A |
| 6 | QC 可见性 | A (默认隐藏) | A |
| 7 | Apply 反馈 | B (toast+.on+label) | B |
| 8 | 重开 modal 初始值 | B (projMode 推断) | B |
| 9 | 头表自动更新 | A (同步 6 字段) | A |
| 10 | 取消按钮 | A (保留) | A |
