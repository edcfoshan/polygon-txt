// 浏览器开发预览固定应用视口；Tauri 与生产构建直接启动应用。
const params = new URLSearchParams(window.location.search);
const browserPreview = import.meta.env.DEV
  && !window.__TAURI_INTERNALS__
  && params.get('desktopPreviewContent') !== '1';

if (browserPreview) {
  const width = 1000;
  const height = 700;
  const style = document.createElement('style');
  style.textContent = `
    html, body { width:100%; height:100%; margin:0; overflow:hidden; }
    body { display:flex; flex-direction:column; align-items:center;
      justify-content:center; gap:12px; padding:16px; box-sizing:border-box; }
    .dev-preview-label { flex:none; font:12px/20px system-ui,sans-serif; color:var(--tx2); }
    .dev-preview-stage { position:relative; flex:none; }
    .dev-preview-frame { position:absolute; top:0; left:0; width:1000px; height:700px;
      border:0; transform-origin:top left; background:var(--srf);
      box-shadow:0 0 0 1px var(--brd),0 8px 32px rgba(0,0,0,.12); }
  `;
  document.head.append(style);

  const label = document.createElement('div');
  label.className = 'dev-preview-label';
  label.textContent = '开发者模式 · 浏览器预览 1000 × 700';
  const stage = document.createElement('div');
  stage.className = 'dev-preview-stage';
  const frame = document.createElement('iframe');
  frame.className = 'dev-preview-frame';
  frame.title = '界址点互转工具 — 1000×700 开发预览';
  const contentUrl = new URL(window.location.href);
  contentUrl.searchParams.set('desktopPreviewContent', '1');
  frame.src = contentUrl.href;
  stage.append(frame);
  document.body.replaceChildren(label, stage);

  const fit = () => {
    const scale = Math.max(0.1, Math.min(1,
      (window.innerWidth - 32) / width,
      (window.innerHeight - 64) / height));
    stage.style.width = `${width * scale}px`;
    stage.style.height = `${height * scale}px`;
    frame.style.transform = `scale(${scale})`;
  };
  fit();
  window.addEventListener('resize', fit);
} else {
  import('./main.js');
}
