# 发版流程（含自动更新签名）

本文档补充 `release` skill：每次发版除原有步骤外，**必须签名 NSIS 安装包并上传 `latest.json`**，否则老版本客户端无法收到更新提醒。

> 自动更新方案采用 Tauri Updater + 国内多源检查加速（jsDelivr + GitHub）+ 百度云下载兜底。

---

## 一次性准备：生成签名密钥对（仅首次）

```powershell
npm run tauri signer generate -- -w $env:USERPROFILE\.tauri\bpoint-converter.key
```

- 命令会要求设置密码（**记牢**），输出**公钥**（base64 字符串）
- 私钥文件 `bpoint-converter.key` 必须妥善备份；**私钥丢失 = 无法再给现有用户推更新**
- 把公钥填入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`
- 私钥**永远不要**提交到 git；`.gitignore` 应排除 `*.key`

> ⚠️ 生产环境建议同时配置 GitHub Action（见末尾），把密钥放到 repo Secrets，本地不留私钥。

---

## 每次发版：在原 release skill 基础上追加 3 步

### A. 设置签名环境变量（构建前）

**前提：** `tauri.conf.json` 的 `bundle.createUpdaterArtifacts` 必须为 `true`（已配置）。否则 tauri build 不会生成 `.sig`，老用户收不到更新提醒。

推荐用交互脚本（密码不回显，最安全）：
```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-signed.ps1
```
脚本自动：读私钥 → 提示输密码 → `npm run tauri build` → 检查 `.sig` 是否生成。

或手动设环境变量：
```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $env:USERPROFILE\.tauri\bpoint-converter.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "你的密码"
npm run tauri build
```

然后执行原有的 `npm run tauri build`。产物目录会多出 `.sig` 签名文件：
```
src-tauri/target/release/bundle/nsis/极思G界址点互转工具_X.Y.Z_x64-setup.exe
src-tauri/target/release/bundle/nsis/极思G界址点互转工具_X.Y.Z_x64-setup.exe.sig   ← 新增
```

### B. 生成 latest.json

```powershell
node scripts/gen-latest-json.js --notes "本次更新内容……"
```

脚本会自动：
- 从 `package.json` 读版本号
- 扫描 NSIS 目录的 `.sig` 文件读签名
- 组装 URL（默认 GitHub releases 直连；`--mirror` 可加 ghproxy 前缀，文件名含中文自动 `encodeURI`）
- 写出仓库根目录 `latest.json`（jsDelivr 端点从这里取）

如不需要镜像：`--no-mirror`。

### C. 上传到 GitHub Release + 提交 latest.json

把以下文件上传到 Release `vX.Y.Z`：
- `极思G界址点互转工具_X.Y_x64-setup.exe`（NSIS 安装包，按原 skill 重命名）
- `latest.json`（**必须**上传，否则 GitHub 端点拉不到）
- 绿色 exe、tbx 等按原 skill 习惯

并提交 latest.json 到仓库：
```powershell
git add latest.json
git commit -m "release: vX.Y.Z latest.json"
git push
```

> **为什么 latest.json 要同时放 Release 资产 + 仓库根目录？**
> - GitHub 端点 `releases/latest/download/latest.json` 取的是 Release 资产
> - jsDelivr 端点 `cdn.jsdelivr.net/gh/.../latest.json` 取的是仓库根文件（国内首选，秒级）
> - 两份内容必须一致，脚本生成的同一文件分别上传/提交即可

---

## latest.json 格式（脚本自动生成，参考）

```json
{
  "version": "1.2.1",
  "notes": "更新说明……",
  "pub_date": "2026-06-26T00:00:00.000Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<.sig 文件全文>",
      "url": "https://mirror.ghproxy.com/https://github.com/edcfoshan/polygon-txt/releases/download/v1.2.1/%E6%9E%81%E6%80%9DG%E7%95%8C%E5%9D%80%E7%82%B9%E4%BA%92%E8%BD%AC%E5%B7%A5%E5%85%B7_1.2_x64-setup.exe"
    }
  }
}
```

---

## 客户端检查更新的流程（已实现）

1. 应用启动 → `checkAppUpdate(false)` 静默并发检查，失败不报错
2. Tauri Updater 按顺序尝试 endpoints 数组（jsDelivr → GitHub），任一返回合法 JSON 即用
3. 比对版本号 → 有新版则标题栏出现绿色脉冲箭头 `#btnUpdate`
4. 点箭头 → 模态框显示版本号/更新说明/进度条 →「立即更新」→ `downloadAndInstall` → 校验签名 → 自动安装重启
5. 下载失败 → 兜底 `confirm` 引导跳浏览器手动下载

---

## 国内加速架构

| 环节 | 机制 |
|------|------|
| 检查更新（拉 latest.json，KB 级） | endpoints 数组：jsDelivr（仓库根 latest.json，秒级）→ GitHub Release |
| 下载安装包（5MB） | latest.json 内 `url` 走 GitHub releases **直连**（国内约 15s 下完） |
| 下载失败兜底 | 前端 confirm → 跳百度云手动下载；弹窗常驻百度云链接 |

**为什么不用 ghproxy 镜像：** 1.3.0 发版实测 `mirror.ghproxy.com` 已失效，多个替代镜像（ghproxy.net / gh-proxy.com / ghps.cc）测试均无加速效果（与直连同耗时 ~15s）且不稳定。直连 GitHub releases 最稳，5MB 文件 15s 可接受。

**若用户量大需更快下载：** 改用付费对象存储（腾讯 COS/阿里 OSS + CDN）承载安装包，把 `latest.json` 的 `url` 指向 CDN 地址即可，endpoint 不变。`gen-latest-json.js` 加 `--mirror` 可切回 ghproxy 前缀（仅当镜像恢复时）。

---

## 进阶：GitHub Action 自动发版（强烈推荐）

避免本地维护私钥，建议用 `tauri-apps/tauri-action@v0`，把 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 放到 repo Secrets。该 action 自动：构建 → 签名 → 生成 `.sig` → 创建 Release → 上传全部资产。latest.json 仍需脚本生成后一并上传（或在 action 里追加一步）。本步骤暂未落地，等发版流程稳定后再做。
