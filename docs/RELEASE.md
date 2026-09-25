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

### B. 上传产物，取回真实下载链接，再生成 latest.json

**先上传后生成**，因为资产链接里的文件名不是你以为的那个：GitHub 有两层名字。

| 层 | 字段 | 规则 |
|---|---|---|
| URL 与落盘名 | `name`（slug） | GitHub 服务端静默吃掉非 ASCII：连续一段折叠成一个 `.`、开头整段删除。`极思G界址点互转工具_4.4.0_x64-setup.exe` → `G._4.4.0_x64-setup.exe`，且它同时是 `Content-Disposition` 里的存盘文件名。**无法避免**，换上传工具/换平台都没用 |
| 列表显示名 | `label` | 可以是中文。`tauri-action` 会自动设成中文原名；`gh release upload` **不会**设 |
| 解法 | 两者分开放 | `scripts/normalize-release-assets.mjs`：slug → `JisigG_{ver}_*`（品牌 ASCII，可辨识），label → 中文原名（列表照旧显示中文）。只改元数据，不重传，签名不受影响 |

```bash
# 下面以 4.4.0 为例，实际发版把 4.4.0 / v4.4.0 换成本次版本号
# 以下命令在 Git Bash 里执行（与 CI 同语法）；PowerShell 需自行转换 $() 与变量写法
# 1) 上传安装包与便携版（slug 会被 GitHub 折叠，属正常）
gh release upload v4.4.0 "src-tauri/target/release/bundle/nsis/极思G界址点互转工具_4.4.0_x64-setup.exe" "…exe.sig" "…_x64-portable.exe" --clobber

# 2) 规范化资产名：slug 改成品牌 ASCII（JisigG_…），中文原名放 label（列表显示名）。
#    不跑这一步，用户复制链接与存盘拿到的都是 G._4.4.0_x64-setup.exe。
#    只 PATCH 元数据、不重传文件；.sig 签的是 exe 字节，签名照旧有效。幂等，可加 --dry-run 预览。
node scripts/normalize-release-assets.mjs --repo edcfoshan/polygon-txt --tag v4.4.0

# 3) 取规范化之后的真实下载链接（只信 API——slug 同时决定 URL 和落盘文件名）
SETUP_URL=$(gh api repos/edcfoshan/polygon-txt/releases/tags/v4.4.0 -q '.assets[] | select(.name | startswith("JisigG_")) | select(.name | endswith("_x64-setup.exe")) | .browser_download_url' | head -1)

# 4) 用真实链接生成 latest.json
node scripts/gen-latest-json.js --version 4.4.0 --tag v4.4.0 --download-url "$SETUP_URL"
```

脚本会自动：从 `package.json`（或 `--version`）读版本号 → 取 NSIS 目录里**最新**的 `.sig` → 用 `--download-url` 的链接 → 写仓库根 `latest.json`（jsDelivr 端点从这里取）。

不带 `--download-url` 时脚本只能按 `productName` 拼链接，含中文会打 **警告**且该链接上传后必 404（v4.4.0 就是这么坏的）。`.sig` 签的是 exe 字节，与文件名/label 无关，改名不影响验签。

### C. 提交并验收

上传 `latest.json` 到同一 Release，并提交到仓库：
```bash
gh release upload v4.4.0 latest.json --clobber
git add latest.json && git commit -m "release: vX.Y.Z latest.json" && git push
curl "https://purge.jsdelivr.net/gh/edcfoshan/polygon-txt@master/latest.json"   # 必须 purge
```

**发版后逐条验收（少一条就可能全量翻车）**：

```bash
# ① 资产名与重复项：label 全中文、slug 全 JisigG_*，且不能出现两份 setup（或 polygon-txt_* / G._* 残留）
gh api repos/edcfoshan/polygon-txt/releases/tags/v4.4.0 -q '.assets[] | "\(.name)\t| \(.label)"'
# ② 两个端点都要返回新版本号，且 url 一致
curl -sL https://cdn.jsdelivr.net/gh/edcfoshan/polygon-txt@master/latest.json | node -pe "JSON.parse(require('fs').readFileSync(0,'utf8')).version"
curl -sL https://github.com/edcfoshan/polygon-txt/releases/download/v4.4.0/latest.json | node -pe "JSON.parse(require('fs').readFileSync(0,'utf8')).platforms['windows-x86_64'].url"
# ③ 该 url 必须实测可达（404 = 老客户端点了更新就失败）
curl -sfIL --max-time 60 "<上一条输出的 url>" && echo "updater OK"
# ④ 落盘文件名必须是 JisigG_…，出现 G._… 说明 Normalize 步骤没跑或没生效
curl -sIL --max-time 60 "<上一条输出的 url>" | grep -i "^content-disposition"
```

> **为什么 latest.json 要同时放 Release 资产 + 仓库根目录？**
> - GitHub 端点 `releases/latest/download/latest.json` 取的是 Release 资产
> - jsDelivr 端点 `cdn.jsdelivr.net/gh/.../latest.json` 取的是仓库根文件（国内首选，秒级）
> - 两份内容必须一致；注意 Windows job 结束后 `finalize-notes` 会合并平台条目并重传 Release 那份

---

## latest.json 格式（脚本自动生成，参考）

```json
{
  "version": "4.4.0",
  "notes": "更新说明……",
  "pub_date": "2026-09-25T00:00:00.000Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<.sig 文件全文>",
      "url": "https://github.com/edcfoshan/polygon-txt/releases/download/v4.4.0/G._4.4.0_x64-setup.exe"
    }
  }
}
```

url 里是 `G._…` 而不是中文名，是对的——那是 GitHub 折叠后的真实 slug（见 B 节）。若某次发版这里出现中文/percent-encode 的中文，说明忘了传 `--download-url`，客户端下载会 404。

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

**已落地（自 v3.3.0 起，v4.4.0 在用）**：`.github/workflows/release.yml` 用 `tauri-apps/tauri-action@v1` + repo Secrets（`TAURI_SIGNING_PRIVATE_KEY` / `..._PASSWORD`）在 push `v*` tag 时四平台构建并签名，Windows job 会自动完成 B/C 节里手工做的那几步（查真实 slug → 生成 latest.json → 给便携版补 label → 重传 latest.json），`finalize-notes` job 最后把 notes 换成 CHANGELOG 段落。**日常发版只需 push tag**，B/C 节是本地补签或排查时才用。发版后仍要按 C 节的三条验收实测一遍。详见 [CI/CD](CI-CD.md)。
