# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**极思G界址点互转工具** — 测绘与国土行业 GIS 桌面工具，实现面要素（SHP/GDB）与标准界址点 TXT 文件的双向转换。Tauri v2 桌面应用（Rust 后端 + Vite/HTML 前端）。

- 仓库：https://github.com/edcfoshan/polygon-txt
- 窗口：默认 880×600px（`minWidth:800/minHeight:540`，可自由拉伸/最大化），无边框（`decorations: false`），自定义标题栏，支持浅色/暗色主题
- 响应式布局：`.app` 占满视口，`.main` 用 CSS grid + `minmax(下限, fr比例)`，拖拽窗口时三栏按 260:260:360 等比变宽，拖到 minWidth:800 时下限保证字段不溢出

## 构建与测试命令

```powershell
npm install                          # 安装前端依赖
npm run tauri dev                    # 开发模式（Vite HMR :1420 + Tauri WebView）
npm run tauri build                  # 生产构建 → src-tauri/target/release/jisig-bpoint-converter.exe
npm run build                        # 仅前端（dist/ 单文件 HTML）

cd src-tauri
cargo build --release                # 仅 Rust 编译（不会嵌入前端）
cargo test                           # 全部测试
cargo test --test integration_test   # 集成测试（SHP/DBF/PRJ/TXT/GDB 往返 + 三模式输出）
cargo test --test debug_output_test  # 调试用：从 TXT 生成 SHP/GDB 输出验证
cargo run --bin diag_read_gdb -- [gdb路径]  # 诊断：打印 GDB 图层/首尾坐标点，对照 arcpy（不传参走默认路径）
```

测试数据依赖 `test_arcpy/` 目录（ArcPy 生成的标准 SHP/TXT/GDB）和 `test_data/` 目录。

## 架构

```
index.html (CSS 内联, Google Fonts CDN)
  ← Vite (vite-plugin-singlefile) 打包 JS 内联到单文件 HTML
  ← Tauri WebView 嵌入 dist/index.html
       ↕ window.__TAURI__.core.invoke() IPC
  Rust 后端 (shapefile / geonative-filegdb / chrono)
       ↕ std::fs
  原生文件系统
```

关键：Vite 将所有 JS 内联到单个 HTML 文件中。`dist/index.html` 包含一切，Tauri 通过 `tauri-codegen` 在构建时嵌入。

### 前端 (src/main.js)

- 使用 ES module `import`（Vite 构建时解析为 `window.__TAURI__` 运行时 API）
- Vanilla JS，无框架。函数通过 `window.*` 导出供 HTML `onclick` 调用
- Markdown 弹窗：`content/about.md` 和 `content/sponsor.md` 通过 `?raw` 导入，`renderMarkdown()` 渲染
- 双模式：`data-mode="s"`（面→TXT，3 列 260+260+360）/ `data-mode="t"`（TXT→面，2 列 300+flex）
- 预设配置 `PP` 数组包含三种模式（基础地块/规划审批/自定义），字段自动匹配规则在 `FIELD_MATCH_RULES`

### Rust 后端模块

| 模块 | 功能 |
|------|------|
| `lib.rs` | Tauri IPC 命令 + IPC 类型定义 |
| `geometry.rs` | 多边形几何共享类型（SurfaceGeometry/PolygonPart/IndexedRing）+ 环向归一化、洞识别、坐标系交换 |
| `shp.rs` | SHP 读写（shapefile crate）、DBF 解析（dbase crate 失败时 catch_unwind + 手动二进制回退，编码探测 .cpg/GBK）、PRJ 坐标系识别 |
| `txt.rs` | TXT 三段式格式解析与生成 |
| `gdb.rs` + `gdb/gdb_templates.rs` | GDB 读取（geonative-filegdb）+ 模板化最小 OpenFileGDB 写入 |
| `convert.rs` | 转换编排：SHP/GDB→TXT（三模式：一对一/按地块拆分/全合并）、TXT→SHP（一对一/合并） |

### 输出模式（面→TXT）
- **一对一 (`one_to_one`)**: 每个导入源（SHP 文件 / GDB 要素类）输出一个 TXT。同名冲突自动追加 `_2/_3`
- **按地块拆分 (`split_by_plot`)**: 按源建子目录 `output_dir/{source_stem}/`，内部每个 feature 一个 TXT。文件名可选 DKMC/DKBH/序号/FID；字段缺失自动用序号兜底，重名追加序号，非法字符替换为 `_`
- **全合并 (`merge_all`)**: 所有源所有地块合并为 `merged_output_YYYYMMDD_HHMMSS.txt`（本地时间秒级时间戳）

