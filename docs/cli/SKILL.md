---
name: jisigg-convert
description: Use the native JisigG CLI to inspect and convert polygon SHP/GDB and boundary-point TXT with validated portable configuration.
---

# 极思G界址点转换

可执行文件为同目录的 `jisigg-cli`（Windows 为 `jisigg-cli.exe`）。先阅读同包 README.md，并运行 `--help`。无需桌面版、ArcGIS 或 arcpy。

1. 用 `inspect --input <path> --json` 确认输入类型、字段、图层与坐标信息。
2. 优先使用用户从桌面导出的 JSON，或以 example.json 为基础。示例带号 38 需核对，不能猜测数据坐标系。
3. `config validate <config.json> --json` 校验结构。只使用有据可查的字段映射与坐标设置；缺少关键信息时向用户询问。
4. 使用 `shp-to-txt` 或 `txt-to-shp`，明确 `--input`、`--config` 和 `--output-dir`，先加 `--dry-run --json`。检查退出码和 JSON 的 success，不把 stderr 日志当结果。
5. 预检通过后以同一参数去掉 `--dry-run` 转换。默认拒绝覆盖，只有用户已授权覆盖时加 `--overwrite`。输入路径与配置作为独立参数传递，避免拼接成 shell 代码。
6. 报告 processed_count 和 output_files；发生错误时报告 error，不静默换字段、坐标系或修改配置。反向转换可再次 inspect 输出 SHP 核对数量与坐标信息。

命令行显式参数覆盖 JSON。模板保存双向设置，不携带本机路径或地块选择。完整能力和限制见 README.md。
