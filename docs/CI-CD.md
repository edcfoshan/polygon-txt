# GitHub Actions CI/CD 配置说明

本项目配置了自动化跨平台构建，支持 Windows、macOS (Intel + Apple Silicon) 和 Linux。

## 工作流概览

### 1. CI Build Test (`.github/workflows/build.yml`)

**触发条件：**
- Push 到 `master` 或 `main` 分支
- 创建 Pull Request
- 手动触发（Actions 页面点击 "Run workflow"）

**功能：**
- 在 4 个平台上自动构建（Windows x64, macOS ARM64, macOS x64, Linux x64）
- 上传构建产物（保留 7 天）
- **不创建 Release**，仅用于测试构建是否成功

### 2. Release (`.github/workflows/release.yml`)

**触发条件：**
- 推送 `v*` 格式的 tag（如 `v3.2.0`）

**功能：**
- 在全部平台自动构建
- 自动创建 GitHub Release
- 上传所有安装包（Windows exe, macOS dmg, Linux AppImage/deb）
- 如果配置了签名密钥，会自动签名

## 发版流程

### 第一步：准备发版

1. 确保代码已提交并推送到 master
2. 更新版本号（`Cargo.toml` 和 `tauri.conf.json` 中的 `version`）
3. 更新 CHANGELOG.md（如有）

### 第二步：打标签并推送

```bash
# 创建带注释的标签
git tag -a v3.2.0 -m "Release v3.2.0"

# 推送标签到 GitHub
git push origin v3.2.0
```

### 第三步：等待构建完成

1. 前往 GitHub 仓库的 Actions 页面
2. 查看 "Release" 工作流运行状态
3. 构建完成后（约 10-20 分钟），前往 Releases 页面查看

## 配置签名（可选，推荐）

为了支持自动更新功能，建议配置签名密钥：

### 1. 生成签名密钥

```bash
# 使用 Tauri CLI 生成密钥对
npx tauri signer generate -w ~/.tauri/myapp.key
```

这会输出：
- 私钥（保存好，不要泄露）
- 公钥（用于验证）

### 2. 配置 GitHub Secrets

前往仓库 **Settings → Secrets and variables → Actions**，添加：

| Secret 名称 | 说明 |
|-------------|------|
| `TAURI_SIGNING_PRIVATE_KEY` | 私钥内容（完整的一行字符串） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码（如果设置了密码） |

### 3. 更新公钥

