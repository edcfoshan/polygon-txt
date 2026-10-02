<div align="center">
  <img src="./docs/brand/app-icon-source.png" width="112" alt="极思G界址点互转工具图标">
  <h1>极思G界址点互转工具</h1>
  <p>让界址点 TXT 与面数据之间的转换，少一些重复操作</p>
  <p>
    <a href="https://github.com/edcfoshan/polygon-txt/releases"><img src="https://img.shields.io/github/v/release/edcfoshan/polygon-txt?label=最新版本&color=teal" alt="最新版本"></a>
    <img src="https://img.shields.io/badge/Windows-macOS-Linux-64748b" alt="支持 Windows、macOS、Linux">
    <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-green" alt="MIT License"></a>
  </p>
</div>

[English](./README.en.md) | 中文

![应用界面](./docs/screenshots/v4-cover.png)

## 做测绘成果时，格式转换不该成为最费时间的一步

一个熟悉的工作场景：拿到一批宗地面数据，要整理成界址点 TXT；外业或协作方交回 TXT，又要重新建成面数据。真正花时间的，往往不是某一步特别难，而是反复导入、核字段、对坐标、改编码、检查输出。

**极思G界址点互转工具只专注做好这件事：让 SHP / GDB 面数据与界址点 TXT 双向转换。**选文件、核对配置、查看预览，再导出结果。转换在本机完成，不需要安装 ArcGIS 或 ArcPy，也不需要把业务文件上传到服务器。

## 面数据转 TXT：把重复整理交给批量流程

导入 SHP 文件，或选择 GDB 中需要处理的要素类。工具读取面几何和属性字段，生成界址点 TXT。输出可以按输入源分别保存，也可以按地块拆分成多个文件；导入多个源时，还可以合并为一个 TXT。

地块拆分时，文件名可以使用地块名称、编号、序号或 FID。先在属性表里筛选需要的地块，预览和导出会使用同一筛选结果，避免整理后再逐个删除不需要的文件。

![面数据与 TXT 的转换流程](./docs/screenshots/v4-flow@2x.png)

导出编码可选 **UTF-8** 或 **ANSI（GBK）**。选择 GBK 时，工具会在写文件前检查整批内容；遇到无法编码的字符，会提示改用 UTF-8，避免只生成一部分文件。

## TXT 转面数据：让外业成果回到 GIS 工作流

导入标准界址点 TXT 后，工具按界址线号恢复环结构，并生成 SHP 面数据及配套文件。标准 4 列坐标行和部备案 6 列格式均可读取。

转换前可以核对坐标系、分带、带号和字段识别结果。对来源复杂或坐标声明不确定的数据，建议先用少量地块试转，再检查成果位置和属性。

## 让 TXT 的每一列符合交付要求

不同项目对坐标行的列顺序和附加内容常有约定。在「界址点」设置中，可以调整点号、环号、Y、X 的位置，还能加入序号、固定值、源属性字段或点间距离列。序号可跨地块连续，也可每个地块重置；距离列可分别设置米、千米或厘米，以及 0–6 位小数。

面积字段也可以按实际要求生成：平方米、亩、公顷或平方千米，并单独设置 0–6 位小数；如果项目已有面积字段，也可以直接映射原值。

![界址点坐标列设置](./docs/screenshots/v4-bb-tab.png)

## 坐标、字段和结果，都能先检查再导出

工具可识别常见 PRJ 信息，支持 CGCS2000、1980 西安、1954 北京和 WGS84，以及高斯–克吕格 3° / 6° 分带相关处理。字段映射提供简单模式、高级模式和补充耕地预设；右侧可查看 TXT 预览、属性表和地块地图。

这些检查适合发现常见配置问题，但不能代替对源数据坐标系和成果精度的核验。尤其是跨带、特殊投影或坐标声明不完整的数据，请以项目要求和目标 GIS 软件复核结果为准。

## 下载后即可使用

前往 [GitHub Releases](https://github.com/edcfoshan/polygon-txt/releases/latest) 下载最新版本。

| 系统 | 下载文件 | 说明 |
| --- | --- | --- |
| Windows 10/11，64 位 | `*_x64-setup.exe` | 安装版，适合大多数用户 |
| Windows 10/11，64 位 | `*_x64-portable.exe` | 便携版，无需安装 |
| macOS 10.15+ | `*_aarch64.dmg` | Apple Silicon（M 系列） |
| macOS 10.15+ | `*_x64.dmg` | Intel 芯片 |
| Linux x64 | `*_amd64.AppImage` 或 `*_amd64.deb` | AppImage 或 Debian 安装包；需要 WebKit2GTK |

Windows 7 不受支持。首次运行若出现 SmartScreen 提示，请确认文件来自本仓库 Releases，再按系统提示继续。Windows 安装版支持在应用内检查并安装更新。

## TXT 坐标行长什么样

默认坐标行为 `J序号,界址线号,Y坐标,X坐标`。Y（北坐标）在 X（东坐标）之前；TXT 中以 `,@` 结尾的地块元数据行与后续坐标行共同描述一个地块。界址线号用于区分外环、内环和多部件。

```text
[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
[地块坐标]
4,1234.56,BC0001,示范宗地,面,,,,@
J1,1,39521000.123,3758100.456
J2,1,39521000.234,3758100.567
J3,1,39521000.345,3758100.678
J4,1,39521000.456,3758100.789
J1,1,39521000.123,3758100.456
```

坐标系名称需符合格式约定：`2000国家大地坐标系`、`1980西安坐标系`、`1954北京坐标系` 或 `WGS84坐标系`。自定义列布局时，请与接收方约定保持一致。

## 使用前了解这些边界

- 面数据输入支持标准 SHP 与 OpenFileGDB；面数据输出为 SHP，不写入 GDB。
- 部分政府业务 SHP 使用非标准文件结构，标准 SHP 解析器可能无法读取。
- GDB 图层读取能力会受具体 OpenFileGDB 结构影响，重要成果请在目标 GIS 软件中复核。
- 转换文件在本机读写。地图底图和网络字体需要联网；离线时仍可转换文件，地图底图不可用时可查看地块轮廓。

## 从源码运行

需要 [Node.js](https://nodejs.org/) 和 [Rust](https://www.rust-lang.org/)。

```bash
npm install
npm run tauri dev
```

构建桌面安装包：

```bash
npm run tauri build
```

只构建前端：

```bash
npm run build
```

## 如果它正好能帮上你的工作

欢迎通过 [Issues](https://github.com/edcfoshan/polygon-txt/issues) 反馈问题和功能建议，也可以前往 [Releases](https://github.com/edcfoshan/polygon-txt/releases) 获取更新。

- [更新日志](./CHANGELOG.md)
- [许可证：MIT](./LICENSE)

由 **极思 G** 提供技术支持。
