// 结构化筛选条件构建器（vanilla，无依赖）：
// 条件按行拆分，行间「且/或」连接（严格按行序左折叠，生成文本自动加括号），
// IN 多选复选弹层、LIKE/比较单值 + 候选值点选，IS NULL 无值控件。
// 生成标准 WHERE 文本，求值仍走 filterparser（预览/导出管线零改动）。

import { quoteField, quoteToken } from './filterparser.js';

const OPS = ['=', '!=', '>', '<', '>=', '<=', 'IN', 'LIKE', 'IS NULL'];
const NEED_VALUE = new Set(['=', '!=', '>', '<', '>=', '<=', 'IN', 'LIKE']);

const state = {
  rows: [],          // {glue:'and'|'or', field:'', op:'=', value:'', values:[]}
  fields: [],        // 可选字段名（来自属性表列）
  errors: new Map(), // rowIndex → message（最近一次校验）
  onChange: null,
  popFor: null,      // 当前弹层归属 {ri, mode:'single'|'multi'}
};

let bound = false;
let popEl = null;

function $(id) { return document.getElementById(id); }

function esc(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function validate(rows, fields) {
  const known = new Set(fields);
  const errors = new Map();
  rows.forEach((r, i) => {
    if (!r.field) return; // 未选字段的行整体忽略
    if (!known.has(r.field)) { errors.set(i, `字段不存在：${r.field}`); return; }
    if (NEED_VALUE.has(r.op)) {
      if (r.op === 'IN') {
        if (!r.values || !r.values.length) errors.set(i, 'IN 未选择值');
      } else if (!String(r.value ?? '').trim()) {
        errors.set(i, '缺少值');
      }
    }
  });
  return errors;
}

// 生成 WHERE 文本：行序左折叠 + 逐层括号（((a AND b) OR c)），与行上连接器所见一致
function buildWhere(rows, fields) {
  state.errors = validate(rows, fields);
  if (state.errors.size) {
    const first = state.errors.entries().next().value;
    return { text: '', error: `第 ${first[0] + 1} 行：${first[1]}` };
  }
  const parts = [];
  for (const r of rows) {
    if (!r.field) continue;
    const f = quoteField(r.field);
    if (r.op === 'IS NULL') parts.push({ glue: r.glue, text: `${f} IS NULL` });
    else if (r.op === 'IN') parts.push({ glue: r.glue, text: `${f} IN (${r.values.map(quoteToken).join(', ')})` });
    else if (r.op === 'LIKE') parts.push({ glue: r.glue, text: `${f} LIKE ${quoteToken(r.value)}` });
    else parts.push({ glue: r.glue, text: `${f} ${r.op} ${quoteToken(r.value)}` });
  }
  if (!parts.length) return { text: '', error: null };
  let acc = parts[0].text;
  for (let i = 1; i < parts.length; i++) {
    acc = `(${acc} ${parts[i].glue.toUpperCase()} ${parts[i].text})`;
  }
  return { text: acc, error: null };
}

function distinctValues(field, limit) {
  const tv = window.TV;
  if (tv && tv.getDistinctValues) return tv.getDistinctValues(field, limit);
  return [];
}

// ─── 候选值弹层（body 级 fixed，防容器 overflow 裁剪，同 advSuggest 先例） ───

function closePop() {
  if (popEl) { popEl.remove(); popEl = null; }
  state.popFor = null;
}

function openPop(ri, anchor, mode) {
  closePop();
  const row = state.rows[ri];
  if (!row || !row.field) return;
  state.popFor = { ri, mode };
  popEl = document.createElement('div');
  popEl.className = 'qb-pop';
  const search = document.createElement('input');
  search.className = 'qb-pop-search';
  search.placeholder = '搜索值…';
  popEl.appendChild(search);

  const list = document.createElement('div');
  list.className = 'qb-pop-list';
  popEl.appendChild(list);

  const all = distinctValues(row.field, 200);
  const selected = new Set(mode === 'multi' ? (row.values || []) : []);
  const btnOk = document.createElement('button');
  btnOk.type = 'button';
  btnOk.className = 'qb-pop-ok';

  const renderList = () => {
    const kw = search.value.trim();
    list.innerHTML = '';
    const hits = all.filter((v) => !kw || v.includes(kw));
    if (!hits.length) {
      const empty = document.createElement('div');
      empty.className = 'qb-pop-empty';
      empty.textContent = '无非空值';
      list.appendChild(empty);
    }
    for (const v of hits.slice(0, 200)) {
      const item = document.createElement(mode === 'multi' ? 'label' : 'button');
      item.className = 'qb-pop-item';
      if (mode === 'multi') {
        item.innerHTML = `<input type="checkbox" value="${esc(v)}"${selected.has(v) ? ' checked' : ''}><span>${esc(v)}</span>`;
        item.querySelector('input').addEventListener('change', (e) => {
          if (e.target.checked) selected.add(v); else selected.delete(v);
          btnOk.textContent = `确定（已选 ${selected.size}）`;
        });
      } else {
        item.type = 'button';
        item.textContent = v;
        item.title = v;
        item.addEventListener('click', () => {
          row.value = v;
          closePop();
          renderRows();
          notify();
        });
      }
      list.appendChild(item);
    }
  };
  search.addEventListener('input', renderList);
  renderList();

  if (mode === 'multi') {
    btnOk.textContent = `确定（已选 ${selected.size}）`;
    btnOk.addEventListener('click', () => {
      row.values = all.filter((v) => selected.has(v));
      closePop();
      renderRows();
      notify();
    });
    popEl.appendChild(btnOk);
  }

  document.body.appendChild(popEl);
  const r = anchor.getBoundingClientRect();
  const pw = 260;
  popEl.style.width = `${pw}px`;
  const left = Math.min(r.left, window.innerWidth - pw - 8);
  let top = r.bottom + 4;
  popEl.style.left = `${Math.max(8, left)}px`;
  popEl.style.top = `${top}px`;
  // 展开后若超出视口底部则改为向上弹
  requestAnimationFrame(() => {
    const h = popEl.offsetHeight;
    if (top + h > window.innerHeight - 8) popEl.style.top = `${Math.max(8, r.top - h - 4)}px`;
  });
  setTimeout(() => search.focus(), 0);
  popEl.addEventListener('click', (e) => e.stopPropagation());
}

// ─── 行渲染 ───

function renderRows() {
  const box = $('qbRows');
  if (!box) return;
  box.innerHTML = '';
  if (!state.rows.length) {
    const hint = document.createElement('div');
    hint.className = 'qb-empty';
    hint.textContent = '尚未添加筛选条件（不筛选 = 导出全部）';
    box.appendChild(hint);
    return;
  }
  state.rows.forEach((r, i) => {
    const rowEl = document.createElement('div');
    rowEl.className = 'qb-row' + (state.errors.has(i) ? ' err' : '');
    rowEl.dataset.i = i;
    if (state.errors.has(i)) rowEl.title = state.errors.get(i);

    if (i > 0) {
      const glue = document.createElement('select');
      glue.className = 'qb-glue';
      glue.innerHTML = '<option value="and">且</option><option value="or">或</option>';
      glue.value = r.glue || 'and';
      glue.dataset.act = 'glue';
      rowEl.appendChild(glue);
    }

    const f = document.createElement('select');
    f.className = 'qb-f';
    f.innerHTML = '<option value="">字段…</option>'
      + state.fields.map((n) => `<option value="${esc(n)}"${n === r.field ? ' selected' : ''}>${esc(n)}</option>`).join('');
    f.dataset.act = 'field';
    rowEl.appendChild(f);

    const op = document.createElement('select');
    op.className = 'qb-op';
    op.innerHTML = OPS.map((o) => `<option value="${o}"${o === r.op ? ' selected' : ''}>${o === 'IS NULL' ? '为空' : o}</option>`).join('');
    op.dataset.act = 'op';
    rowEl.appendChild(op);

    const v = document.createElement('span');
    v.className = 'qb-v';
    v.dataset.act = 'value';
    if (r.op === 'IS NULL') {
      const none = document.createElement('span');
      none.className = 'qb-none';
      none.textContent = '—';
      v.appendChild(none);
    } else if (r.op === 'IN') {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'qb-vbtn';
      const n = (r.values || []).length;
      btn.textContent = n ? `已选 ${n} 项` : '选择值…';
      btn.dataset.act = 'pop-multi';
      v.appendChild(btn);
    } else {
      const inp = document.createElement('input');
      inp.type = 'text';
      inp.className = 'qb-vinp';
      inp.value = r.value ?? '';
      inp.placeholder = r.op === 'LIKE' ? '%通配' : '值';
      inp.dataset.act = 'vinput';
      v.appendChild(inp);
      const pick = document.createElement('button');
      pick.type = 'button';
      pick.className = 'qb-vpick';
      pick.textContent = '▾';
      pick.title = '从数据中选值';
      pick.dataset.act = 'pop-single';
      v.appendChild(pick);
    }
    rowEl.appendChild(v);

    const del = document.createElement('button');
    del.type = 'button';
    del.className = 'qb-del';
    del.textContent = '✕';
    del.title = '删除此条件';
    del.dataset.act = 'del';
    rowEl.appendChild(del);

    box.appendChild(rowEl);
  });
}

function notify() {
  if (typeof state.onChange === 'function') state.onChange();
}

function bind() {
  if (bound) return;
  bound = true;
  $('btnQbAdd')?.addEventListener('click', () => {
    const last = state.rows[state.rows.length - 1];
    state.rows.push({
      glue: 'and',
      field: '',
      op: '=',
      value: '',
      values: [],
      // 新行默认沿用上一行的字段，减少重复选择
      ...(last && last.field ? { field: last.field } : {}),
    });
    renderRows();
    notify();
  });
  $('btnQbClear')?.addEventListener('click', () => {
    state.rows = [];
    state.errors.clear();
    renderRows();
    notify();
  });
  $('qbRows')?.addEventListener('change', (e) => {
    const act = e.target.dataset.act;
    if (!act) return;
    const ri = parseInt(e.target.closest('.qb-row')?.dataset.i, 10);
    const row = state.rows[ri];
    if (!row) return;
    if (act === 'glue') { row.glue = e.target.value; notify(); return; }
    if (act === 'field') { row.field = e.target.value; }
    else if (act === 'op') {
      row.op = e.target.value === '为空' ? 'IS NULL' : e.target.value;
    } else { return; }
    state.errors = validate(state.rows, state.fields);
    renderRows();
    notify();
  });
  $('qbRows')?.addEventListener('input', (e) => {
    if (e.target.dataset.act !== 'vinput') return;
    const ri = parseInt(e.target.closest('.qb-row')?.dataset.i, 10);
    const row = state.rows[ri];
    if (!row) return;
    row.value = e.target.value;
    state.errors = validate(state.rows, state.fields);
    e.target.closest('.qb-row')?.classList.toggle('err', state.errors.has(ri));
    notify();
  });
  $('qbRows')?.addEventListener('click', (e) => {
    const t = e.target.closest('[data-act]');
    if (!t) return;
    const act = t.dataset.act;
    if (act !== 'pop-single' && act !== 'pop-multi' && act !== 'del') return;
    const ri = parseInt(t.closest('.qb-row')?.dataset.i, 10);
    if (act === 'del') {
      state.rows.splice(ri, 1);
      // 首行连接器无意义，移除
      if (state.rows[0]) state.rows[0].glue = 'and';
      state.errors = validate(state.rows, state.fields);
      renderRows();
      notify();
    } else {
      e.stopPropagation();
      openPop(ri, t, act === 'pop-multi' ? 'multi' : 'single');
    }
  });
  document.addEventListener('click', (e) => {
    if (popEl && !popEl.contains(e.target)) closePop();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && popEl) closePop();
  });
}

export const QB = {
  init(onChange) {
    state.onChange = onChange;
    bind();
  },

  // 导入数据后刷新字段下拉（保留已选字段，选中的字段被列删除后校验会标红）
  setFields(fields) {
    state.fields = fields || [];
    state.errors = validate(state.rows, state.fields);
    renderRows();
  },

  // 生成 WHERE 文本；error 非 null 时 text 为空
  getWhere() {
    return buildWhere(state.rows, state.fields);
  },

  getRows() {
    return JSON.parse(JSON.stringify(state.rows));
  },

  loadRows(rows) {
    if (!Array.isArray(rows)) return;
    state.rows = rows
      .filter((r) => r && typeof r === 'object')
      .map((r) => ({
        glue: r.glue === 'or' ? 'or' : 'and',
        field: String(r.field || ''),
        op: OPS.includes(r.op) ? r.op : '=',
        value: String(r.value ?? ''),
        values: Array.isArray(r.values) ? r.values.map(String) : [],
      }));
    state.errors = validate(state.rows, state.fields);
    renderRows();
  },

  clearRows() {
    state.rows = [];
    state.errors.clear();
    renderRows();
  },
};

// 持久化由调用方（main.js）通过 getRows/loadRows + localStorage 完成
