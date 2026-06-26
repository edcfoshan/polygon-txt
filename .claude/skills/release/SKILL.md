---
name: release
description: 发布极思G界址点互转工具新版本。同步版本号、更新 CHANGELOG、构建、打 tag、上传 GitHub Release（NSIS 安装包 + 绿色 exe + 配套 ArcMap tbx 工具箱）。Use when user wants to publish / release / 发版 / 发布新版本 of this Tauri app.
---

# 发布流程（极思G界址点互转工具）

本 skill 把"发新版本"标准化，每次发版按下列步骤执行。仓库固定为 `edcfoshan/polygon-txt`。

## 步骤

### 1. 确认版本参数（问用户，每项给推荐）
- 配置文件版本号 `X.Y.Z`（语义化，如 `1.2.0`）
- 界面显示写法 `VX.Y`（如 `V1.2`，与历史 `V1.0` 风格一致）
- git tag 名（如 `v1.2` 或 `v1.2.0`）
- 本次变更内容（用于 CHANGELOG / release notes）

### 2. 同步 5 处版本号
| 文件 | 字段 | 改为 |
|------|------|------|
| `package.json` | `"version"` | `X.Y.Z` |
| `src-tauri/Cargo.toml` | `version =` | `X.Y.Z` |
| `src-tauri/tauri.conf.json` | `"version"` | `X.Y.Z` |
| `index.html` | `<span class="brand-sub">` | `VX.Y` |
| `content/about.md` | `**版本：**` | `VX.Y` |

> `Cargo.lock` / `resource.rc`（exe Windows 版本信息）/ `dist/index.html` 由构建自动更新，无需手改。

### 3. 更新 CHANGELOG.md
在 `## [1.0.0]` 之前（即顶部最新位置）插入：
```
## [X.Y.Z] - YYYY-MM-DD
- 修复/新增：……（只列用户可见变更）
```

### 4. 构建签名（自动更新所需，必做）

**构建前**先设签名环境变量（每次发版都要，私钥密码让用户提供）：
```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $env:USERPROFILE\.tauri\bpoint-converter.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<用户密码>"
npm run tauri build
```
产出：
- `src-tauri/target/release/bundle/nsis/极思G界址点互转工具_X.Y.Z_x64-setup.exe`
- `src-tauri/target/release/bundle/nsis/极思G界址点互转工具_X.Y.Z_x64-setup.exe.sig` ← **签名文件（自动更新必需）**
- `src-tauri/target/release/jisig-bpoint-converter.exe`

> 若未设私钥环境变量，构建不会生成 `.sig`，老用户将收不到更新提醒——必须确认 `.sig` 存在。

### 4.5 生成 latest.json（自动更新清单，必做）
```
node scripts/gen-latest-json.js --notes "<本次更新内容>"
```
自动读 `package.json` 版本号 + 扫描 `.sig` + 组装国内加速 URL，输出仓库根目录 `latest.json`。
> 不传 `--notes` 则从 `CHANGELOG.md` 提取对应版本段落兜底。

### 5. 集中产物到 `其他相关tbx放进去release/`
此目录是**发布物的正式暂存地**（用户约定，目录名本身即指令"放进去 release"）。构建后把产物按下列命名规则放入（**注意重命名**，Tauri 默认产物名用完整 `X.Y.Z`，发布文件名用简短 `X.Y`）：

| 构建产物（源） | 发布文件名（目标，统一英文 polygon-txt 前缀） |
|---|---|
| `src-tauri/target/release/jisig-bpoint-converter.exe` | `polygon-txt_X.Y_x64-portable.exe` |
| `src-tauri/target/release/bundle/nsis/极思G界址点互转工具_X.Y.Z_x64-setup.exe` | `polygon-txt_X.Y_x64-setup.exe` |

外加该目录里用户已准备好的配套附件（如 `Extra_CoordConvert_ArcOptimize_ArcMap_Toolbox.zip`）。

> 目录已被 `.gitignore` 排除（`*.exe` / `*.zip` 规则覆盖），不入库。发版前若该目录已有旧版本产物，先删除旧的同名文件再放新的。

### 6. 展示清单 + 等用户确认
列出 `dist-release/` 下所有文件（路径 + 大小）+ release notes 草稿。**等用户确认后才执行 7、8。**

### 7. git 提交 + tag（用户确认后）
```
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json index.html content/about.md CHANGELOG.md latest.json
git commit -m "release: vX.Y — <一句话变更>"
git tag -a vX.Y -m "vX.Y: <变更摘要>"
git push && git push origin vX.Y
```
> **`latest.json` 必须提交进仓库根目录**——jsDelivr 端点（`cdn.jsdelivr.net/gh/.../latest.json`）从这里取，国内用户靠它快速检查更新。

### 8. 创建 GitHub Release（用户确认后）
```
gh release create vX.Y 其他相关tbx放进去release/*.exe 其他相关tbx放进去release/*.zip latest.json \
  --repo edcfoshan/polygon-txt --title "VX.Y" --notes-file <release-notes.md>
```
或提示用户在 https://github.com/edcfoshan/polygon-txt/releases/new 网页上传。
> **`latest.json` 必须作为 Release 资产上传**——GitHub 端点（`releases/latest/download/latest.json`）从这里取，作为 jsDelivr 的兜底源。

## 注意事项
- 仓库永远指向 `edcfoshan/polygon-txt`（CLAUDE.md / AGENTS.md 的仓库链接也须保持一致，发现旧名 `boundary-point-converter` 一并修正）
- 发布物暂存目录固定为 `其他相关tbx放进去release/`（不是 `dist-release/` 或其他）
- 发布文件名用简短版本 `X.Y`（如 `1.2`），但配置文件 `tauri.conf.json` 的 `version` 仍用完整 `X.Y.Z`（如 `1.2.0`）以保证 exe 内嵌 ProductVersion 精确——构建后手动重命名产物
- 不做构建时版本号注入，5 处手动同步即可（项目体量小）
- 导出文件命名规则（convert.rs）与本流程无关，不要借发版改动
- **自动更新（v1.3+）**：每次发版必须 ① 构建前设 `TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 环境变量（用 `scripts/build-signed.ps1` 交互输密码最稳）② 确认 `tauri.conf.json` 的 `bundle.createUpdaterArtifacts: true`（否则不生成 .sig！）③ 运行 `node scripts/gen-latest-json.js` 生成 `latest.json` ④ 把 `latest.json` 同时 git 提交进仓库根目录 + 作为 Release 资产上传。遗漏任一步 → 老用户收不到更新提醒（不影响应用本身）。完整说明见 [docs/RELEASE.md](docs/RELEASE.md)。
- **私钥安全**：`C:\Users\Administrator\.tauri\bpoint-converter.key` 是签名私钥，**严禁**提交进 git / 发给任何人 / 截图。只在本机使用，建议网盘 + U盘双重备份。公钥 `.key.pub` 已写入 `tauri.conf.json` 的 `plugins.updater.pubkey`（公开信息，可入库）。
