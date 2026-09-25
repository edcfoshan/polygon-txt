# AGENTS.md

This file provides guidance to Qoder (qoder.com) when working with code in this repository.

## Project Overview

**极思G界址点互转工具** — GIS utility for bidirectional conversion between polygon features (SHP/GDB) and standard boundary-point TXT files. Tauri v2 desktop app (Rust backend + Vite/HTML frontend).

**GitHub:** https://github.com/edcfoshan/polygon-txt

## Core Rules

### No arcpy in production code
**arcpy is FORBIDDEN in the software's build/code.** This tool's purpose is to eliminate arcpy dependency. 

**Allowed:**
- Using arcpy for verification, testing, and debugging (演算和校正)
- Comparing with arcpy-generated output to validate format correctness

**Forbidden:**
- Using arcpy in the compiled application
- Shelling out to arcpy from Rust code
- Bundling any arcpy dependency
- Using arcpy as a runtime dependency

### GDB ArcGIS Pro Compatibility
Pure Rust OpenFileGDB writer (gdb.rs) cannot fully match ArcGIS Pro's binary format. Recommended workflow for Pro users:
1. Export as SHP → Open in ArcGIS Pro
2. Convert SHP → GDB using ArcGIS Pro's built-in tools

The Rust GDB writer is compatible with `geonative-filegdb` for read-back verification.

## Architecture

```
index.html (CSS inline, Google Fonts loaded from CDN)
  → Vite (vite-plugin-singlefile) bundles JS inline
    → Tauri WebView
      ↕ window.__TAURI__.core.invoke() IPC
  Rust backend (shapefile + geonative-filegdb crates)
    ↕ std::fs
  Native filesystem
```

Key: Vite inlines all JS into a single HTML file. The built `dist/index.html` contains everything. Tauri embeds this at build time via `tauri-codegen`.

### Dual-Mode Layout
`data-mode="s"` (面→TXT, 3 columns: 260+260+360) vs `data-mode="t"` (TXT→面, 2 columns: 300+flex). CSS toggles panels.

**v4.3 重排**：左栏 = ①导入数据 + ②输出与选项（两卡固定展开，无折叠箭头）；中栏 = 页签切换（投影 / 表头 / 字段 / 界址点，默认「字段」），`.mid-tabs`/`.mid-pane` 驱动；右栏 = 预览（TXT 预览/属性表/地图）不变。

Layout is responsive: `.main` uses CSS grid with `minmax(lower-bound, fr-ratio)`. Columns scale proportionally when the window is resized; lower bounds prevent field overflow at `minWidth:800`.

## Build & Run

```powershell
npm install                  # First time only
npm run tauri dev            # Dev (Vite HMR on :1420 + Tauri WebView)
npm run tauri build          # Production → src-tauri/target/release/jisig-bpoint-converter.exe
npm run build                # Vite only (dist/)
cd src-tauri; cargo build --release  # Rust only (no frontend embed, for compile checks)
```

### Tests
```powershell
cd src-tauri
cargo test --test integration_test          # 30 integration tests (SHP/DBF/PRJ/TXT/GDB round-trips + 三模式输出)
cargo test --test debug_output_test         # Debug: generate SHP/GDB output from test TXT
cargo test --test bubeian_txt_test          # 界址点布局 TXT（标准4列/部备案6列/自定义列序）
cargo test                                   # All tests
```

**Test data:** `test_arcpy/std_shp/` (5 ArcPy-generated SHP sets), `test_arcpy/txt_output/` (5 TXT), `test_arcpy/test.gdb/`, `test_data/` (government-format SHP + TXT). Integration tests require `test_arcpy/` directory to exist.

## File Structure

