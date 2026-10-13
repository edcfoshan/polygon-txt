# R2 更新通道（国内加速）配置与成本护栏

用途：应用内自动更新的**主下载源**走 Cloudflare Worker + R2；Worker 判断无法命中 R2 时
302 回落 GitHub，客户端在网络层不可达时也会自行回落 GitHub，最后才引导百度网盘。

**硬约束：不产生任何 Cloudflare 费用。**

## 现状（2026-10-13 已上线）

| 项 | 值 |
| --- | --- |
| R2 桶 | `polygon-txt-updates`（私有，公开访问已禁用） |
| Worker | `polygon-txt-updates` → `https://polygon-txt-updates.chendeguan.workers.dev` |
| Worker secrets | `TICKET_KEY`（客户端密钥）、`GITHUB_REPO` |
| 对象布局 | `versions/<版本>/setup.exe`、`latest.txt` |
| 每 IP 限流 | 30 次 / 60 秒 |
| 已同步版本 | 4.5.0（6,362,978 字节） |

## 成本为什么不会爆

| 环节 | 免费额度 | 本方案上限 |
| --- | --- | --- |
| Workers 请求 | Free 套餐 100,000 次/天，超限 **1027**（不转付费） | 全局硬闸：每天最多 10 万次下载请求 |
| R2 B 类操作（读） | 1,000 万次/月 | 每请求最多读 1 次 → 约 300 万次/月，用不满 |
| R2 出站流量 | 免费 | 无计费项 |
| R2 存储 | 10 GB/月 | 只保留最新 2 版，约 13 MB |

前提两条必须同时成立：

1. Worker 必须留在 **Workers Free** 套餐（不要升级 Paid，Paid 无每日请求上限）。
2. 该域名链路上不要启用按请求计费的付费产品。

## 发版怎么同步

CI **不再**上传 R2（避免为它维护一份长期 API Token）。发版后在本机执行一次：

```powershell
node scripts/sync-r2-local.mjs --version 4.6.0 `
  --file "00安装包相关/极思G界址点互转工具_4.6.0_x64-setup.exe" --prune
```

脚本用 `npx wrangler login` 的 OAuth 会话调 REST API：上传 → 列举核对字节数 →
写 `latest.txt`（含 SHA-256）→ `--prune` 只留最新两版。
凭据过期时重跑一次 `npx wrangler login` 即可。

没同步也不会让用户下载失败：Worker 找不到对象就 302 到 GitHub，`latest.json`
里还有 `github_url` 兜底。

## 自检

```powershell
# 1) 带密钥：期望 200 并开始下载（大陆需按 DoH 真实 IP 访问，见下方“已知限制”）
curl.exe -sI -H "X-Client-Key: <密钥>" "https://polygon-txt-updates.chendeguan.workers.dev/download/4.5.0"

# 2) 不带密钥：期望 302 到 GitHub（防外部盗链刷 R2 读次数）
curl.exe -sI "https://polygon-txt-updates.chendeguan.workers.dev/download/4.5.0"

# 3) Worker 护栏单测（路径校验 / 回落 / 限流 / 超限拒绝）
node --test cloudflare/update-download/worker.test.mjs

# 4) 用量核对：R2 概览页“总储存 / A 类 / B 类”与账单仪表板应保持 $0.00
```

## 已知限制

- `*.workers.dev` 在中国大陆宽带被 DNS 污染（实测解析到 `28.0.0.x`），
  需要按真实 IP 才能连通；客户端会因此回退 GitHub 直链。
  该域名未备案，大陆全设备稳定访问需要备案域名 + 自定义域，属于额外成本项，明确不做。
- 旧客户端（≤4.5.0）不认识 `github_url`：Worker 可达时正常更新，不可达时落到百度云提示。
- 客户端密钥随二进制分发、可被逆向提取；它只是提高盗链门槛，
  真正的成本护栏是 Workers Free 的每日请求上限。