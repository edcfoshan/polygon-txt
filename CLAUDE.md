# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**界址点互转工具 (Boundary Point Conversion Tool)** — A GIS utility for bidirectional conversion between polygon features (面, in SHP/GDB format) and standard boundary-point TXT files. The project is redesigning legacy Delphi-based EXE tools as a modern desktop-style web application.

## Codebase Structure

- `prototype_v4.html` ～ `prototype_v6.html` — **Active UI design.** Six iterations of a pure HTML/CSS/JS single-page web app. `v6` is the latest ("测镜" brand, dark theme, brass/amber accents). Older versions (v1-v3) are superseded, not shown in listing but exist as earlier iterations.
- `_reference_project/` — Legacy Delphi-compiled EXEs being replaced:
  - `shp转txt.exe` — SHP→TXT converter (Sep 2023)
  - `坐标转ShapeFile3.0.exe` — Coordinate→ShapeFile converter (Mar 2025)
- `界址点互转工具提取10.8.tbx` — ArcGIS Toolbox containing embedded Python scripts with the actual ArcPy conversion logic (two scripts: `import arcpy`, `GetParameterAsText`, `InsertCursor`, `CreateFeatureclass`).
- `A8SHP与txt互转软件(最终版本）.zip` — Archive of original Delphi tools.
- `analyze_*.py` / `extract_*.py` — Python scripts used during reverse-engineering of the `.tbx` and `.exe` files (not part of the target app).

## Architecture (Web App)

The target app is a single HTML file (`prototype_v6.html`) — no build step, no framework.

### Conversion Directions
- **面 → TXT** — Read polygon features from SHP/GDB, output TXT in the standard boundary-point format
- **TXT → 面** — Parse boundary-point TXT, create polygon features in SHP/GDB

### TXT Format
```
[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]
```

### UI Panels (3-column layout)
| Panel | Contents |
|-------|----------|
| **Left** | File selector (SHP/GDB drag/drop), field mapping (6 fields: 地块名称/编号/面积/用途/图幅号/地类), output directory |
| **Center** | Header preview, conversion results (result tags + log), run button + progress bar |
| **Right** | Coordinate parameters (decimal places, zone number, buffer), conversion options (6 checkboxes), custom header editor |

### Preset System
10 built-in configurations stored as `PP` array: 基础地块, 规划审批, 工程测绘, 农用地, 城市更新, 地籍调查, 生态红线, 临时用地, 分类汇总, 自定义. Persisted to LocalStorage under key `tg_dark`.

### Key JS Objects
- `PP` — Built-in preset definitions (10 presets, each with `id`, `n`, `h`/header, `p`/params, `f`/fields)
- `cfgs` — Runtime configs (localStorage-backed, merges with PP on init)
- `cur` — Currently active preset id
- Functions: `ld()` (load preset), `sv()` (save), `nw()` (new), `ex()` (export JSON), `im()` (import JSON), `g()` (gather current config), `up()` (update preview), `run()` (simulated conversion)

### Field Mapping Configurable Fields
| UI Label | Default | Other Options |
|----------|---------|--------------|
| 地块名称 | DKMC | MC, NAME |
| 地块编号 | DKBH | BH, ID |
| 面积 | (auto) | MJ, AREA |
| 用途 | (default) | DKYT, YT, YONGTU |
| 图幅号 | (default) | TFH |
| 地类 | (default) | DLBM, DL |

## Key Decisions & Constraints

- **No build step** — Pure HTML/CSS/JS, serve directly. No npm, bundler, or framework.
- **Desktop-app UX** — Fixed-size window (690×448px for v6), title bar with window controls, tab bar, column panels, simulated desktop behaviors.
- **Presets via LocalStorage** — Configs persist in `localStorage.tg_dark` as JSON. Import/export via `.json` files.
- **ArcPy dependency** — The actual conversion logic (in the `.tbx` file) uses `arcpy`. The web app currently has a simulated `run()` function that just animates progress — real conversion will need either a server-side ArcPy backend or a different approach.
- **Delphi EXE reference** — The two EXEs in `_reference_project/` are Delphi/C++Builder based (confirmed by binary analysis), serve as the behavioral reference for what the web app should replicate.
