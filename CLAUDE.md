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
cargo test --test dynamic_projection_test          # 投影函数单元测试（GK 往返、reband、zone 推断）
cargo test --test dynamic_projection_pipeline_test # 动态投影管线测试（keep/A/B/C/F/G + header 同步 + 预览一致性）
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
- **动态投影弹窗**（`#projModal`）：推荐区（顶部，基于经纬度范围智能推荐分带/带号/CM）+ 导入识别网格 + 目标形式下拉（`#projFormSelect`：3°带/6°带/转为大地坐标）+ 含带号开关（`#projPrefixToggle`，智能默认跟随导入数据 X 量级 >1e6 则开）+ 带号输入（`#projZoneInput`）↔ 中央经线输入（`#projCMInput`，双向联动，CM 立即规整到最近标称值）。三个维度（分带/含带号/转大地）解耦为独立控件——「不含带号」只控制 X 坐标前缀，不再禁用带号输入。输入是大地时「转为大地坐标」选项禁选；选中「转为大地坐标」时含带号开关+带号/CM 输入置灰。模式推断函数 `inferProjMode(inputIsDegree, inputBand, targetVal)`。模式自动推断表：

| 输入形式 | 目标分带 | mode |
|---------|---------|------|
| 大地(度) | 3° | A（大地→投影 3°） |
| 大地(度) | 6° | B（大地→投影 6°） |
| 投影(米) 同带 | — | C（同带前缀调整） |
| 投影(米) 源3°→6° | 6° | F（换带 3°→6°） |
| 投影(米) 源6°→3° | 3° | G（换带 6°→3°） |
| 任意 | 转为大地坐标 | D（投影→大地，逆投影） |

### Rust 后端模块

| 模块 | 功能 |
|------|------|
| `lib.rs` | Tauri IPC 命令 + IPC 类型定义 |
| `geometry.rs` | 多边形几何共享类型（SurfaceGeometry/PolygonPart/IndexedRing）+ 环向归一化、洞识别、坐标系交换 |
| `shp.rs` | SHP 读写（shapefile crate）、DBF 解析（dbase crate 失败时 catch_unwind + 手动二进制回退，编码探测 .cpg/GBK）、PRJ 坐标系识别 |
| `txt.rs` | TXT 三段式格式解析与生成 |
| `gdb.rs` + `gdb/gdb_templates.rs` | GDB 读取（geonative-filegdb）+ 模板化最小 OpenFileGDB 写入 |
| `convert.rs` | 转换编排：SHP/GDB→TXT（三模式：一对一/按地块拆分/全合并）、TXT→SHP（一对一/合并） |
| `projection.rs` | 高斯-克吕格投影正/反算 + 换带（proj-core EPSG 标准，回退经典 Krüger 公式 + 告警）。提供 `gauss_kruger_forward`、`gauss_kruger_inverse`、`reband_projected`、`infer_zone_from_x`、`detect_crs_completeness` |

### 输出模式（面→TXT）
- **一对一 (`one_to_one`)**: 每个导入源（SHP 文件 / GDB 要素类）输出一个 TXT。同名冲突自动追加 `_2/_3`
- **按地块拆分 (`split_by_plot`)**: 按源建子目录 `output_dir/{source_stem}/`，内部每个 feature 一个 TXT。文件名可选 DKMC/DKBH/序号/FID；字段缺失自动用序号兜底，重名追加序号，非法字符替换为 `_`
- **全合并 (`merge_all`)**: 所有源所有地块合并为 `merged_output_YYYYMMDD_HHMMSS.txt`（本地时间秒级时间戳）

### 转换选项（面→TXT，`ShpToTxtOptions` in convert.rs）
- `ox` XY 坐标标反 / `oj` 点号前加"J" / `on` 起始点西北角 / `oo` 首末点重合 / `oc` 闭合点编号模式
- `og` 输出公里网：仅当输入为大地坐标系（度）时可用。与动态投影互斥（`proj_mode ≠ "keep"` 时前端强制 og=false 并置灰）
- `proj_mode` 动态投影模式：`"keep"`（不转换）/ `"A"`（大地→3°投影）/ `"B"`（大地→6°投影）/ `"C"`（同带前缀调整，仅加减 zone×1,000,000 不做实际投影）/ `"D"`（投影→大地，逆投影）/ `"F"`（3°→6°换带）/ `"G"`（6°→3°换带）
- `proj_zone`: 用户填的带号（null=自动推算），`proj_no_prefix`: 不含带号前缀（自然值）
- `output_mode`（一对一/按地块拆分/全合并）、`filename_field`（拆分模式文件名字段）
- 前端 `getOptions()` 收集 → `applyProjMode()` 写入全局变量 → `updatePreview()`/`runShpToTxt()` 发送 IPC

### Tauri IPC 命令

