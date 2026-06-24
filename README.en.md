# polygon-txt

> Boundary-Point Converter — bidirectional conversion between polygon features (SHP/GDB) and standard boundary-point TXT files

English | [中文](./README.md)

![Screenshot](./docs/screenshots/webview_screenshot.png)

## Overview

A lightweight GIS desktop tool for the surveying & land-management industries. Converts polygon features (SHP / GDB) to and from standard boundary-point TXT files. Pure Rust — **no ArcPy / ArcGIS required**.

## Features

- **Polygon → TXT**: import SHP / GDB polygons, export standard boundary-point TXT
- **TXT → Polygon**: parse TXT, generate SHP polygons
- Three output modes: one-to-one / split-by-plot / merge-all
- Supports **CGCS2000, Xi'an 1980, Beijing 1954, WGS84**
- Gauss-Krüger 3°/6° zones with automatic zone detection
- Automatic field mapping & CRS recognition
- Automatic area calculation
- Light / dark theme, custom frameless window

## Download

Get the latest Windows installer from [Releases](https://github.com/edcfoshan/polygon-txt/releases).

## Build from Source

Prerequisites: [Node.js](https://nodejs.org/), [Rust](https://www.rust-lang.org/)

```bash
npm install         # install frontend dependencies
npm run tauri dev   # development mode (HMR)
npm run tauri build # production build (outputs NSIS installer)
```

## Tech Stack

- **Tauri v2** (Rust backend + WebView frontend)
- **Rust**: shapefile / geonative-filegdb / chrono
- **Frontend**: vanilla JS + Vite (single-file bundle)

## Known Limitations

- GDB writing is a minimal OpenFileGDB implementation; ArcGIS Pro compatibility is limited (fall back to `ogr2ogr`)
- Some government-issued SHP files use non-standard formats (magic ≠ 9994) and may fail to read

## License

[MIT License](./LICENSE)
