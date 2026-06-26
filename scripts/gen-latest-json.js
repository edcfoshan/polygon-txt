// 生成 Tauri Updater 的 latest.json
//
// 用法（在仓库根目录执行）：
//   node scripts/gen-latest-json.js --notes "本次更新内容..." [--tag v1.2.1] [--no-mirror]
//
// 行为：
//   1. 从 package.json 读版本号（可用 --version 覆盖）
//   2. 扫描 src-tauri/target/release/bundle/nsis/ 找 *.sig，读取签名内容
//   3. 推断同名 .exe（Tauri NSIS 产物，文件名可能含中文）
//   4. 组装 url：默认走 mirror.ghproxy.com 镜像加速（--no-mirror 关闭）
//   5. 写入仓库根目录 latest.json（jsDelivr 源会从这里取）
//
// 发布时把生成的 latest.json 和 NSIS exe 一起上传到 GitHub Release。
import { readFileSync, writeFileSync, readdirSync, existsSync } from 'node:fs';
import { resolve, dirname, basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');

const args = parseArgs(process.argv.slice(2));
const version = args.version || JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const tag = args.tag || `v${version}`;
const repo = 'edcfoshan/polygon-txt';

const nsisDir = join(root, 'src-tauri/target/release/bundle/nsis');
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
if (sigFiles.length > 1) {
  console.warn(`⚠ 发现多个 .sig 文件，使用第一个：${sigFiles[0]}`);
}

const sigName = sigFiles[0];
// 签名文件 .sig 与同名 exe（Tauri 默认产物，文件名可能含中文）配对
const sourceExeName = sigName.replace(/\.sig$/, '');
if (!existsSync(join(nsisDir, sourceExeName))) {
  console.error(`✗ 签名对应的 exe 不存在：${sourceExeName}`);
  process.exit(1);
}

// 发布时按 release skill 规范重命名为英文前缀 + 简短版本（如 polygon-txt_1.2_x64-setup.exe）
// 签名内容基于 exe 字节，与文件名无关，重命名后 .sig 仍有效
const shortVer = version.split('.').slice(0, 2).join('.');
const publishedExeName = `polygon-txt_${shortVer}_x64-setup.exe`;

const signature = readFileSync(join(nsisDir, sigName), 'utf8').trim();
const notes = args.notes || readNotesFromChangelog(version) || `版本 ${version} 更新`;

// ghproxy 镜像前缀：国内访问 GitHub Releases 加速。ghproxy 服务偶有抖动，
// 端点配置的 jsDelivr + GitHub 多重 endpoint 仅用于 latest.json 本身的拉取加速，
// 这里包体的加速由 url 前缀承担；下载失败时前端兜底引导用户浏览器手动下载。
const mirror = args.mirror === false ? '' : 'https://mirror.ghproxy.com/';
const downloadUrl = `${mirror}https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(publishedExeName)}`;

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
console.log(`  1. 把 nsis/${sourceExeName} 重命名为 ${publishedExeName} 后上传到 GitHub Release ${tag}`);
console.log(`  2. 把 latest.json 上传到同一 Release`);
console.log(`  3. git add latest.json && git commit && git push（jsDelivr 源从仓库 master 取此文件）`);

function parseArgs(argv) {
  const out = { mirror: true };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--version') out.version = argv[++i];
    else if (a === '--tag') out.tag = argv[++i];
    else if (a === '--notes') out.notes = argv[++i];
    else if (a === '--no-mirror') out.mirror = false;
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