文件选择：`pick_shp_files`、`import_gdb`、`pick_txt_files`、`pick_output_dir`
拖放导入：`pick_shp_files_from_paths`、`pick_txt_files_from_paths`
预览：`read_shp_to_txt_preview`、`read_txt_preview`
转换：`run_shp_to_txt`、`run_txt_to_shp`
投影：`apply_dynamic_projection`（独立 IPC，前端暂未使用；实际投影走 convert 管线）
窗口控制：`minimize_window`、`toggle_maximize`、`close_window`

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
SHP 存储 (X, Y) = (东坐标, 北坐标)。TXT 存储 (Y, X) = (北坐标, 东坐标)。转换层负责交换。动态投影 `transform_xy` 内部：
- A/B 模式：`x=lat, y=lon`，返回 `(easting, northing)`，调用方写回 `coord = (ny, nx)` = `(Y, X)`
- C 模式（同带前缀）：`x=northing, y=easting`，含带号时 `y+zone×1e6`，不含时剥离前缀
- D 模式（逆投影）：`x=northing, y=easting`，调用 `gauss_kruger_inverse(y, x, cm)` 返回 `(lon, lat)`
- **预览与转换路径必须一致**，否则预览坐标与输出 TXT 不对齐

### 预览坐标管线（关键）

`shp_to_txt_preview` → `shp_files_to_plots` / `gdb_features_to_plots`（og 投影）→ `apply_dynamic_projection_to_plots`（动态投影）→ `txt::generate_txt`

**陷阱**：`PlotData` 同时有 `coords`（扁平坐标列表）和 `rings`（结构化环坐标）。`generate_txt` 优先使用 `rings`（非空时）。`apply_dynamic_projection_to_plots` 必须**同时更新 `coords` 和 `rings`**，否则预览显示原始坐标而非投影后坐标。

### PRJ 坐标系识别（shp.rs `read_prj`）
支持匹配：`CGCS2000` / `Xian_1980` / `Beijing_1954` / `WGS84` / `WGS_84` / **`WGS_1984`**（最后一个是 v2.0 新增，之前遗漏导致 WGS84 PRJ 坐标系显示为空）。
提取字段存入 `crs_info` HashMap：`c`（坐标系名）、`u`（单位 度/米）、`b`（分带 3/6）、`z`（带号）。

### 精度设置

属性表"精度"行不是普通 `<select>` 或 `<input>`，而是 **range 滑块**（`#attrRows` 内 `.prec-slider`）。滑块值 0-8 映射到 10 的负幂次：

| 滑块值 | 精度 |
|--------|------|
| 0 | 1 |
| 1 | 0.1 |
| ... | ... |
| 8 | 0.00000001 |

- 前端：`precisionToExponent(s)` / `exponentToPrecision(exp)` 做精度字符串↔指数转换。**不要用 `parseFloat`**，否则小值变科学记数 `1e-8`；直接用 `toFixed` 返回字符串。
- Rust：`txt.rs` 的 `precision_to_decimals` 动态统计小数点后位数，`format_coord` 用 `format!("{:.prec$}", val)` 支持任意精度。
- `collectAttrRows` 对 `.prec-slider` 特殊处理，从 `slider.value`（0-8）转为精度字符串。
- `bindAttrRowEvents` 的 `input` 事件中检测 `.prec-slider`，同步更新旁边 `.prec-val` 显示文本。

### 导入结果扩展
`ShpFileItem` 和 `GdbImportResult` 含 `xmin`/`xmax`/`ymin`/`ymax` 字段（坐标范围），前端用于动态投影弹窗推荐文案（投影数据时近似逆投影得经纬度范围）。

### TXT 格式
- 坐标行：`J{序号},{界址线号},Y坐标,X坐标`（Y 在前）。第二列"界址线号"= `IndexedRing.part_index`（外环=1、洞=2、多部件下一 part=3…逐环递增），是反向解析 TXT→SHP 切环的唯一依据，**严禁删除或重算**
- J 序号在**单个地块内跨环连续递增**，每个地块从 J1 起（含 merge_all）。闭合点（首末点重合的末点，仅 `oo=true` 时存在）写法由 `oc` 选项决定：`oc=false`（默认"回到环首点"）→ 写本环首点序号、不占号；`oc=true`（"续编"）→ 占下一个连续序号
- 解析侧（`txt.rs` `parse_txt`）**只读第二列 part_index 切环，完全忽略第一列序号**——J 编号算法变化不影响反向解析
- 地块元数据行以 `,@` 结尾
- 坐标系字符串必须精确匹配：`2000国家大地坐标系`、`1980西安坐标系`、`1954北京坐标系`、`WGS84坐标系`

### DBF 写入
手动二进制写入（未使用 dbase crate API）。字段偏移量必须为 4 字节（LE），不是 2 字节。

