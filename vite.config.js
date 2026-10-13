import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import fs from "fs";
import path from "path";

// ─── 弹窗配置（读取 content/modal-config.json） ───
function modalConfigPlugin() {
  const configPath = path.resolve("content/modal-config.json");
  return {
    name: "modal-config",
    transformIndexHtml(html) {
      let cfg = { about: {} };
      try {
        cfg = JSON.parse(fs.readFileSync(configPath, "utf-8"));
      } catch { /* use defaults */ }
      const a = cfg.about || {};
      const vars = `
/* ─── modal-config.json injected ─── */
:root{
--m-a-w:${a.cardWidth || "420px"};--m-a-p:${a.padding || "24px 28px 20px"};--m-a-ts:${a.titleFontSize || "13px"};--m-a-bs:${a.bodyFontSize || "12px"};--m-a-im:${a.imageMaxHeight || "200px"};
}
`;
      return html.replace("</style>", vars + "\n</style>");
    },
    handleHotUpdate({ file, server }) {
      if (file === configPath) {
        server.ws.send({ type: "full-reload" });
        return [];
      }
    },
  };
}

export default defineConfig({
  define: {
    // 构建时间戳：注入 about 弹窗，排查「跑的是哪个构建」的版本错位问题
    __BUILD_TS__: JSON.stringify(new Date().toLocaleString("sv-SE", { hour12: false }).replace("T", " ")),
    // R2 下载通道的客户端密钥：只在打包时从环境变量注入（CI 用 GitHub secret），
    // 不写进仓库源码。为空时前端不带该头，Worker 会 302 回 GitHub。
    // 说明：该值仍可从二进制中提取，它只是提高盗链门槛，真正的成本护栏是
    // Workers Free 的每日 10 万请求硬上限。
    __R2_CLIENT_KEY__: JSON.stringify(process.env.R2_CLIENT_KEY || ""),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/target/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "esnext",
    minify: false,
  },
  plugins: [modalConfigPlugin(), viteSingleFile()],
});
