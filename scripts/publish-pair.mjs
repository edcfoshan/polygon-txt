// Runs only after all four build jobs have succeeded. Upload everything before publication.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { verifyReleaseSignature } from './verify-release-signature.mjs';

const repo = process.env.GITHUB_REPOSITORY;
const sha = process.env.GITHUB_SHA;
const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
const tag = `v${version}`, cliTag = `cli-v${version}`;
if (!repo || !sha || process.env.GITHUB_REF_NAME !== tag) throw new Error('Release tag/version mismatch');
const gh = (...args) => execFileSync('gh', [...args, '--repo', repo], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
const api = (endpoint) => JSON.parse(execFileSync('gh', ['api', `repos/${repo}/${endpoint}`], { encoding: 'utf8' }));
const labels = ['windows-x64', 'linux-x64', 'macos-x64', 'macos-arm64'];
const manifests = labels.map(label => {
  const m = JSON.parse(readFileSync(join('dist-release', label, 'manifest.json'), 'utf8'));
  if (m.version !== version || m.label !== label || !m.updater?.signature) throw new Error(`Invalid manifest: ${label}`);
  return m;
});
const desktopConfig = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
for (const manifest of manifests) {
  verifyReleaseSignature(join('dist-release', manifest.label, 'gui', manifest.updater.asset),
    manifest.updater.signature, desktopConfig.plugins.updater.pubkey);
}
// 附注 tag 触发时 GITHUB_SHA 是 tag 对象而非提交；统一解析成提交再比对，并作为 --target。
const targetSha = execFileSync('git', ['rev-parse', `${sha}^{commit}`], { encoding: 'utf8' }).trim();
if (execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() !== targetSha) throw new Error('Commit mismatch');
const notes = execFileSync('node', ['scripts/extract-changelog.js', version], { encoding: 'utf8' }).trim();
writeFileSync('desktop-notes.md', notes + `\n\n配套命令行：[CLI ${version}](https://github.com/${repo}/releases/tag/${cliTag})。\n`);
writeFileSync('cli-notes.md', `原生 Rust CLI ${version}，与桌面版来自同一提交 ${targetSha}。\n\n支持 SHP/GDB → TXT、TXT → SHP，配置校验、预检、JSON 结果和默认防覆盖。四个平台压缩包均附使用说明、示例配置及 SKILL.md。\n\n[使用说明](https://github.com/${repo}/blob/${tag}/docs/CLI.md) · [配套桌面版](https://github.com/${repo}/releases/tag/${tag})\n`);
for (const [releaseTag, title, body] of [[tag, `V${version}`, 'desktop-notes.md'], [cliTag, `CLI ${version}`, 'cli-notes.md']]) {
  let existing;
  try { existing = JSON.parse(gh('release', 'view', releaseTag, '--json', 'isDraft')); } catch { /* first publication */ }
  if (existing && !existing.isDraft) throw new Error(`${releaseTag} already published; refusing to replace assets`);
  if (!existing) gh('release', 'create', releaseTag, '--target', targetSha, '--title', title, '--notes-file', body, '--draft');
  for (const label of labels) {
    const dir = join('dist-release', label, releaseTag === tag ? 'gui' : 'cli');
    for (const file of readdirSync(dir)) {
      const display = releaseTag === tag && file.startsWith(`JisigG_${version}`)
        ? file.replace(`JisigG_${version}`, `${desktopConfig.productName}_${version}`) : file;
      gh('release', 'upload', releaseTag, `${join(dir, file)}#${display}`, '--clobber');
    }
  }
}
// Draft assets use temporary untagged URLs. Publish both releases first, then read stable tag URLs.
const windows = manifests.find(m => m.label === 'windows-x64');
gh('release', 'edit', tag, '--draft=false', '--latest');
gh('release', 'edit', cliTag, '--draft=false', '--latest=false');
const assets = api(`releases/tags/${tag}`).assets;
const setup = assets.find(a => a.name === windows.updater.asset);
if (!setup) throw new Error('Windows updater missing');
execFileSync('node', ['scripts/gen-latest-json.js', '--version', version, '--tag', tag,
  '--nsis-dir', 'dist-release/windows-x64/gui', '--download-url', setup.browser_download_url], { stdio: 'inherit' });
const latest = JSON.parse(readFileSync('latest.json', 'utf8'));
for (const manifest of manifests) {
  const asset = assets.find(a => a.name === manifest.updater.asset);
  if (!asset) throw new Error(`Updater missing: ${manifest.platform}`);
  latest.platforms[manifest.platform] = { signature: manifest.updater.signature, url: asset.browser_download_url };
}
writeFileSync('latest.json', JSON.stringify(latest, null, 2) + '\n');
gh('release', 'upload', tag, 'latest.json', '--clobber');
console.log(`Published ${tag} then ${cliTag} from ${targetSha}`);
