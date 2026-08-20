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
