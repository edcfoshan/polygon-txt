# CLAUDE.md

## 项目概述

**极思G界址点互转工具 (Boundary Point Conversion Tool)** — 测绘与国土行业的 GIS 实用工具，实现 SHP 文件 / File Geodatabase (.gdb) 与标准界址点 TXT 文件的双向转换。

- 前端：HTML + CSS + Vanilla JS（880×600px 窗口，支持浅色/暗色主题切换）
- 后端：Rust（Tauri v2 桌面壳）
- 仓库：https://github.com/edcfoshan/boundary-point-converter

## 架构

```
index.html (CSS内联) ← Vite构建 → Tauri WebView
         ↕ window.__TAURI__.core.invoke() IPC
   Rust 后端 (shapefile + geonative-filegdb crate)
         ↕ std::fs
   原生文件系统
```

### 双模式切换
- `data-mode="s"` — 面→TXT（导入 SHP/GDB，输出 TXT）
- `data-mode="t"` — TXT→面（导入 TXT，输出 SHP/GDB）

面→TXT 模式布局：3 列面板（260+260+360）
TXT→面模式布局：2 列面板（300+flex，中间面板隐藏）

### TXT 标准格式
```
[项目信息]          ← 可选
项目名称=xxx
...
[属性描述]          ← 固定字段
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]          ← 元数据行 + 坐标行
6,1.2247,FID_0,DKMC,面,TFH,DKYT,DLBM,@
J1,1,Y坐标,X坐标
...
```

### Rust 后端模块
| 模块 | 文件 | 功能 |
|------|------|------|
| shp | `src-tauri/src/shp.rs` | SHP 读写 (shapefile crate)、DBF 解析、PRJ 坐标系识别 |
| txt | `src-tauri/src/txt.rs` | TXT 格式解析与生成 |
| gdb | `src-tauri/src/gdb.rs` | GDB 读取 (geonative-filegdb)、最小化 OpenFileGDB 写入 |
| convert | `src-tauri/src/convert.rs` | 转换编排：SHP↔TXT、GDB→TXT、TXT→SHP/GDB |
| lib | `src-tauri/src/lib.rs` | Tauri 命令注册（8 个 IPC 命令）|

### Tauri IPC 命令
- `pick_shp_files` — 选择 SHP 文件，返回字段列表和坐标系信息
- `import_gdb` — 选择 GDB 文件夹，读取所有要素类
- `pick_txt_files` — 选择 TXT 文件，解析并返回摘要
- `pick_output_dir` — 选择输出目录
- `read_shp_to_txt_preview` — 生成 TXT 预览
- `read_txt_preview` — 读取 TXT 文件并解析
- `run_shp_to_txt` — 执行 SHP/GDB→TXT 转换
- `run_txt_to_shp` — 执行 TXT→SHP/GDB 转换

### 前端文件
`src/main.js` — 包含所有 UI 逻辑和 Tauri IPC 调用（使用 `window.__TAURI__` 运行时 API，Vite 构建时不打包是已知行为）

## 开发环境要求

### 必需
- **Rust** >= 1.74（`rustup` 安装）
- **Node.js** >= 18（含 npm）
- **Windows 10+**（WebView2 运行时，Win10 1809+ 自带）

### 前端依赖
```json
{
  "@tauri-apps/api": "^2",       // Tauri IPC
  "@tauri-apps/plugin-dialog": "^2",  // 原生文件对话框
  "@tauri-apps/plugin-fs": "^2",
  "vite": "^6",                    // 前端构建
  "@tauri-apps/cli": "^2"          // Tauri CLI
}
```

### Rust 依赖（Cargo）
- `tauri = "2"` — Tauri 框架
- `tauri-plugin-dialog = "2"` — 对话框
- `tauri-plugin-fs = "2"` — 文件系统
- `shapefile = "0.8"` — SHP 读写
- `dbase = "0.3"` — DBF 读写
- `geonative-core = "0.2"` — 地理空间数据模型
- `geonative-filegdb = "0.2"` — 纯 Rust GDB 读取
- `serde = "1"` — 序列化
- `tempfile = "3"` — 临时文件

## 构建与运行

### 开发模式（热更新）
```powershell
cd 项目目录
npm install
npm run tauri dev
```
这会启动 Vite 开发服务器（localhost:1420）和 Tauri WebView 窗口。

### 生产构建
```powershell
npm run tauri build
```
输出路径：`src-tauri/target/release/jisig-bpoint-converter.exe`

构建后手动重命名：
```powershell
Copy-Item src-tauri/target/release/jisig-bpoint-converter.exe 极思G界址点互转工具.exe
```

