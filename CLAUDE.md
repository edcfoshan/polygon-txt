# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

**极思G界址点互转工具** — 测绘与国土行业 GIS 桌面工具，实现面要素（SHP/GDB/GPKG）与标准界址点 TXT 文件的双向转换。Tauri v2 桌面应用（Rust 后端 + Vite/HTML 前端）。

- 仓库：https://github.com/edcfoshan/boundary-point-converter
- 窗口：880×600px，无边框（`decorations: false`），自定义标题栏，支持浅色/暗色主题

## 构建与测试命令

```powershell
npm install                          # 安装前端依赖
npm run tauri dev                    # 开发模式（Vite HMR :1420 + Tauri WebView）
npm run tauri build                  # 生产构建 → src-tauri/target/release/jisig-bpoint-converter.exe
npm run build                        # 仅前端（dist/ 单文件 HTML）

cd src-tauri
cargo build --release                # 仅 Rust 编译（不会嵌入前端）
cargo test                           # 全部测试
cargo test --test integration_test   # 集成测试（SHP/DBF/PRJ/TXT/GDB/GPKG）
cargo test --test release_smoke_test # Release 冒烟测试（TXT→GPKG 往返）
cargo test --test debug_output_test  # 调试用：从 TXT 生成 SHP/GDB 输出验证
```

测试数据依赖 `test_arcpy/` 目录（ArcPy 生成的标准 SHP/TXT/GDB）和 `test_data/` 目录。

## 架构

```
index.html (CSS 内联, Google Fonts CDN)
  ← Vite (vite-plugin-singlefile) 打包 JS 内联到单文件 HTML
  ← Tauri WebView 嵌入 dist/index.html
       ↕ window.__TAURI__.core.invoke() IPC
  Rust 后端 (shapefile / geonative-filegdb / rusqlite)
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
| `lib.rs` | 12 个 Tauri IPC 命令 + IPC 类型定义 |
| `shp.rs` | SHP 读写（shapefile crate）、DBF 解析、PRJ 坐标系识别 |
| `txt.rs` | TXT 三段式格式解析与生成 |
| `gdb.rs` + `gdb/gdb_templates.rs` | GDB 读取（geonative-filegdb）+ 模板化最小 OpenFileGDB 写入 |
| `gpkg.rs` | GeoPackage 读写（rusqlite，OGC 标准） |
| `convert.rs` | 转换编排：SHP/GDB/GPKG→TXT、TXT→SHP/GPKG |
| `smoke.rs` | Release 冒烟测试逻辑 |

### Tauri IPC 命令（12 个）

文件选择：`pick_shp_files`、`import_gdb`、`import_gpkg`、`pick_txt_files`、`pick_output_dir`
拖放导入：`pick_shp_files_from_paths`、`pick_txt_files_from_paths`
预览：`read_shp_to_txt_preview`、`read_txt_preview`
转换：`run_shp_to_txt`、`run_txt_to_shp`

## 项目目录结构

```
├─ index.html              ← Vite 入口（引用 src/main.js）
├─ package.json / vite.config.js
├─ CLAUDE.md / AGENTS.md   ← Claude / Qoder 指导文件
│
├─ content/                ← Markdown 弹窗内容（?raw 导入，热更新）
│   ├─ about.md
│   └─ sponsor.md
├─ src/                    ← 前端源码
│   └─ main.js
├─ src-tauri/              ← Rust 后端
│   ├─ src/                ←   lib/shp/txt/gdb/gpkg/convert/smoke
│   ├─ tests/              ←   integration_test / release_smoke_test / debug_output_test
│   ├─ templates/          ←   GDB 写入模板二进制
│   └─ capabilities/       ←   Tauri 权限声明
│
├─ scripts/                ← Python 验证/测试脚本 + PowerShell 发布脚本
├─ docs/                   ← 设计文档 + screenshots/
├─ versions/               ← 历史 UI 原型 (v7/v8/v9) + mockups
├─ _archive/               ← 逆向工程资料（tbx 解码、分析脚本）
│
├─ test_arcpy/             ← 测试数据：ArcPy 生成的标准 SHP/TXT/GDB
├─ test_data/              ← 测试数据：政府格式 SHP + TXT
└─ 00测试数据/              ← 测试数据：实际业务 TXT + 转换产物
```

`关注、赞赏码.png` 留在根目录 — `content/*.md` 中以相对路径引用，运行时从页面根加载。

## 关键注意事项

### 坐标顺序交换
SHP 存储 (X, Y) = (东坐标, 北坐标)。TXT 存储 (Y, X) = (北坐标, 东坐标)。转换层负责交换。

### TXT 格式
- 坐标行：`J序号,1,Y坐标,X坐标`（Y 在前）
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
需要：`core:default`、`dialog:default/open/save`、`fs:default/read/write/exists/mkdir/remove/rename/stat`、`shell:allow-open`

## 已知问题

1. **政府 SHP 格式**：`test_data/` 中部分 `.shp` 使用非标准格式（magic ≠ 9994），标准库无法读取
2. **GDB 写入**：最小化 OpenFileGDB 实现，ArcGIS Pro 兼容性有限。回退方案：`ogr2ogr -f "OpenFileGDB"`
3. **MSI 打包**：WiX 工具可能缺失导致失败，不影响 exe 生成
4. **Google Fonts**：需联网加载 Inter/Noto Sans SC/JetBrains Mono，离线回退系统字体

## 依赖

### 前端（package.json）
`@tauri-apps/api ^2`、`@tauri-apps/plugin-dialog ^2`、`@tauri-apps/plugin-shell ^2`、`vite ^6`、`vite-plugin-singlefile ^2`、`@tauri-apps/cli ^2`

### Rust（Cargo.toml）
`tauri 2`、`tauri-plugin-dialog/fs/shell 2`、`shapefile 0.8`、`dbase 0.3`、`geonative-core/filegdb/shapefile 0.2`、`rusqlite 0.31`（bundled）、`encoding_rs 0.8`、`geo-types 0.7`、`serde 1`、`tempfile 3`
