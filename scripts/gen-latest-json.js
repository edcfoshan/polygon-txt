// 生成 Tauri Updater 的 latest.json
//
// 用法（在仓库根目录执行）：
//   node scripts/gen-latest-json.js --version X.Y.Z --tag vX.Y.Z \
//     --nsis-dir src-tauri/target/<triple>/release/bundle/nsis \
//     --download-url <Release 上 setup 的 browser_download_url>
//
// 行为：
//   1. 从 package.json 读版本号（可用 --version 覆盖）
//   2. 扫描 nsis 目录找最新的 *.sig，读取签名内容
//   3. 推断同名 .exe（Tauri NSIS 产物，文件名含中文）
//   4. 组装 url：必须用 --download-url 传真实资产链接；不传则按 productName 拼（含中文，上传后 404）
//   5. 写入仓库根目录 latest.json（jsDelivr 源会从这里取）
//
// 发布时把生成的 latest.json 和 NSIS exe 一起上传到 GitHub Release。
import { readFileSync, writeFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import { resolve, dirname, basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');

const args = parseArgs(process.argv.slice(2));
const version = args.version || JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const tag = args.tag || `v${version}`;
const repo = 'edcfoshan/polygon-txt';

// 默认本地路径；CI 交叉构建时产物在 target/<triple>/release/...，用 --nsis-dir 指定
const nsisDir = args['nsis-dir'] ? resolve(root, args['nsis-dir']) : join(root, 'src-tauri/target/release/bundle/nsis');
if (!existsSync(nsisDir)) {
  console.error(`✗ 找不到 NSIS 产物目录：${nsisDir}`);
  console.error('  请先执行：npm run tauri build（并确保已设置 TAURI_SIGNING_PRIVATE_KEY）');
  process.exit(1);
}

const sigFiles = readdirSync(nsisDir).filter((f) => f.endsWith('.sig'));
if (sigFiles.length === 0) {
  console.error(`✗ 在 ${nsisDir} 未找到 .sig 签名文件。`);
  console.error('  检查环境变量 TAURI_SIGNING_PRIVATE_KEY / TAURI_SIGNING_PRIVATE_KEY_PASSWORD 是否已设置。');
  process.exit(1);
}
// 构建目录可能残留旧版本 .sig；updater 必须使用最新一次构建生成的签名。
const [newestSig] = sigFiles
  .map((name) => ({ name, mtimeMs: statSync(join(nsisDir, name)).mtimeMs }))
  .sort((a, b) => b.mtimeMs - a.mtimeMs);
const sigName = newestSig.name;
if (sigFiles.length > 1) {
  console.warn(`⚠ 发现多个 .sig 文件，使用最新构建：${sigName}`);
}
// 签名文件 .sig 与同名 exe（Tauri 默认产物，文件名可能含中文）配对
const sourceExeName = sigName.replace(/\.sig$/, '');
if (!existsSync(join(nsisDir, sourceExeName))) {
  console.error(`✗ 签名对应的 exe 不存在：${sourceExeName}`);
  process.exit(1);
}

// 资产有两层名字：slug（API 的 name，进 URL）会被 GitHub 静默剥掉非 ASCII
// （极思G界址点互转工具_X.exe → G._X.exe），label 才是 Release 列表里显示的名字。
// 所以 updater 的 url 必须是真实 slug —— CI 用 --download-url 传 API 的 browser_download_url。
// 签名基于 exe 字节，与文件名无关，改名不影响 .sig。
const productName = JSON.parse(readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8')).productName;
const publishedExeName = `${productName}_${version}_x64-setup.exe`;

const signature = readFileSync(join(nsisDir, sigName), 'utf8').trim();
const notes = args.notes || readNotesFromChangelog(version) || `版本 ${version} 更新`;

// 默认用 GitHub releases 直连（国内约 15s 下完 5MB，可接受）。公益镜像（ghproxy 等）
// 长期不稳定——1.3.0 发版时 mirror.ghproxy.com 已失效，多镜像测试均无加速效果。
// 如需镜像用 --mirror 显式开启。下载失败时前端兜底引导用户跳转百度云手动下载。
const mirror = args.mirror === true ? 'https://mirror.ghproxy.com/' : '';
// url 优先用 --download-url（CI 传入 Release API 的真实 browser_download_url，即 GitHub
// 清洗过非 ASCII 之后的 slug）。不传则按 productName 拼——含中文时该链接上传后会 404。
const explicitUrl = args['download-url'];
const downloadUrl = `${mirror}${
  explicitUrl ||
  `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(publishedExeName)}`
}`;
if (!explicitUrl && /[^\x20-\x7E]/.test(publishedExeName)) {
  console.warn(`⚠ url 里含非 ASCII 资产名，GitHub 会把 slug 清洗成 ASCII（如 G._${version}_x64-setup.exe），`);
  console.warn(`  这个 url 上传后必然 404。正式发布请在 CI 里传 --download-url <Release API 的 browser_download_url>。`);
}

const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: downloadUrl,
    },
  },
};

const outPath = join(root, 'latest.json');
writeFileSync(outPath, JSON.stringify(latest, null, 2));
console.log(`✓ 已生成 ${outPath}`);
console.log(`  version: ${version}`);
console.log(`  url:     ${downloadUrl}`);
console.log(`  tag:     ${tag}`);
console.log('');
console.log('下一步：');
console.log(`  1. 把 nsis/${sourceExeName} 与 ${sigName} 上传到 GitHub Release ${tag}（GitHub 会把 slug 里的中文剥成 ASCII）`);
console.log(`  2. 取真实链接：gh api repos/${repo}/releases/tags/${tag} -q '.assets[].browser_download_url'`);
console.log(`  3. 用该链接重跑本脚本加 --download-url <url>，再把生成的 latest.json 上传到同一 Release`);
console.log(`  4. git add latest.json && git commit && git push（jsDelivr 源从仓库 master 取此文件）+ purge CDN`);

function parseArgs(argv) {
  const out = { mirror: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--version') out.version = argv[++i];
    else if (a === '--tag') out.tag = argv[++i];
    else if (a === '--notes') out.notes = argv[++i];
    else if (a === '--nsis-dir') out['nsis-dir'] = argv[++i];
    else if (a === '--download-url') out['download-url'] = argv[++i];
    else if (a === '--mirror') out.mirror = true;
  }
  return out;
}

// 从 CHANGELOG.md 提取当前版本对应段落作为 notes 兜底
function readNotesFromChangelog(ver) {
  const cl = join(root, 'CHANGELOG.md');
  if (!existsSync(cl)) return null;
  const text = readFileSync(cl, 'utf8');
  const re = new RegExp(`##\\s*\\[${ver.replace(/\./g, '\\.')}\\][^\\n]*\\n([\\s\\S]*?)(?=\\n##\\s*\\[|$)`);
  const m = text.match(re);
  return m ? m[1].trim() : null;
}
