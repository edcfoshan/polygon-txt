import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize, LogicalPosition } from '@tauri-apps/api/dpi';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import { check as checkForUpdater } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { ask as dialogAsk } from '@tauri-apps/plugin-dialog';
import { getVersion } from '@tauri-apps/api/app';
import aboutContent from '../content/about.md?raw';
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
let projMode = "keep"; // prototype: dynamic projection selection
let projZone = null;   // dynamic projection zone (A/B: dst zone; C: src zone; F/G: auto or user)
let lastPreviewKey = "";
let previewTimer = null;
let autoSaveTimer = null;
let theme = "light";
let sourceType = null;
let sourcePath = null;
let gdbLayers = [];
let selectedLayers = [];

// 字段槽 → 源数据候选列名表：fn=地块名 / fi=编号 / fa=面积 / fu=用途 / fm=图幅号 / fd=地类编码。
// 测绘数据列名各地不统一，每个槽位列出可能的源列名按优先级匹配。
const FIELD_MATCH_RULES = {
  fn: ["DKMC", "MC", "NAME"],
  fi: ["DKBH", "BH", "ID"],
  fa: ["MJ", "AREA"],
  fu: ["DKYT", "YT", "YONGTU"],
  fm: ["TFH"],
  fd: ["DLBM", "DL"],
};

// [属性描述] 默认种子行（与后端 txt.rs 旧 default_attrs 一致）
const DEFAULT_ATTRS = [
  { k: "坐标系",   v: "2000国家大地坐标系" },
  { k: "几度分带", v: "3" },
  { k: "投影类型", v: "高斯克吕格" },
  { k: "计量单位", v: "米" },
  { k: "带号",     v: "" },
  { k: "精度",     v: "0.001" },
  { k: "转换参数", v: ",,,,,," },
];

// 把任意 h 对象规范化为 { attrs, project_info }。兼容旧结构（crs/band/... 字段）。
function normalizeH(h) {
  if (!h) return { attrs: DEFAULT_ATTRS.map((r) => ({ ...r })), project_info: "" };
  if (Array.isArray(h.attrs)) {
    return {
      attrs: h.attrs.map((r) => ({ k: (r && r.k) || "", v: (r && r.v) || "" })),
      project_info: h.project_info || "",
    };
  }
  // 旧结构 → 7 行种子
  return {
    attrs: [
      { k: "坐标系",   v: h.crs || "" },
      { k: "几度分带", v: h.band || "" },
      { k: "投影类型", v: h.proj || "" },
      { k: "计量单位", v: h.unit || "" },
      { k: "带号",     v: h.zone || "" },
      { k: "精度",     v: h.precision || "" },
      { k: "转换参数", v: h.transform || "" },
    ],
    project_info: h.project_info || "",
  };
}

// 内置预设种子：id=标识 / n=显示名 / h=表头配置 / p=转换选项 / f=字段映射。
// 只内置"自定义"一个空预设，其余方案由用户保存后存 localStorage（cfgs）。
const PP = [
  { id: "usr", n: "自定义", h: { attrs: DEFAULT_ATTRS.map((r) => ({ ...r })), project_info: "" }, p: { pp: 3, pz: "auto", ox: 0, oj: 0, on: 0, oo: 1, oc: 0, og: 0, oz: "3", om: 0 }, f: { fn: "", fi: "", fa: "__area_ha__", fu: "", fm: "", fd: "" } },
];

const $ = (id) => document.getElementById(id);

// ═══ Toast ═══
function toast(m) {
  const t = $("toast");
  if (!t) return;
  t.textContent = m;
  t.classList.add("on");
  clearTimeout(t._h);
  t._h = setTimeout(() => t.classList.remove("on"), 20000);
}

// ═══ Theme（主题：浅/暗 + 8 色系，v3.0；v3.1 起入口收进设置面板） ═══
const THEME_COLORS = ["normal", "brass", "green", "blue", "cyan", "purple", "orange", "rose"];
let themeColor = localStorage.getItem("tg_color") || "normal";

function pickTheme(t) {
  theme = t === "dark" ? "dark" : "light";
  document.documentElement.setAttribute("data-t", theme);
  localStorage.setItem("tg_theme", theme);
  syncThemeUI();
}
function pickColor(c) {
  if (!THEME_COLORS.includes(c)) return;
  themeColor = c;
  document.documentElement.setAttribute("data-c", c);
  localStorage.setItem("tg_color", c);
  syncThemeUI();
}
function syncThemeUI() {
  const m = $("settingsModal");
  if (!m) return;
  m.querySelectorAll(".thopt").forEach((b) => b.classList.toggle("on", b.dataset.t === theme));
  m.querySelectorAll(".copt").forEach((b) => b.classList.toggle("on", b.dataset.c === themeColor));
}

// ═══ 三区字号缩放（CSS zoom：a=标题栏+弹窗 b=折叠卡内容 c=预览文本） ═══
const FS_KEYS = { a: "tg_zma", b: "tg_zmb", c: "tg_zmc" };
function readFontScale(k) {
  const v = parseFloat(localStorage.getItem(FS_KEYS[k]));
  return Number.isFinite(v) ? Math.min(140, Math.max(85, v)) / 100 : 1;
}
function applyFontScale(k) {
  document.documentElement.style.setProperty("--z" + k, String(readFontScale(k)));
}
function setFontScale(k, pct) {
  localStorage.setItem(FS_KEYS[k], String(pct));
  applyFontScale(k);
}

// ═══ 手风琴分组折叠（v3.0 方案A） ═══
window.togAcc = function (h) { h.parentElement.classList.toggle("open"); };

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
    if (result.zone) autoFillHeader({ z: result.zone });
    // 合并 extent 信息
    const crsWithExtent = result.crs_info || {};
    if (result.xmin != null) crsWithExtent.xmin = result.xmin;
    if (result.ymin != null) crsWithExtent.ymin = result.ymin;
    if (result.xmax != null) crsWithExtent.xmax = result.xmax;
    if (result.ymax != null) crsWithExtent.ymax = result.ymax;
    syncOgGate(crsWithExtent);
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
  // 所选图层字段可能与导入时（取第一个图层）填充的不同，按所选图层刷新字段下拉框，
  // 否则下拉框停在别的图层字段、当前图层取不到值，预览字段会全空
  const firstSel = gdbLayers.find((l) => l.name === selectedLayers[0]);
  if (firstSel && firstSel.field_names && firstSel.field_names.length) {
    autoMatchFields(firstSel.field_names);
  }
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
  const tag = $("impTag");
  if (tag) tag.textContent = loadedFiles.length ? `${loadedFiles.length} 个文件` : "未导入";
}

window.removeFile = function (i) {
  loadedFiles.splice(i, 1);
  if (!loadedFiles.length) { const fl = $("fl"); if (fl) fl.innerHTML = ""; lastPreviewKey = ""; updatePreview(); return; }
  renderFileList();
  updatePreview();
};


// ═══ Dynamic Projection Modal ═══

// URL hash demo（开发调试用）：
//   #demo=geodetic        假装大地（度）
//   #demo=projected-3     假装投影 3°带
//   #demo=projected-6     假装投影 6°带
//   #demo=unknown         假装未知
function getDemoCrInfo(type) {
  switch (type) {
    case 'geodetic':    return { c: 'CGCS2000', u: '度',  b: '3', z: 38 };
    case 'projected-3': return { c: 'CGCS2000', u: '米',  b: '3', z: 38 };
    case 'projected-6': return { c: 'CGCS2000', u: '米',  b: '6', z: 20 };
    case 'unknown':     return { c: '',         u: '米',  b: null, z: null };
  }
  return null;
}
function applyDemoSeed(type) {
  const info = getDemoCrInfo(type);
  if (!info) return false;
  loadedFiles = [{ file_name: 'demo.shp', field_names: [], crs_info: info }];
  txtFiles = [];
  processImport();
  toast('已注入 demo 场景: ' + type);
  return true;
}

// 坐标系全称 → 简称（开关文字用）
const CRS_SHORT = {
  '2000国家大地坐标系': 'CGCS2000',
  '1980西安坐标系': '西安80',
  '1954北京坐标系': '北京54',
  'WGS84坐标系': 'WGS84',
};

function updateProjButton() {
  const toggle = $('projSwitchToggle');
  const label = $('projSwitchLabel');
  if (!toggle || !label) return;
  const ok = loadedFiles.length === 1;
  toggle.disabled = !ok;
  label.disabled = !ok;
  const on = (projMode !== 'keep' && ok);
  toggle.classList.toggle('on', on);
  label.classList.toggle('on', on);
  if (!on) { label.textContent = '动态投影'; return; }
  // 读属性表实时值（apply 后属性表已是目标投影态）
  const rows = collectAttrRows();
  const get = (k) => { const r = rows.find(a => a.k === k); return r ? r.v : ''; };
  const c = CRS_SHORT[get('坐标系')] || get('坐标系') || '?';
  if (get('计量单位') === '度') {
    label.textContent = c + '（度）';
  } else {
    const b = get('几度分带');
    const z = get('带号');
    label.textContent = c + ' ' + (b === '6' ? '6°带' : '3°带') + (z || '?') + '带';
  }
}

/// 模式推断：输入形式 + 用户选的目标形式 → A/B/C/D/F/G
/// inputIsDegree: 输入是否大地(度)；inputBand: 输入分带 '3'|'6'|''；targetVal: '3'|'6'|'deg'
function inferProjMode(inputIsDegree, inputBand, targetVal, srcZone, dstZone) {
  if (targetVal === 'deg') return 'D';
  const tBand = parseInt(targetVal, 10) || 3;
  if (inputIsDegree) return tBand === 6 ? 'B' : 'A';
  if (inputBand === String(tBand)) {
    const sz = srcZone ? String(srcZone) : '';
    const dz = dstZone ? String(dstZone) : '';
    if (sz && dz && sz !== dz) return 'H';  // 同分带不同带号 → 换带
    return 'C';
  }
  if (inputBand === '3' && tBand === 6) return 'F';
  if (inputBand === '6' && tBand === 3) return 'G';
  return 'C';
}