将公钥更新到 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey` 字段。

## 自动化发布流程（v3.1 实测打通）

配置好签名 Secrets 后，推送 `v*` 标签即可全自动完成**跨平台构建 + 签名 + 发布 + 自动更新清单**：

```bash
git tag -a v3.2.0 -m "Release v3.2.0"
git push origin v3.2.0
```

### Release 自动生成的内容

| 显示名（资产 label） | slug（`name`，进 URL 与落盘文件名） | 说明 |
|------|------|------|
| `极思G界址点互转工具_{ver}_x64-setup.exe` + `.sig` | `JisigG_{ver}_x64-setup.exe` | Windows NSIS 安装版（签名） |
| `极思G界址点互转工具_{ver}_x64-portable.exe` | `JisigG_{ver}_x64-portable.exe` | Windows 便携版（额外步骤上传） |
| `极思G界址点互转工具_{ver}_aarch64.dmg` / `_x64.dmg` | `JisigG_{ver}_aarch64.dmg` / `_x64.dmg` | macOS 安装包 |
| `极思G界址点互转工具_{ver}_{arch}.app.tar.gz` + `.sig` | `JisigG_{ver}_{arch}.app.tar.gz` | macOS 更新包（updater 用） |
| `极思G界址点互转工具_{ver}_amd64.AppImage` / `_amd64.deb` | `JisigG_{ver}_amd64.AppImage` / `.deb` | Linux |
| `latest.json` | `latest.json` | 全平台自动更新清单（含签名），url 指向 setup 的 `browser_download_url` |

> **资产有两层名字，各管各的（GitHub 服务端规则）**
> - `name`（slug）：决定下载 URL 和 `Content-Disposition` 里的落盘文件名。非 ASCII 会被静默吃掉——连续一段折叠成一个 `.`、位于开头的整段直接删除，所以中文原名传上去会变成 `G._{ver}_*`。中文 slug 物理上不可能，换上传工具/换 runner 平台都无用。
> - `label`：Release 列表里显示的名字，可以是中文。`tauri-action` 会自动设成中文原名；`gh release upload` 不设。
> - 解法是两者分开放：`Build and Release Tauri app`（+ 便携版上传）之后跑 `Normalize asset names` 步骤 = `node scripts/normalize-release-assets.mjs --repo <owner/repo> --tag <vX.Y.Z>`，把 slug 统一改成品牌 ASCII `JisigG_{ver}_*`、label 保留中文原名。只 PATCH 元数据，不重传文件；`.sig` 签的是 exe 字节，与文件名无关，所以签名照旧有效。脚本幂等，可 `--dry-run` 预览。
> - 因此 `latest.json` 的 url 必须在 normalize **之后**取 API 的 `browser_download_url`（CI 里现查现用），绝不能再拼中文文件名——v4.4.0 拼了中文名，url 实测 404，装有 ≤4.3.0 的 Windows 客户端收到更新提示后下载必失败。
> - 核对资产一律用 `gh api repos/.../releases/tags/<tag> -q '.assets[] | "\(.name) | \(.label)"'`，不要看页面文本或摘要工具猜（slug 才是真实链接）。

### 已解决的坑（勿回退）

1. **`bundle.targets: "nsis"` 导致 mac/Linux 不打包** → CI 在 `args` 里显式传 `--bundles nsis` / `dmg,app` / `appimage,deb`
2. **未配置私钥时构建失败**（`A public key has been found, but no private key`）→ 工作流 `Patch Tauri config for CI` 步骤：无私钥则删 `pubkey` + 关 `createUpdaterArtifacts`；有则保留并签名
3. **`TAURI_SIGNING_PRIVATE_KEY` 空字符串会报 `Missing comment in secret key`** → 不能无条件设置该 env；条件分支处理
4. **中文 productName 使 GitHub 资产 slug 损坏**（`极思G界址点互转工具_3.1.0.dmg` → `G._3.1.0.dmg`，落盘文件名也跟着难看）→ **不要改 productName**（NSIS 卸载键/安装目录/快捷方式名派生自它，v3.2 改写曾致更新后安装目录漂移），也不要再复制一份 ASCII 别名副本（会出现两套重复资产）。正解：上传后跑 `Normalize asset names` 步骤把 slug 改成 `JisigG_*`、中文留在 label（见上一节）
5. **`TAURI_CONFIG` 环境变量在本 Tauri 版本不生效** → 用 node 直接改 `tauri.conf.json`（勿用 PowerShell 写 JSON，会加 BOM）

### 发布后必须同步的 `latest.json`

tauri-action 生成的 `latest.json` 在 GitHub Release 资产里（第二端点 `releases/latest/download/latest.json` 可直接用）。但**第一个端点 jsDelivr 读取的是仓库根 `latest.json`**——发版后必须把 Release 资产里的 `latest.json` 复制到仓库根并 push，然后 purge：

```bash
# 1. 下载 Release 资产里的 latest.json
gh release download v3.2.0 --pattern "latest.json" --repo edcfoshan/polygon-txt --clobber
# 2. 复制到仓库根并 push
cp latest.json . && git add latest.json && git commit -m "release: 同步 latest.json" && git push
# 3. purge jsDelivr（@master 缓存可能滞后，push 后数分钟~更久）
curl "https://purge.jsdelivr.net/gh/edcfoshan/polygon-txt@master/latest.json"
# 4. 复验（应返回新版本号 + 9 平台）
curl -s "https://cdn.jsdelivr.net/gh/edcfoshan/polygon-txt@master/latest.json"
```

> 若磁盘上旧目录残留 `*.sig`，`gen-latest-json.js` 取第一个会误嵌旧签名（见 CLAUDE.md），发版前先清理。

## macOS 构建注意事项

### Apple Silicon (M1/M2/M3)

- 使用 `macos-latest` + `aarch64-apple-darwin` 目标
- 生成的 `.dmg` 文件名包含 `aarch64`

### Intel Mac

- 使用 `macos-latest` + `x86_64-apple-darwin` 目标
- 生成的 `.dmg` 文件名包含 `x64`

### 代码签名（可选）

如果需要发布到 Mac App Store 或者避免 macOS 安全警告，需要：

1. Apple 开发者账号（$99/年）
2. 在 GitHub Secrets 中配置：
   - `APPLE_CERTIFICATE`: 证书文件（base64 编码）
   - `APPLE_CERTIFICATE_PASSWORD`: 证书密码
   - `APPLE_SIGNING_IDENTITY`: 签名身份
   - `APPLE_ID`: Apple ID
   - `APPLE_PASSWORD`: App 专用密码
   - `APPLE_TEAM_ID`: 团队 ID

## Linux 构建注意事项

### 系统依赖

Ubuntu 构建需要安装：
- `libwebkit2gtk-4.1-dev`
- `libappindicator3-dev`
- `librsvg2-dev`
- `patchelf`

这些已在 CI 配置中自动安装。

### 其他发行版

当前 CI 使用 Ubuntu 22.04，生成的 `.deb` 和 `.AppImage` 可在大多数基于 Debian/Ubuntu 的发行版上运行。

如需支持其他发行版（如 Fedora、Arch），需要修改 CI 配置添加对应的构建环境。

## 常见问题

### Q: 构建失败怎么办？

1. 查看 Actions 页面的错误日志
2. 常见原因：
   - 依赖版本不兼容
   - 系统依赖缺失
   - 代码编译错误

### Q: 如何只构建某个平台？

手动触发时可以修改 CI 配置，或者使用 `act` 工具本地测试。

### Q: Release 创建失败？

检查：
1. 格式是否为 `v*`（如 `v1.0.0`，不是 `1.0.0`）
2. GitHub Token 权限是否足够
3. 是否有同名 Release 已存在

### Q: 如何回滚 Release？

1. 前往 GitHub Releases 页面
2. 删除有问题的 Release 和对应的 tag
3. 修复问题后重新打 tag

## 参考链接

- [Tauri 官方文档 - 发布](https://tauri.app/v1/guides/building/)
- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [tauri-action 文档](https://github.com/tauri-apps/tauri-action)