### 仅构建前端
```powershell
npm run build
```
输出目录：`dist/`（Vite 将 HTML + 内联 JS 打包至此）

### 仅编译 Rust（不运行 tauri-codegen）
```powershell
cd src-tauri
cargo build --release
```
**注意**：这样不会重新嵌入前端文件，仅用于 Rust 编译测试。

### 运行测试
```powershell
cd src-tauri
cargo test --test integration_test
```
10 个集成测试覆盖：SHP 读取、DBF 解析、PRJ 识别、TXT 解析/生成、转换流程、GDB 读取。

## 项目文件结构
```
│  index.html                ← 入口 HTML（含全部 CSS 样式，JS 内联）
│  index.html          ← Vite 入口（引用 /src/main.js）
│  AGENTS.md                 ← Codex 指导
│  CLAUDE.md                 ← Claude 指导（本文档）
│  package.json              ← npm 配置
│  vite.config.js            ← Vite 配置
│  .gitignore
├─ src/
│  └─ main.js                ← 前端 JS（Tauri IPC 调用）
├─ dist/                     ← Vite 构建输出（gitignored）
├─ node_modules/             ← npm 依赖（gitignored）
├─ src-tauri/
│  ├─ Cargo.toml             ← Rust 依赖配置
│  ├─ tauri.conf.json        ← Tauri 配置（窗口、CSP、打包）
│  ├─ capabilities/
│  │  └─ default.json        ← Tauri 权限声明
│  ├─ tests/
│  │  └─ integration_test.rs ← 集成测试（10 个测试用例）
│  ├─ icons/                 ← 应用图标
│  ├─ target/                ← Rust 编译输出（gitignored）
│  └─ src/
│     ├─ lib.rs              ← Tauri 命令入口
│     ├─ main.rs             ← 程序入口
│     ├─ shp.rs              ← SHP/DBF/PRJ 读写
│     ├─ txt.rs              ← TXT 解析/生成
│     ├─ gdb.rs              ← GDB 读写
│     └─ convert.rs          ← 转换编排
```

## 配置说明

### Tauri 配置（tauri.conf.json）
- `build.frontendDist: "../dist"` — 构建后的前端目录
- `build.beforeBuildCommand: "npm run build"` — 构建前运行 Vite
- `app.security.csp` — 内容安全策略（必须包含 `script-src 'unsafe-inline'`）
- `bundle.windows.nsis.installMode: "currentUser"` — 安装模式
- 窗口大小：880×600，最小 800×540

### 权限（capabilities/default.json）
需要 `dialog:default`、`dialog:allow-open`、`dialog:allow-save`、`fs:default` 等权限。

## 已知问题

### CSP 策略
内联脚本需要在 CSP 中声明 `script-src 'unsafe-inline' 'unsafe-eval'`，否则 WebView2 会阻止 JS 执行。

### 政府 SHP 格式兼容性
测试数据中的 `.shp` 文件采用非标准格式（magic number ≠ 9994），无法被标准 shapefile 库（包括 ArcPy）读取。原始的 Delphi `shp转txt.exe` 使用的是自定义格式解析器。当前工具仅支持标准 ESRI Shapefile 格式和 File Geodatabase (.gdb)。

### MSI 打包
`npm run tauri build` 的最后一步（WiX 打包 MSI）可能会因缺少 WiX 工具而失败，但这不影响主 exe 的生成。

### GDB 写入
当前 GDB 写入（TXT→GDB）使用的是最小化 OpenFileGDB 格式实现。如果需要完全兼容的 GDB 输出，可以：
1. 使用 `ogr2ogr -f "OpenFileGDB" output.gdb input.shp`
2. 或等待 geonative 生态的 GDB 写入支持

### 前端构建注意事项
`@tauri-apps/api` 包在 Vite 生产构建中不会被打包到 JS 文件内。前端代码使用 `window.__TAURI__` 运行时 API 来调用 Tauri IPC。开发模式下通过 `npm run tauri dev` 使用 Vite dev server 正常工作。

## 测试数据
测试数据位于开发者机器的 `D:\00结束\本地肇庆高新区数据治理\05开始录入\2所有都是0错误`，包含：
- `shp/` — 60 个非标准格式 SHP 文件  
- `肇庆高新区txt/` — 121 个标准 TXT 文件

标准测试数据可由 ArcPy 生成：
```powershell
& "C:\Program Files\ArcGIS\Pro\bin\Python\envs\arcgispro-py3\python.exe" test_arcpy_gen.py
```