/// 根据导入数据的经纬度范围，智能推荐最佳分带/带号/中央经线
function buildRecommendText(info) {
  if (!info) return '导入数据后可显示坐标范围建议';
  let lonMin, lonMax;
  if (info.u === '度') {
    // 地理数据：直接用 xmin/xmax 作为经纬度范围
    if (info.xmin == null || info.xmax == null) return '导入数据后可显示坐标范围建议';
    lonMin = info.xmin; lonMax = info.xmax;
  } else if (info.b && info.z && info.xmin != null && info.ymin != null) {
    // 投影数据：近似逆投影得到经纬度范围
    const cm = info.b === '6' ? info.z * 6 - 3 : info.z * 3;
    // 含带号前缀时剥离（X 量级 ≥ 1e6 → 减去 zone×1e6），避免经度算成几百上千度
    const zoneF = Math.abs(info.xmin) >= 1000000 ? info.z * 1000000 : 0;
    const latMid = ((info.ymin + info.ymax) / 2) / 111320; // 近似纬度
    const mPerDeg = 111320 * Math.cos(latMid * Math.PI / 180);
    lonMin = cm + ((info.xmin - zoneF - 500000) / mPerDeg);
    lonMax = cm + ((info.xmax - zoneF - 500000) / mPerDeg);
  } else {
    return '导入数据后可显示坐标范围建议';
  }
  // 推荐最佳分带
  const midLon = (lonMin + lonMax) / 2;
  const z3 = Math.round(midLon / 3);
  const z6 = Math.round((midLon + 3) / 6);
  const cm3 = z3 * 3;
  const cm6 = z6 * 6 - 3;
  return [
    `经纬度范围：${lonMin.toFixed(2)}° ~ ${lonMax.toFixed(2)}°E`,
    `中央经线（经度中点）：${midLon.toFixed(1)}°`,
    `3°带推荐中央经线：${cm3}°（带号 ${z3}）`,
    `6°带推荐中央经线：${cm6}°（带号 ${z6}）`,
  ].join('\n');
}

/// 简化：根据 info + band 返回推荐 zone/cm
function computeRecommendedZone(info, band) {
  if (!info) return { zone: null, cm: null };
  let midLon;
  if (info.u === '度' && info.xmin != null) {
    midLon = (info.xmin + info.xmax) / 2;
  } else if (info.b && info.z && info.xmin != null && info.ymin != null) {
    const cm = info.b === '6' ? info.z * 6 - 3 : info.z * 3;
    const zoneF = Math.abs(info.xmin) >= 1000000 ? info.z * 1000000 : 0;
    const latMid = ((info.ymin + info.ymax) / 2) / 111320;
    const mPerDeg = 111320 * Math.cos(latMid * Math.PI / 180);
    midLon = cm + (((info.xmin + info.xmax) / 2 - zoneF - 500000) / mPerDeg);
  } else {
    return { zone: null, cm: null };
  }
  const zone = band === 6 ? Math.round((midLon + 3) / 6) : Math.round(midLon / 3);
  const cm = band === 6 ? zone * 6 - 3 : zone * 3;
  return { zone, cm };
}

function renderProjModal(info) {
  const u = info && info.u || '';
  const c = info && info.c || '';
  const b = info && info.b || '';
  const z = info && info.z;

  // 导入识别
  const det = $('projDetectGrid');
  if (det) {
    const form = u === '度' ? '大地（度）' : u === '米' ? '投影（米）' : '<span class="na">未识别</span>';
    let band = '<span class="na">—</span>';
    if (b === '3') band = '3°带';
    else if (b === '6') band = '6°带';
    let zone = '<span class="na">—</span>';
    const zn = typeof z === 'number' ? z : parseInt(z, 10);
    if (zn > 0) zone = zn + ' <span class="ok">✓</span>';
    det.innerHTML = [
      '<span class="k">坐标系</span><span class="v">' + (c || '<span class="na">—</span>') + '</span>',
      '<span class="k">形式</span><span class="v">' + form + '</span>',
      '<span class="k">分带</span><span class="v">' + band + '</span>',
      '<span class="k">带号</span><span class="v">' + zone + '</span>'
    ].join('');
  }

  // 推荐文案
  const rm = $('projRecommend');
  if (rm) rm.textContent = buildRecommendText(info);

  // u='度' 但坐标量级>360 → PRJ 错标地理，实际投影（米）
  const inputIsDegree = u === '度' && !(info && info.xmax != null && Math.abs(info.xmax) > 360);
  const sel = $('projFormSelect');
  const zi = $('projZoneInput');
  const cmInput = $('projCMInput');

  // 目标形式下拉默认：已 apply 过→恢复上次；否则 输入投影→同带，输入大地→3°带
  const restoreVal = (projMode !== 'keep' && window._projFormValue) ? window._projFormValue
    : inputIsDegree ? '3' : (b === '6' ? '6' : '3');
  if (sel) {
    Array.from(sel.options).forEach(o => { o.selected = (o.value === restoreVal); });
    // 边界：输入已是大地 → "转为大地坐标"禁选
    const degOpt = Array.from(sel.options).find(o => o.value === 'deg');
    if (degOpt) {
      degOpt.disabled = inputIsDegree;
      degOpt.textContent = inputIsDegree ? '转为大地坐标（输入已是大地）' : '转为大地坐标（度）';
    }
  }

  /// 当前目标分带（deg 视为 3 占位，因 deg 模式下带号/CM 已置灰不参与计算）
  function curBand() {
    const v = sel ? sel.value : '3';
    return v === 'deg' ? 3 : (parseInt(v, 10) || 3);
  }

  /// 带号 → CM，立即规整（CM 框回写标称值）
  function syncCMFromZone() {
    const band = curBand();
    const min = band === 6 ? 13 : 24;
    const max = band === 6 ? 23 : 45;
    const v = zi && zi.value ? parseInt(zi.value, 10) : 0;
    if (cmInput) cmInput.value = (v >= min && v <= max) ? (band === 6 ? v * 6 - 3 : v * 3) : '';
  }

  /// CM → 带号，立即规整（CM 跳到最近标称值，带号同步）
  function syncZoneFromCM() {
    const band = curBand();
    const min = band === 6 ? 13 : 24;
    const max = band === 6 ? 23 : 45;
    const cm = cmInput && cmInput.value ? parseFloat(cmInput.value) : 0;
    if (!zi) return;
    if (cm > 0) {
      let zone = band === 6 ? Math.round((cm + 3) / 6) : Math.round(cm / 3);
      zone = Math.max(min, Math.min(max, zone));
      zi.value = zone > 0 ? zone : '';
      if (cmInput && zone > 0) cmInput.value = band === 6 ? zone * 6 - 3 : zone * 3;
    } else {
      zi.value = '';
    }
  }

  /// 失焦时 clamp 带号到合法范围（3°带 24-45 / 6°带 13-23）
  function clampZone() {
    const band = curBand();
    const min = band === 6 ? 13 : 24;
    const max = band === 6 ? 23 : 45;
    if (zi && zi.value) {
      const v = parseInt(zi.value, 10);
      if (!isNaN(v)) {
        if (v < min) zi.value = String(min);
        else if (v > max) zi.value = String(max);
        syncCMFromZone();
      }
    }
  }

  /// 根据下拉值刷新带号/CM/置灰
  function refreshForm() {
    const val = sel ? sel.value : '3';
    const isDeg = (val === 'deg');
    const band = curBand();
    if (zi) zi.disabled = isDeg;
    if (cmInput) cmInput.disabled = isDeg;
    if (isDeg) {
      if (zi) { zi.value = ''; zi.placeholder = '逆投影用源带号'; }
      if (cmInput) { cmInput.value = ''; cmInput.placeholder = '—'; }
      return;
    }
    const rec = computeRecommendedZone(info, band);
    if (zi) {
      zi.min = band === 6 ? 13 : 24;
      zi.max = band === 6 ? 23 : 45;
      zi.placeholder = rec.zone ? '推荐 ' + rec.zone : '自动推算';
      zi.value = rec.zone ? String(rec.zone) : '';
    }
    syncCMFromZone();
  }

  refreshForm();

  // 恢复上次填的带号（优先级高于推荐）
  if (projZone != null && zi && !zi.disabled) {
    zi.value = String(projZone);
    syncCMFromZone();
  }

  // 事件绑定（_projBound 守卫防重复）
  if (sel && !sel._projBound) { sel.addEventListener('change', refreshForm); sel._projBound = true; }
  if (zi && !zi._projBound) { zi.addEventListener('input', syncCMFromZone); zi._projBound = true; }
  if (zi && !zi._clampBound) { zi.addEventListener('change', clampZone); zi._clampBound = true; }
  if (cmInput && !cmInput._projBound) { cmInput.addEventListener('input', syncZoneFromCM); cmInput._projBound = true; }
}

window.openProjModal = function () {
  const overlay = $('projModal');
  if (!overlay) return;
  const info = currentCrsInfo || (loadedFiles[0] && loadedFiles[0].crs_info);
  renderProjModal(info);
  overlay.classList.add('on');
};

window.closeProjModal = function () {
  const overlay = $('projModal');
  if (overlay) overlay.classList.remove('on');
};

window.applyProjMode = function () {
  const sel = $('projFormSelect');
  const val = sel ? sel.value : '3';
  const zi = $('projZoneInput');

  window._projFormValue = val; // 记忆下次恢复

  if (val === 'deg') {
    projMode = 'D';
    projZone = null;
    window._projNoPrefix = false;
  } else {
    const xmax = currentCrsInfo && currentCrsInfo.xmax;
    // u='度' 但坐标量级>360 → PRJ 错标地理，实际是投影（米）
    const inputIsDegree = currentCrsInfo && currentCrsInfo.u === '度' && !(xmax != null && Math.abs(xmax) > 360);
    const zRaw = zi ? zi.value.trim() : '';
    projZone = zRaw ? parseInt(zRaw, 10) : null;
    // srcZone 优先 currentCrsInfo.z，空则从坐标范围推断（含带号前缀时）——避免 PRJ 无带号时误判 C
    let srcZone = currentCrsInfo && currentCrsInfo.z;
    if (!srcZone && currentCrsInfo && currentCrsInfo.xmax != null && currentCrsInfo.xmax > 1000000) {
      srcZone = Math.round((currentCrsInfo.xmax - 500000) / 1000000);
    }
    // inputBand 优先 currentCrsInfo.b，空则从 srcZone 推断（3°带 24-45 / 6°带 13-23）
    let inputBand = (currentCrsInfo && currentCrsInfo.b) || '';
    if (!inputBand && srcZone) {
      inputBand = (srcZone >= 24 && srcZone <= 45) ? '3' : (srcZone >= 13 && srcZone <= 23) ? '6' : '';
    }
    projMode = inferProjMode(inputIsDegree, inputBand, val, srcZone, projZone);
    window._projNoPrefix = false;
  }

  // toast 文案
  const bStr = val === 'deg' ? '' : (parseInt(val, 10) + '°带');
  const zStr = projZone ? String(projZone) : (val === 'deg' ? '源带号' : '?');
  const noPre = window._projNoPrefix ? ' 自然值' : '';
  const label = projMode === 'D' ? '投影→大地'
    : (projMode + ' ' + bStr + noPre + ' ' + zStr).trim();
  // og 与动态投影互斥
  syncOgGate(currentCrsInfo);
  toast('动态投影: ' + label);

  // 同步头表 CRS 字段（键名对齐属性表真实 key：几度分带/计量单位/带号/投影类型）
  const rows = collectAttrRows();
  const setRow = (key, v) => { const r = rows.find(a => a.k === key); if (r) r.v = v; };
  if (projMode === 'D') {
    // 投影→大地：大地坐标无分带/带号/投影类型
    setRow('计量单位', '度');
    setRow('几度分带', '');
    setRow('带号', '');
    setRow('投影类型', '无');
  } else {
    const bw = parseInt(val, 10) || 3;
    const srcZone = currentCrsInfo && currentCrsInfo.z ? parseInt(currentCrsInfo.z, 10) : null;
    const zone = projZone || srcZone;
    setRow('计量单位', '米');
    setRow('几度分带', String(bw));
    setRow('投影类型', '高斯克吕格');
    if (zone) setRow('带号', String(zone));
  }
  renderAttrRows(rows);

  updateProjButton();
  updatePreview();
  window.closeProjModal();
};

