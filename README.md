<div align="center">
  <img src="./docs/brand/app-icon-source.png" width="120" alt="极思G界址点互转工具 logo">
  <h1>极思G界址点互转工具</h1>
  <p>面要素（SHP / GDB）与标准界址点 TXT 文件的双向转换 — 轻量级 GIS 桌面工具，专为测绘与国土行业设计</p>
  <p>
    <a href="https://github.com/edcfoshan/polygon-txt/releases"><img src="https://img.shields.io/github/v/release/edcfoshan/polygon-txt?label=%E7%89%88%E6%9C%AC&color=teal" alt="latest release"></a>
    <img src="https://img.shields.io/badge/平台-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-64748b" alt="platforms">
    <img src="https://img.shields.io/badge/Rust-Tauri%20v2-orange" alt="tech">
    <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-green" alt="MIT license"></a>
  </p>
</div>

[English](./README.en.md) | 中文

![v4.4 主界面](./docs/screenshots/v4-cover.png)

## 简介

**极思G界址点互转工具** 是一款面向测绘与国土行业的轻量级 GIS 桌面工具，实现面要素（SHP / GDB）与标准界址点 TXT 文件的双向转换。

传统作业流程里，界址点 TXT 与 GIS 面要素之间的互转往往要靠 ArcMap 配合 Python 脚本或人工处理，繁琐且易错。本工具把这件**最常做的小事**做成了**一键操作**——选文件、点转换、出结果，中间不用写一行代码。纯 Rust 实现，**无需安装 ArcPy / ArcGIS**，全程离线，数据不出本机。

## 主要功能

- **面 → TXT**：导入 SHP、GDB 面要素，输出标准界址点 TXT 文件
- **TXT → 面**：解析 TXT 文件（标准 4 列 / 部备案 6 列），生成 SHP 矢量面数据
- 三种输出模式：一对一 / 按地块拆分 / 全合并
- **界址点坐标行列自定义**：点号/环号/Y/X 核心列 + 固定值/源字段/点距离列自由编排，距离单位与小数位逐列独立
- 支持 **2000 国家大地坐标系、1980 西安、1954 北京、WGS84**
- 高斯-克吕格投影 3°/6° 分带，自动识别 PRJ 并提取带号；一键输出公里网平面坐标 + 带号前缀
- 字段自动匹配（简单 / 高级 / 补充耕地三档预设），面积按平方米 / 公顷自动计算
- 投影 / 表头 / 字段 / 界址点四页签各带**一键恢复默认**
- 浅色 / 暗色 × 8 色系 = 16 种主题组合，全部满足 WCAG 对比度要求；窗口大小与位置全部记忆
- **应用内自动更新**：标题栏绿色箭头一键升级（签名校验）

## 核心功能详解

### 1. 双向转换 · 无损往返

转换不是简单的"复制粘贴"。**面 → TXT** 方向会先做环向归一化（外环逆时针、洞顺时针），识别洞与多部件并逐环编号界址线号，从 PRJ 自动识别坐标系并提取带号；**TXT → 面** 方向则按界址线号切环重建多边形、校验首末点闭合、回填 DBF 属性表。整个过程保证**无损往返**——转过去再转回来，坐标一根不差。

![双向转换数据流](./docs/screenshots/v4-flow@2x.png)

- **三种输出模式**：一对一（每源一个 TXT）/ 按地块拆分（文件名取自 DKMC 等字段）/ 全合并（整库归档带时间戳）
- SHP 读取失败会在界面上明确提示文件与原因，不再静默

### 2. 界址点坐标行列 · 布局自定义

v4 的招牌能力：坐标行的每一列都可编排——核心列（点号/环号/Y/X）固定在场，可再添加**固定值列**（如写死面积单位）、**源字段列**（把 DBF 属性直接拼进坐标行）、**点距离列**（按投影坐标平面直算，单位米/千米/厘米、小数位 0~6 逐列独立）。拖动 ⠿ 调整列序，预览与三种输出模式共用同一布局。闭合点距离为 0，开口环自动补回首点闭合边长，多部件与内环分别闭合计。

![界址点列布局](./docs/screenshots/v4-bb-tab.png)

### 3. 字段映射 · 从简单到专业

