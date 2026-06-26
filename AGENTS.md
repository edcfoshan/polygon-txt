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
cargo test --test integration_test          # 17 integration tests (SHP/DBF/PRJ/TXT/GDB round-trips + 三模式输出)
cargo test --test debug_output_test         # Debug: generate SHP/GDB output from test TXT
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
src-tauri/
  Cargo.toml          ← Rust deps
  tauri.conf.json     ← Window/CSP/bundle config
  capabilities/
    default.json      ← Tauri permissions
  tests/
    integration_test.rs   ← 17 integration tests（SHP/DBF/PRJ/TXT/GDB 往返 + 三模式输出）
    debug_output_test.rs  ← Debug output generation tests
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