window.resetProjMode = function () {
  projMode = 'keep';
  projZone = null;
  window._projNoPrefix = false;
  window._projFormValue = null;
  // 恢复属性表到导入态（currentCrsInfo）
  if (currentCrsInfo) {
    const rows = collectAttrRows();
    const setRow = (k, v) => { const r = rows.find(a => a.k === k); if (r) r.v = v; };
    setRow('坐标系', currentCrsInfo.c || '');
    setRow('计量单位', currentCrsInfo.u || '米');
    setRow('几度分带', currentCrsInfo.b || '');
    setRow('带号', currentCrsInfo.z ? String(currentCrsInfo.z) : '');
    setRow('投影类型', currentCrsInfo.u === '度' ? '无' : '高斯克吕格');
    renderAttrRows(rows);
  }
  syncOgGate(currentCrsInfo);
  updateProjButton();
  updatePreview();
  toast('动态投影已关闭');
};

// 当前导入源的 CRS 信息（og 门禁 / 软提示用）
let currentCrsInfo = null;

// og 按钮门禁：仅大地坐标系（单位=度）输入时可点；切源时重置勾选残留。
function syncOgGate(crsInfo) {
  currentCrsInfo = crsInfo || null;
  const og = $("og");
  const oz = $("oz");
  if (!og) return;
  const isDegree = currentCrsInfo?.u === "度";
  const projActive = projMode !== "keep";
  // og 与动态投影互斥：projMode 非 keep 时强制 og=false 并置灰
  og.disabled = !isDegree || projActive;
  if (!isDegree || projActive) og.checked = false;
  if (oz) oz.disabled = !(isDegree && og.checked && !projActive);
  refreshOgWarn();
}

// og 软提示：勾选且基准非 CGCS2000/WGS84（西安80/北京54 等）时提示百米级偏差。
function refreshOgWarn() {
  const og = $("og");
  const warn = $("ogWarn");
  if (!og || !warn) return;
  const c = currentCrsInfo?.c || "";
  const nonStd = !!c && !/2000|CGCS|WGS/i.test(c);
  warn.style.display = og.checked && nonStd && !og.disabled ? "block" : "none";
}