- **简单模式**：地块名称、编号、面积、用途、图幅号、地类六个槽位，源字段下拉直选
- **高级模式**：14 项固定字段清单自由增删、拖拽排序，可存为自定义方案复用
- **补充耕地预设**：一键载入 12 字段模板（图斑面积、图斑编号、补充耕地实施年份、耕地坡度级别……），直接对接上报格式

字段名支持中英文占位（DKMC / DKBH 行业惯例），面积可自动按平方米或公顷计算，未选字段输出空值列保持列序稳定。

### 4. 动态投影 · 坐标系全覆盖

支持四大坐标系，PRJ 自动识别。**动态投影**在导出时一键完成 3° / 6° 分带互换、换带（如 38 带 → 39 带）、转大地坐标——弹窗内自动推荐目标形式与中央经线；**带号前缀**开关独立于投影，导入时按坐标系智能推荐默认值。

### 5. 界面与主题

三栏工作区：**左栏**导入数据 + 输出与选项，**中栏**四页签（投影 / 表头 / 字段 / 界址点），**右栏**实时预览（TXT 预览 / 属性表 / 地图）。每个页签都有**恢复默认**按钮，配置改乱了随时一键回出厂。

**浅色 / 暗色** × **8 个色系**（经典黑白 / 测绘黄铜 / 墨绿 / 海蓝 / 青蓝 / 绛紫 / 珊瑚橙 / 玫红）共 16 种组合，全套配色联动且文字对比度全部达标；所有设置连同窗口大小位置一起持久化，关掉再开接着用。

![暗色模式](./docs/screenshots/v4-dark.png)

## 三个典型场景

**场景一：不动产登记 · 批量出界址点表**

宗地数据库里成百上千个面要素，要在期限内交齐界址点材料。导入 SHP，选「按地块拆分」模式，文件名用地块编号——点一次转换，每个宗地一个 TXT，文件名即编号，交付清单清清爽爽。面积按公顷自动算好写进属性段，不用再开 Excel 逐个算。

**场景二：补充耕地 · 数据上报对接**

补充耕地项目要求按 12 字段格式上报。高级字段模式选「补充耕地模式」预设，12 个字段一次到位，字段映射到源 GDB 属性列即可批量导出，直接对接接收系统的约定列序。

**场景三：外业 TXT · 回流质检**

外业回来的一批界址点 TXT，需要回建为面数据并做质检。TXT → 面方向会按界址线号切环、校验首末点闭合——**坐标串漏点、坐标顺序颠倒、带号填错**这些老问题，在反向转换时会直接暴露出来；转换结果与原始数据做无损往返比对，心里更有底。

## 技术架构

工具基于 **Tauri v2**（Rust 后端 + WebView 前端）构建：界面层只负责交互与实时预览，转换逻辑全部在 Rust 原生模块中完成（txt 解析生成 / SHP·DBF·PRJ 读写 / OpenFileGDB / 转换编排 / 高斯-克吕格投影），通过 IPC 总线通信，本地文件直读直写。

![应用架构](./docs/screenshots/v4-arch@2x.png)

纯 Rust 编译型原生代码，无解释器开销——万级地块分钟级完成；无需安装 ArcPy / ArcGIS，绿色便携版解压即用；安装包约 6MB，秒级启动，全程离线，数据不出本机。

## 下载安装

