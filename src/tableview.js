// 右栏「属性表」页签：手写表格 + 分页；筛选条件由 querybuilder.js 构建生成 WHERE 文本，
// 本模块只负责求值（filterparser）与所见即所得状态（筛出的行 = 导出范围）
import { FP } from './filterparser.js';

const PAGE_SIZE = 100;

// 常见字段别名（SHP/DBF 无别名元数据，按国土行业惯例内置；GDB 文件别名优先级更高）
const BUILTIN_ALIASES = {
  'DKMC': '地块名称', 'DKBH': '地块编号', 'MJ': '面积', 'DKMJ': '地块面积',
  'DKYT': '地块用途', 'TFH': '图幅号', 'DLBM': '地类编码', 'DL': '地类',
  'DKLX': '地块类型', 'DKJJ': '地块价格', 'BZ': '备注', 'LXR': '联系人', 'LXDH': '联系电话',
  'SJXZQMC': '省级名称', 'SJXZQDM': '省级代码', 'XJXZQMC': '县级名称', 'XJXZQDM': '县级代码',
  'ZYYT': '主要用途', 'TDZL': '土地坐落', 'RJLXX': '容积率下限', 'RJLSX': '容积率上限',
  'JZMDXX': '建筑密度下限', 'JZMDSX': '建筑密度上限', 'JZXG': '建筑限高',
  'SHAPE_Length': '形状长度', 'SHAPE_Area': '形状面积', 'SHAPE_Leng': '形状长度',
};

const state = {
  sources: [],   // [{name, degraded, geodetic}]
  rows: [],      // [{uid:'si:pi', si, pi, cells:{col:value}}]
  columns: [],   // ['源','序号', ...属性字段（首次出现序）]
  aliasMap: {},  // 字段名 → 别名（文件别名覆盖内置字典）
  aliasVisible: false,
  filtered: null, // 命中行数组；null = 未筛选（全量）
  uids: null,    // 命中 [si,pi] 数组；null = 无筛选/条件有误
  error: null,   // {pos,message}
  empty: false,
  page: 0,
  selUid: null,
};

let bound = false;

function $(id) { return document.getElementById(id); }

