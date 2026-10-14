// 把已签名的 NSIS 安装包发布到 Gitee Release（应用内更新的主下载源）
//
// 用法：
//   node scripts/publish-gitee.mjs --version 4.6.0 --file "00安装包相关/…_4.6.0_x64-setup.exe"
//
// 凭据（按优先级）：
//   --token <t>  /  环境变量 GITEE_TOKEN  /  已登录的 gitee CLI（gitee auth login 后自动读取）
//
// 行为：
//   1. 找 tag 对应的 Release，没有就建（vX.Y.Z，正文取 CHANGELOG 对应段落）
//   2. 以 multipart 上传附件，文件名固定 JisigG_<版本>_x64-setup.exe（ASCII，URL 干净）
//   3. 从稳定地址回读整个文件并比对 SHA-256，确认字节一致
//
// 失败即非 0 退出。

import { createReadStream, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = parseArgs(process.argv.slice(2));

const version = args.version;
if (!version || !/^\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(version)) fail('缺少或非法的 --version（应为 x.y.z）');
const file = args.file ? resolve(args.file) : '';
if (!file || !statSync(file).isFile()) fail(`缺少或非法的 --file：${file || '(空)'}`);

const repo = args.repo || process.env.GITEE_REPO || 'cdg_tools/polygon-txt';
const token = await resolveToken();
const tag = `v${version}`;
const assetName = `JisigG_${version}_x64-setup.exe`;
const api = `https://gitee.com/api/v5/repos/${repo}`;
const q = (extra) => extra ? `${extra}&` : '';

console.log(`→ 目标仓库 ${repo}`);
console.log(`  tag:   ${tag}`);
console.log(`  附件:  ${assetName}`);
console.log(`  本地:  ${file}（${statSync(file).size} 字节）`);

// ── 0) 确保仓库存在且有分支（公开；Gitee 免费发行只对公开仓库开放）──
const branch = await ensureRepo();

// ── 1) 找或建 Release ──
let rel = await findRelease(tag);
if (rel) {
  console.log(`✓ 复用已有 Release（id=${rel.id}）`);
} else {
  const body = await changelogSection(version);
  const created = await apiJson(`${api}/releases?${q()}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ access_token: token, tag_name: tag, name: tag, body, prerelease: false, target_commitish: branch }),
  });
  rel = created;
  console.log(`✓ 已创建 Release（id=${rel.id}）`);
}

// ── 2) 上传附件（multipart；Gitee 要求字段名 file）──
// 同名附件先删掉，保证重复执行不会在 Release 里堆出多份。
const dup = await findAttachment(rel.id, assetName);
if (dup) {
  await apiJson(`${api}/releases/${rel.id}/attach_files/${dup.id}?access_token=${encodeURIComponent(token)}`, { method: 'DELETE' });
  console.log(`  已删除同名旧附件（id=${dup.id}），避免堆叠`);
}
const buf = await readFile(file);
const fd = new FormData();
fd.append('file', new Blob([buf], { type: 'application/octet-stream' }), assetName);
const up = await fetch(`${api}/releases/${rel.id}/attach_files?access_token=${encodeURIComponent(token)}`, {
  method: 'POST',
  body: fd,
});
const upText = await up.text();
if (!up.ok) fail(`附件上传失败：HTTP ${up.status} ${upText.slice(0, 300)}`);
console.log('✓ 附件已上传');

// ── 3) 回读校验：稳定地址必须可达且字节一致 ──
const url = `https://gitee.com/${repo}/releases/download/${tag}/${assetName}`;
const local = sha256(buf);
console.log(`  校验地址 ${url}`);
const resp = await fetch(url, { redirect: 'follow' });
if (!resp.ok) fail(`稳定地址不可达：HTTP ${resp.status}`);
const remote = sha256(Buffer.from(await resp.arrayBuffer()));
if (remote !== local) fail(`回读不一致：remote=${remote} local=${local}`);
console.log(`✓ 回读校验一致（${buf.length} 字节 / ${local.slice(0, 16)}…）`);

console.log('');
console.log('下一步：');
console.log(`  用该地址作为 latest.json 的主 url：`);
console.log(`    node scripts/gen-latest-json.js --version ${version} --tag ${tag} \\`);
console.log(`      --nsis-dir <nsis 目录> --download-url <GitHub 直链> --gitee-url ${url}`);
console.log(`  下载地址：${url}`);

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--version') out.version = argv[++i];
    else if (argv[i] === '--file') out.file = argv[++i];
    else if (argv[i] === '--repo') out.repo = argv[++i];
    else if (argv[i] === '--token') out.token = argv[++i];
  }
  return out;
}

async function resolveToken() {
  if (args.token) return args.token.trim();
  if (process.env.GITEE_TOKEN) return process.env.GITEE_TOKEN.trim();
  try {
    const t = execFileSync('gitee', ['auth', 'token'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    if (t) return t;
  } catch { /* 未安装或未登录 */ }
  fail('拿不到 Gitee 令牌：先执行 gitee auth login，或设置环境变量 GITEE_TOKEN');
}

async function apiJson(url, init) {
  const resp = await fetch(url, init);
  const text = await resp.text();
  // 204/空响应（如删除附件）没有 body，按成功处理
  if (!resp.ok) fail(`Gitee API 失败：HTTP ${resp.status} ${text.slice(0, 300)}`);
  if (!text.trim()) return null;
  try {
    return JSON.parse(text);
  } catch {
    fail(`Gitee 返回非 JSON：HTTP ${resp.status} ${text.slice(0, 200)}`);
  }
}

// 仓库必须存在且至少有一个分支，否则 Gitee 拒绝建 Release
// （空仓库报 target_commitish is missing）。返回可用的分支名。
async function ensureRepo() {
  const [owner, name] = repo.split('/');
  const auth = `access_token=${encodeURIComponent(token)}`;
  let info = await fetch(`${api}?${auth}`);
  if (info.status === 404) {
    console.log(`  仓库 ${repo} 不存在，创建中…`);
    // Gitee 建仓库接口以 form 编码为准；JSON 里的 private 会被忽略（实测建成私有）
    const form = new URLSearchParams({
      access_token: token,
      name,
      description: '极思G界址点互转工具 — SHP/GDB ↔ TXT 双向转换（应用内更新主下载源）',
      private: 'false',
      has_issues: 'true',
      has_wiki: 'false',
      auto_init: 'true',
    });
    const created = await apiJson('https://gitee.com/api/v5/user/repos', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: form.toString(),
    });
    if (created.full_name && created.full_name !== `${owner}/${name}`) {
      console.log(`  ⚠ Gitee 返回的仓库是 ${created.full_name}，与请求的 ${repo} 不一致`);
    }
    console.log(`✓ 已创建公开仓库 ${repo}`);
    info = await fetch(`${api}?${auth}`);
  } else if (!info.ok) {
    fail(`仓库查询失败：HTTP ${info.status}`);
  }
  let detail = await info.json();
  if (detail.private) {
    console.log('  ⚠ 仓库是私有的（匿名下载会 403），改为公开…');
    const patch = new URLSearchParams({ access_token: token, name, private: 'false' });
    await apiJson(`${api}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: patch.toString(),
    });
    detail = await (await fetch(`${api}?${auth}`)).json();
    if (detail.private) fail('仓库仍为私有，无法用于公开下载');
    console.log('✓ 仓库已改为公开');
  }
  if (!detail.default_branch) {
    console.log('  仓库为空（尚无分支），先写一个 README 初始化…');
    await apiJson(`${api}/contents/README.md`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        access_token: token,
        content: Buffer.from('# polygon-txt\n\n极思G界址点互转工具 — SHP/GDB ↔ TXT 双向转换\n\n本仓库仅用于存放应用内更新的安装包附件。\n').toString('base64'),
        message: 'init: README',
      }),
    });
    detail = await (await fetch(`${api}?${auth}`)).json();
  }
  const branch = detail.default_branch || 'master';
  console.log(`✓ 仓库就绪（默认分支 ${branch}）`);
  return branch;
}

async function findAttachment(releaseId, name) {
  const list = await apiJson(`${api}/releases/${releaseId}/attach_files?access_token=${encodeURIComponent(token)}&per_page=100`);
  const arr = Array.isArray(list) ? list : (list ? [list] : []);
  return arr.find((a) => a.name === name) || null;
}

async function findRelease(tagName) {
  for (let page = 1; page <= 5; page++) {
    const list = await apiJson(`${api}/releases?access_token=${encodeURIComponent(token)}&per_page=100&page=${page}`);
    if (!Array.isArray(list) || !list.length) return null;
    const hit = list.find((r) => r.tag_name === tagName);
    if (hit) return hit;
    if (list.length < 100) return null;
  }
  return null;
}

async function changelogSection(ver) {
  try {
    const text = await readFile(join(root, 'CHANGELOG.md'), 'utf8');
    const re = new RegExp(`##\\s*\\[${ver.replace(/\./g, '\\.')}\\][^\\n]*\\n([\\s\\S]*?)(?=\\n##\\s*\\[|$)`);
    const m = text.match(re);
    if (m) return m[1].trim();
  } catch { /* 缺 CHANGELOG 不阻塞发布 */ }
  return `版本 ${ver} 更新`;
}

function sha256(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

function fail(msg) {
  console.error(`✗ ${msg}`);
  process.exit(1);
}