function processImport() {
  if (!loadedFiles.length) return;
  const first = loadedFiles[0];
  autoMatchFields(first.field_names || []);
  if (first.crs_info) {
    // 合并 extent 信息到 crs_info
    if (first.xmin != null) first.crs_info.xmin = first.xmin;
    if (first.ymin != null) first.crs_info.ymin = first.ymin;
    if (first.xmax != null) first.crs_info.xmax = first.xmax;
    if (first.ymax != null) first.crs_info.ymax = first.ymax;
    autoFillHeader(first.crs_info);
  }
  syncOgGate(first.crs_info);
  updatePreview();
  updateProjButton();
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

const FIELD_PLACEHOLDER = { fn: "DKMC", fi: "DKBH", fa: "MJ", fu: "DKYT", fm: "TFH", fd: "DLBM" };

function autoMatchFields(fieldNames) {
  // 统一三段式下拉：① 不填 ② 占位文字 ③ 数据字段名
  // SHP/GDB 来源不同但结构一致；面积(fa)额外支持自动计算。
  // 默认：fa=公顷(自动)，其余=不填
  lastFieldNames = fieldNames || [];
  for (const key of Object.keys(FIELD_PLACEHOLDER)) {
    const sel = $(key);
    if (!sel) continue;
    const isArea = key === "fa";
    let html = '<option value="">不填</option>';
    html += `<option value="__placeholder__">${FIELD_PLACEHOLDER[key]} (占位)</option>`;
    if (isArea) {
      html += '<option value="__area_sqm__">平方米(自动)</option>';
      html += '<option value="__area_ha__" selected>公顷(自动)</option>';
    }
    fieldNames.forEach((fn) => {
      html += `<option value="${fn}">${fn}</option>`;
    });
    sel.innerHTML = html;
    if (!isArea) sel.value = ""; // 非面积字段默认不填
  }
  // 高级模式开启时同步重建行内映射源下拉（保留仍存在的选中值）
  if ($("advMode")?.checked && advInitialized) {
    renderFieldRows(collectFieldRows());
  }
}

// ═══ 字段映射高级模式 ═══
// 固定清单 14 项；与后端 adv_placeholder / STANDARD_META_FIELDS 联动，增删需双端同步
const ADV_FIELD_NAMES = ["坐标点个数","地块面积","图斑面积","地块编号","图斑编号","地块名称","补充耕地实施年份","耕地坡度级别","图形属性","图幅号","地块用途","备注","地类","耕地质量等级"];
// 锁定字段（按映射源判定——字段名可自由编辑，改名不丢锁定能力）：源固定不可改、行不可删
const ADV_LOCKED_SRC = { "__count__": 1, "__geom__": 1 };
// 高级字段名 → 占位文字（与 Rust convert.rs adv_placeholder 镜像）
const ADV_PLACEHOLDER = { "地块名称": "DKMC", "地块编号": "DKBH", "图斑编号": "DKBH", "地块面积": "MJ", "图斑面积": "MJ", "地块用途": "DKYT", "图幅号": "TFH", "地类": "DLBM" };
const ADV_AREA_FIELDS = ["地块面积", "图斑面积"];
// 8 字段标准预设（默认，与后端 STANDARD_META_FIELDS 一致）
const STD_ADV_ROWS = [
  { name: "坐标点个数", source: "__count__" },
  { name: "地块面积", source: "__area_ha__" },
  { name: "地块编号", source: "" },
  { name: "地块名称", source: "" },
  { name: "图形属性", source: "__geom__" },
  { name: "图幅号", source: "" },
  { name: "地块用途", source: "" },
  { name: "地类", source: "" },
];
// 12 字段补充耕地预设（20260818 模板）
const BCG_ADV_ROWS = [
  { name: "坐标点个数", source: "__count__" },
  { name: "图斑面积", source: "__area_ha__" },
  { name: "图斑编号", source: "" },
  { name: "地块名称", source: "" },
  { name: "补充耕地实施年份", source: "" },
  { name: "耕地坡度级别", source: "" },
  { name: "图形属性", source: "__geom__" },
  { name: "图幅号", source: "" },
  { name: "地块用途", source: "" },
  { name: "备注", source: "" },
  { name: "地类", source: "" },
  { name: "耕地质量等级", source: "" },
];
let advInitialized = false; // 首次开启高级模式时从简单模式 6 下拉继承
let advPreset = "std";      // "std" | "bcg" | "custom"（补充耕地开关的三态）
let lastFieldNames = [];    // 最近一次导入的源字段表（供行内映射源下拉重建）

// 生成某字段名的映射源下拉选项（锁定行按 source 判定，返回单一锁定选项；面积选项按 name 或既有 source）
function buildAdvSourceOptions(name, cur) {
  if (ADV_LOCKED_SRC[cur]) {
    return `<option value="${cur}" selected>${cur === "__count__" ? "自动统计点数" : "固定「面」"}</option>`;
  }
  // 首项文案：空字段名行引导「选字段」，已选字段行显示「不填」（该列输出空值）
  let html = `<option value="">${name ? "不填" : "选字段"}</option>`;
  if (ADV_PLACEHOLDER[name]) {
    html += `<option value="__placeholder__"${cur === "__placeholder__" ? " selected" : ""}>${ADV_PLACEHOLDER[name]} (占位)</option>`;
  }
  if (ADV_AREA_FIELDS.includes(name) || (cur && cur.startsWith("__area_"))) {
    html += `<option value="__area_sqm__"${cur === "__area_sqm__" ? " selected" : ""}>平方米(自动)</option>`;
    html += `<option value="__area_ha__"${cur === "__area_ha__" ? " selected" : ""}>公顷(自动)</option>`;
  }
  lastFieldNames.forEach((fn) => {
    html += `<option value="${escAttr(fn)}"${cur === fn ? " selected" : ""}>${escAttr(fn)}</option>`;
  });
  // 当前值不在候选（如源字段已移除）→ 置顶保留，避免静默丢配置
  if (cur && cur !== "__placeholder__" && cur !== "__area_sqm__" && cur !== "__area_ha__"
      && !lastFieldNames.includes(cur)) {
    html = `<option value="${escAttr(cur)}" selected>${escAttr(cur)}</option>` + html;
  }
  return html;
}

function renderFieldRows(rows) {
  const box = $("fieldRows");
  if (!box) return;
  box.innerHTML = "";
  rows.forEach((row, i) => {
    const locked = !!ADV_LOCKED_SRC[row.source];
    const div = document.createElement("div");
    div.className = "attr-row field-row";
    div.dataset.i = String(i);
    // 左列字段名可编辑（自绘候选弹层 advSuggest）：空值 = 无名值列（placeholder「新行」），候选 = 14 项清单
    const btnHtml = locked ? "" : `<button class="abtn del" data-act="del" data-i="${i}" title="删除此行">✕</button>`;
    div.innerHTML =
      `<span class="grip" title="拖动排序">⠿</span>` +
      `<input class="fk" data-i="${i}" data-f="name" maxlength="30" autocomplete="off" spellcheck="false" placeholder="新行" value="${escAttr(row.name)}">` +
      `<span class="feq">←</span>` +
      `<select class="fmap" data-i="${i}" data-f="source"${locked ? " disabled" : ""}>${buildAdvSourceOptions(row.name, row.source)}</select>` +
      btnHtml;
    box.appendChild(div);
  });
  hideAdvSuggest();
}

function collectFieldRows() {
  const box = $("fieldRows");
  if (!box || !box.children.length) return STD_ADV_ROWS.map((r) => ({ ...r }));
  const rows = [];
  box.querySelectorAll(".field-row").forEach((div) => {
    rows.push({
      name: div.querySelector(".fk")?.value ?? "",
      source: div.querySelector(".fmap")?.value ?? "",
    });
  });
  return rows;
}

function advRowsMatchPreset(rows, preset) {
  const p = preset === "bcg" ? BCG_ADV_ROWS : STD_ADV_ROWS;
  return rows.length === p.length && rows.every((r, i) => r.name === p[i].name && r.source === p[i].source);
}

// ─── 用户字段方案 CRUD（localStorage tg_adv_tpl）───
// 同名覆盖即更新：新名=增 / 下拉选中=查 / 同名保存=改 / ✕=删
function readAdvTpls() {
  try {
    const arr = JSON.parse(localStorage.getItem("tg_adv_tpl") || "[]");
    return Array.isArray(arr)
      ? arr.filter((t) => t && t.id && t.n && Array.isArray(t.rows)).slice(0, 50)
      : [];
  } catch { return []; }
}
function writeAdvTpls(tpls) {
  try { localStorage.setItem("tg_adv_tpl", JSON.stringify(tpls)); } catch (e) { console.warn("保存字段方案失败:", e); }
}
function matchAdvTpl(rows) {
  for (const t of readAdvTpls()) {
    if (t.rows.length === rows.length && t.rows.every((r, i) => r.name === rows[i].name && r.source === rows[i].source)) return t.id;
  }
  return null;
}
// 存为方案：行内命名输入（WebView2 对 window.prompt 静默返回 null，弃用 prompt）
window.saveAdvTpl = function () {
  const inp = $("tplNameInput");
  if (!inp || inp.style.display !== "none") return;
  $("bcgSel").style.display = "none";
  inp.value = "";
  inp.style.display = "";
  $("btnTplOk").style.display = "";
  $("btnTplCancel").style.display = "";
  inp.focus();
};
function closeTplNameInput() {
  ["tplNameInput", "btnTplOk", "btnTplCancel"].forEach((id) => { const e = $(id); if (e) e.style.display = "none"; });
  pendingOverwrite = "";
  disarmButton($("btnTplOk"));
  const sel = $("bcgSel");
  if (sel) sel.style.display = "";
  renderBcgSel();
}
window.cancelAdvTplSave = function () { closeTplNameInput(); };
let pendingOverwrite = ""; // 重名待确认覆盖的方案名；改名/失焦/关闭即失效，须显式再点「覆盖?」才执行
let tplSaving = false; // 防重入：异步保存未完成时再点 ✓ 不重复执行
window.confirmAdvTplSave = function () {
  const inp = $("tplNameInput");
  if (!inp || inp.style.display === "none" || tplSaving) return;
  const n = inp.value.trim();
  if (!n) { toast("方案名不能为空"); inp.focus(); return; }
  const hit = readAdvTpls().find((t) => t.n === n);
  if (hit && n !== pendingOverwrite) {
    pendingOverwrite = n;
    armVisual($("btnTplOk"), "覆盖?");
    toast("方案「" + n + "」已存在：再点一次「覆盖?」确认覆盖，或改个名字");
    return;
  }
  tplSaving = true;
  try {
    const rows = collectFieldRows().map((r) => ({ name: r.name, source: r.source }));
    // 内容与预设/已有方案完全相同 → 拒绝保存（防冗余方案与「切换后名字不变化」的困惑）
    const sameAs = (p) => p.length === rows.length && p.every((r, i) => r.name === rows[i].name && r.source === rows[i].source);
    if (advRowsMatchPreset(rows, "std")) { closeTplNameInput(); toast("与预设「8 字段标准」内容完全相同，无需另存方案"); return; }
    if (advRowsMatchPreset(rows, "bcg")) { closeTplNameInput(); toast("与预设「补充耕地模式」内容完全相同，无需另存方案"); return; }
    const dup = readAdvTpls().find((t) => t.n !== n && sameAs(t.rows));
    if (dup) { closeTplNameInput(); toast("与已有方案「" + dup.n + "」内容完全相同，无需重复保存"); return; }
    const tpls = readAdvTpls();
    const h = tpls.find((t) => t.n === n);
    if (h) h.rows = rows; else tpls.push({ id: "t" + Date.now(), n, rows });
    writeAdvTpls(tpls);
    advPreset = (h || tpls[tpls.length - 1]).id; // 保存即选中：内容与预设相同时也显示方案名（否则删除入口找不到 t 前缀态）
    closeTplNameInput();
    toast(h ? "已更新方案「" + n + "」" : "已保存方案「" + n + "」");
  } finally {
    tplSaving = false;
  }
};
window.delAdvTpl = function () {
  if (!readAdvTpls().length) { toast("还没有保存过自定义方案"); return; }
  if (!String(advPreset).startsWith("t")) { toast("请先在下拉中选中要删除的方案"); return; }
  const t = readAdvTpls().find((x) => x.id === advPreset);
  if (!t) { toast("方案数据异常：请重新在下拉中选择后重试"); return; }
  const id = t.id, name = t.n;
  armButton($("btnDelTpl"), "确认删除?", () => {
    writeAdvTpls(readAdvTpls().filter((x) => x.id !== id));
    advPreset = "custom"; // 行保留不动，仅预设态回自定义
    renderBcgSel();
    toast("已删除方案「" + name + "」");
  });
};

// 行改动后判定当前命中哪个预设（std → bcg → 用户方案 → custom；完全一致才亮预设态）
function syncAdvPresetState() {
  const rows = collectFieldRows();
  advPreset = advRowsMatchPreset(rows, "std") ? "std"
    : advRowsMatchPreset(rows, "bcg") ? "bcg"
    : (matchAdvTpl(rows) || "custom");
  renderBcgSel();
}

// 预设下拉：固定项（std/bcg/custom）+ 用户方案动态项（value=方案 id，前缀 t 与固定项区分）
function renderBcgSel() {
  const sel = $("bcgSel");
  if (!sel) return;
  sel.innerHTML =
    `<option value="std">8 字段标准</option>` +
    `<option value="bcg">补充耕地模式</option>` +
    readAdvTpls().map((t) => `<option value="${escAttr(t.id)}">${escAttr(t.n)}</option>`).join("") +
    `<option value="custom">自定义</option>`;
  sel.value = String(advPreset);
  if (sel.value !== String(advPreset)) { advPreset = "custom"; sel.value = "custom"; } // 方案已删 → 回自定义
  // 有自定义方案即显示删除入口（不依赖选中态——改过字段后 ✕ 消失曾让用户找不到删除入口）
  const del = $("btnDelTpl");
  if (del) del.style.display = readAdvTpls().length > 0 ? "" : "none";
}

// 补充耕地开关三态视觉（v2.2 旧 checkbox 版）→ v3.0 已由预设下拉 bcgSel 取代
function renderBcgToggle() { renderBcgSel(); }

// ─── 字段名候选弹层（自绘，body 级 fixed 定位防 #fieldRows overflow 裁剪）───
let advSug = null;
function ensureAdvSuggest() {
  if (advSug) return advSug;
  advSug = document.createElement("div");
  advSug.id = "advSuggest";
  document.body.appendChild(advSug);
  advSug.addEventListener("mousedown", (e) => {
    const opt = e.target.closest(".opt");
    if (!opt) return;
    e.preventDefault(); // 阻止 input 先失焦
    const inp = advSug._inp;
    if (inp) {
      if (opt.dataset.v !== "__keep__") inp.value = opt.dataset.v;
      commitFieldName(inp);
    }
  });
  return advSug;
}
function showAdvSuggest(inp) {
  const box = ensureAdvSuggest();
  const q = inp.value.trim();
  const hits = ADV_FIELD_NAMES.filter((n) => !q || n.includes(q));
  box.innerHTML =
    hits.map((n) => `<div class="opt" data-v="${escAttr(n)}">${escAttr(n)}</div>`).join("") +
    (q && !ADV_FIELD_NAMES.includes(q) ? `<div class="opt dim" data-v="__keep__">用「${escAttr(q)}」作自定义名</div>` : "");
  const r = inp.getBoundingClientRect();
  box.style.left = r.left + "px";
  box.style.top = r.bottom + 2 + "px";
  box.style.minWidth = r.width + "px";
  box.style.display = "block";
  box._inp = inp;
}
function hideAdvSuggest() { if (advSug) advSug.style.display = "none"; }

// 字段名提交（回车/失焦/候选点击）：trim → 重建该行源下拉（占位/面积选项随名变）→ 同步预设态。
// 不全量 renderFieldRows——避免打断其他行正在进行的输入
function commitFieldName(inp) {
  inp.value = inp.value.trim();
  const fmap = inp.closest(".field-row")?.querySelector("select.fmap");
  if (fmap && !fmap.disabled) {
    let cur = fmap.value;
    if (cur === "__placeholder__" && !ADV_PLACEHOLDER[inp.value]) cur = ""; // 新名无占位映射 → 该列转空值
    fmap.innerHTML = buildAdvSourceOptions(inp.value, cur);
    fmap.value = cur;
  }
  hideAdvSuggest();
  syncAdvPresetState();
  updatePreview();
}

function bindFieldRowEvents() {
  const box = $("fieldRows");
  if (!box) return;
  box.addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-act='del']");
    if (!btn) return;
    const rows = collectFieldRows();
    rows.splice(parseInt(btn.dataset.i, 10), 1);
    renderFieldRows(rows);
    syncAdvPresetState();
    updatePreview();
  });
  // 字段名 input：输入中只更新候选与自动保存，绝不重渲染（保焦点光标）
  box.addEventListener("input", (e) => {
    if (e.target.matches("input.fk")) { showAdvSuggest(e.target); scheduleAutoSave(); }
  });
  box.addEventListener("focusin", (e) => { if (e.target.matches("input.fk")) showAdvSuggest(e.target); });
  box.addEventListener("focusout", (e) => { if (e.target.matches("input.fk")) hideAdvSuggest(); });
  box.addEventListener("keydown", (e) => {
    if (!e.target.matches("input.fk")) return;
    if (e.key === "Enter" && !e.isComposing) { e.preventDefault(); e.target.blur(); } // 借 blur 的 change 提交
    else if (e.key === "Escape") hideAdvSuggest();
  });
  box.addEventListener("scroll", hideAdvSuggest, { passive: true });
  box.addEventListener("change", (e) => {
    if (e.target.matches("input.fk")) { commitFieldName(e.target); return; }
    if (e.target.closest("select.fmap")) {
      syncAdvPresetState();
      updatePreview();
    }
  });
  // 拖拽排序（同 attr 行手动 mouse 实现，避免 WebView2 HTML5 DnD 兼容问题）
  box.addEventListener("mousedown", (e) => {
    const grip = e.target.closest(".grip");
    if (!grip || e.button !== 0) return;
    const row = grip.closest(".field-row");
    if (!row) return;
    e.preventDefault();
    row.classList.add("dragging");
    const onMove = (ev) => {
      const el = document.elementFromPoint(ev.clientX, ev.clientY);
      const targetRow = el && el.closest(".field-row");
      if (!targetRow || targetRow === row) return;
      const rect = targetRow.getBoundingClientRect();
      if (ev.clientY - rect.top > rect.height / 2) targetRow.after(row);
      else targetRow.before(row);
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      row.classList.remove("dragging");
      renderFieldRows(collectFieldRows());
      syncAdvPresetState();
      updatePreview();
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  });
}