export const TV = {
  init() {
    if (bound) return;
    bound = true;
    state.aliasVisible = localStorage.getItem('tg_tbl_alias') === '1';
    const aliasCk = $('tblAlias');
    if (aliasCk) {
      aliasCk.checked = state.aliasVisible;
      aliasCk.addEventListener('change', () => TV.setAliasVisible(aliasCk.checked));
    }
    document.addEventListener('click', (e) => {
      const pop = $('cellPop');
      if (pop && !pop.contains(e.target) && e.target.closest('.tbl td') === null) pop.remove();
    });
    $('pgPrev')?.addEventListener('click', () => { if (state.page > 0) { state.page--; render(); } });
    $('pgNext')?.addEventListener('click', () => {
      const total = (state.filtered || state.rows).length;
      if ((state.page + 1) * PAGE_SIZE < total) { state.page++; render(); }
    });
  },

  // 收到 read_plot_table_geo 数据（全量，无筛选口径）；筛选由 main.js 在 setData 后经 applyFromBuilder 应用
  setData(geo) {
    if (!geo || !geo.plots || !geo.plots.length) { TV.reset(); return; }
    state.sources = geo.sources || [];
    const cols = ['源', '序号'];
    const seen = new Set(['源', '序号']);
    state.rows = geo.plots.map((p) => {
      const cells = { '源': state.sources[p.si]?.name || `源${p.si}`, '序号': String(p.pi + 1) };
      for (const [k, v] of p.attrs || []) {
        if (!seen.has(k)) { seen.add(k); cols.push(k); }
        cells[k] = v;
      }
      return { uid: `${p.si}:${p.pi}`, si: p.si, pi: p.pi, cells };
    });
    state.columns = cols;
    state.aliasMap = { ...BUILTIN_ALIASES };
    for (const [k, v] of geo.source_aliases || []) {
      if (v) state.aliasMap[k] = v; // 文件别名覆盖内置字典
    }
    state.filtered = null;
    state.uids = null;
    state.error = null;
    state.empty = false;
    state.page = 0;
    render();
  },

  // 表头显示别名（无别名回退原名）
  setAliasVisible(v) {
    state.aliasVisible = !!v;
    try { localStorage.setItem('tg_tbl_alias', v ? '1' : '0'); } catch (e) {}
    renderTable();
  },

  // 全部地块 uid（导航遍历的「无筛选=全部」口径）
  getAllUids() {
    return state.rows.map((r) => [r.si, r.pi]);
  },

  reset() {
    state.sources = []; state.rows = []; state.columns = [];
    state.filtered = null; state.uids = null; state.error = null;
    state.empty = false; state.page = 0; state.selUid = null;
    render();
  },

  // 当前筛选结果（缓存于最近一次 applyFromBuilder）
  getFilterUids() {
    return { uids: state.uids, error: state.error, empty: state.empty };
  },

  // 当前选中行 uid（地图页签打开时补高亮用）
  getSelectedUid() {
    return state.selUid;
  },

  // 清除选中（导入新数据时旧 uid 不应残留）
  clearSelection() {
    state.selUid = null;
    renderSelection(false);
  },

  // 属性字段名（查询器字段下拉用）
  getFieldNames() {
    return state.columns.slice(2);
  },

  // 字段去重值（查询器候选值弹层用）
  getDistinctValues(field, limit = 200) {
    const set = new Set();
    for (const r of state.rows) {
      const v = String(r.cells[field] ?? '').trim();
      if (v) set.add(v);
      if (set.size >= limit) break;
    }
    return [...set];
  },

  // 应用查询器结果：error 非空 → 拦截导出并提示；text 空 → 全量；否则解析求值
  applyFromBuilder(res) {
    if (res.error) {
      state.error = { pos: 0, message: res.error };
      state.filtered = null;
      state.uids = null;
      state.empty = false;
    } else if (!res.text) {
      state.error = null;
      state.filtered = null;
      state.uids = null;
      state.empty = false;
    } else {
      applyFilterSilently(res.text);
    }
    state.page = 0;
    syncMapFilter();
    render();
  },

  // uid 选中（src: 'table' | 'map'），双向联动各自跳过自身侧的滚动/飞行
  setUid(uid, src) {
    state.selUid = uid;
    renderSelection(src === 'table');
    if (src !== 'map' && window.MV) MV.highlight(uid);
  },
};

function applyFilterSilently(text) {
  state.error = null;
  state.empty = false;
  const t = (text || '').trim();
  if (!t) {
    state.filtered = null;
    state.uids = null;
  } else {
    try {
      const ast = FP.parse(t);
      const known = new Set(state.columns);
      const missing = [...FP.fieldNames(ast)].filter((f) => !known.has(f));
      if (missing.length) throw { pos: 0, message: `字段不存在: ${missing.join('、')}` };
      const rows = state.rows.filter((r) => FP.evalRow(ast, r.cells));
      state.filtered = rows;
      state.uids = rows.map((r) => [r.si, r.pi]);
      if (!rows.length) state.empty = true;
    } catch (e) {
      state.error = { pos: e?.pos ?? 0, message: e?.message || String(e) };
      state.filtered = null;
      state.uids = null;
    }
  }
  state.page = 0;
}

function syncMapFilter() {
  if (window.MV) MV.setFilter(state.error ? null : state.uids);
}

function render() {
  renderTable();
  renderPager();
  renderToolbarState();
}

