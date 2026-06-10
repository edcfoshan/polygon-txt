# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

**界址点互转工具 (Boundary Point Conversion Tool)** — A GIS utility for bidirectional conversion between polygon features (SHP/GDB) and standard boundary-point TXT files. Legacy Delphi EXEs are being replaced by a single-file web app.

**Active file:** `prototype_v7.html` (880×600px, 浅色默认, 支持暗色切换)

## Architecture

Single HTML file — no build step, no framework, no npm. Pure HTML/CSS/JS. Serve directly via file:// or any static server.

### Dual-Mode, 3→2 Column Layout

The `.app` container has `data-mode="s"` (面→TXT) or `data-mode="t"` (TXT→面). CSS `.app[data-mode="t"] .mode-s{display:none}` / `.app[data-mode="s"] .mode-t{display:none}` toggles content blocks. The `sw()` function sets `data-mode` on the `.app` div.

**面→TXT mode (3 columns, 220+220+360=800/880px):**
- Left: SHP/GDB import + field mapping (3×2 grid, `.fgrid`) + output directory
- Middle: conversion checkboxes + buffer + header editor (属性描述/项目信息 tabs)
- Right: full-height TXT preview (`.pv`, `flex:1`) + run button

**TXT→面 mode (2 columns, 300+flex):**
- Middle panel hidden (`.app[data-mode="t"] .pnl-m{display:none}`)
- Left (300px): TXT import + file list + clear button
- Right: parse result log + run button

### TXT Format (3 sections)
```
[项目信息]          ← optional, from textarea in header editor
项目名称=xxx
...
[属性描述]          ← from hcfg form fields
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]          ← metadata line + coordinate lines
6,1.2247,FID_0,DKMC,面,TFH,DKYT,DLBM,@
J1,1,Y坐标,X坐标
...
```

### Theme System
CSS custom properties on `:root` (light default) and `[data-t="dark"]`. Two complete token sets. Theme toggle persists to `localStorage.tg_theme`.

### Preset System
3 built-in presets in `PP` array: 基础地块, 规划审批, 自定义. Custom presets stored in `localStorage.tg_dark` key. `cfgs` object merges PP defaults with localStorage overrides. Title bar has editable name span (`contenteditable`) + save + delete buttons. `saveOnly()` auto-detects new vs overwrite. Chip bar in tab bar for quick switching.

### JS Module Map
- **File handling**: `initFileInput()`, `handleFiles()`, `renderFileList()`, `removeFile()`, `clearAllFiles()` — groups by basename into `loadedFiles[]`
- **SHP parsing**: `parseDbfFields()`, `parseShpHeader()`, `prjToHeader()` — reads .dbf field names, validates .shp, extracts spatial ref from WKT
- **Auto-match**: `autoMatchFields()` replaces dropdown options + auto-selects; `autoFillHeader()` fills hcfg from .prj
- **Preview**: `up()` rebuilds full TXT output; `schedulePreview()` debounces at 150ms
- **TXT→面**: `handleTxtFiles()`, `parseTxtPreview()`, `renderTxtParseLog()`, `runTxt()` — parses TXT sections, logs results
- **Config**: `ld()`, `saveOnly()`, `delCfg()`, `renderChips()`, `g()`, `prefillProject()`, `switchHdrTab()`
- **Theme**: `togTheme()` toggles `data-t` on `:root`
- **Tab switch**: `sw()` toggles `data-mode` on `.app`

### Key DOM IDs
- `#dropZone` / `#fl` — SHP import (mode-s)
- `#dropZoneTxt` / `#flT` — TXT import (mode-t)
- `#fn, #fi, #fa, #fu, #fm, #fd` — field mapping selects
- `#hc, #hb, #hj, #hu, #hz, #ha, #ht` — header config inputs
- `#hpi` — project info textarea
- `#pv` / `#pvT` — preview areas
- `#cn` — editable config name
- `#ch` — preset chips

### Global State
- `loadedFiles[]` — `[{shp, dbf, prj, shx, name}]`
- `txtFiles[]` — `[{name, size, plots:[{count, area, name, coords}]}]`
- `cur` — active preset id
- `cfgs` — all config objects
- `headerManual{}` — tracks user-edited header fields (prevents auto-overwrite)
- `theme` — 'light'|'dark'

## Key Constraints
- No build step. No npm. No framework.
- 880×600px fixed window. No body-level scrollbars. Panel bodies use `overflow:hidden` — content must fit.
- Field matching is exact (no fuzzy). Match fails → dropdown shows "无".
- Precision dropdown (`#ha`) controls both TXT output precision and coordinate decimal places.
- `.prj` parsing uses regex on WKT text — handles CGCS2000/Xian80/Beijing54/WGS84.
- SHP files require companion .dbf + .prj in same selection for full auto-detection.
- Actual SHP→TXT and TXT→SHP conversion logic is simulated (progress animation). Real conversion needs ArcPy backend or equivalent.