前往 [Releases](https://github.com/edcfoshan/polygon-txt/releases) 下载最新版本。

### 系统要求与文件选择

| 平台 | 要求 | 下载文件 |
|------|------|----------|
| **Windows** | Windows 10/11 64位 | `polygon-txt_X.X_x64-setup.exe` 安装版（推荐）或 `-portable.exe` 便携版 |
| **macOS** | macOS 10.15+ (Catalina) | `.dmg`（arm64 = Apple Silicon，x64 = Intel） |
| **Linux** | Ubuntu 20.04+ / 其他主流发行版 | `.AppImage` 或 `.deb` |

> Release 页中资产显示为中文名「极思G界址点互转工具」，实际下载链接为 ASCII 文件名（GitHub 限制），按上表认文件即可。

> ⚠️ **Windows 7 不支持**：Tauri v2 依赖的 WebView2 需要 Windows 10 或更高版本。首次运行若被 SmartScreen 拦下，点「更多信息」→「仍要运行」。

**百度云备选**：<https://pan.baidu.com/s/1xyW3-hyZrFDDG9ijYOf46g> 提取码 `e8vy`

**已安装旧版本的用户无需手动下载**——打开应用，标题栏出现绿色箭头即为有新版本，点击即可签名校验 + 一键自动更新。

### 从源码构建（所有平台）

前置要求：[Node.js](https://nodejs.org/) 、[Rust](https://www.rust-lang.org/)

```bash
npm install         # 安装前端依赖
npm run tauri dev   # 开发模式（热重载）
npm run tauri build # 生产构建
```

- **Windows**：输出 NSIS 安装包 + 便携版
- **macOS**：输出 `.dmg` 安装包
- **Linux**：输出 `.AppImage` 和 `.deb`

### GitHub Actions 自动构建

推送 `v*` 标签即触发四平台（Windows / macOS arm64 / macOS x64 / Linux）并行构建、updater 签名与 Release 自动发布，详见 [CI-CD 说明](./docs/CI-CD.md)。

## TXT 格式示例

工具输出的界址点 TXT 采用三段式结构，最小示例（高级字段模式，无字段名列表行）：

```text
[J1,1,39521000.123,3758100.456]
[J2,1,39521000.234,3758100.567]
[J3,1,39521000.345,3758100.678]
[J4,1,39521000.456,3758100.789]
[J1,1,39521000.123,3758100.456],@
[地块基本信息]
地块名称=示范宗地
地块编号=BC0001
地块用途=住宅
权利人=张三
宗地面积=1234.56
图幅号=50.00-25.00
坐标系=2000国家大地坐标系
计量单位=平方米
```

- 坐标行 `J{序号},{界址线号},Y坐标,X坐标`——**Y 在前**（北坐标 / 东坐标），J 序号在单个地块内跨环连续递增，从 J1 起
- 第二列 `界址线号` = `IndexedRing.part_index`（外环 = 1、洞 = 2、多部件下一 part = 3 ……逐环递增），是反向解析 TXT → SHP 切环的唯一依据
- 闭合点（首末点重合的末点）默认写本环首点序号、不占号（开启 `oc` 选项时改"续编"占号）
- 元数据行以 `,@` 结尾；坐标系字符串必须精确匹配 `2000国家大地坐标系` / `1980西安坐标系` / `1954北京坐标系` / `WGS84坐标系`
- 列布局可自定义：点号/环号/Y/X 之外可插入固定值、源字段与点距离列（见「界址点坐标行列」）

## 技术栈

- **Tauri v2**（Rust 后端 + WebView 前端）
- **Rust**：`shapefile` / `geonative-filegdb` / `chrono` / `dbase` / `encoding_rs` / `geo-types`
- **前端**：原生 JS + Vite（单文件打包，所有 JS 内联到 `dist/index.html`）

## 已知限制

- **GDB 写入**：最小化 OpenFileGDB 实现，ArcGIS Pro 兼容性有限（可回退 `ogr2ogr -f "OpenFileGDB"`）
- **政府 SHP 格式**：`test_data/` 中部分 `.shp` 使用非标准格式（magic ≠ 9994），标准库无法读取
- **打包方式**：`bundle.targets` 为 `nsis`（不含 MSI / WiX）。若 NSIS 打包失败，`src-tauri/target/release/jisig-bpoint-converter.exe` 仍可直接运行
- **Google Fonts**：需联网加载 Inter / Noto Sans SC / JetBrains Mono，离线回退系统字体
- **G 模式（6° → 3° 换带）**：`gauss_kruger_inverse` 对 6° 带源坐标的换带测试标记 `#[ignore]`，可靠性低于其他模式

## 许可证

[MIT License](./LICENSE)

## 交流与支持

扫码加入讨论群交流反馈：

![讨论群](./content/讨论群.jpg)

如果这个工具对你有帮助，欢迎赞赏支持：

![赞赏码](./content/关注、赞赏码.png)

仓库 [Issues](https://github.com/edcfoshan/polygon-txt/issues) 反馈 bug 与需求；[Discussions](https://github.com/edcfoshan/polygon-txt/discussions) 交流使用经验。

---

由 **极思 G** 提供技术支持