function renderTable() {
  const tbl = $('tbl');
  if (!tbl) return;
  tbl.innerHTML = '';
  const rows = state.filtered || state.rows;
  if (!state.rows.length) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.className = 'tbl-empty';
    td.colSpan = 1;
    td.textContent = '导入 SHP / GDB 后显示属性表';
    tr.appendChild(td);
    tbl.appendChild(tr);
    return;
  }
  const thead = document.createElement('thead');
  const htr = document.createElement('tr');
  for (const c of state.columns) {
    const th = document.createElement('th');
    const alias = state.aliasMap[c];
    if (state.aliasVisible && alias && c !== '源' && c !== '序号') {
      th.textContent = alias;
      th.title = `${c}（${alias}）`;
    } else {
      th.textContent = c;
      th.title = c;
    }
    htr.appendChild(th);
  }
  thead.appendChild(htr);
  tbl.appendChild(thead);

  const start = state.page * PAGE_SIZE;
  const slice = rows.slice(start, start + PAGE_SIZE);
  const tbody = document.createElement('tbody');
  for (const r of slice) {
    const tr = document.createElement('tr');
    tr.dataset.uid = r.uid;
    if (r.uid === state.selUid) tr.classList.add('sel');
    for (const c of state.columns) {
      const td = document.createElement('td');
      td.textContent = r.cells[c] ?? '';
      td.title = r.cells[c] ?? '';
      td.addEventListener('click', () => showCellPop(td));
      tr.appendChild(td);
    }
    tr.addEventListener('click', () => TV.setUid(r.uid, 'table'));
    tbody.appendChild(tr);
  }
  tbl.appendChild(tbody);
}

function renderSelection(scrollToSel) {
  const tbl = $('tbl');
  if (!tbl) return;
  tbl.querySelectorAll('tr.sel').forEach((tr) => tr.classList.remove('sel'));
  if (!state.selUid) return;
  const tr = tbl.querySelector(`tr[data-uid="${CSS.escape(state.selUid)}"]`);
  if (tr) {
    tr.classList.add('sel');
    if (scrollToSel) tr.scrollIntoView({ block: 'nearest' });
  } else {
    // 不在当前页 → 跳到命中行所在页
    const rows = state.filtered || state.rows;
    const idx = rows.findIndex((r) => r.uid === state.selUid);
    if (idx >= 0) {
      state.page = Math.floor(idx / PAGE_SIZE);
      renderTable();
      const tr2 = $('tbl').querySelector(`tr[data-uid="${CSS.escape(state.selUid)}"]`);
      if (tr2 && scrollToSel) tr2.scrollIntoView({ block: 'nearest' });
    }
  }
}

function renderPager() {
  const rows = state.filtered || state.rows;
  const pages = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  if (state.page >= pages) state.page = pages - 1;
  const info = $('pgInfo');
  if (info) info.textContent = `${state.page + 1} / ${pages} 页`;
}

// 单元格内容被截断时，点击弹出完整值气泡（复用 qb-pop 视觉）
function showCellPop(td) {
  const old = $('cellPop');
  if (old) old.remove();
  if (td.scrollWidth <= td.clientWidth + 2) return; // 未截断不弹
  const pop = document.createElement('div');
  pop.className = 'qb-pop cell-pop';
  pop.id = 'cellPop';
  pop.textContent = td.textContent;
  document.body.appendChild(pop);
  const r = td.getBoundingClientRect();
  const w = Math.min(380, window.innerWidth - 16);
  pop.style.width = `${w}px`;
  pop.style.left = `${Math.max(8, Math.min(r.left, window.innerWidth - w - 8))}px`;
  pop.style.top = `${Math.min(r.bottom + 4, window.innerHeight - pop.offsetHeight - 8)}px`;
  pop.addEventListener('click', (e) => e.stopPropagation());
}

function renderToolbarState() {
  const count = $('fltCount');
  if (count) {
    if (state.error) { count.textContent = `条件有误：${state.error.message}`; count.style.color = '#e0533d'; }
    else if (state.empty) { count.textContent = '无命中'; count.style.color = '#e0533d'; }
    else if (state.filtered) {
      count.textContent = `命中 ${state.filtered.length} / 共 ${state.rows.length}`;
      count.style.color = '';
    } else { count.textContent = `共 ${state.rows.length} 条`; count.style.color = ''; }
  }
}