### 转换选项（面→TXT，`ShpToTxtOptions` in convert.rs）
- `ox` XY 坐标标反 / `oj` 点号前加"J" / `on` 起始点西北角 / `oo` 首末点重合（勾上才在每个环末尾输出闭合点）/ `oc` 闭合点编号模式（false=回到环首点 默认，true=续编；前端下拉，`oo` 未勾时置灰）
- `output_mode`（一对一/按地块拆分/全合并）、`filename_field`（拆分模式文件名字段）
- 前端三处同步：`getOptions()` 收集（[src/main.js](src/main.js)）、`applyPreset` 恢复、`PP` 预设 `p` 对象存储。**新增选项必须三处都加 + PP 默认值**，否则预设保存/恢复丢失

### Tauri IPC 命令

文件选择：`pick_shp_files`、`import_gdb`、`pick_txt_files`、`pick_output_dir`
拖放导入：`pick_shp_files_from_paths`、`pick_txt_files_from_paths`
预览：`read_shp_to_txt_preview`、`read_txt_preview`
转换：`run_shp_to_txt`、`run_txt_to_shp`
窗口控制（无边框标题栏必需）：`minimize_window`、`close_window`

## 项目目录结构

```
├─ index.html              ← Vite 入口（引用 src/main.js）
├─ package.json / vite.config.js
├─ CLAUDE.md / AGENTS.md   ← Claude / Qoder 指导文件
│
├─ content/                ← Markdown 弹窗内容 + 图片资源 + modal-config.json（弹窗尺寸/字体，热更新）
│   ├─ about.md
│   ├─ sponsor.md
│   ├─ 关注、赞赏码.png
│   └─ 讨论群.jpg
├─ src/                    ← 前端源码
│   └─ main.js
├─ src-tauri/              ← Rust 后端
│   ├─ src/                ←   lib/geometry/shp/txt/gdb/convert
│   ├─ tests/              ←   integration_test / debug_output_test
│   ├─ templates/          ←   GDB 写入模板二进制
│   └─ capabilities/       ←   Tauri 权限声明
│
├─ scripts/                ← 发版 + 验证脚本（build-signed.ps1 / gen-latest-json.js / check_ogr / compare_gdb / test_arcpy 等）
├─ latest.json             ← 自动更新清单（jsDelivr 端点从仓库根取此文件，发版时生成更新）
├─ .cargo/config.toml      ← 国内 crates 镜像（rsproxy）
├─ .claude/skills/         ← Claude skills（release：发版流程）
├─ docs/                   ← 设计文档 + screenshots/
├─ versions/               ← 历史 UI 原型 (v7/v8/v9) + mockups
├─ _archive/               ← 逆向工程资料（tbx 解码、分析脚本）
│
├─ test_arcpy/             ← 测试数据：ArcPy 生成的标准 SHP/TXT/GDB
├─ test_data/              ← 测试数据：政府格式 SHP + TXT
└─ 00测试数据/              ← 测试数据：实际业务 TXT + 转换产物
```

图片资源放在 `content/` 中，`about.md`/`sponsor.md` 以 `content/xxx` 相对路径引用，由 `renderMarkdown()` 渲染为 `<img>` 标签，浏览器从页面根（dist/）发起请求。

## 关键注意事项

### 坐标顺序交换
SHP 存储 (X, Y) = (东坐标, 北坐标)。TXT 存储 (Y, X) = (北坐标, 东坐标)。转换层负责交换。

### TXT 格式
- 坐标行：`J{序号},{界址线号},Y坐标,X坐标`（Y 在前）。第二列"界址线号"= `IndexedRing.part_index`（外环=1、洞=2、多部件下一 part=3…逐环递增），是反向解析 TXT→SHP 切环的唯一依据，**严禁删除或重算**
- J 序号在**单个地块内跨环连续递增**，每个地块从 J1 起（含 merge_all）。闭合点（首末点重合的末点，仅 `oo=true` 时存在）写法由 `oc` 选项决定：`oc=false`（默认"回到环首点"）→ 写本环首点序号、不占号；`oc=true`（"续编"）→ 占下一个连续序号
- 解析侧（`txt.rs` `parse_txt`）**只读第二列 part_index 切环，完全忽略第一列序号**——J 编号算法变化不影响反向解析
- 地块元数据行以 `,@` 结尾
- 坐标系字符串必须精确匹配：`2000国家大地坐标系`、`1980西安坐标系`、`1954北京坐标系`、`WGS84坐标系`

### DBF 写入
手动二进制写入（未使用 dbase crate API）。字段偏移量必须为 4 字节（LE），不是 2 字节。