// 首次开启高级模式：继承简单模式 6 下拉已选的源字段（输出与现状等价，仅多列表行）
function inheritAdvFromSimple() {
  const byName = {
    "地块名称": $("fn")?.value || "",
    "地块编号": $("fi")?.value || "",
    "地块面积": $("fa")?.value || "__area_ha__",
    "地块用途": $("fu")?.value || "",
    "图幅号": $("fm")?.value || "",
    "地类": $("fd")?.value || "",
  };
  return STD_ADV_ROWS.map((r) => ({ ...r, source: byName[r.name] !== undefined ? byName[r.name] : r.source }));
}

function setAdvModeOn(on) {
  if (on && !advInitialized) {
    advInitialized = true;
    renderFieldRows(inheritAdvFromSimple());
    syncAdvPresetState();
  }
  const simple = $("fieldSimple");
  const advBox = $("fieldAdv");
  if (simple) simple.style.display = on ? "none" : "";
  if (advBox) advBox.style.display = on ? "" : "none";
  lastPreviewKey = "";
  updatePreview();
}

// 恢复配置（ld 用）：adv = { on, preset, rows } 或 null（旧配置 → 关闭态）
function applyAdvConfig(adv) {
  const on = !!(adv && adv.on);
  if (on) {
    advInitialized = true;
    const rows = Array.isArray(adv.rows) && adv.rows.length
      ? adv.rows.map((r) => ({ name: r?.name || "", source: r?.source || "" }))
      : inheritAdvFromSimple();
    renderFieldRows(rows);
    advPreset = adv.preset || "std";
    syncAdvPresetState();
  } else {
    advInitialized = false;
    advPreset = "std";
  }
  const cb = $("advMode");
  if (cb) cb.checked = on;
  const simple = $("fieldSimple");
  const advBox = $("fieldAdv");
  if (simple) simple.style.display = on ? "none" : "";
  if (advBox) advBox.style.display = on ? "" : "none";
}

// 简写 → 中文键名（autoFillHeader 的 info 可能用简写或中文键）
const ATTR_AUTO_KEY = { c: "坐标系", b: "几度分带", j: "投影类型", u: "计量单位", z: "带号" };

function autoFillHeader(info) {
  if (!info) return;
  const rows = collectAttrRows();
  let changed = false;
  for (const [short, cnKey] of Object.entries(ATTR_AUTO_KEY)) {
    const val = info[short] || info[cnKey];
    if (val && !headerManual[cnKey]) {
      const row = rows.find((r) => r.k.trim() === cnKey);
      if (row && row.v !== String(val)) { row.v = String(val); changed = true; }
    }
  }
  if (changed) { renderAttrRows(rows); updatePreview(); }
}

function autoFillHeaderFromTxt(info) {
  autoFillHeader(info);
}

