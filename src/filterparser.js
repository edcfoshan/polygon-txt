// 类 SQL WHERE 迷你解析器（纯函数，无依赖）。
// 文法：
//   expr = or
//   or   = and { "OR" and }
//   and  = not { "AND" not }
//   not  = "NOT" not | pri
//   pri  = "(" expr ")" | cond
//   cond = field op val
//        | field "IS" [ "NOT" ] "NULL"
//        | field [ "NOT" ] "LIKE" str
//        | field [ "NOT" ] "IN" "(" val { "," val } ")"
// 字段名可双引号包裹；值 = 数字 | '串' | "串"；关键字不分大小写。
// LIKE 中 % 匹配任意串、_ 匹配单字符，忽略大小写。

const WORD_RE = /[^()\s,=<>!]+/;
const NUM_RE = /^-?\d+(\.\d+)?/;

function tokenize(src) {
  const toks = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (/\s/.test(ch)) { i++; continue; }
    if (ch === '(') { toks.push({ t: '(', pos: i }); i++; continue; }
    if (ch === ')') { toks.push({ t: ')', pos: i }); i++; continue; }
    if (ch === ',') { toks.push({ t: ',', pos: i }); i++; continue; }
    if (ch === "'" || ch === '"') {
      const quote = ch;
      let j = i + 1;
      while (j < src.length && src[j] !== quote) j++;
      if (j >= src.length) throw { pos: i, message: '字符串缺少收尾引号' };
      toks.push({ t: 'str', v: src.slice(i + 1, j), pos: i });
      i = j + 1;
      continue;
    }
    if (ch === '<' && src[i + 1] === '=') { toks.push({ t: 'op', v: '<=', pos: i }); i += 2; continue; }
    if (ch === '>' && src[i + 1] === '=') { toks.push({ t: 'op', v: '>=', pos: i }); i += 2; continue; }
    if (ch === '!' && src[i + 1] === '=') { toks.push({ t: 'op', v: '!=', pos: i }); i += 2; continue; }
    if (ch === '<' && src[i + 1] === '>') { toks.push({ t: 'op', v: '!=', pos: i }); i += 2; continue; }
    if (ch === '<') { toks.push({ t: 'op', v: '<', pos: i }); i++; continue; }
    if (ch === '>') { toks.push({ t: 'op', v: '>', pos: i }); i++; continue; }
    if (ch === '=') { toks.push({ t: 'op', v: '=', pos: i }); i++; continue; }
    const rest = src.slice(i);
    const m = rest.match(NUM_RE);
    if (m && m.index === 0) {
      toks.push({ t: 'num', v: parseFloat(m[0]), pos: i });
      i += m[0].length;
      continue;
    }
    const w = rest.match(WORD_RE);
    if (w && w.index === 0) {
      const upper = w[0].toUpperCase();
      const kw = ['AND', 'OR', 'NOT', 'LIKE', 'IN', 'IS', 'NULL'];
      toks.push({ t: kw.includes(upper) ? upper : 'word', v: w[0], pos: i });
      i += w[0].length;
      continue;
    }
    throw { pos: i, message: `无法识别的字符 "${ch}"` };
  }
  toks.push({ t: 'EOF', pos: src.length });
  return toks;
}