### CSP（tauri.conf.json）
必须包含 `script-src 'self' 'unsafe-inline' 'unsafe-eval'`，否则 WebView2 阻止内联脚本。

### 不使用 arcpy
生产代码中严禁引入 arcpy 依赖。允许用 arcpy 做验证和测试对比。

### 前端构建特殊行为
`@tauri-apps/api` 在 Vite 生产构建中不会被包含在输出 JS 内。ES module `import` 由 Vite 在构建时解析，运行时通过 `window.__TAURI__` 调用。

### 权限（capabilities/default.json）
需要：`core:default`、`dialog:default/open/save`、`fs:default/read/write/exists/mkdir/remove/rename/stat`、`shell:allow-open`、`updater:default`、`process:allow-restart/exit`

### 自动更新（v1.3+）
应用启动时通过 Tauri Updater 静默检查更新，标题栏常驻三态按钮（idle 刷新图标 / available 绿箭头脉冲 / skipped 灰箭头），检测到新版本自动弹窗，支持「跳过此版本」、24h 节流、下载失败兜底百度云。
- 配置：`src-tauri/tauri.conf.json` 的 `plugins.updater`（双端点 jsDelivr + GitHub）+ `bundle.createUpdaterArtifacts: true`（**必开，否则不生成 .sig**）
- 公钥：`pubkey` 字段（公开信息），私钥 `C:\Users\Administrator\.tauri\bpoint-converter.key`（本机保管，严禁入库）
- 前端逻辑：`src/main.js` 的 `checkAppUpdate` / `setUpdateBtnState` / `skipCurrentVersion` / `doUpdate`
- 百度云兜底链接硬编码在 `main.js` 的 `BAIDU_PAN_URL`（about.md 同步）
- **下载 url 走 GitHub releases 直连**（不用 ghproxy 镜像——1.3.0 实测已失效且无加速）
- **`gen-latest-json.js` 多 `.sig` 陷阱**：脚本扫 `src-tauri/target/release/bundle/nsis/*.sig` 取第一个。若该目录残留旧版本 `.sig`，会误把旧签名嵌入 latest.json（下载 URL 是新版、签名是旧版 → 自动更新验签必然失败）。发版前先删该目录下旧版本的 `*-setup.exe` + `.sig`
- **jsDelivr `@master` 缓存滞后**：push 后 `cdn.jsdelivr.net/.../@master/latest.json` 可能数分钟~更久仍返回旧版本，`purge.jsdelivr.net` 不一定立即生效且有 throttle。updater 第一端点是 jsDelivr、拿到旧 JSON 就不会 fallback 到 GitHub 端点。发版当天必须复验 `@master` 已切到新版本号
- **发版必须**：`scripts/build-signed.ps1`（交互输密码签名构建）+ 删旧 nsis `.sig` 后 `node scripts/gen-latest-json.js --tag vX.Y`（生成 latest.json）+ 提交进仓库根目录并上传 Release。完整流程见 [docs/RELEASE.md](docs/RELEASE.md) 和 `release` skill。

## 已知问题

1. **政府 SHP 格式**：`test_data/` 中部分 `.shp` 使用非标准格式（magic ≠ 9994），标准库无法读取
2. **GDB 写入**：最小化 OpenFileGDB 实现，ArcGIS Pro 兼容性有限。回退方案：`ogr2ogr -f "OpenFileGDB"`
3. **打包方式**：`bundle.targets` 为 `nsis`（不含 MSI/WiX）。若 NSIS 打包失败，`src-tauri/target/release/jisig-bpoint-converter.exe` 仍可直接运行
4. **Google Fonts**：需联网加载 Inter/Noto Sans SC/JetBrains Mono，离线回退系统字体

## 依赖

### 前端（package.json）
`@tauri-apps/api ^2`、`@tauri-apps/plugin-dialog ^2`、`@tauri-apps/plugin-shell ^2`、`@tauri-apps/plugin-updater ^2`、`@tauri-apps/plugin-process ^2`、`vite ^6`、`vite-plugin-singlefile ^2`、`@tauri-apps/cli ^2`

### Rust（Cargo.toml）
`tauri 2`、`tauri-plugin-dialog/fs/shell/updater/process 2`、`shapefile 0.8`、`dbase 0.3`、`geonative-core/filegdb/shapefile 0.2`、`chrono 0.4`、`encoding_rs 0.8`、`geo-types 0.7`、`serde 1`、`serde_json 1`、`tempfile 3`

**GPKG 已移除**（v1.1+）：读取仅 SHP/GDB，输出仅 SHP。`gpkg.rs`/`smoke.rs`/`rusqlite` 依赖已删除。