### 动态投影 `proj_no_prefix`
`ShpToTxtOptions.proj_no_prefix = true` 时，`transform_xy` 在 A/B/F/G 模式中不添加 `zone × 1,000,000` 前缀。对 C 模式（前缀调整）：`true`=剥离前缀取自然值，`false`=自然值加前缀。对 D 模式（逆投影）：无影响（输出经纬度无前缀概念）。

### 属性表字段 key 命名（applyProjMode 同步陷阱）
属性表（`#attrRows`，`DEFAULT_ATTRS` in main.js）的真实 key：`坐标系`/`几度分带`/`投影类型`/`计量单位`/`带号`/`精度`/`转换参数`。**没有** `形式` 或 `分带` key——弹窗"导入识别"区的 label（形式/分带）只是显示文案，不对应属性表行。`applyProjMode` 点"应用"后用 `setRow(key,v)` 同步属性表，必须用真实 key（如 `setRow('几度分带', String(bw))`、`setRow('计量单位','米')`），写成 `setRow('形式'/'分带',…)` 是 **no-op**（`rows.find` 找不到匹配行静默跳过）。「几度分带」值是 `"3"`/`"6"` 字符串（对齐 `ATTR_SELECT_OPTIONS["几度分带"]=["3","6"]`），不是 `"3°带"`。`currentCrsInfo`（弹窗导入识别数据源）只在导入时经 `syncOgGate` 设一次，用户改属性表**不回写**——两者独立；若要让弹窗反映属性表实时值，需在 `openProjModal` 从 `collectAttrRows()` 读。

### 发布打包
`npm run tauri build` → 产物复制到 `其他相关tbx放进去release/`（便携版 + NSIS 安装包）。签名需 `TAURI_SIGNING_PRIVATE_KEY` 环境变量或 `scripts/build-signed.ps1`。

### CSP（tauri.conf.json）
必须包含 `script-src 'self' 'unsafe-inline' 'unsafe-eval'`，否则 WebView2 阻止内联脚本。

### 不使用 arcpy
生产代码中严禁引入 arcpy 依赖。允许用 arcpy 做验证和测试对比。

### 前端构建特殊行为
`@tauri-apps/api` 在 Vite 生产构建中不会被包含在输出 JS 内。ES module `import` 由 Vite 在构建时解析，运行时通过 `window.__TAURI__` 调用。

### 构建陷阱：vite html-inline-proxy 间歇失败
`npm run build`（vite + vite-plugin-singlefile）在改过 `index.html`（含内联 `<style>`）后偶发报 `No matching HTML proxy module found`（"2 modules transformed"，正常 15）。**Windows 文件系统时序竞态，非源码 bug**——清缓存前后 JS hash 一致。
- 单独 `npm run build` 稳定；用 **bash** `rm -rf node_modules/.vite dist` 清缓存（PowerShell `Remove-Item` 大目录后立即 build 反而易触发竞态）
- **`npm run tauri build` 的 `beforeBuildCommand` 会确定性失败**（4/4），但单独 build 5/5 成功；TAURI_* env 不是元凶，疑为 tauri 子进程 cwd/shell 差异
- **绕过方案**（已验证）：① 单独 `npm run build` 生成 dist → ② 临时清空 `tauri.conf.json` 的 `beforeBuildCommand`（改 `""`）→ ③ `npm run tauri build`（用现成 dist，cargo+NSIS 正常）→ ④ **务必恢复** `beforeBuildCommand: "npm run build"`

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
5. **G 模式 (6°→3° 换带)**：`gauss_kruger_inverse` 对 6° 带源坐标的前缀剥离假定 3° 带号（`proj-core` 无 6° 带 EPSG 代码），proj-core + classic 均可能失败。测试标记 `#[ignore]`
6. **`_projBand` 残骸已清理**；`om` 复选框残骸未清理（非动态投影范围）

## 依赖

### 前端（package.json）
`@tauri-apps/api ^2`、`@tauri-apps/plugin-dialog ^2`、`@tauri-apps/plugin-shell ^2`、`@tauri-apps/plugin-updater ^2`、`@tauri-apps/plugin-process ^2`、`vite ^6`、`vite-plugin-singlefile ^2`、`@tauri-apps/cli ^2`

### Rust（Cargo.toml）
`tauri 2`、`tauri-plugin-dialog/fs/shell/updater/process 2`、`shapefile 0.8`、`dbase 0.3`、`geonative-core/filegdb/shapefile 0.2`、`chrono 0.4`、`encoding_rs 0.8`、`geo-types 0.7`、`serde 1`、`serde_json 1`、`tempfile 3`

**GPKG 已移除**（v1.1+）：读取仅 SHP/GDB，输出仅 SHP。`gpkg.rs`/`smoke.rs`/`rusqlite` 依赖已删除。
