// 极思G界址点互转工具 — 更新安装包下载 Worker
//
// 成本边界（用户硬约束：只用免费额度、不产生账单）：
//   1. 本 Worker 必须跑在 Workers Free 套餐上，账号级硬上限 100,000 请求/天，
//      超限 Cloudflare 直接返回 1027，不会转成付费请求。
//   2. 因此本路由的 R2 读次数 <= 100,000/天（约 300 万/月），低于 R2 每月
//      1000 万次 B 类操作免费额度；R2 出站流量免费。
//   3. 每个请求最多读 1 次 R2，无重试、无 list、无 head。
//   4. 任何 R2 侧异常（对象缺失 / 读取失败）一律 302 跳 GitHub，
//      不读 R2、不产生 R2 计费。
//   5. 未携带客户端密钥的请求同样 302 跳 GitHub，避免被外部盗链刷 R2 读次数。
//
// 诚实说明：客户端密钥随软件二进制分发，能被逆向提取，所以它只是
// 提高盗链门槛的“路障”，不是密码学保证。真正的成本护栏是第 1 条的
// 账号级每日请求硬上限。
//
// 绑定：UPDATES（R2 桶）、TICKET_KEY（客户端密钥）、GITHUB_REPO（回落仓库）
// 环境变量：DOWNLOAD_LIMIT（每 IP 限流，见 wrangler.jsonc）

const MAX_INSTALLER_BYTES = 64 * 1024 * 1024;
const VERSION_RE = /^\d{1,3}\.\d{1,3}\.\d{1,3}$/;

function json(status, body) {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json",
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
    },
  });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const match = /^\/download\/(\d{1,3}\.\d{1,3}\.\d{1,3})$/.exec(url.pathname);
    if (request.method !== "GET" || !match || url.search) {
      return json(404, { error: "Not found" });
    }
    const version = match[1];
    if (!VERSION_RE.test(version)) return json(404, { error: "Not found" });
    if (!env.GITHUB_REPO) return json(503, { error: "Download temporarily unavailable" });

    const github = `https://github.com/${env.GITHUB_REPO}/releases/download/v${version}/JisigG_${version}_x64-setup.exe`;
    const redirect = () => Response.redirect(github, 302);

    // 未配置密钥时整体走 GitHub 回落，绝不读 R2（本地/未签名构建即此路径）。
    if (!env.TICKET_KEY) return redirect();
    if (request.headers.get("X-Client-Key") !== env.TICKET_KEY) return redirect();

    const ip = request.headers.get("CF-Connecting-IP") || "unknown";
    if (env.DOWNLOAD_LIMIT && !(await env.DOWNLOAD_LIMIT.limit({ key: ip })).success) {
      return json(429, { error: "Too many requests" });
    }

    try {
      const object = await env.UPDATES.get(`versions/${version}/setup.exe`);
      if (!object) return redirect();
      if (object.size > MAX_INSTALLER_BYTES) {
        await object.body.cancel();
        return json(413, { error: "Installer too large" });
      }
      return new Response(object.body, {
        headers: {
          "Content-Type": "application/octet-stream",
          "Content-Length": String(object.size),
          "Content-Disposition": `attachment; filename="JisigG_${version}_x64-setup.exe"`,
          "Cache-Control": "no-store",
          "X-Content-Type-Options": "nosniff",
        },
      });
    } catch {
      // R2 异常不得变成对用户的失败：退回 GitHub。
      return redirect();
    }
  },
};