```
index.html            ← Entry HTML (all CSS inline, Google Fonts CDN)
package.json          ← npm deps
vite.config.js        ← Vite config (vite-plugin-singlefile, port 1420)
content/
  about.md            ← 关于弹窗内容（Markdown，热更新）
  sponsor.md          ← 赞助弹窗内容（Markdown，热更新）
src/
  main.js             ← Frontend JS (all Tauri IPC + UI logic)
  assets/
    brand-mark.png    ← 品牌标记（单色 RGBA，main.js 经 ?inline 内联为 data URL + CSS mask 上色，随色系走 --ac）
src-tauri/
  Cargo.toml          ← Rust deps
  tauri.conf.json     ← Window/CSP/bundle config
  capabilities/
    default.json      ← Tauri permissions
  tests/
    integration_test.rs   ← 30 integration tests（SHP/DBF/PRJ/TXT/GDB 往返 + 三模式输出）
    debug_output_test.rs  ← Debug output generation tests
    bubeian_txt_test.rs   ← 界址点布局 TXT（标准4列/部备案6列/自定义列序/距离/埋桩）
  src/
    lib.rs            ← Tauri IPC commands + serde types
    main.rs           ← Entry point
    shp.rs            ← SHP/DBF/PRJ read/write
    txt.rs            ← TXT 3-section parse/generate
    gdb.rs            ← GDB read (geonative-filegdb) + minimal write
    gdb/
      gdb_templates.rs  ← GDB template binary data for minimal writer
    convert.rs        ← Conversion orchestration（三模式输出：一对一/按地块拆分/全合并）
```

### Output Modes (面→TXT)
- **一对一 (`one_to_one`)**: 每个导入源（SHP 文件 / GDB 要素类）输出一个 TXT。同名冲突自动追加 `_2/_3`
- **按地块拆分 (`split_by_plot`)**: 按源建子目录 `output_dir/{source_stem}/`，内部每个 feature 一个 TXT。文件名可选 DKMC/DKBH/序号/FID；字段缺失自动用序号兜底，重名追加序号，非法字符替换为 `_`
- **全合并 (`merge_all`)**: 所有源所有地块合并为 `merged_output_YYYYMMDD_HHMMSS.txt`（本地时间秒级时间戳）
- **界址点布局 (`options.point_layout`)**: 中栏「界址点」配置有序坐标行；核心列为点号/环号/Y/X，可新增固定值、源字段、点距离列并拖动排序。每个点距离列支持独立单位 米/千米/厘米 与小数位 0~6；闭合点（与首点重合）距离为 0，开口环末点距离=到首点闭合边长。预览区下方统一导出，复用上述三种输出模式与全部转换选项。测试 `cargo test --test bubeian_txt_test`

## Key Gotchas

### Frontend JS (src/main.js)
- Uses ES module `import` statements (`import { invoke } from '@tauri-apps/api/core'`). Vite inlines these into the single HTML file during build. In production, `window.__TAURI__` is the runtime API — the imports are resolved at build time by Vite, not at runtime.
- Functions exported to `window.*` for HTML `onclick` handlers (no framework, vanilla JS).
- Uses `@tauri-apps/plugin-shell` for `shellOpen` (opening output folders in Explorer).

### Markdown-Driven Modals (content/)
About 和赞助弹窗的内容托管在 `content/about.md` 和 `content/sponsor.md`，通过 `?raw` 导入在 `main.js` 中渲染。
- 编辑 `.md` 文件后保存，Vite 热更新即时生效
- `npm run build` 构建时自动内联进单文件 HTML
- 支持的语法：`###`标题、`**加粗**`、`- 列表`、`[链接](url)`、`![图片](src)`、`---`分隔线
- 图片路径相对于项目根目录（如 `关注、赞赏码.png`）
- 渲染函数 `renderMarkdown()` 位于 `main.js` 中，处理弹窗专用的行内样式

### CSP (tauri.conf.json)
**Critical:** Must include `script-src 'self' 'unsafe-inline' 'unsafe-eval'` or WebView2 blocks inline `<script>`.

### Permissions (capabilities/default.json)
Requires: `core:default`, `dialog:default/open/save`, `fs:default/read/write/exists/mkdir/remove/rename/stat`, `shell:allow-open`.

### DBF Writing
Manually written binary (avoids `dbase` crate API). Field offset must be 4 bytes (LE), not 2 bytes.