function parse(src) {
  if (!src || !src.trim()) throw { pos: 0, message: '筛选语句为空' };
  const toks = tokenize(src);
  let p = 0;
  const peek = () => toks[p];
  const next = () => toks[p++];
  const expect = (t, what) => {
    const tk = peek();
    if (tk.t !== t) throw { pos: tk.pos, message: `应为 ${what || t}` };
    return next();
  };

  function parseExpr() { return parseOr(); }
  function parseOr() {
    let a = parseAnd();
    while (peek().t === 'OR') { next(); a = { op: 'or', a, b: parseAnd() }; }
    return a;
  }
  function parseAnd() {
    let a = parseNot();
    while (peek().t === 'AND') { next(); a = { op: 'and', a, b: parseNot() }; }
    return a;
  }
  function parseNot() {
    if (peek().t === 'NOT') { next(); return { op: 'not', a: parseNot() }; }
    return parsePri();
  }
  function parsePri() {
    if (peek().t === '(') {
      next();
      const e = parseExpr();
      expect(')', ')');
      return e;
    }
    return parseCond();
  }
  function fieldToken() {
    const tk = peek();
    if (tk.t === 'word' || tk.t === 'str') return next();
    throw { pos: tk.pos, message: '应为字段名（可用双引号包裹）' };
  }
  function valueToken(what) {
    const tk = peek();
    if (tk.t === 'num' || tk.t === 'str') return next();
    throw { pos: tk.pos, message: `应为${what || '值'}` };
  }
  function parseCond() {
    const f = fieldToken();
    const tk = peek();
    if (tk.t === 'op') { next(); return { op: 'cmp', field: f.v, cmp: tk.v, value: valueToken().v }; }
    if (tk.t === 'IS') {
      next();
      let neg = false;
      if (peek().t === 'NOT') { next(); neg = true; }
      expect('NULL', 'NULL');
      return { op: 'null', field: f.v, neg };
    }
    let neg = false;
    if (tk.t === 'NOT') { next(); neg = true; }
    const tk2 = peek();
    if (tk2.t === 'LIKE') {
      next();
      return { op: 'like', field: f.v, pattern: String(valueToken('匹配串').v), neg };
    }
    if (tk2.t === 'IN') {
      next();
      expect('(', '(');
      const vals = [valueToken('列表值').v];
      while (peek().t === ',') { next(); vals.push(valueToken('列表值').v); }
      expect(')', ')');
      return { op: 'in', field: f.v, values: vals.map(String), neg };
    }
    throw { pos: tk2.pos, message: '应为比较运算符 / LIKE / IN / IS NULL' };
  }

  const ast = parseExpr();
  const end = peek();
  if (end.t !== 'EOF') throw { pos: end.pos, message: '存在多余的字符' };
  return ast;
}

// 收集 AST 中引用的字段名（用于字段存在性校验）
function fieldNames(ast, out = new Set()) {
  if (!ast) return out;
  if (ast.op === 'and' || ast.op === 'or') { fieldNames(ast.a, out); fieldNames(ast.b, out); }
  else if (ast.op === 'not') fieldNames(ast.a, out);
  else out.add(ast.field);
  return out;
}

function likeToRegex(pattern) {
  let re = '';
  for (const ch of pattern) {
    if (ch === '%') re += '[\\s\\S]*';
    else if (ch === '_') re += '[\\s\\S]';
    else re += ch.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }
  return new RegExp(`^${re}$`, 'i');
}

function toNum(v) {
  const s = String(v).trim();
  return NUM_RE.test(s) ? parseFloat(s) : null;
}

function cmpValues(a, cmp, b) {
  if (cmp === '=') return String(a) === String(b);
  if (cmp === '!=') return String(a) !== String(b);
  const na = toNum(a);
  const nb = toNum(b);
  if (na !== null && nb !== null) {
    if (cmp === '>') return na > nb;
    if (cmp === '<') return na < nb;
    if (cmp === '>=') return na >= nb;
    if (cmp === '<=') return na <= nb;
  }
  const sa = String(a);
  const sb = String(b);
  if (cmp === '>') return sa > sb;
  if (cmp === '<') return sa < sb;
  if (cmp === '>=') return sa >= sb;
  if (cmp === '<=') return sa <= sb;
  return false;
}

// row：普通对象 { 字段名: 值 }；缺失字段按空串参与运算
function evalRow(ast, row) {
  switch (ast.op) {
    case 'or': return evalRow(ast.a, row) || evalRow(ast.b, row);
    case 'and': return evalRow(ast.a, row) && evalRow(ast.b, row);
    case 'not': return !evalRow(ast.a, row);
    case 'cmp': return cmpValues(row[ast.field] ?? '', ast.cmp, ast.value);
    case 'null': {
      const v = row[ast.field];
      const isNull = v === undefined || v === null || String(v).trim() === '';
      return ast.neg ? !isNull : isNull;
    }
    case 'like': {
      const hit = likeToRegex(ast.pattern).test(String(row[ast.field] ?? ''));
      return ast.neg ? !hit : hit;
    }
    case 'in': {
      const hit = ast.values.includes(String(row[ast.field] ?? ''));
      return ast.neg ? !hit : hit;
    }
    default: return false;
  }
}

// 值/字段名的文本化（构建器生成 WHERE 用，与解析器约定一致）
export function quoteToken(v) {
  const s = String(v);
  if (/^-?\d+(\.\d+)?$/.test(s)) return s;
  return `'${s.replace(/'/g, '')}'`;
}

export function quoteField(name) {
  return /^[^()\s,=<>!]+$/.test(name) ? name : `"${name}"`;
}

export const FP = { parse, fieldNames, evalRow, quoteField, quoteToken };
