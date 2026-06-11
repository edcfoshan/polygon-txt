import { invoke } from '@tauri-apps/api/core';
import { open as shellOpen } from '@tauri-apps/plugin-shell';

// Tauri IPC 调用
async function tauriInvoke(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    console.error('[Tauri] invoke error:', cmd, e);
    throw e;
  }
}

// ═══ State ═══
let loadedFiles = [];
let txtFiles = [];
let cur = "basic";
let cfgs = {};
let headerManual = {};
let lastPreviewKey = "";
let previewTimer = null;
let theme = "light";
let gdbPath = null;

const FIELD_MATCH_RULES = {
  fn: ["DKMC", "MC", "NAME"],
  fi: ["DKBH", "BH", "ID"],
  fa: ["MJ", "AREA"],
  fu: ["DKYT", "YT", "YONGTU"],
  fm: ["TFH"],
  fd: ["DLBM", "DL"],
};

const PP = [
  { id: "basic", n: "基础地块", h: { c: "2000国家大地坐标系", b: "3", j: "高斯克吕格", u: "米", z: "", a: "0.001", t: ",,,,,," }, p: { pp: 3, pz: "auto", pb: 0, ox: 0, oj: 1, op: 0, on: 0, oo: 1, om: 0 }, f: { fn: "DKMC", fi: "DKBH", fa: "", fu: "", fm: "", fd: "" } },
  { id: "gov", n: "规划审批", h: { c: "2000国家大地坐标系", b: "3", j: "高斯克吕格", u: "米", z: "", a: "0.001", t: ",,,,,," }, p: { pp: 3, pz: "auto", pb: 0, ox: 0, oj: 1, op: 0, on: 0, oo: 1, om: 0 }, f: { fn: "DKMC", fi: "DKBH", fa: "MJ", fu: "DKYT", fm: "TFH", fd: "DLBM" } },
  { id: "usr", n: "自定义", h: { c: "2000国家大地坐标系", b: "3", j: "高斯克吕格", u: "米", z: "", a: "0.001", t: ",,,,,," }, p: { pp: 3, pz: "auto", pb: 0, ox: 0, oj: 1, op: 0, on: 0, oo: 1, om: 0 }, f: { fn: "DKMC", fi: "DKBH", fa: "", fu: "", fm: "", fd: "" } },
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

// ═══ SHP 导入 ═══
window.importShp = async function () {
  try {
    const result = await tauriInvoke("pick_shp_files");
    if (!result.files || result.files.length === 0) return;
    loadedFiles = result.files;
    gdbPath = null;
    renderFileList();
    processImport();
  } catch (e) {
    toast("导入失败: " + e);
  }
};

// ═══ GDB 导入 ═══
window.importGdb = async function () {
  try {
    const result = await tauriInvoke("import_gdb");
    if (!result || !result.path) return;
    gdbPath = result.path;
    loadedFiles = [];
    const fl = $("fl");
    fl.innerHTML = `<div class="fitem"><span class="fn">◈ ${result.name}.gdb</span><span class="fs">${result.num_features}个要素</span></div>`;
    autoMatchFields(result.field_names);
    toast(`已导入 GDB: ${result.name} (${result.layers.length} 个图层)`);
    updatePreview();
  } catch (e) {
    toast("导入 GDB 失败: " + e);
  }
};

function renderFileList() {
  const fl = $("fl");
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

// ═══ TXT 导入 ═══
window.importTxt = async function () {
  try {
    const result = await tauriInvoke("pick_txt_files");
    if (!result.files || result.files.length === 0) return;
    txtFiles = result.files;
    renderTxtFileList();
    renderTxtParseLog();
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

window.clearAllFiles = function () { loadedFiles = []; gdbPath = null; const fl = $("fl"); if (fl) fl.innerHTML = ""; toast("已清空"); };
window.clearAllFilesTxt = function () { txtFiles = []; const fl = $("flT"); if (fl) fl.innerHTML = ""; const pv = $("pvT"); if (pv) pv.textContent = "等待导入 TXT 文件…"; toast("已清空"); };

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

  if (shpPaths.length > 0 || gdbPath) {
    try {
      const txt = await tauriInvoke("read_shp_to_txt_preview", { shpPaths, gdbPath, headerCfg: cfg.h, fieldMapping: cfg.f, options: opt });
      if (txt) { const pv = $("pv"); if (pv) pv.textContent = txt; return; }
    } catch (e) { console.log("Preview error:", e); }
  }
  const pv = $("pv");
  if (pv) pv.textContent = out || "等待导入 SHP 或 GDB 文件…";
  lastPreviewKey = out;
}

// ═══ Run ═══
window.runShpToTxt = async function () {
  const shpPaths = loadedFiles.map((f) => f.shp_path).filter(Boolean);
  if (!shpPaths.length && !gdbPath) { toast("请先导入 SHP 或 GDB 文件"); return; }

  let outDir = $("out_dir_s")?.value || "";
  if (!outDir) {
    outDir = await tauriInvoke("pick_output_dir");
    if (!outDir) { toast("请选择输出目录"); return; }
    const inp = $("out_dir_s");
    if (inp) inp.value = outDir;
  }

  const cfg = getConfig();
  const opt = getOptions();
  try {
    const result = await tauriInvoke("run_shp_to_txt", { shpPaths, gdbPath, headerCfg: cfg.h, fieldMapping: cfg.f, options: opt, outputDir: outDir });
    toast("✓ " + result.message);
    const pf = $("pf"); const ps = $("ps");
    if (pf) pf.style.width = "100%";
    if (ps) ps.textContent = "完成";
  } catch (e) { toast("转换失败: " + e); }
};

window.runTxtToShp = async function () {
  if (!txtFiles.length) { toast("请先导入 TXT 文件"); return; }
  const outDir = await tauriInvoke("pick_output_dir");
  if (!outDir) { toast("请选择输出目录"); return; }

  const outputShp = $("of_shp")?.checked || false;
  const outputGdb = $("of_gdb")?.checked || false;
  if (!outputShp && !outputGdb) { toast("请至少选择一种输出格式"); return; }

  const txtPaths = txtFiles.map((f) => f.path);
  const cfg = getConfig();
  try {
    const result = await tauriInvoke("run_txt_to_shp", {
      txtPaths,
      options: { output_shp: outputShp, output_gdb: outputGdb, merge: $("org_merge")?.checked || false, output_dir: outDir },
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
  return { ox: $("ox")?.checked || false, oj: $("oj")?.checked || false, op: $("op")?.checked || false, on: $("on")?.checked || false, oo: $("oo")?.checked || false, om: $("om")?.checked || false, buffer: parseFloat($("pb")?.value) || 0 };
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
    if ($("pb")) $("pb").value = c.p.pb ?? 0;
    if ($("ox")) $("ox").checked = !!c.p.ox;
    if ($("oj")) $("oj").checked = !!c.p.oj;
    if ($("op")) $("op").checked = !!c.p.op;
    if ($("on")) $("on").checked = !!c.p.on;
    if ($("oo")) $("oo").checked = !!c.p.oo;
    if ($("om")) $("om").checked = !!c.p.om;
  }
  if (c.f) Object.keys(c.f).forEach((k) => { const e = $(k); if (e) e.value = c.f[k]; });
  const cn = $("cn");
  if (cn) cn.textContent = c.n || "基础地块";
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
  let existing = null;
  for (const [, v] of Object.entries(cfgs)) { if (v.n === newName) { existing = v; break; } }
  const c = getConfig();
  const cfgObj = { id: existing ? existing.id : "u" + Date.now(), n: newName, h: c.h, p: getOptions(), f: c.f };
  cfgs[cfgObj.id] = cfgObj;
  localStorage.setItem("tg_dark", JSON.stringify(cfgs));
  cur = cfgObj.id;
  cn.textContent = cfgObj.n;
  renderChips();
  toast("已保存 「" + cfgObj.n + "」");
};

window.delCfg = function () {
  if (cur === "basic" || cur === "gov" || cur === "usr") { toast("内置预设不可删除"); return; }
  if (!confirm("确定删除方案「" + ($("cn")?.textContent || "") + "」？")) return;
  delete cfgs[cur];
  localStorage.setItem("tg_dark", JSON.stringify(cfgs));
  cur = "basic";
  ld("basic");
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
  if (e.key === "Escape") { const sm = $("sponsorModal"); const am = $("aboutModal"); if (sm) sm.classList.remove("on"); if (am) am.classList.remove("on"); }
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
  ld(localStorage.getItem("tg_last") || "basic");

  // ─── Bind click events (replaces inline onclick) ───
  const bind = (id, fn) => { const el = $(id); if (el) el.addEventListener("click", fn); };
  bind("btnGitHub", () => openGitHub());
  bind("btnSponsor", () => openSponsor());
  bind("btnTheme", () => togTheme());
  bind("btnAbout", () => openAbout());
  bind("btnSave", () => saveOnly());
  bind("btnDel", () => delCfg());
  bind("dropZone", () => importShp());
  bind("dropGdb", () => importGdb());
  bind("btnClearS", () => clearAllFiles());
  bind("btnBrowseS", () => selectOutputDirS());
  bind("dropZoneTxt", () => importTxt());
  bind("btnClearT", () => clearAllFilesTxt());
  bind("out_btn", () => selectOutputDir());
  bind("hdrTabAttr", () => switchHdrTab("attr"));
  bind("hdrTabProj", () => switchHdrTab("proj"));
  bind("btnPrefill", () => prefillProject());
  bind("btnRunStt", () => runShpToTxt());
  bind("btnRunTts", () => runTxtToShp());
  bind("btnCloseAbout", () => closeAbout());
  bind("btnCloseSponsor", () => closeSponsor());

  // Tab bar switching
  document.querySelectorAll(".tab[data-t]").forEach((tab) => {
    tab.addEventListener("click", () => sw(tab.dataset.t));
  });

  // Modal overlay click-to-close
  const aboutModal = $("aboutModal");
  if (aboutModal) aboutModal.addEventListener("click", (e) => { if (e.target === aboutModal) closeAbout(); });
  const sponsorModal = $("sponsorModal");
  if (sponsorModal) sponsorModal.addEventListener("click", (e) => { if (e.target === sponsorModal) closeSponsor(); });
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
        if (result.files && result.files.length > 0) {
          loadedFiles = result.files; gdbPath = null; renderFileList(); processImport();
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
        }
      } catch (err) { toast("拖放导入失败: " + err); }
    });
  }

  up();
}

init();
