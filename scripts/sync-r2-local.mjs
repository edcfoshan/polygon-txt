// 本地同步安装包到 Cloudflare R2（使用 `npx wrangler login` 的 OAuth 凭据，免长期 API Key）
//
// 用法（在仓库根目录执行）：
//   node scripts/sync-r2-local.mjs --version 4.6.0 \
//     --file "00安装包相关/极思G界址点互转工具_4.6.0_x64-setup.exe" [--prune]
//
// 前置：`npx wrangler login` 授权过一次；wrangler 的 OAuth token 过期后重跑 login 即可。
//
// 为什么不用 S3 API：那需要在 Cloudflare 控制台建长期 API Token 并写进 GitHub Secrets。
// 本脚本改用 wrangler 现成的 OAuth 会话 + REST API，少一个长期凭据和轮换负担。
//
// 行为：
//   1. 上传 versions/<version>/setup.exe（REST PUT，二进制流式上传）
//   2. 读回对象列表核对 size，并复算本地 SHA-256 写入 latest.txt
//   3. --prune 时只保留最新两版（按版本号排序），其余删除
//
// 失败即非 0 退出。

import { createReadStream, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdtemp } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { tmpdir, homedir } from 'node:os';

const ACCOUNT_ID = process.env.R2_ACCOUNT_ID || '2462886b85004245476e1c63bcd092c8';
const BUCKET = process.env.R2_BUCKET || 'polygon-txt-updates';
const args = parseArgs(process.argv.slice(2));
const version = args.version;
const file = args.file ? resolve(args.file) : '';

if (!version || !/^\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(version)) fail('缺少或非法的 --version（应为 x.y.z）');
if (!file || !statSync(file).isFile()) fail(`缺少或非法的 --file：${file || '(空)'}`);

const token = await readWranglerToken();
const api = `https://api.cloudflare.com/client/v4/accounts/${ACCOUNT_ID}/r2/buckets/${BUCKET}/objects`;
const auth = { Authorization: `Bearer ${token}` };

const key = `versions/${version}/setup.exe`;
const localSize = statSync(file).size;
const localSha = await sha256File(file);

console.log(`→ 上传 ${file}`);
console.log(`  key:    ${key}`);
console.log(`  size:   ${localSize} bytes`);
console.log(`  sha256: ${localSha}`);

await putObject(key, file);

const after = await findObject(key);
if (!after) fail(`上传后未在 R2 找到 ${key}`);
if (after.size !== localSize) fail(`R2 大小不一致：remote=${after.size} local=${localSize}`);
console.log(`✓ 上传核对通过：remote size=${after.size}`);

const tmp = await mkdtemp(join(tmpdir(), 'r2-latest-'));
const latestPath = join(tmp, 'latest.txt');
await writeFile(latestPath, `${version}\n${localSha}\n`, 'utf8');
await putObject('latest.txt', latestPath);
console.log('✓ 写入 latest.txt');

if (args.prune) {
  const objects = await listObjects('versions/');
  const versions = [...new Set(objects.map((o) => /^versions\/([^/]+)\//.exec(o.key)?.[1]).filter(Boolean))]
    .sort((a, b) => compareVersion(b, a));
  const stale = versions.slice(2);
  for (const v of stale) {
    await deleteObject(`versions/${v}/setup.exe`);
    console.log(`✓ 已删除旧版本：versions/${v}/setup.exe`);
  }
  if (!stale.length) console.log(`✓ 无需清理（当前 ${versions.length} 版）`);
}

console.log('\n下一步：');
console.log(`  curl -sI -H "X-Client-Key: <密钥>" "https://<worker 域名>/download/${version}"  → 期望 200`);

function parseArgs(argv) {
  const out = { prune: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--version') out.version = argv[++i];
    else if (argv[i] === '--file') out.file = argv[++i];
    else if (argv[i] === '--prune') out.prune = true;
  }
  return out;
}

async function readWranglerToken() {
  const cfgPath = process.env.WRANGLER_CONFIG_PATH
    || join(process.env.APPDATA || join(homedir(), 'AppData', 'Roaming'), 'xdg.config', '.wrangler', 'config', 'default.toml');
  let raw;
  try {
    raw = await readFile(cfgPath, 'utf8');
  } catch {
    fail(`读不到 wrangler 凭据：${cfgPath}\n  请先执行 npx wrangler login`);
  }
  const token = /^\s*oauth_token\s*=\s*"?([^"\n]+)"?/m.exec(raw)?.[1];
  if (!token) fail('wrangler 凭据里没有 oauth_token，请重跑 npx wrangler login');
  const exp = /^\s*expiration_time\s*=\s*"?([^"\n]+)"?/m.exec(raw)?.[1];
  if (exp) {
    const t = Date.parse(exp);
    if (Number.isFinite(t) && t < Date.now()) {
      fail(`wrangler 凭据已于 ${new Date(t).toLocaleString()} 过期，请重跑 npx wrangler login`);
    }
  }
  return token;
}

async function putObject(objectKey, filePath) {
  const resp = await fetch(`${api}/${objectKey}`, {
    method: 'PUT',
    headers: { ...auth, 'Content-Type': 'application/octet-stream' },
    body: createReadStream(filePath),
    duplex: 'half',
  });
  if (!resp.ok) fail(`上传 ${objectKey} 失败：HTTP ${resp.status} ${await resp.text()}`);
}

// 注意：GET /objects/<key> 返回的是对象字节，不是元数据；查元数据必须走列举接口。
async function findObject(objectKey) {
  const objects = await listObjects(objectKey);
  return objects.find((o) => o.key === objectKey) || null;
}

async function listObjects(prefix) {
  const resp = await fetch(`${api}?prefix=${encodeURIComponent(prefix)}&per_page=1000`, { headers: auth });
  if (!resp.ok) fail(`列举对象失败：HTTP ${resp.status} ${await resp.text()}`);
  const body = await resp.json();
  const result = body.result;
  if (!result) return [];
  return Array.isArray(result) ? result : [result];
}

async function deleteObject(objectKey) {
  const resp = await fetch(`${api}/${objectKey}`, { method: 'DELETE', headers: auth });
  if (!resp.ok) fail(`删除 ${objectKey} 失败：HTTP ${resp.status} ${await resp.text()}`);
}

async function sha256File(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}

function compareVersion(a, b) {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0);
  return 0;
}

function fail(msg) {
  console.error(`✗ ${msg}`);
  process.exit(1);
}