### App Icons & Brand Assets
- `src-tauri/icons/*`（32x32 / 128x128 / 128x128@2x / icon.icns / icon.ico）由 `npx tauri icon docs/brand/app-icon-source.png` 生成；输入必须是**去白底后的 RGBA PNG**（带白角会导致任务栏出现白方块），生成的多余平台目录（android/ios/Square*Logo）已被删除，只保留 `tauri.conf.json › bundle.icon` 引用到的文件。
- `docs/brand/social-preview-1280x640.png` 需在 GitHub 仓库 Settings › Social preview 手动上传（无 API）。
- 标题栏 22px 与关于弹窗 64px 的标记共用一个单色 PNG：`.brand-mark`/`.about-mark` 用 CSS mask + `background:var(--ac)` 上色，因此 8 套色系与明暗主题下都自动适配（不要改回彩色位图，否则暗色主题下会变成隐形的黑方块）。
- 品牌视觉为青绿渐变底（左上 #047887 → 右下 #067298）+ 米白「开口界址点环双追箭」图形，与应用内青色系主题同族。`docs/brand/app-icon-source-doubao-original.jpeg` 是未处理原始稿（带水印、全出血方角）；`app-icon-source.png`（去水印 + RGBA 圆角）、单色 `src/assets/brand-mark.png`（亮度 smoothstep 取 alpha）、`social-preview-1280x640.png`（渐变底 + 左图形 + 右产品名自动字号）均由 `docs/brand/make-brand-assets.ps1` 一键派生——换源图后先跑该脚本再跑 `npx tauri icon`。
- **Release 资产命名**：前缀一律 `极思G界址点互转工具_X.X.0_`（如 `极思G界址点互转工具_4.4.0_x64-setup.exe`），配套 `.sig` 与 `latest.json`（updater 依赖）；v4.3.0 时代的 `polygon-txt_4.3_*` 命名已废弃。暂存目录为根目录 `其他相关tbx放进去release/`。
- **图标嵌入两个坑**：① NSIS 安装包图标历史上回退 NSIS 默认图标（v4.3.0 的安装包就是），现由 `bundle.windows.nsis.installerIcon: icons/icon.ico` 显式指定；② 换图标后 exe 仍嵌旧图 = tauri build.rs 的资源编译被 cargo 缓存，改 `tauri.conf.json`（或 cargo clean -p 本 crate）触发重跑即可。

### Coordinate Swapping
SHP stores (X, Y) = (easting, northing). TXT stores (Y, X) = (northing, easting). The conversion layer swaps these.

## TXT Format Rules

- 坐标行：`J序号,1,Y坐标,X坐标` — Y (northing) first, X (easting) second
- 地块元数据行以 `,@` 结尾
- 坐标系字符串必须精确匹配：`2000国家大地坐标系`、`1980西安坐标系`、`1954北京坐标系`、`WGS84坐标系`

## Supported Input Formats

| Format | Module | Read | Write | Notes |
|--------|--------|------|-------|-------|
| SHP    | shp.rs | shapefile crate | shapefile crate + manual DBF | Standard ESRI Shapefile only |
| GDB    | gdb.rs + gdb/gdb_templates.rs | geonative-filegdb | Minimal OpenFileGDB (template-based) | ArcGIS Pro compat limited；多要素类支持 |
| TXT    | txt.rs | Custom parser | Custom generator | 3-section boundary point format |

**GPKG 已移除**（v1.1+）。读取仅支持 SHP/GDB，输出仅 SHP。

## Known Issues

1. **Government SHP format:** Test data `.shp` files in `test_data/` use proprietary format (magic ≠ 9994). Standard shapefile libraries cannot read them. Only the legacy Delphi EXE handles them.
2. **GDB write:** Minimal OpenFileGDB implementation. May not be compatible with all ArcGIS versions. Fallback: `ogr2ogr -f "OpenFileGDB" output.gdb input.shp`.
3. **MSI bundling:** WiX tool may fail on some Windows configs. The `.exe` output is unaffected.
4. **Google Fonts:** `index.html` loads Inter/Noto Sans SC/JetBrains Mono from `fonts.googleapis.com`. Offline builds may fall back to system fonts.
