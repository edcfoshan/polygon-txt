# 极思G界址点互转工具 CLI

原生 Rust 命令行工具，与桌面版共用转换核心，支持 SHP/GDB → TXT、TXT → SHP。无需安装桌面版、Python、ArcGIS 或 arcpy。Linux CLI 无需 WebKit2GTK。

## 下载与版本

下载 [CLI 4.7.0](https://github.com/edcfoshan/polygon-txt/releases/tag/cli-v4.7.0)，按 Windows x64、Linux x64、macOS Intel、macOS Apple Silicon 选择压缩包。解压后包含可执行文件、本文、`example.json` 和 `SKILL.md`。Windows 使用 `jisigg-cli.exe`；macOS/Linux 使用 `./jisigg-cli`。

桌面版与 CLI 使用相同版本号，分别发布在 `v4.7.0` 与 `cli-v4.7.0`，两者来自同一提交。仓库的 Latest 指向桌面版。

## 快速使用

下面以 Windows PowerShell 为例；其他平台替换可执行文件名即可。路径包含空格时使用引号。输入参数可以重复。

```powershell
.\jisigg-cli.exe --help
.\jisigg-cli.exe shp-to-txt --help
.\jisigg-cli.exe inspect --input "D:\数据\地块.shp" --json
.\jisigg-cli.exe config validate .\example.json --json
.\jisigg-cli.exe shp-to-txt --input "D:\数据\地块.shp" --config .\example.json --output-dir "D:\结果" --dry-run --json
.\jisigg-cli.exe shp-to-txt --input "D:\数据\地块.shp" --config .\example.json --output-dir "D:\结果" --json
.\jisigg-cli.exe txt-to-shp --input "D:\结果\地块.txt" --config .\example.json --output-dir "D:\还原" --json
```

`example.json` 的带号为示例值 **38**，使用前按实际数据修改。TXT→SHP 优先依据 TXT 和坐标识别带号；均无法确定时使用配置表头中的带号，仍无法确定则停止。面→TXT 涉及投影时，也需确认坐标系、分带与带号。

```powershell
# 多个 SHP 合并（命令行显式参数优先于 JSON）
.\jisigg-cli.exe shp-to-txt --input a.shp --input b.shp --config example.json --output-mode merge_all --output-dir out
# 单个 GDB，选择要素类；省略 --layer 时处理全部面图层
.\jisigg-cli.exe inspect --input data.gdb --json
.\jisigg-cli.exe shp-to-txt --input data.gdb --layer 地块 --layer 用地 --config example.json --output-dir out --encoding gbk
# 拆分输出，字段缺失时沿用序号兜底规则
.\jisigg-cli.exe shp-to-txt --input a.shp --output-mode split_by_plot --filename-field DKMC --output-dir out
# 一次性选择源 0 的第 0、2 个地块；下标从 0 开始
.\jisigg-cli.exe shp-to-txt --input a.shp --plot 0:0 --plot 0:2 --output-dir out
```

`--plot` 的源下标按 SHP 输入顺序，或 GDB 的 `--layer` 指定顺序（未指定则为面图层目录顺序）；地块下标按源内顺序。配置模板不保存这些下标。

## 配置交换

桌面版点击「配置交换」，可导入/导出 JSON 文件，或复制/粘贴同格式文本。导入会新增配置并立即切换，重名追加 `_2`、`_3`，保留原配置。

配置 `schema_version: 1` 同时保存两个方向的设置和当前方向，包含表头、简单/高级字段映射、面积精度、投影、界址点布局、编码及输出选项；不携带文件路径、输出目录、具体地块勾选或主题。`editor` 保存桌面编辑状态；高级模式下 `editor.rows` 必须与 `fields.columns` 一致。

CLI 的子命令决定本次转换方向，不受配置中的当前方向限制。常用参数 `--output-mode`、`--filename-field`、`--encoding` 覆盖 JSON；其余复杂选项通过 JSON 提供。可用 `config default` 获取完整默认 JSON（PowerShell 7 可用 `| Set-Content -Encoding utf8 config.json` 保存）。修改后先执行 `config validate`。

未知键、无效精度、布局缺失核心列、矛盾投影选项都会列出错误并停止。转换前还检查数据字段；配置明确选用自动面积、占位或既有命名/埋桩兜底的，沿用对应规则。`omit_coord_section` 可生成外部专用 TXT，但本工具无法再解析缺少 `[地块坐标]` 标题的文件。

## 预检、覆盖与结果

- `--dry-run` 在内存生成同一套转换字节，校验输入、配置、编码和整批文件冲突，列出计划文件；不创建目录或文件。大型任务所需内存随输出总大小增长。
- 默认任一同名文件已存在就停止，列出冲突；只有显式 `--overwrite` 才覆盖，包括 SHP 的 SHX/DBF/PRJ/CPG 配套文件。同批地块重名沿用核心的自动编号规则。
- 写入前的已知冲突不会产生半批结果。实际写入期间若发生权限、磁盘或其他 I/O 错误，可能留下部分结果；根据返回错误检查输出目录。
- `--json`：stdout 为单个 JSON 结果，诊断日志写 stderr。成功退出码 `0`，配置/数据/转换错误 `1`，命令行参数错误 `2`。`--help`、`--version` 仍输出标准帮助/版本文本。
- 成功结果含 `success`、`dry_run`、`message`、`processed_count`、`output_files`、`output_bytes`；失败含 `success:false`、`error`。`inspect` 返回 `sources`；配置校验返回名称与版本。

## 从源码构建

```powershell
cargo build --locked --release --no-default-features --bin jisigg-cli --manifest-path src-tauri/Cargo.toml
cargo test --locked --no-default-features --manifest-path src-tauri/Cargo.toml --tests
```

CLI 位于 `src-tauri/target/release/`。CLI 构建不启用 Tauri 桌面依赖。
