// 规范化 GitHub Release 资产名：slug 换成品牌 ASCII，中文原名放 label
//
// 背景：GitHub 服务端会吃掉资产 name（slug，进下载 URL 与 Content-Disposition）里的非 ASCII，
// 把 `极思G界址点互转工具_4.4.0_x64-setup.exe` 折叠成 `G._4.4.0_x64-setup.exe`。Release 列表显示的是
// label，但用户存盘的文件名和复制链接看的都是 slug。本脚本把两者分开安置：
//   name  → JisigG_4.4.0_x64-setup.exe        （可辨识的品牌 ASCII，落盘名不再是一串 G._）
//   label → 极思G界址点互转工具_4.4.0_x64-setup.exe（列表里显示中文）
// 只 PATCH 元数据，不重传文件，.sig 内容基于 exe 字节所以签名照旧有效。
//
// 用法：node scripts/normalize-release-assets.mjs --repo edcfoshan/polygon-txt --tag v4.4.0 [--prefix JisigG] [--dry-run]
// 需要 gh 已登录（CI 里用 GITHUB_TOKEN 即可）。
import { execFileSync } from 'node:child_process';
import { writeFileSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const args = parseArgs(process.argv.slice(2));
const repo = args.repo || process.env.GITHUB_REPOSITORY;
const tag = args.tag;
if (!repo || !tag) {
  console.error('用法：node scripts/normalize-release-assets.mjs --repo <owner/repo> --tag <vX.Y.Z> [--prefix JisigG] [--dry-run]');
  process.exit(1);
}
const prefix = args.prefix || 'JisigG';
const productName = run(['-p', "require('./src-tauri/tauri.conf.json').productName"], 'node').trim();

const release = JSON.parse(run(['api', `repos/${repo}/releases/tags/${tag}`]));
const changed = [];
const skipped = [];

for (const asset of release.assets || []) {
  const suffix = versionSuffix(asset) ;
  if (!suffix) {
    skipped.push(`${asset.name}（非版本产物，跳过）`);
    continue;
  }
  const wantName = `${prefix}_${suffix}`;
  const wantLabel = `${productName}_${suffix}`;
  const body = {};
  if (asset.name !== wantName) body.name = wantName;
  if ((asset.label || '') !== wantLabel) body.label = wantLabel;
  if (!Object.keys(body).length) {
    skipped.push(`${asset.name}（已符合）`);
    continue;
  }
  if (args['dry-run']) {
    console.log(`[dry-run] ${asset.name} → name=${wantName} label=${wantLabel}`);
    changed.push(wantName);
    continue;
  }
  patch(asset.id, body);
  console.log(`${asset.name} → ${wantName}  | label=${wantLabel}`);
  changed.push(wantName);
}

console.log(`\n${args['dry-run'] ? '待改' : '已改'} ${changed.length} 个，跳过 ${skipped.length} 个。`);
if (changed.length && !args['dry-run']) {
  console.log('注意：slug 变了 → 旧的下载链接失效。若本 Release 有 latest.json，必须随后用新的 browser_download_url 重新生成并上传。');
}

// 从 label（优先）或 name 里取出 `4.4.0_x64-setup.exe` 这段；非版本产物返回 null
function versionSuffix(asset) {
  for (const raw of [asset.label, asset.name]) {
    if (!raw) continue;
    const i = raw.indexOf('_');
    if (i < 0) continue;
    const suffix = raw.slice(i + 1);
    if (/^\d/.test(suffix)) return suffix;
  }
  return null;
}

function patch(assetId, body) {
  const file = join(tmpdir(), `asset-${assetId}.json`);
  writeFileSync(file, JSON.stringify(body)); // UTF-8 无 BOM：中文只走文件，不经 shell 参数
  try {
    run(['api', '-X', 'PATCH', `repos/${repo}/releases/assets/${assetId}`, '--input', file]);
  } finally {
    unlinkSync(file);
  }
}

function run(argv, cmd = 'gh') {
  return execFileSync(cmd, argv, { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
}

function parseArgs(argv) {
  const out = { 'dry-run': false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--repo') out.repo = argv[++i];
    else if (a === '--tag') out.tag = argv[++i];
    else if (a === '--prefix') out.prefix = argv[++i];
    else if (a === '--dry-run') out['dry-run'] = true;
  }
  return out;
}
