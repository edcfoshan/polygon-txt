import { invoke } from '@tauri-apps/api/core';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import aboutContent from '../content/about.md?raw';
import sponsorContent from '../content/sponsor.md?raw';
import aboutQrImage from '../content/讨论群.jpg?inline';
import sponsorQrImage from '../content/关注、赞赏码.png?inline';

// Tauri IPC 调用
async function tauriInvoke(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    console.error('[Tauri] invoke error:', cmd, summarizeInvokeArgs(args), e);
    throw e;
  }
}

function summarizeInvokeArgs(args) {
  if (!args || typeof args !== "object") return args;
  const summary = {};
  for (const [key, value] of Object.entries(args)) {
    if (Array.isArray(value)) {
      summary[key] = `array(${value.length})`;
    } else if (typeof value === "string") {
      summary[key] = value.length > 120 ? `${value.slice(0, 120)}...` : value;
    } else if (value && typeof value === "object") {
      summary[key] = `object(${Object.keys(value).length})`;
    } else {
      summary[key] = value;
    }
  }
  return summary;
}

async function runWindowCommand(command) {
  try {
    await tauriInvoke(command);
  } catch (e) {
    console.error(`[Tauri] ${command} failed:`, e);
    toast(`窗口控制失败: ${e}`);
    throw e;
  }
}

const MARKDOWN_IMAGE_MAP = {
  "content/讨论群.jpg": aboutQrImage,
  "讨论群.jpg": aboutQrImage,
  "content/关注、赞赏码.png": sponsorQrImage,
  "关注、赞赏码.png": sponsorQrImage,
};

// ═══ State ═══
let loadedFiles = [];
let txtFiles = [];
let cur = "usr";
let cfgs = {};
let headerManual = {};
let lastPreviewKey = "";
let previewTimer = null;
let theme = "light";
let sourceType = null;
let sourcePath = null;
let gdbLayers = [];
let selectedLayers = [];

const FIELD_MATCH_RULES = {
  fn: ["DKMC", "MC", "NAME"],
  fi: ["DKBH", "BH", "ID"],
  fa: ["MJ", "AREA"],
  fu: ["DKYT", "YT", "YONGTU"],
  fm: ["TFH"],
  fd: ["DLBM", "DL"],
};

const PP = [
  { id: "usr", n: "自定义", h: { c: "2000国家大地坐标系", b: "3", j: "高斯克吕格", u: "米", z: "", a: "0.001", t: ",,,,,," }, p: { pp: 3, pz: "auto", ox: 0, oj: 0, on: 0, oo: 1, om: 0 }, f: { fn: "DKMC", fi: "DKBH", fa: "", fu: "", fm: "", fd: "" } },
];

const $ = (id) => document.getElementById(id);

// ═══ Toast ═══
function toast(m) {
  const t = $("toast");
  if (!t) return;
  t.textContent = m;
  t.classList.add("on");
  clearTimeout(t._h);
  t._h = setTimeout(() => t.classList.remove("on"), 1500);
}

// ═══ Theme ═══
window.togTheme = function () {
  theme = theme === "light" ? "dark" : "light";
  document.documentElement.setAttribute("data-t", theme);
  localStorage.setItem("tg_theme", theme);
};

