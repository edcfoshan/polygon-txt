# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with this repository.

## Project Overview

**极思G界址点互转工具 (Boundary Point Conversion Tool)** — A GIS utility for bidirectional conversion between polygon features (SHP/GDB) and standard boundary-point TXT files. Built as a Tauri v2 desktop application.

**Active build:** `极思G界址点互转工具.exe` (12.2 MB, pure Rust + Tauri WebView)
**GitHub:** https://github.com/edcfoshan/boundary-point-converter

## Architecture

```
index.html (CSS inline) ← Vite build → Tauri WebView
         ↕ window.__TAURI__.core.invoke() IPC
   Rust backend (shapefile + geonative-filegdb crates)
         ↕ std::fs
   Native filesystem
```

### Dual-Mode Layout
The `.app` container has `data-mode="s"` (面→TXT) or `data-mode="t"` (TXT→面). CSS toggles panels.

- **面→TXT (3 columns, 260+260+360):** Left: SHP/GDB import + field mapping. Middle: options + header editor. Right: TXT preview + run button.
- **TXT→面 (2 columns, 300+flex):** Left: TXT import + file list. Right: parse log + run button. Middle panel hidden.

## File Structure
```
│  index.html           ← Entry HTML (all CSS inline, JS inlined by Vite)
│  package.json         ← npm deps (@tauri-apps/api, vite, @tauri-apps/cli)
│  vite.config.js       ← Vite config
│  ./src/
│  └─ main.js           ← Frontend JS (Tauri IPC via window.__TAURI__)
│  ./src-tauri/
│  ├─ Cargo.toml        ← Rust deps (tauri 2, shapefile 0.8, geonative-filegdb 0.2)
│  ├─ tauri.conf.json   ← Window config, CSP, bundle settings
│  ├─ capabilities/
│  │  └─ default.json   ← Permissions (dialog, fs)
│  ├─ tests/
│  │  └─ integration_test.rs  ← 10 integration tests
│  ├─ icons/
│  └─ src/
│     ├─ lib.rs         ← Tauri commands (8 IPC handlers)
│     ├─ main.rs        ← Entry point
│     ├─ shp.rs         ← SHP/DBF/PRJ read/write
│     ├─ txt.rs         ← TXT parse/generate
│     ├─ gdb.rs         ← GDB read (geonative-filegdb) + minimal write
│     └─ convert.rs     ← Conversion orchestration
```

## Key Rust Modules

### shp.rs
- `read_shp(path)` — Read SHP features (Polygon/Point/Polyline) using `shapefile::ShapeReader`
- `read_dbf(path)` — Parse .dbf using `dbase::read()`, returns field names + records
- `read_prj(path)` — Parse WKT text, extract CRS info (CGCS2000/Xian80/Beijing54/WGS84 + Gauss Kruger)
- `read_shp_file_group(path)` — Validate SHP header (magic 9994), read companions, return `ShpFileInfo`
- `write_shapefile(out_dir, stem, geometries, attributes, crs, zone)` — Write .shp + .shx + .dbf + .prj
- `write_prj(path, crs, zone)` — Generate WKT file
- `write_dbf_manual(path, attributes)` — Bare-metal DBF binary writer (avoids `dbase` crate's complex API)

### txt.rs
- `parse_txt(text)` — Parse 3-section TXT format → `TxtParseResult { project_info, attrs, plots }`
- `generate_txt(project_info, attrs, features)` — Generate TXT content

### gdb.rs
- `read_gdb(path)` — Open `.gdb` folder via `geonative_filegdb::open()`, read all feature classes
- `write_gdb_output(...)` — Minimal OpenFileGDB writer (creates .gdbtable + .gdbtablx files)
- Coordinate extraction from `geonative_core::Geometry` tree (Polygon → exterior.coords + holes)

### convert.rs (orchestration)
- `FieldMapping` — Maps DBF field names → TXT columns (name, id, area, use_field, tfh, dlbm)
- `HeaderConfig` — CRS settings + project info text
- `ShpToTxtOptions` — ox/oj/op/on/oo/om flags
- `TxtToShpOptions` — output_shp, output_gdb, merge, output_dir
- `convert_shp_to_txt()` / `convert_txt_to_shp()` — Main conversion functions
- `shp_to_txt_preview()` — Preview generation (first 200 lines)
- Coordinate swapping: SHP stores (X, Y) = (easting, northing), TXT stores (Y, X) = (northing, easting)

### lib.rs (Tauri commands)
8 IPC commands exported with `#[tauri::command]`:
1. `pick_shp_files` → `ShpImportResult`
2. `import_gdb` → `GdbImportResult`
3. `pick_txt_files` → `TxtImportResult`
4. `pick_output_dir` → `Option<String>`
5. `read_shp_to_txt_preview` → preview string
6. `read_txt_preview` → parse log string
7. `run_shp_to_txt` → `ConvertResultPayload`
8. `run_txt_to_shp` → `ConvertResultPayload`

## Frontend JS (src/main.js)

- **No import statements** — Uses `window.__TAURI__?.core?.invoke()` directly (Vite doesn't bundle `@tauri-apps/api` in production)
- `tauriInvoke(cmd, args)` — Async wrapper for IPC calls
- Functions exported to `window.*` for HTML `onclick` handlers
- State management: `loadedFiles[]`, `txtFiles[]`, `cfgs`, `headerManual`
- Theme: `localStorage.tg_theme`, toggled via `togTheme()`
- Presets: 3 built-in (`PP`) + custom (`localStorage.tg_dark`)

## Tauri Config Notes

### CSP (tauri.conf.json)
**Critical**: Must include `script-src 'self' 'unsafe-inline' 'unsafe-eval'` or WebView2 will block the inline `<script>` tag.

### Permissions (capabilities/default.json)
```json
{
  "permissions": [
    "core:default",
    "dialog:default", "dialog:allow-open", "dialog:allow-save",
    "fs:default", "fs:allow-read", "fs:allow-write",
    "fs:allow-exists", "fs:allow-mkdir", "fs:allow-remove",
    "fs:allow-rename", "fs:allow-stat"
  ]
}
```

## Build & Run

```powershell
# First time setup
npm install

# Development (hot reload)
npm run tauri dev

# Production build
npm run tauri build

# Run tests
cd src-tauri
cargo test --test integration_test
```

The release binary is at `src-tauri/target/release/jisig-bpoint-converter.exe`.

## TXT Format Rules
- 坐标行格式：`J序号,1,Y坐标,X坐标`
- Y坐标在前（北坐标），X坐标在后（东坐标）
- 地块元数据行以 `,@` 结尾
- 坐标系字符串必须精确匹配：`2000国家大地坐标系`、`1980西安坐标系`、`1954北京坐标系`、`WGS84坐标系`

## Known Issues & Constraints

1. **Government SHP format**: Test data `.shp` files use a proprietary format (magic ≠ 9994). Cannot be read by standard shapefile libraries or ArcPy. The legacy Delphi EXE (`shp转txt.exe`) has its own parser.
2. **GDB write**: Minimal implementation may not produce GDB files compatible with all ArcGIS versions. If issues arise, use `ogr2ogr -f "OpenFileGDB" output.gdb input.shp` as fallback.
3. **MSI bundling**: Fails on some Windows configurations (WiX tool issue). The release `.exe` is still produced correctly.
4. **Vite + Tauri API**: `@tauri-apps/api` is not bundled in production builds by Vite. Frontend uses `window.__TAURI__` global instead.
5. **DBF encoding**: Manually written DBF files use binary format directly. Field offset must be 4 bytes (LE), not 2 bytes.