// ═══ TXT 导入 ═══
window.importTxt = async function () {
  try {
    const result = await tauriInvoke("pick_txt_files");
    if (result.failed && result.failed.length) {
      toast("以下文件解析失败：" + result.failed.join("、"));
    }
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
  const tag = $("impTTag");
  if (tag) tag.textContent = txtFiles.length ? `${txtFiles.length} 个文件` : "未导入";
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
  scheduleAutoSave();
}

// 自动保存 usr 工作副本（命名预设由「保存」按钮管理，避免 HMR / 刷新丢失未保存编辑）
function flushAutoSave() {
  if (cur !== "usr") return;
  const c = getConfig();
  cfgs["usr"] = { ...cfgs["usr"], h: c.h, p: getOptions(), f: c.f };
  try { localStorage.setItem("tg_dark", JSON.stringify(cfgs)); } catch (e) { console.warn("autoSave failed:", e); }
}

function scheduleAutoSave() {
  clearTimeout(autoSaveTimer);
  autoSaveTimer = setTimeout(flushAutoSave, 400);
}

window.up = async function () {
  const hpi = $("hpi")?.value || "";
  const attrLines = collectAttrRows()
    .filter((r) => r.k.trim() !== "" || r.v.trim() !== "")
    .map((r) => `${r.k}=${r.v}`);
  let out = "";
  if (hpi.trim()) out += `[项目信息]\n${hpi.trim()}\n`;
  out += `[属性描述]\n${attrLines.join("\n")}\n[地块坐标]`;

  const cfg = getConfig();
  const opt = getOptions();
  const shpPaths = loadedFiles.map((f) => f.shp_path).filter(Boolean);

  if (shpPaths.length > 0 || sourcePath) {
    const spin = $('pvSpin');
    try {
      if (spin) spin.classList.add('on');
      const txt = await tauriInvoke("read_shp_to_txt_preview", { shpPaths, sourceType, sourcePath, headerCfg: cfg.h, fieldMapping: cfg.f, options: opt, selectedLayers: sourceType === "gdb" ? selectedLayers : [] });
      if (txt) { const pv = $("pv"); if (pv) pv.textContent = txt; return; }
    } catch (e) { console.error("Preview error:", e); toast("预览失败: " + (e?.message || e)); }
    finally { if (spin) spin.classList.remove('on'); }
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

    const zoneVal = collectAttrRows().find((r) => r.k.trim() === "带号")?.v.trim() || "";
    if (!zoneVal || !/^\d+$/.test(zoneVal)) {
      toast("请填写带号后再输出"); return;
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
  const advOn = $("advMode")?.checked || false;
  return {
    h: { attrs: collectAttrRows(), project_info: $("hpi")?.value || "" },
    f: {
      name: $("fn")?.value || "", id: $("fi")?.value || "", area: $("fa")?.value || "", use_field: $("fu")?.value || "", tfh: $("fm")?.value || "", dlbm: $("fd")?.value || "",
      // 高级模式：on=开关态 / preset=预设态 / rows=行状态（持久化用）
      adv: { on: advOn, preset: advPreset, rows: advInitialized ? collectFieldRows() : null },
      // 发给 Rust FieldMapping.columns（高级模式开启才非空；空字段名行 = 占位「新行N」，输出空值列）
      columns: advOn && advInitialized
        ? collectFieldRows().map((r) => ({ name: r.name, source: r.source }))
        : [],
    },
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
    oc: $("oc")?.value === "1",
    og: ($("og")?.checked && !$("og")?.disabled && projMode === "keep") || false,
    zone_type: parseInt($("oz")?.value, 10) || 3,
    proj_mode: projMode || "keep",
    proj_zone: typeof projZone !== "undefined" ? (projZone || null) : null,
    proj_no_prefix: !!window._projNoPrefix,
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

// ═══ 属性描述动态行（紧凑行式 + 拖拽排序 + 键名驱动下拉） ═══
function escAttr(s) {
  return String(s).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// WebView2 的 confirm/prompt 均静默失败（不弹框返回 falsy）——Tauri 环境统一走 plugin-dialog 原生弹窗，浏览器开发回退 confirm
async function uiConfirm(msg) {
  if (window.__TAURI_INTERNALS__) {
    try { return await dialogAsk(msg, { title: "确认" }); }
    catch (e) { console.warn("ask:", e); toast("确认框失败: " + (e && e.message ? e.message : e)); return false; }
  }
  return window.confirm(msg);
}

// ─── 两段式确认按钮（GitHub 删除模式）：首次点击变红提示，再点执行，点按钮外任意处取消 ───
// WebView2 下所有确认弹窗（prompt/confirm/plugin-dialog ask）均不可靠，破坏性操作一律走此纯 DOM 方案。
// 不设超时——用户犹豫期间红字保持可见（3s 自动复原曾让用户以为「点了没反应」）
function armVisual(btn, label) {
  if (!btn || btn.dataset.arm === "1") return;
  btn.dataset.arm = "1";
  btn.dataset.label = btn.textContent;
  btn.textContent = label;
  btn.classList.add("arm");
}
function disarmButton(btn) {
  if (!btn) return;
  if (btn.dataset.label !== undefined) btn.textContent = btn.dataset.label;
  delete btn.dataset.label;
  delete btn.dataset.arm;
  btn.classList.remove("arm");
}
// 点任何确认态按钮以外的地方 = 取消确认（全局一次绑定）
document.addEventListener("mousedown", (e) => {
  document.querySelectorAll("button.arm").forEach((b) => {
    if (!e.target.closest("button") || e.target.closest("button") !== b) disarmButton(b);
  });
}, true);
function armButton(btn, confirmLabel, action) {
  if (!btn) return;
  if (btn.dataset.arm === "1") { disarmButton(btn); action(); return; }
  armVisual(btn, confirmLabel);
}

// 键名 → 固定候选值。这些键的值框渲染为 <select>（限选），其余键为自由 <input>
const ATTR_SELECT_OPTIONS = {
  "几度分带": ["3", "6"],
};

/// 精度字符串 → slider 指数: "0.001"→3, "1"→0, "0.00000001"→8
function precisionToExponent(s) {
  const v = parseFloat(s);
  if (isNaN(v) || v <= 0 || v > 1) return 3;
  const exp = Math.round(-Math.log10(v));
  return Math.max(0, Math.min(8, exp));
}
/// slider 指数 → 精度十进制字符串: 3→"0.001", 0→"1"
function exponentToPrecision(exp) {
  // 不用 parseFloat，避免 1e-8 科学记数；toFixed 直接给 "0.00000001"
  if (exp <= 0) return "1";
  return Math.pow(10, -exp).toFixed(exp);
}

function renderAttrRows(attrs) {
  const box = $("attrRows");
  if (!box) return;
  const defaultCount = DEFAULT_ATTRS.length; // 前 defaultCount 行为标准行，不可删除
  box.innerHTML = "";
  attrs.forEach((row, i) => {
    const div = document.createElement("div");
    div.className = "attr-row";
    div.dataset.i = String(i);
    // 值控件：精度行 → range 滑块；键名命中候选表 → select；否则 input
    const opts = ATTR_SELECT_OPTIONS[row.k];
    let valueCtrl;
    if (row.k === '精度') {
      const exp = precisionToExponent(row.v);
      valueCtrl = `<span class="prec-wrap">`
        + `<input type="range" class="prec-slider av" data-i="${i}" data-f="v" min="0" max="8" value="${exp}" step="1">`
        + `<span class="prec-val">${exponentToPrecision(exp)}</span>`
        + `</span>`;
    } else if (opts) {
      const inOpts = opts.includes(row.v);
      const std = opts.map((o) => `<option value="${o}"${o === row.v ? " selected" : ""}>${o}</option>`).join("");
      const extra = inOpts ? "" : `<option value="${escAttr(row.v)}" selected>${escAttr(row.v)}</option>`;
      valueCtrl = `<select class="av" data-i="${i}" data-f="v">${std}${extra}</select>`;
    } else {
      valueCtrl = `<input class="av" data-i="${i}" data-f="v" value="${escAttr(row.v)}" placeholder="值">`;
    }
    // 标准行（i < defaultCount）无 ✕ 按钮；用户新增行才可删
    const btnHtml = i < defaultCount
      ? ""
      : `<button class="abtn del" data-act="del" data-i="${i}" title="删除此行">✕</button>`;
    div.innerHTML =
      `<span class="grip" title="拖动排序">⠿</span>` +
      `<input class="ak" data-i="${i}" data-f="k" value="${escAttr(row.k)}" placeholder="键名">` +
      `<span class="aeq">=</span>` +
      valueCtrl +
      btnHtml;
    box.appendChild(div);
  });
}

function collectAttrRows() {
  const box = $("attrRows");
  if (!box) return DEFAULT_ATTRS.map((r) => ({ ...r }));
  const rows = [];
  box.querySelectorAll(".attr-row").forEach((div) => {
    const k = div.querySelector(".ak")?.value ?? "";
    let v;
    const slider = div.querySelector(".prec-slider");
    if (slider) {
      v = String(exponentToPrecision(parseInt(slider.value, 10)));
    } else {
      v = div.querySelector(".av")?.value ?? "";
    }
    rows.push({ k, v });
  });
  return rows;
}

function bindAttrRowEvents() {
  const box = $("attrRows");
  if (!box) return;
  // 删除
  box.addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-act='del']");
    if (!btn) return;
    const i = parseInt(btn.dataset.i, 10);
    const rows = collectAttrRows();
    rows.splice(i, 1);
    renderAttrRows(rows);
    updatePreview();
  });
  // 输入实时更新预览 + headerManual 标记（input 和 select 都走 input 事件）
  box.addEventListener("input", (e) => {
    const slider = e.target.closest(".prec-slider");
    if (slider) {
      const val = exponentToPrecision(parseInt(slider.value, 10));
      const disp = slider.parentNode.querySelector(".prec-val");
      if (disp) disp.textContent = val;
    }
    const ctrl = e.target.closest(".ak, .av");
    if (ctrl) {
      const row = ctrl.closest(".attr-row");
      const k = row?.querySelector(".ak")?.value?.trim();
      if (k) headerManual[k] = true;
    }
    updatePreview();
    updateProjButton();
  });
  // 键名失焦：若值控件类型需要切换（input↔select）才重渲染，避免无谓重建
  box.addEventListener("change", (e) => {
    const ak = e.target.closest("input.ak");
    if (!ak) return;
    const row = ak.closest(".attr-row");
    if (!row) return;
    const i = parseInt(row.dataset.i || "0", 10);
    const rows = collectAttrRows();
    const needSelect = !!ATTR_SELECT_OPTIONS[(rows[i]?.k || "").trim()];
    const isSelectNow = !!row.querySelector("select.av");
    if (needSelect !== isSelectNow) {
      renderAttrRows(rows);
      updatePreview();
    }
  });
  // 拖拽排序（手动 mouse 实现，避免 WebView2 HTML5 DnD 兼容问题；实时排序视觉）
  box.addEventListener("mousedown", (e) => {
    const grip = e.target.closest(".grip");
    if (!grip || e.button !== 0) return;
    const row = grip.closest(".attr-row");
    if (!row) return;
    e.preventDefault();
    row.classList.add("dragging");
    const onMove = (ev) => {
      const el = document.elementFromPoint(ev.clientX, ev.clientY);
      const targetRow = el && el.closest(".attr-row");
      if (!targetRow || targetRow === row) return;
      const rect = targetRow.getBoundingClientRect();
      if (ev.clientY - rect.top > rect.height / 2) targetRow.after(row);
      else targetRow.before(row);
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      row.classList.remove("dragging");
      renderAttrRows(collectAttrRows());
      updatePreview();
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  });
}

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
  renderAttrRows(DEFAULT_ATTRS.map((r) => ({ ...r })));
  updatePreview();
  // 立即落盘「已恢复默认」的结果，避免下次加载时旧自定义值再次复活
  clearTimeout(autoSaveTimer);
  flushAutoSave();
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
  if (c.h) {
    const hn = normalizeH(c.h);
    renderAttrRows(hn.attrs);
    if ($("hpi")) $("hpi").value = hn.project_info;
  }
  if (c.p) {
    if ($("ox")) $("ox").checked = !!c.p.ox;
    if ($("oj")) $("oj").checked = !!c.p.oj;
    if ($("on")) $("on").checked = !!c.p.on;
    if ($("oo")) $("oo").checked = !!c.p.oo;
    if ($("oc")) {
      $("oc").value = c.p.oc ? "1" : "0";
      $("oc").disabled = !$("oo").checked;
    }
    if ($("og")) $("og").checked = !!c.p.og;
    if ($("oz")) $("oz").value = c.p.oz === "6" ? "6" : "3";
    if ($("oz") && $("og")) $("oz").disabled = !$("og").checked || $("og").disabled;
    refreshOgWarn();
    if ($("om")) $("om").checked = !!c.p.om;
  }
  if (c.f) {
    // 6 槽位按元素 ID 恢复（f 键名为 DOM id：fn/fi/fa/fu/fm/fd）
    ["fn", "fi", "fa", "fu", "fm", "fd"].forEach((k) => { const e = $(k); if (e && typeof c.f[k] === "string") e.value = c.f[k]; });
    // 高级模式配置恢复（旧配置无 adv → 关闭态）
    applyAdvConfig(c.f.adv || null);
  }
  const cn = $("cn");
  if (cn) cn.textContent = c.n || "自定义";
  document.querySelectorAll("#ch .chip").forEach((e) => e.classList.remove("on"));
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

const doDelCfg = () => {
  delete cfgs[cur];
  localStorage.setItem("tg_dark", JSON.stringify(cfgs));
  cur = "usr";
  ld("usr");
  renderChips();
  toast("已删除");
};
window.delCfg = doDelCfg;

// ═══ Open GitHub ═══
window.openGitHub = async function () {
  try {
    await shellOpen("https://github.com/edcfoshan/polygon-txt");
  } catch (e) { console.error("openGitHub:", e); }
};

// ═══ 自动更新（Tauri Updater） ═══
// 标题栏常驻三态按钮：idle(刷新图标) / available(绿箭头脉冲) / skipped(灰箭头) / loading(旋转)
// 流程：启动检查(24h节流) → 检测到新版亮箭头+自动弹窗 → 立即更新/稍后/跳过 → 失败兜底百度云
const BAIDU_PAN_URL = "https://pan.baidu.com/s/1xyW3-hyZrFDDG9ijYOf46g?pwd=e8vy";
const LS_LAST_CHECK = "tg_update_lastCheck";
const LS_SKIPPED = "tg_update_skipped";
const LS_KNOWN = "tg_update_known";
const CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000; // 24h 节流

let pendingUpdate = null;
let isUpdating = false;

let APP_VERSION = ""; // 启动时由 initVersion() 填充（= tauri.conf.json 的 version）

// 启动时拉取 Tauri 版本并填充标题栏 brand-sub
async function initVersion() {
  try { APP_VERSION = await getVersion(); } catch { APP_VERSION = ""; }
  const el = document.querySelector(".brand-sub");
  if (el) el.textContent = "V" + APP_VERSION;
}

// 当前应用版本（缓存），用于和远端比对
function currentAppVersion() {
  return APP_VERSION || "0.0.0";
}

// 语义化版本比较：a > b 返回 1，相等 0，小于 -1
function compareVersion(a, b) {
  const pa = String(a).split(".").map((n) => parseInt(n, 10) || 0);
  const pb = String(b).split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if ((pa[i] || 0) > (pb[i] || 0)) return 1;
    if ((pa[i] || 0) < (pb[i] || 0)) return -1;
  }
  return 0;
}

// 设置标题栏按钮状态
function setUpdateBtnState(state) {
  const btn = $("btnUpdate");
  if (!btn) return;
  btn.classList.remove("idle", "available", "skipped", "loading");
  btn.classList.add(state);
  const titles = {
    idle: "检查更新",
    available: "发现新版本，点击查看",
    skipped: "已跳过此版本（点击重新查看）",
    loading: "正在检查更新…",
  };
  btn.title = titles[state] || "检查更新";
}

// 读取/写入 localStorage 辅助
function readKnown() {
  try { return JSON.parse(localStorage.getItem(LS_KNOWN) || "null"); } catch { return null; }
}
function writeKnown(version, body) {
  localStorage.setItem(LS_KNOWN, JSON.stringify({ version, body }));
}
function readSkipped() { return localStorage.getItem(LS_SKIPPED) || ""; }
function writeSkipped(v) { localStorage.setItem(LS_SKIPPED, v); }

// 根据"已知最新版本"恢复按钮态（不触发网络）
function applyKnownState() {
  const known = readKnown();
  if (!known || !known.version) { setUpdateBtnState("idle"); return; }
  const cur = currentAppVersion();
  if (compareVersion(known.version, cur) !== 1) {
    setUpdateBtnState("idle");
    return;
  }
  const skipped = readSkipped();
  setUpdateBtnState(compareVersion(known.version, skipped) === 0 ? "skipped" : "available");
  return known;
}

// 检查更新。manual=true 表示用户手动触发（忽略节流、查完给反馈、有新版自动弹窗）
async function checkAppUpdate(manual = false) {
  // 启动模式 + 24h 内已查过：直接恢复态，不查网络、不弹窗
  if (!manual) {
    const last = parseInt(localStorage.getItem(LS_LAST_CHECK) || "0", 10);
    if (last && Date.now() - last < CHECK_INTERVAL_MS) {
      applyKnownState();
      return;
    }
  }
  setUpdateBtnState("loading");
  try {
    const upd = await checkForUpdater();
    localStorage.setItem(LS_LAST_CHECK, String(Date.now()));
    if (upd && upd.available) {
      pendingUpdate = upd;
      writeKnown(upd.version, upd.body || "");
      const isSkipped = compareVersion(upd.version, readSkipped()) === 0;
      setUpdateBtnState(isSkipped ? "skipped" : "available");
      // 启动时未跳过 → 自动弹窗；手动检查时无论是否跳过都弹（用户主动想知道）
      if (manual || !isSkipped) openUpdateModal();
    } else {
      pendingUpdate = null;
      setUpdateBtnState("idle");
      if (manual) toast("已是最新版本");
    }
  } catch (e) {
    console.warn("checkAppUpdate:", e);
    // 检查失败：恢复到已知态（若有），避免一直 loading
    applyKnownState();
    if (manual) toast("检查更新失败，请稍后重试");
  }
}

// 打开更新模态框：填充版本号 / 更新说明 / 重置进度条 / 跳过按钮显隐
window.openUpdateModal = function () {
  // 没有 pendingUpdate 时，从 localStorage 恢复一份给弹窗展示（用于"已跳过态"重新打开）
  if (!pendingUpdate) {
    const known = readKnown();
    if (known) pendingUpdate = { version: known.version, body: known.body, __restored: true };
  }
  if (!pendingUpdate) { checkAppUpdate(true); return; }
  const m = $("updateModal");
  if (!m) return;
  const vEl = $("updateVersion");
  if (vEl) vEl.textContent = pendingUpdate.version || "";
  const nEl = $("updateNotes");
  if (nEl) nEl.innerHTML = renderMarkdown(pendingUpdate.body || "*（暂无更新说明）*");
  const prog = $("updateProgress");
  if (prog) { prog.value = 0; prog.parentElement.style.display = "none"; }
  const doBtn = $("btnDoUpdate");
  if (doBtn) { doBtn.disabled = false; doBtn.textContent = "立即更新"; }
  // 已跳过态打开时（restored），隐藏"跳过此版本"按钮，置灰态不重复跳过
  const skipBtn = $("btnSkipUpdate");
  if (skipBtn) skipBtn.hidden = !!pendingUpdate.__restored;
  m.classList.add("on");
};

window.closeUpdateModal = function (e) {
  if (e && e.target !== $("updateModal")) return;
  if (isUpdating) return; // 下载安装中禁止关闭
  const m = $("updateModal");
  if (m) m.classList.remove("on");
};

// 跳过当前版本：写入 localStorage，按钮转灰态，关弹窗
window.skipCurrentVersion = function () {
  if (!pendingUpdate?.version) return;
  writeSkipped(pendingUpdate.version);
  setUpdateBtnState("skipped");
  closeUpdateModal();
  toast("已跳过 " + pendingUpdate.version + "，下次有更新还会提醒");
};

// 下载并安装：监听进度 → 更新进度条 → relaunch。失败兜底百度云。
window.doUpdate = async function () {
  // restored 态（从 localStorage 恢复的）没有真实 update 对象，需重新检查拿真实句柄
  if (pendingUpdate?.__restored) {
    setUpdateBtnState("loading");
    try {
      pendingUpdate = await checkForUpdater();
    } catch (e) {
      toast("检查失败，请稍后重试");
      setUpdateBtnState("available");
      return;
    }
  }
  if (!pendingUpdate || isUpdating) return;
  isUpdating = true;
  const doBtn = $("btnDoUpdate");
  const prog = $("updateProgress");
  const pct = $("updatePct");
  try {
    if (doBtn) { doBtn.disabled = true; doBtn.textContent = "下载中…"; }
    if (prog) { prog.parentElement.style.display = "block"; prog.value = 0; }
    let downloaded = 0, total = 0;
    await pendingUpdate.downloadAndInstall((event) => {
      if (event.event === "Started" && event.data.contentLength) {
        total = event.data.contentLength;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        if (prog && total > 0) prog.value = downloaded / total;
        if (pct && total > 0) pct.textContent = Math.round((downloaded / total) * 100) + "%";
      }
    });
    if (doBtn) doBtn.textContent = "安装完成，即将重启…";
    await relaunch();
  } catch (e) {
    console.error("doUpdate:", e);
    isUpdating = false;
    if (doBtn) { doBtn.disabled = false; doBtn.textContent = "立即更新"; }
    if (prog) prog.parentElement.style.display = "none";
    // 兜底：引导跳转百度云手动下载（国内访问稳定）
    if (await uiConfirm("自动下载/安装失败，是否打开百度网盘手动下载？")) {
      shellOpen(BAIDU_PAN_URL);
    }
  }
};

// ═══ Modals ═══
window.openAbout = function () { const m = $("aboutModal"); if (m) m.classList.add("on"); };
window.closeAbout = function (e) { if (e && e.target !== $("aboutModal")) return; const m = $("aboutModal"); if (m) m.classList.remove("on"); };
window.openSettings = function () { const m = $("settingsModal"); if (m) m.classList.add("on"); };
window.closeSettings = function (e) { if (e && e.target !== $("settingsModal")) return; const m = $("settingsModal"); if (m) m.classList.remove("on"); };
document.addEventListener("keydown", function (e) {
  if (e.key === "Escape") { const am = $("aboutModal"); const gm = $("gdbSelectModal"); const um = $("updateModal"); const rm = $("settingsModal"); if (am) am.classList.remove("on"); if (gm) gm.classList.remove("on"); if (um && !isUpdating) um.classList.remove("on"); if (rm) rm.classList.remove("on"); }
});

// ═══ Display ratio (s-mode three-column) ═══
const RATIO_PRESETS = [
  { id: "even",    label: "三栏等宽", c1: "100fr", c2: "100fr", c3: "100fr" },
  { id: "default", label: "默认",     c1: "100fr", c2: "100fr", c3: "130fr" },
  { id: "wide",    label: "宽预览",   c1: "100fr", c2: "100fr", c3: "180fr" },
  { id: "input",   label: "宽输入",   c1: "150fr", c2: "100fr", c3: "130fr" },
];
function applyRatio(preset) {
  document.documentElement.style.setProperty("--col1", preset.c1);
  document.documentElement.style.setProperty("--col2", preset.c2);
  document.documentElement.style.setProperty("--col3", preset.c3);
}
function getCurrentRatioId() {
  const saved = localStorage.getItem("tg_ratio");
  return saved && RATIO_PRESETS.some(p => p.id === saved) ? saved : "default";
}
function renderRatioChips() {
  const box = $("ratioChips");
  if (!box) return;
  const cur = getCurrentRatioId();
  box.innerHTML = "";
  for (const p of RATIO_PRESETS) {
    const c = document.createElement("div");
    c.className = "chip" + (p.id === cur ? " on" : "");
    c.textContent = p.label;
    c.tabIndex = 0;
    c.onclick = () => {
      localStorage.setItem("tg_ratio", p.id);
      applyRatio(p);
      renderRatioChips();
    };
    box.appendChild(c);
  }
}

// ═══ Init ═══
async function init() {
  // ─── 窗口长宽+位置记忆（v3.0）：恢复上次状态；同步快存（防快速关闭丢失）+ 异步精修（含 x/y） ───
  try {
    const appWin = getCurrentWindow();
    // 恢复：尺寸必恢复；位置做屏幕内 clamp（防换小屏/拔显示器后窗口跑到屏幕外）
    const savedWin = JSON.parse(localStorage.getItem("tg_win") || "null");
    if (savedWin && savedWin.w >= 800 && savedWin.h >= 540) {
      appWin.setSize(new LogicalSize(savedWin.w, savedWin.h)).then(() => {
        if (Number.isFinite(savedWin.x) && Number.isFinite(savedWin.y)) {
          const sw = window.screen.availWidth || window.screen.width;
          const sh = window.screen.availHeight || window.screen.height;
          const x = Math.min(Math.max(savedWin.x, -savedWin.w + 200), sw - 200);
          const y = Math.min(Math.max(savedWin.y, 0), sh - 100);
          return appWin.setPosition(new LogicalPosition(x, y));
        }
      }).catch(() => {});
    }
    // 同步快存：仅尺寸（window.innerWidth 同步可读，防拖完立即关闭丢失）
    const quickSave = () => {
      try {
        const w = window.innerWidth, h = window.innerHeight;
        if (w >= 800 && h >= 540) {
          const old = JSON.parse(localStorage.getItem("tg_win") || "{}");
          localStorage.setItem("tg_win", JSON.stringify({ ...old, w, h }));
        }
      } catch (e) {}
    };
    // 异步精修：尺寸 + 位置 + 最大化过滤
    const fullSave = async () => {
      try {
        if (await appWin.isMaximized()) return;
        const [s, p, scale] = await Promise.all([appWin.innerSize(), appWin.outerPosition(), appWin.scaleFactor()]);
        const rec = {
          w: Math.round(s.width / scale), h: Math.round(s.height / scale),
          x: Math.round(p.x / scale), y: Math.round(p.y / scale),
        };
        if (rec.w >= 800 && rec.h >= 540) localStorage.setItem("tg_win", JSON.stringify(rec));
      } catch (e) {}
    };
    appWin.onResized(() => { quickSave(); fullSave(); }).catch(() => {});
    appWin.onMoved(() => fullSave()).catch(() => {});
    window.addEventListener("beforeunload", quickSave);
  } catch (e) { /* 浏览器 dev 环境无窗口 API，跳过 */ }

  const savedTheme = localStorage.getItem("tg_theme") || "light";
  // prototype: URL hash demo seeding (#demo=geodetic|projected-3|projected-6|unknown)
  const demoType = (location.hash.match(/demo=([\w-]+)/) || [])[1];
  if (demoType) {
    try {
      applyDemoSeed(demoType);
    } catch (e) {
      console.warn("demo seed failed:", e);
    }
  }
  theme = savedTheme;
  document.documentElement.setAttribute("data-t", theme);
  document.documentElement.setAttribute("data-c", themeColor); // v3.0 色系恢复
  syncThemeUI();

  // 应用保存的显示比例（s 模式三栏）
  const ratioId = getCurrentRatioId();
  const preset = RATIO_PRESETS.find(p => p.id === ratioId) || RATIO_PRESETS[1];
  applyRatio(preset);
  renderRatioChips();

  // 三区字号恢复 + 滑块事件（拖动即时生效，自动保存）
  const FS_SLIDERS = [["a", "fsA", "fsAv"], ["b", "fsB", "fsBv"], ["c", "fsC", "fsCv"]];
  FS_SLIDERS.forEach(([k, sid, vid]) => {
    applyFontScale(k);
    const sl = $(sid), vv = $(vid);
    if (!sl) return;
    sl.value = String(Math.round(readFontScale(k) * 100));
    if (vv) vv.textContent = sl.value + "%";
    sl.addEventListener("input", () => {
      setFontScale(k, parseInt(sl.value, 10));
      if (vv) vv.textContent = sl.value + "%";
    });
  });

  const s = localStorage.getItem("tg_dark");
  if (s) cfgs = JSON.parse(s);
  PP.forEach((p) => {
    if (p.id === "usr") {
      // 恢复：localStorage 里的 usr 必须包含全部 DEFAULT_ATTRS 键，否则重建
      const u = cfgs[p.id];
      const ok = u && typeof u === "object" && u.h && Array.isArray(u.h.attrs)
        && DEFAULT_ATTRS.every((d) => u.h.attrs.some((r) => r.k === d.k));
      if (!ok) cfgs[p.id] = p;
    } else if (!cfgs[p.id]) {
      cfgs[p.id] = p;
    }
  });
  renderChips();
  ld(localStorage.getItem("tg_last") || "usr");

  await initVersion(); // 填充 APP_VERSION + 标题栏 brand-sub（须在 about 渲染前）

  // ─── 注入弹窗内容（Markdown → HTML） ───
  const ab = $("aboutBody");
  if (ab) ab.innerHTML = renderMarkdown(aboutContent).replace(/\{\{version\}\}/g, APP_VERSION ? "V" + APP_VERSION : "")
    + `<div style="font-size:9px;color:var(--tx3);margin-top:10px;text-align:center">构建时间 ${typeof __BUILD_TS__ !== "undefined" ? __BUILD_TS__ : "dev"}</div>`;

  // ─── Bind click events (replaces inline onclick) ───
  const bind = (id, fn) => { const el = $(id); if (el) el.addEventListener("click", fn); };
  bind("btnGitHub", () => openGitHub());
  // 主题/色系/比例/字号统一收进设置面板（v3.1）
  const setModalEl = $("settingsModal");
  if (setModalEl) {
    setModalEl.querySelectorAll(".thopt").forEach((b) => b.addEventListener("click", () => pickTheme(b.dataset.t)));
    setModalEl.querySelectorAll(".copt").forEach((b) => b.addEventListener("click", () => pickColor(b.dataset.c)));
  }
  bind("btnAbout", () => openAbout());
  bind("btnSettings", () => openSettings());
  bind("btnSave", () => saveOnly());
  bind("btnDel", () => {
    if (cur === "usr") { toast("内置预设不可删除"); return; }
    armButton($("btnDel"), "确认删除?", doDelCfg);
  });
  bind("btnWinMin", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    await runWindowCommand("minimize_window");
  });
  bind("btnWinMax", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    await runWindowCommand("toggle_maximize");
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

  // 闭合点编号下拉：未勾「首末点重合」时置灰
  const oo = $("oo");
  const oc = $("oc");
  if (oo && oc) {
    const syncOcDisabled = () => { oc.disabled = !oo.checked; };
    syncOcDisabled();
    oo.addEventListener("change", syncOcDisabled);
    oc.addEventListener("change", () => { lastPreviewKey = ""; updatePreview(); });
  }

  // og 公里网：未勾时 oz 置灰；og/oz 变更刷新预览与软提示
  const og = $("og");
  const oz = $("oz");
  if (og && oz) {
    const syncOz = () => {
      oz.disabled = !og.checked;
      refreshOgWarn();
      lastPreviewKey = "";
      updatePreview();
    };
    og.addEventListener("change", syncOz);
    oz.addEventListener("change", () => { lastPreviewKey = ""; updatePreview(); });
  }

  // 字段映射下拉框改选后刷新预览（fn/fi/fa/fu/fm/fd = 地块名/编号/面积/用途/图幅号/地类编码）
  ["fn", "fi", "fa", "fu", "fm", "fd"].forEach((id) => {
    const el = $(id);
    if (el) el.addEventListener("change", () => { lastPreviewKey = ""; updatePreview(); });
  });

  // 字段映射高级模式：开关 / 补充耕地预设开关 / 添加 / 恢复默认 / 动态行事件
  const advModeEl = $("advMode");
  if (advModeEl) advModeEl.addEventListener("change", () => setAdvModeOn(advModeEl.checked));
  // 预设下拉（v3.0 取代补充耕地 checkbox）：选预设即重填列表；「自定义」仅是状态不主动载入
  const bcgSelEl = $("bcgSel");
  if (bcgSelEl) {
    bcgSelEl.addEventListener("change", () => {
      const v = bcgSelEl.value;
      if (v === "std" || v === "bcg") {
        const preset = v === "bcg" ? BCG_ADV_ROWS : STD_ADV_ROWS;
        renderFieldRows(preset.map((r) => ({ ...r })));
        advPreset = v;
      } else if (v && v !== "custom") {
        // 用户方案（id 前缀 t）：选中即载入整套字段清单
        const t = readAdvTpls().find((x) => x.id === v);
        if (t) {
          renderFieldRows(t.rows.map((r) => ({ name: r.name || "", source: r.source || "" })));
          advPreset = v;
        }
      }
      // 显式选中即锁定显示：syncAdvPresetState 的 std→bcg→方案 判定会把「内容==预设」的方案覆盖成预设名
      renderBcgSel();
      updatePreview();
    });
  }
  // 方案命名输入框：回车提交（借 blur）、Esc 取消、失焦自动提交（点外部不再卡在编辑态导致「存为方案」按钮消失）
  const tplNameEl = $("tplNameInput");
  if (tplNameEl) {
    tplNameEl.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.isComposing) { e.preventDefault(); tplNameEl.blur(); }
      else if (e.key === "Escape") { tplNameEl.dataset.cancel = "1"; tplNameEl.blur(); }
    });
    tplNameEl.addEventListener("input", () => {
      pendingOverwrite = ""; // 改名后覆盖意图失效
      disarmButton($("btnTplOk"));
    });
    tplNameEl.addEventListener("blur", () => {
      if (tplNameEl.style.display === "none") return;
      if (tplNameEl.dataset.cancel === "1") { delete tplNameEl.dataset.cancel; cancelAdvTplSave(); return; }
      const n = tplNameEl.value.trim();
      if (!n) { cancelAdvTplSave(); return; } // 空名失焦 = 放弃
      if (pendingOverwrite === n) { cancelAdvTplSave(); return; } // 重名待覆盖时失焦 = 放弃（防误覆盖）
      confirmAdvTplSave();
    });
  }
  // ✓/✕ 按钮阻止 mousedown 默认行为（防输入框 blur 先触发提交造成双保存），click 走 HTML onclick
  ["btnTplOk", "btnTplCancel"].forEach((id) => {
    const b = $(id);
    if (b) b.addEventListener("mousedown", (e) => e.preventDefault());
  });
  bind("btnAddField", () => {
    const rows = collectFieldRows();
    rows.push({ name: "", source: "" });
    renderFieldRows(rows);
    syncAdvPresetState();
    const box = $("fieldRows");
    if (box) box.scrollTop = box.scrollHeight;
    updatePreview();
  });
  bindFieldRowEvents();

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
  bind("projSwitchToggle", () => { projMode === 'keep' ? openProjModal() : resetProjMode(); });
  bind("projSwitchLabel", () => openProjModal());
  bind("btnProjClose", () => closeProjModal());
  bind("btnProjCancel", () => closeProjModal());
  bind("btnProjApply", () => applyProjMode());
  // click outside proj modal closes it
  const projOverlay = projModal;
  if (projOverlay) projOverlay.addEventListener("click", (e) => { if (e.target === projOverlay) closeProjModal(); });
  // esc closes proj modal
  document.addEventListener("keydown", (e) => { if (e.key === "Escape" && projOverlay && projOverlay.classList.contains("on")) closeProjModal(); });
  // initial gate state
  updateProjButton();
  bind("btnAddAttr", () => {
    const rows = collectAttrRows();
    rows.push({ k: "", v: "" });
    renderAttrRows(rows);
    const box = $("attrRows");
    if (box) box.scrollTop = box.scrollHeight;
    updatePreview();
  });
  bindAttrRowEvents();
  bind("btnRunStt", () => runShpToTxt());
  bind("btnRunTts", () => runTxtToShp());
  bind("btnCloseAbout", () => closeAbout());
  bind("btnCloseSet", () => closeSettings());
  bind("btnFsReset", () => {
    ["a", "b", "c"].forEach((k) => setFontScale(k, 100));
    [["a", "fsA", "fsAv"], ["b", "fsB", "fsBv"], ["c", "fsC", "fsCv"]].forEach(([, sid, vid]) => {
      const sl = $(sid), vv = $(vid);
      if (sl) sl.value = "100";
      if (vv) vv.textContent = "100%";
    });
    toast("字号已恢复默认");
  });
  bind("btnUpdate", () => {
    // available/skipped 态：打开弹窗；idle/loading 态：手动触发检查
    const btn = $("btnUpdate");
    if (btn && (btn.classList.contains("available") || btn.classList.contains("skipped"))) {
      openUpdateModal();
    } else if (btn && !btn.classList.contains("loading")) {
      checkAppUpdate(true);
    }
  });
  bind("btnDoUpdate", () => doUpdate());
  bind("btnSkipUpdate", () => skipCurrentVersion());
  bind("btnCloseUpdate", () => closeUpdateModal());
  bind("btnCancelUpdate", () => closeUpdateModal());
  const bdpan = $("bdpanLink");
  if (bdpan) bdpan.addEventListener("click", (e) => { e.preventDefault(); shellOpen(BAIDU_PAN_URL); });
  bind("btnCancelGdbSelect", () => closeGdbSelectModal());
  bind("btnConfirmGdbSelect", () => confirmGdbSelect());

  // Tab bar switching
  document.querySelectorAll(".tab[data-t]").forEach((tab) => {
    tab.addEventListener("click", () => sw(tab.dataset.t));
  });

  // Modal overlay click-to-close
  const aboutModal = $("aboutModal");
  if (aboutModal) aboutModal.addEventListener("click", (e) => { if (e.target === aboutModal) closeAbout(); });
  const settingsModal = $("settingsModal");
  if (settingsModal) settingsModal.addEventListener("click", (e) => { if (e.target === settingsModal) closeSettings(); });
  const gdbSelectModal = $("gdbSelectModal");
  if (gdbSelectModal) gdbSelectModal.addEventListener("click", (e) => { if (e.target === gdbSelectModal) closeGdbSelectModal(); });
  const updateModal = $("updateModal");
  if (updateModal) updateModal.addEventListener("click", (e) => { if (e.target === updateModal) closeUpdateModal(); });

  // ─── 启动时静默检查更新（失败不报错，仅在有新版本时显示绿色箭头）───
  checkAppUpdate(false);
  // Prevent modal card click from closing
  document.querySelectorAll(".modal-card").forEach((card) => {
    card.addEventListener("click", (e) => e.stopPropagation());
  });

  // ─── Bind input/change events (replaces inline oninput/onchange) ───
  // hc/hb/hj/hu/hz/ha/ht 已改为动态属性行，事件由 bindAttrRowEvents() 在 #attrRows 上委托
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
        if (result.failed && result.failed.length) {
          toast("以下文件解析失败：" + result.failed.join("、"));
        }
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