// ═══ 弹窗内容渲染（Markdown → HTML） ═══
function renderMarkdown(md) {
  let html = '';
  const lines = md.split('\n');
  let inList = false;
  for (const line of lines) {
    if (line.startsWith('### ')) { closeList(); html += `<h3 style="text-align:center;margin-bottom:8px">${escHtml(line.slice(4))}</h3>\n`; continue; }
    if (line.startsWith('- ')) {
      if (!inList) { html += '<ul style="margin:0 0 8px 16px;padding:0">\n'; inList = true; }
      html += `  <li>${inlineMd(line.slice(2))}</li>\n`;
      continue;
    }
    if (line.trim() === '') { closeList(); continue; }
    if (line.startsWith('---')) { closeList(); html += '<hr style="border:none;border-top:1px solid var(--brd);margin:10px 0">\n'; continue; }
    closeList();
    html += `<p style="margin-bottom:6px">${inlineMd(line)}</p>\n`;
  }
  closeList();
  function closeList() { if (inList) { html += '</ul>\n'; inList = false; } }
  function escHtml(s) { return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;'); }
  function normalizeMdImageSrc(src) {
    const s = src.trim().replace(/\\/g, '/');
    const contentIdx = s.lastIndexOf('/content/');
    if (contentIdx >= 0) {
      const normalized = s.slice(contentIdx + 1);
      return MARKDOWN_IMAGE_MAP[normalized] || normalized;
    }
    if (/^[A-Za-z]:\//.test(s)) return '';
    if (/^(https?:|data:|asset:|\/)/.test(s)) return s;
    if (s.startsWith('content/')) return MARKDOWN_IMAGE_MAP[s] || s;
    return MARKDOWN_IMAGE_MAP[s] || `content/${s}`;
  }
  function inlineMd(t) {
    t = escHtml(t);
    t = t.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
    t = t.replace(/!\[(.+?)\]\((.+?)\)/g, (_, alt, src) => {
      const normalizedSrc = normalizeMdImageSrc(src);
      // 不再硬编码小尺寸，交给 modal CSS 的 max-width/max-height 控制，
      // 二维码图片可放大到 280×280 方便扫码
      return `<img src="${escHtml(normalizedSrc)}" alt="${alt}" style="border-radius:6px;border:1px solid var(--brd);display:block;margin:0 auto">`;
    });
    t = t.replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" target="_blank" style="color:var(--ac)">$1</a>');
    return t;
  }
  return html;
}

// ═══ SHP 导入 ═══
window.importShp = async function () {
  try {
    const result = await tauriInvoke("pick_shp_files");
    if (result.skipped && result.skipped.length) {
      toast(`以下文件不是面状要素，已忽略：${result.skipped.join("、")}`);
    }
    if (!result.files || result.files.length === 0) return;
    loadedFiles = result.files;
    sourceType = null;
    sourcePath = null;
    gdbLayers = [];
    selectedLayers = [];
    renderFileList();
    processImport();
    autoSetOutputDirS(loadedFiles[0]?.shp_path);
  } catch (e) {
    toast("导入失败: " + e);
  }
};

// ═══ GDB 导入 ═══
// 流程：选 .gdb 文件夹 → 后端枚举面状要素类 → 自动弹出选择框（默认不选）
// → 用户勾选并确认 → 结果以单行汇总落在左栏，预览区刷新。
window.importGdb = async function () {
  try {
    const result = await tauriInvoke("import_gdb");
    if (!result || !result.path) return;
    if (result.skipped && result.skipped.length) {
      toast(`以下图层不是面状要素，已过滤：${result.skipped.join("、")}`);
    }
    sourceType = "gdb";
    sourcePath = result.path;
    loadedFiles = [];           // SHP/GDB 互斥：导入 GDB 清空 SHP 列表
    gdbLayers = result.layers || [];
    selectedLayers = [];        // 弹窗内默认不选；确认时才提交
    window._gdbName = result.name;
    autoMatchFields(result.field_names);
    autoSetOutputDirS(result.path);
    toast(`已读取 GDB: ${result.name}（${result.layers.length} 个面状要素类），请在弹窗中勾选`);
    renderLeftGdbSummary();     // 左栏先显示"待选择"占位行
    openGdbSelectModal();       // 直接弹出要素类选择框
  } catch (e) {
    toast("导入 GDB 失败: " + e);
  }
};

// ── 弹窗临时态：弹窗打开期间记录勾选，确认时才写入 selectedLayers ──
let gdbTempSelected = [];

// 打开要素类选择弹窗（用当前 selectedLayers 初始化临时态，实现"重开恢复上次勾选"）
window.openGdbSelectModal = function () {
  const m = $("gdbSelectModal");
  if (!m) return;
  gdbTempSelected = [...selectedLayers];
  const sub = $("gdbSelectGdbName");
  if (sub && window._gdbName) sub.textContent = `${window._gdbName}.gdb · 共 ${gdbLayers.length} 个面状要素类`;
  renderGdbSelectList();
  m.classList.add("on");
};

// 关闭弹窗：不改 selectedLayers（保留上次确认值）；点遮罩/取消/Esc 走这里
window.closeGdbSelectModal = function (e) {
  if (e && e.target && e.target.id && e.target.id !== "gdbSelectModal" && e.type === "click") return;
  const m = $("gdbSelectModal");
  if (m) m.classList.remove("on");
};

// 确认：提交临时态 → 关闭 → 刷新左栏汇总 + 右栏预览
window.confirmGdbSelect = function () {
  if (gdbTempSelected.length === 0) {
    toast("请至少选择一个要素类");
    return;
  }
  selectedLayers = [...gdbTempSelected];
  window.closeGdbSelectModal();
  renderLeftGdbSummary();
  toast(`已选定 ${selectedLayers.length}/${gdbLayers.length} 个要素类`);
  updatePreview();
};

// 渲染弹窗内的要素类勾选列表（操作 gdbTempSelected，不动 selectedLayers）
function renderGdbSelectList() {
  const list = $("gdbSelectList");
  if (!list) return;
  const updateCount = () => {
    const c = $("gdbSelectCount");
    if (c) c.textContent = `已选 ${gdbTempSelected.length} / ${gdbLayers.length}`;
    const btn = $("btnConfirmGdbSelect");
    if (btn) {
      const empty = gdbTempSelected.length === 0;
      btn.disabled = empty;
      btn.textContent = empty ? "确认" : `确认（${gdbTempSelected.length} 个）`;
    }
  };

  const header = '<div class="gdb-sel-th"><span>要素类名</span><span>几何类型</span><span>要素数</span></div>';
  const toolbar = '<div class="gdb-sel-toolbar">'
    + '<a data-act="all">全选</a><a data-act="none">全不选</a><a data-act="invert">反选</a>'
    + '</div>';
  const rows = gdbLayers.map((layer) => {
    const checked = gdbTempSelected.includes(layer.name) ? "checked" : "";
    const gtype = layer.geometry_type || "?";
    return `<label class="ck" style="padding:5px 8px;border-bottom:1px solid var(--brd);font-size:11px"><input type="checkbox" data-layer="${layer.name}" ${checked}><span style="flex:1">${layer.name}</span><span style="width:60px;text-align:center;color:var(--tx3);font-size:10px">${gtype}</span><span style="width:55px;text-align:right;color:var(--tx3);font-size:10px">${layer.num_features}</span></label>`;
  }).join("");

  list.innerHTML = `<div class="gdb-sel-tbl">${header}${toolbar}${rows}</div>`;

  // checkbox 变化 → 更新临时态 + 计数/按钮态（不刷新预览，预览留到确认时）
  list.querySelectorAll("input[type=checkbox]").forEach((cb) => {
    cb.addEventListener("change", () => {
      const name = cb.dataset.layer;
      if (cb.checked) {
        if (!gdbTempSelected.includes(name)) gdbTempSelected.push(name);
      } else {
        gdbTempSelected = gdbTempSelected.filter((n) => n !== name);
      }
      updateCount();
    });
  });

  // 全选/全不选/反选
  list.querySelectorAll("a[data-act]").forEach((a) => {
    a.addEventListener("click", (e) => {
      e.preventDefault();
      const act = a.dataset.act;
      if (act === "all") {
        gdbTempSelected = gdbLayers.map((l) => l.name);
      } else if (act === "none") {
        gdbTempSelected = [];
      } else if (act === "invert") {
        gdbTempSelected = gdbLayers.filter((l) => !gdbTempSelected.includes(l.name)).map((l) => l.name);
      }
      renderGdbSelectList();
    });
  });

  updateCount();
}

// ═══ 左栏 GDB 单行汇总 ═══
// 渲染为一行 ◈ xxx.gdb 选中N/M ×，点行重开弹窗、点×清空导入
// selectedLayers 为空（尚未确认选择）时显示"待选择"提示，避免误触发后端"空=全选"
function renderLeftGdbSummary() {
  const fl = $("fl");
  if (!fl) return;
  if (sourceType !== "gdb" || !gdbLayers.length) { fl.innerHTML = ""; return; }
  const total = gdbLayers.length;
  const sel = selectedLayers.length;
  const name = window._gdbName || "GDB";
  const cntText = sel === 0 ? "待选择" : `选中 ${sel}/${total}`;
  fl.innerHTML = `<div class="gitem${sel === 0 ? " gitem-pending" : ""}" id="gdbSumRow" title="点击选择要素类">`
    + `<span class="gicon">◈</span>`
    + `<span class="gname">${name}.gdb</span>`
    + `<span class="gcnt">${cntText}</span>`
    + `<span class="gclose" id="gdbSumClose" title="移除 GDB 导入">×</span>`
    + `</div>`;
  const row = $("gdbSumRow");
  if (row) row.addEventListener("click", (e) => {
    if (e.target.id === "gdbSumClose") return;   // × 单独处理
    openGdbSelectModal();
  });
  const close = $("gdbSumClose");
  if (close) close.addEventListener("click", (e) => {
    e.stopPropagation();
    clearGdbImport();
  });
}

// 清空 GDB 导入（× 按钮触发；与 clearAllFiles 共用清理逻辑，但不清输出目录、不 toast "已清空"）
function clearGdbImport() {
  sourceType = null;
  sourcePath = null;
  gdbLayers = [];
  selectedLayers = [];
  gdbTempSelected = [];
  window._gdbName = "";
  renderFileList();   // 复用现有函数清空 #fl
  updatePreview();
  toast("已移除 GDB 导入");
}


function renderFileList() {
  const fl = $("fl");
  if (!fl) return;
  fl.innerHTML = "";
  loadedFiles.forEach((g, i) => {
    fl.innerHTML += `<div class="fitem"><span class="fn">◈ ${g.name}.shp</span><span class="fs">${g.num_features}个</span><button class="fitem-close" data-remove-file="${i}">×</button></div>`;
  });
}

window.removeFile = function (i) {
  loadedFiles.splice(i, 1);
  if (!loadedFiles.length) { const fl = $("fl"); if (fl) fl.innerHTML = ""; lastPreviewKey = ""; updatePreview(); return; }
  renderFileList();
  updatePreview();
};

function processImport() {
  if (!loadedFiles.length) return;
  const first = loadedFiles[0];
  autoMatchFields(first.field_names || []);
  if (first.crs_info) autoFillHeader(first.crs_info);
  updatePreview();
  toast("已导入 " + loadedFiles.length + " 个文件");
}

function autoSetOutputDirS(filePath) {
  if (!filePath) return;
  const sep = filePath.includes("\\") ? "\\" : "/";
  const parts = filePath.split(sep);
  parts.pop();
  const dir = parts.join(sep) + sep + "临时数据";
  const inp = $("out_dir_s");
  if (inp && !inp.value) inp.value = dir;
}

function autoSetOutputDirT(filePath) {
  if (!filePath) return;
  const sep = filePath.includes("\\") ? "\\" : "/";
  const parts = filePath.split(sep);
  parts.pop();
  const dir = parts.join(sep) + sep + "临时数据";
  const inp = $("out_dir");
  if (inp && !inp.value) inp.value = dir;
}

function autoMatchFields(fieldNames) {
  for (const [key, rules] of Object.entries(FIELD_MATCH_RULES)) {
    const sel = $(key);
    if (!sel) continue;
    let matched = "";
    for (const r of rules) {
      if (fieldNames.includes(r)) { matched = r; break; }
    }
    sel.innerHTML = "";
    if (!matched) sel.innerHTML = '<option value="">无</option>';
    fieldNames.forEach((fn) => {
      sel.innerHTML += `<option value="${fn}"${fn === matched ? " selected" : ""}>${fn}</option>`;
    });
  }
}

function autoFillHeader(info) {
  const map = { c: "hc", b: "hb", j: "hj", u: "hu", z: "hz" };
  for (const [k, id] of Object.entries(map)) {
    if (!headerManual[id] && info[k]) {
      const el = $(id);
      if (el) { el.value = info[k]; el.style.borderColor = "var(--ac)";
        setTimeout(() => { el.style.borderColor = ""; }, 2000); }
    }
  }
}

function autoFillHeaderFromTxt(info) {
  const map = { "坐标系": "hc", "几度分带": "hb", "投影类型": "hj", "计量单位": "hu", "带号": "hz", "精度": "ha" };
  for (const [k, id] of Object.entries(map)) {
    if (!headerManual[id] && info[k]) {
      const el = $(id);
      if (el) { el.value = info[k]; el.style.borderColor = "var(--ac)";
        setTimeout(() => { el.style.borderColor = ""; }, 2000); }
    }
  }
}

// ═══ TXT 导入 ═══
window.importTxt = async function () {
  try {
    const result = await tauriInvoke("pick_txt_files");
    if (!result.files || result.files.length === 0) return;
    txtFiles = result.files;
    renderTxtFileList();
    renderTxtParseLog();
    if (txtFiles[0]?.crs_info) autoFillHeaderFromTxt(txtFiles[0].crs_info);
    autoSetOutputDirT(txtFiles[0]?.path);
  } catch (e) {
    toast("导入失败: " + e);
  }
};

function renderTxtFileList() {
  const fl = $("flT");
  if (!fl) return;
  fl.innerHTML = "";
  txtFiles.forEach((f, i) => {
    fl.innerHTML += `<div class="fitem"><span class="fn">◈ ${f.name}</span><span class="fs">${(f.size / 1024).toFixed(0)}KB</span><button class="fitem-close" data-remove-txt="${i}">×</button></div>`;
  });
}

window.removeTxtFile = function (i) {
  txtFiles.splice(i, 1);
  if (!txtFiles.length) {
    const fl = $("flT"); if (fl) fl.innerHTML = "";
    const pv = $("pvT"); if (pv) pv.textContent = "等待导入 TXT 文件…";
    return;
  }
  renderTxtFileList();
  renderTxtParseLog();
};

function renderTxtParseLog() {
  const pv = $("pvT");
  if (!pv) return;
  pv.textContent = txtFiles.length
    ? txtFiles.map((f) => f.parse_log).join("\n\n")
    : "等待导入 TXT 文件…";
}

window.clearAllFiles = function () { loadedFiles = []; sourceType = null; sourcePath = null; gdbLayers = []; selectedLayers = []; gdbTempSelected = []; window._gdbName = ""; const fl = $("fl"); if (fl) fl.innerHTML = ""; const out = $("out_dir_s"); if (out) out.value = ""; toast("已清空"); };
window.clearAllFilesTxt = function () { txtFiles = []; const fl = $("flT"); if (fl) fl.innerHTML = ""; const pv = $("pvT"); if (pv) pv.textContent = "等待导入 TXT 文件…"; const out = $("out_dir"); if (out) out.value = ""; toast("已清空"); };

// ═══ Preview ═══
function updatePreview() {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(() => window.up(), 150);
}

window.up = async function () {
  const ha = $("ha")?.value || "0.001";
  const hpi = $("hpi")?.value || "";
  let out = "";
  if (hpi.trim()) out += `[项目信息]\n${hpi.trim()}\n`;
  out += `[属性描述]\n坐标系=${$("hc")?.value || ""}\n几度分带=${$("hb")?.value || ""}\n投影类型=${$("hj")?.value || ""}\n计量单位=${$("hu")?.value || ""}\n带号=${$("hz")?.value || ""}\n精度=${ha}\n转换参数=${$("ht")?.value || ""}\n[地块坐标]`;

  const cfg = getConfig();
  const opt = getOptions();
  const shpPaths = loadedFiles.map((f) => f.shp_path).filter(Boolean);

  if (shpPaths.length > 0 || sourcePath) {
    try {
      const txt = await tauriInvoke("read_shp_to_txt_preview", { shpPaths, sourceType, sourcePath, headerCfg: cfg.h, fieldMapping: cfg.f, options: opt, selectedLayers: sourceType === "gdb" ? selectedLayers : [] });
      if (txt) { const pv = $("pv"); if (pv) pv.textContent = txt; return; }
    } catch (e) { console.log("Preview error:", e); }
  }
  const pv = $("pv");
  if (pv) pv.textContent = out || "请先导入 SHP 或 GDB 文件";
  lastPreviewKey = out;
}

// ═══ Run ═══
window.runShpToTxt = async function () {
  const shpPaths = loadedFiles.map((f) => f.shp_path).filter(Boolean);
  if (!shpPaths.length && !sourcePath) { toast("请先导入 SHP 或 GDB 文件"); return; }
  // GDB 已导入但未勾选任何要素类：拦截，避免后端把"空"当作"全选"
  if (sourceType === "gdb" && selectedLayers.length === 0) {
    toast("请先在左栏点击 GDB 行，勾选要转换的要素类"); return;
  }

  const outDir = $("out_dir_s")?.value || "";
  if (!outDir) { toast("请先导入文件以设置输出路径"); return; }

  const cfg = getConfig();
  const opt = getOptions();
  try {
    const result = await tauriInvoke("run_shp_to_txt", { shpPaths, sourceType, sourcePath, headerCfg: cfg.h, fieldMapping: cfg.f, options: opt, outputDir: outDir, selectedLayers: sourceType === "gdb" ? selectedLayers : [] });
    toast("✓ " + result.message);
    const pf = $("pf"); const ps = $("ps");
    if (pf) pf.style.width = "100%";
    if (ps) ps.textContent = "完成";
  } catch (e) { toast("转换失败: " + e); }
};

window.runTxtToShp = async function () {
  if (!txtFiles.length) { toast("请先导入 TXT 文件"); return; }
  const outDir = $("out_dir")?.value || "";
  if (!outDir) { toast("请先导入文件以设置输出路径"); return; }

  const txtPaths = txtFiles.map((f) => f.path);
  const cfg = getConfig();
  try {
    const result = await tauriInvoke("run_txt_to_shp", {
      txtPaths,
      options: {
        output_shp: true,
        output_mode: document.querySelector('input[name="t_output_mode"]:checked')?.value || "one_to_one",
        filename_field: $("t_filename_field")?.value ?? "",
        output_dir: outDir,
        keep_lujin: $("t_keep_lujin")?.checked || false,
        keep_mingc: $("t_keep_mingc")?.checked || false,
      },
      headerCfg: cfg.h,
    });
    toast("✓ " + result.message);
    const pf = $("pfT");
    if (pf) pf.style.width = "100%";
  } catch (e) { toast("转换失败: " + e); }
};

// ═══ Config ═══
function getConfig() {
  return {
    h: { crs: $("hc")?.value || "", band: $("hb")?.value || "", proj: $("hj")?.value || "", unit: $("hu")?.value || "", zone: $("hz")?.value || "", precision: $("ha")?.value || "", transform: $("ht")?.value || "", project_info: $("hpi")?.value || "" },
    f: { name: $("fn")?.value || "", id: $("fi")?.value || "", area: $("fa")?.value || "", use_field: $("fu")?.value || "", tfh: $("fm")?.value || "", dlbm: $("fd")?.value || "" },
  };
}

function getOptions() {
  const outputMode = document.querySelector('input[name="output_mode"]:checked')?.value || "one_to_one";
  const filenameField = $("filename_field")?.value ?? "";
  return {
    ox: $("ox")?.checked || false,
    oj: $("oj")?.checked || false,
    on: $("on")?.checked || false,
    oo: $("oo")?.checked || false,
    output_mode: outputMode,
    filename_field: filenameField,
  };
}

// ═══ Tab switch ═══
window.sw = function (t) {
  document.querySelectorAll(".tab").forEach((e) => e.classList.remove("on"));
  const tab = document.querySelector(`[data-t="${t}"]`);
  if (tab) tab.classList.add("on");
  document.querySelector(".app").setAttribute("data-mode", t);
};

window.tg = function (h) {
  const a = h.querySelector(".arr");
  const b = h.nextElementSibling;
  if (a) a.classList.toggle("o");
  if (b) b.classList.toggle("o");
};

// ═══ Header tabs ═══
window.switchHdrTab = function (t) {
  document.querySelectorAll(".hdr-tab").forEach((e) => e.classList.toggle("on", e.dataset.tab === t));
  $("hdrAttr")?.classList.toggle("on", t === "attr");
  $("hdrProj")?.classList.toggle("on", t === "proj");
};

window.prefillProject = function () {
  const hpi = $("hpi");
  if (!hpi) return;
  hpi.value = `项目名称=
项目所在县区代码=
项目所在市县名称=
项目类别=
项目投资额=
开发用途=
总用地面积=
占用基本农田面积=
农用地面积=
耕地面积=
园地面积=
林地面积=
养殖水面面积=
其他农用地面积=
带K地类面积=
建设用地面积=
未利用地面积=
围填海面积=
是否增减挂钩项目=否
是否属于增减挂钩中发展改革小城镇试点项目=否
是否属于建设用地指标调整项目=否
备注=`;
  updatePreview();
};

window.resetDefaults = function () {
  if ($("hc")) $("hc").value = "2000国家大地坐标系";
  if ($("hb")) $("hb").value = "3";
  if ($("hj")) $("hj").value = "高斯克吕格";
  if ($("hu")) $("hu").value = "米";
  if ($("hz")) $("hz").value = "";
  if ($("ha")) $("ha").value = "0.001";
  if ($("ht")) $("ht").value = ",,,,,,";
  updatePreview();
  toast("已恢复默认");
};

window.tp = function (id) { const el = $(id); if (el) el.classList.toggle("hide"); };

window.selectOutputDir = async function () {
  const dir = await tauriInvoke("pick_output_dir");
  if (dir) { const inp = $("out_dir"); if (inp) inp.value = dir; }
};

window.selectOutputDirS = async function () {
  const dir = await tauriInvoke("pick_output_dir");
  if (dir) { const inp = $("out_dir_s"); if (inp) inp.value = dir; }
};

// ═══ Presets ═══
function renderChips() {
  const ch = $("ch");
  if (!ch) return;
  ch.innerHTML = "";
  Object.values(cfgs).forEach((c) => {
    ch.innerHTML += `<span class="chip${c.id === cur ? " on" : ""}" data-chip="${c.id}">${c.n}</span>`;
  });
}

window.ld = function (id) {
  if (!id) return;
  const c = cfgs[id] || PP.find((p) => p.id === id);
  if (!c) return;
  cur = id;
  if (c.h) { $("hc").value = c.h.c; $("hb").value = c.h.b; $("hj").value = c.h.j; $("hu").value = c.h.u; $("hz").value = c.h.z; $("ha").value = c.h.a; $("ht").value = c.h.t; }
  if (c.p) {
    if ($("ox")) $("ox").checked = !!c.p.ox;
    if ($("oj")) $("oj").checked = !!c.p.oj;
    if ($("on")) $("on").checked = !!c.p.on;
    if ($("oo")) $("oo").checked = !!c.p.oo;
    if ($("om")) $("om").checked = !!c.p.om;
  }
  if (c.f) Object.keys(c.f).forEach((k) => { const e = $(k); if (e) e.value = c.f[k]; });
  const cn = $("cn");
  if (cn) cn.textContent = c.n || "自定义";
  document.querySelectorAll(".chip").forEach((e) => e.classList.remove("on"));
  const cp = document.querySelector(`.chip[data-chip="${id}"]`);
  if (cp) cp.classList.add("on");
  localStorage.setItem("tg_last", id);
  updatePreview();
};

window.saveOnly = function () {
  const cn = $("cn");
  if (!cn) return;
  const newName = (cn.textContent || "").trim();
  if (!newName) { toast("请输入配置名称"); return; }
  let dupId = null;
  for (const [, v] of Object.entries(cfgs)) {
    if (v.n === newName) {
      if (v.id === cur) { dupId = cur; break; }
      toast("配置名「" + newName + "」已存在"); return;
    }
  }
  const c = getConfig();
  const cfgObj = { id: dupId || "u" + Date.now(), n: newName, h: c.h, p: getOptions(), f: c.f };
  cfgs[cfgObj.id] = cfgObj;
  localStorage.setItem("tg_dark", JSON.stringify(cfgs));
  cur = cfgObj.id;
  cn.textContent = cfgObj.n;
  renderChips();
  toast("已保存 「" + cfgObj.n + "」");
};

window.delCfg = function () {
  if (cur === "usr") { toast("内置预设不可删除"); return; }
  if (!confirm("确定删除方案「" + ($("cn")?.textContent || "") + "」？")) return;
  delete cfgs[cur];
  localStorage.setItem("tg_dark", JSON.stringify(cfgs));
  cur = "usr";
  ld("usr");
  renderChips();
  toast("已删除");
};

// ═══ Open GitHub ═══
window.openGitHub = async function () {
  try {
    await shellOpen("https://github.com/edcfoshan/txt-gdb-converter");
  } catch (e) { console.error("openGitHub:", e); }
};

// ═══ Modals ═══
window.openSponsor = function () { const m = $("sponsorModal"); if (m) m.classList.add("on"); };
window.closeSponsor = function (e) { if (e && e.target !== $("sponsorModal")) return; const m = $("sponsorModal"); if (m) m.classList.remove("on"); };
window.openAbout = function () { const m = $("aboutModal"); if (m) m.classList.add("on"); };
window.closeAbout = function (e) { if (e && e.target !== $("aboutModal")) return; const m = $("aboutModal"); if (m) m.classList.remove("on"); };
document.addEventListener("keydown", function (e) {
  if (e.key === "Escape") { const sm = $("sponsorModal"); const am = $("aboutModal"); const gm = $("gdbSelectModal"); if (sm) sm.classList.remove("on"); if (am) am.classList.remove("on"); if (gm) gm.classList.remove("on"); }
});

// ═══ Init ═══
function init() {
  const savedTheme = localStorage.getItem("tg_theme") || "light";
  theme = savedTheme;
  document.documentElement.setAttribute("data-t", theme);

  const s = localStorage.getItem("tg_dark");
  if (s) cfgs = JSON.parse(s);
  PP.forEach((p) => { if (!cfgs[p.id]) cfgs[p.id] = p; });
  renderChips();
  ld(localStorage.getItem("tg_last") || "usr");

  // ─── 注入弹窗内容（Markdown → HTML） ───
  const ab = $("aboutBody");
  if (ab) ab.innerHTML = renderMarkdown(aboutContent);
  const sb = $("sponsorBody");
  if (sb) sb.innerHTML = renderMarkdown(sponsorContent);

  // ─── Bind click events (replaces inline onclick) ───
  const bind = (id, fn) => { const el = $(id); if (el) el.addEventListener("click", fn); };
  bind("btnGitHub", () => openGitHub());
  bind("btnSponsor", () => openSponsor());
  bind("btnTheme", () => togTheme());
  bind("btnAbout", () => openAbout());
  bind("btnSave", () => saveOnly());
  bind("btnDel", () => delCfg());
  bind("btnWinMin", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    await runWindowCommand("minimize_window");
  });
  bind("btnWinClose", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    await runWindowCommand("close_window");
  });
  bind("dropZone", () => importShp());
  bind("dropGdb", () => importGdb());
  bind("btnClearS", () => clearAllFiles());
  bind("btnBrowseS", () => selectOutputDirS());

  // 输出模式切换：控制文件名字段下拉框显示，并立即刷新预览
  const outputModeRadios = document.querySelectorAll('input[name="output_mode"]');
  outputModeRadios.forEach((r) => {
    r.addEventListener("change", () => {
      const row = $("filenameFieldRow");
      if (row) row.style.display = r.checked && r.value === "split_by_plot" ? "block" : "none";
      if (r.checked) { lastPreviewKey = ""; updatePreview(); }
    });
  });
  const ff = $("filename_field");
  if (ff) ff.addEventListener("change", () => { lastPreviewKey = ""; updatePreview(); });

  // TXT→面 输出模式切换：控制地块拆分文件名下拉框显示
  const tOutputModeRadios = document.querySelectorAll('input[name="t_output_mode"]');
  tOutputModeRadios.forEach((r) => {
    r.addEventListener("change", () => {
      const row = $("t_filenameFieldRow");
      if (row) row.style.display = r.checked && r.value === "split_by_plot" ? "block" : "none";
    });
  });

  // 初始化：确保两个 filenameFieldRow 的显隐与当前选中模式一致
  // （防御 WebView2 对部分 inline style 的解析差异）
  function syncFilenameRows() {
    const sMode = document.querySelector('input[name="output_mode"]:checked')?.value;
    const tMode = document.querySelector('input[name="t_output_mode"]:checked')?.value;
    const sf = $("filenameFieldRow");
    if (sf) sf.style.display = sMode === "split_by_plot" ? "block" : "none";
    const tf = $("t_filenameFieldRow");
    if (tf) tf.style.display = tMode === "split_by_plot" ? "block" : "none";
  }
  syncFilenameRows();
  bind("dropZoneTxt", () => importTxt());
  bind("btnClearT", () => clearAllFilesTxt());
  bind("out_btn", () => selectOutputDir());
  bind("hdrTabAttr", () => switchHdrTab("attr"));
  bind("hdrTabProj", () => switchHdrTab("proj"));
  bind("btnPrefill", () => prefillProject());
  bind("btnResetDefaults", () => resetDefaults());
  bind("btnRunStt", () => runShpToTxt());
  bind("btnRunTts", () => runTxtToShp());
  bind("btnCloseAbout", () => closeAbout());
  bind("btnCloseSponsor", () => closeSponsor());
  bind("btnCloseGdbSelect", () => closeGdbSelectModal());
  bind("btnCancelGdbSelect", () => closeGdbSelectModal());
  bind("btnConfirmGdbSelect", () => confirmGdbSelect());

  // Tab bar switching
  document.querySelectorAll(".tab[data-t]").forEach((tab) => {
    tab.addEventListener("click", () => sw(tab.dataset.t));
  });

  // Modal overlay click-to-close
  const aboutModal = $("aboutModal");
  if (aboutModal) aboutModal.addEventListener("click", (e) => { if (e.target === aboutModal) closeAbout(); });
  const sponsorModal = $("sponsorModal");
  if (sponsorModal) sponsorModal.addEventListener("click", (e) => { if (e.target === sponsorModal) closeSponsor(); });
  const gdbSelectModal = $("gdbSelectModal");
  if (gdbSelectModal) gdbSelectModal.addEventListener("click", (e) => { if (e.target === gdbSelectModal) closeGdbSelectModal(); });
  // Prevent modal card click from closing
  document.querySelectorAll(".modal-card").forEach((card) => {
    card.addEventListener("click", (e) => e.stopPropagation());
  });

  // ─── Bind input/change events (replaces inline oninput/onchange) ───
  ["hc", "hj", "hu", "hz", "ht"].forEach((id) => {
    const el = $(id); if (el) el.addEventListener("input", () => { headerManual[id] = true; updatePreview(); });
  });
  ["hb", "ha"].forEach((id) => {
    const el = $(id); if (el) el.addEventListener("change", () => { headerManual[id] = true; updatePreview(); });
  });
  const hpi = $("hpi");
  if (hpi) hpi.addEventListener("input", updatePreview);

  // All other inputs/selects trigger preview update
  document.querySelectorAll("input,select").forEach((el) => {
    el.addEventListener("input", updatePreview);
    el.addEventListener("change", updatePreview);
  });

  // ─── Event delegation for dynamic elements ───
  // File list remove buttons
  const fl = $("fl");
  if (fl) fl.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-remove-file]");
    if (btn) removeFile(parseInt(btn.dataset.removeFile, 10));
  });
  // TXT file list remove buttons
  const flT = $("flT");
  if (flT) flT.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-remove-txt]");
    if (btn) removeTxtFile(parseInt(btn.dataset.removeTxt, 10));
  });
  // Config chips
  const ch = $("ch");
  if (ch) ch.addEventListener("click", (e) => {
    const chip = e.target.closest("[data-chip]");
    if (chip) ld(chip.dataset.chip);
  });

  // Drag & Drop — SHP
  const dz = $("dropZone");
  if (dz) {
    dz.addEventListener("dragover", (e) => { e.preventDefault(); dz.style.borderColor = "var(--ac)"; dz.style.background = "var(--acg)"; });
    dz.addEventListener("dragleave", () => { dz.style.borderColor = ""; dz.style.background = ""; });
    dz.addEventListener("drop", async (e) => {
      e.preventDefault(); dz.style.borderColor = ""; dz.style.background = "";
      const files = Array.from(e.dataTransfer?.files || []).filter((f) => f.name.toLowerCase().endsWith(".shp"));
      if (!files.length) { toast("请拖入 .shp 文件"); return; }
      try {
        const paths = files.map((f) => f.path || f.name);
        const result = await tauriInvoke("pick_shp_files_from_paths", { paths });
        if (result.skipped && result.skipped.length) {
          toast(`以下文件不是面状要素，已忽略：${result.skipped.join("、")}`);
        }
        if (result.files && result.files.length > 0) {
          loadedFiles = result.files; sourceType = null; sourcePath = null; gdbLayers = []; selectedLayers = []; renderFileList(); processImport();
        }
      } catch (err) { toast("拖放导入失败: " + err); }
    });
  }
  // Drag & Drop — TXT
  const dzT = $("dropZoneTxt");
  if (dzT) {
    dzT.addEventListener("dragover", (e) => { e.preventDefault(); dzT.style.borderColor = "var(--ac)"; dzT.style.background = "var(--acg)"; });
    dzT.addEventListener("dragleave", () => { dzT.style.borderColor = ""; dzT.style.background = ""; });
    dzT.addEventListener("drop", async (e) => {
      e.preventDefault(); dzT.style.borderColor = ""; dzT.style.background = "";
      const files = Array.from(e.dataTransfer?.files || []).filter((f) => f.name.toLowerCase().endsWith(".txt"));
      if (!files.length) { toast("请拖入 .txt 文件"); return; }
      try {
        const paths = files.map((f) => f.path || f.name);
        const result = await tauriInvoke("pick_txt_files_from_paths", { paths });
        if (result.files && result.files.length > 0) {
          txtFiles = result.files; renderTxtFileList(); renderTxtParseLog();
          if (result.files[0]?.crs_info) autoFillHeaderFromTxt(result.files[0].crs_info);
        }
      } catch (err) { toast("拖放导入失败: " + err); }
    });
  }

  up();
}

init();
