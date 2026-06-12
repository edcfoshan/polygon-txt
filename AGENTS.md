# AGENTS.md

This file provides guidance to Qoder (qoder.com) when working with code in this repository.

## Project Overview

**极思G界址点互转工具** — GIS utility for bidirectional conversion between polygon features (SHP/GDB/GPKG) and standard boundary-point TXT files. Tauri v2 desktop app (Rust backend + Vite/HTML frontend).

**GitHub:** https://github.com/edcfoshan/boundary-point-converter

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
cargo test --test integration_test          # 10 integration tests (SHP/DBF/PRJ/TXT/GDB round-trips)
cargo test --test debug_output_test         # Debug: generate SHP/GDB output from test TXT
cargo test                                   # All tests
```

**Test data:** `test_arcpy/std_shp/` (5 ArcPy-generated SHP sets), `test_arcpy/txt_output/` (5 TXT), `test_arcpy/test.gdb/`, `test_data/` (government-format SHP + TXT). Integration tests require `test_arcpy/` directory to exist.

## File Structure

```
index.html            ← Entry HTML (all CSS inline, Google Fonts CDN)
package.json          ← npm deps
vite.config.js        ← Vite config (vite-plugin-singlefile, port 1420)
src/
  main.js             ← Frontend JS (614 lines, all Tauri IPC + UI logic)
src-tauri/
  Cargo.toml          ← Rust deps
  tauri.conf.json     ← Window/CSP/bundle config
  capabilities/
    default.json      ← Tauri permissions
  tests/
    integration_test.rs   ← 10 integration tests
    debug_output_test.rs  ← Debug output generation tests
  src/
    lib.rs            ← 11 Tauri IPC commands + serde types
    main.rs           ← Entry point
    shp.rs            ← SHP/DBF/PRJ read/write
    txt.rs            ← TXT 3-section parse/generate
    gdb.rs            ← GDB read (geonative-filegdb) + minimal write
    gdb/
      gdb_templates.rs  ← GDB template binary data for minimal writer
    gpkg.rs           ← GeoPackage read/write (rusqlite, OGC standard)
    convert.rs        ← Conversion orchestration
```

## Key Gotchas

### Frontend JS (src/main.js)
- Uses ES module `import` statements (`import { invoke } from '@tauri-apps/api/core'`). Vite inlines these into the single HTML file during build. In production, `window.__TAURI__` is the runtime API — the imports are resolved at build time by Vite, not at runtime.
- Functions exported to `window.*` for HTML `onclick` handlers (no framework, vanilla JS).
- Uses `@tauri-apps/plugin-shell` for `shellOpen` (opening output folders in Explorer).

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
| GDB    | gdb.rs + gdb/gdb_templates.rs | geonative-filegdb | Minimal OpenFileGDB (template-based) | ArcGIS Pro compat limited |
| GPKG   | gpkg.rs | rusqlite (OGC GeoPackage) | rusqlite | SQLite-based vector format |
| TXT    | txt.rs | Custom parser | Custom generator | 3-section boundary point format |

## Known Issues

1. **Government SHP format:** Test data `.shp` files in `test_data/` use proprietary format (magic ≠ 9994). Standard shapefile libraries cannot read them. Only the legacy Delphi EXE handles them.
2. **GDB write:** Minimal OpenFileGDB implementation. May not be compatible with all ArcGIS versions. Fallback: `ogr2ogr -f "OpenFileGDB" output.gdb input.shp`.
3. **MSI bundling:** WiX tool may fail on some Windows configs. The `.exe` output is unaffected.
4. **Google Fonts:** `index.html` loads Inter/Noto Sans SC/JetBrains Mono from `fonts.googleapis.com`. Offline builds may fall back to system fonts.
