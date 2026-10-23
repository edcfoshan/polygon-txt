import { readFileSync } from 'node:fs';
const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
if (tauri !== version || cargo !== version || process.env.GITHUB_REF_NAME !== `v${version}`) {
  throw new Error(`Version mismatch: package=${version}, desktop=${tauri}, Rust=${cargo}, tag=${process.env.GITHUB_REF_NAME}`);
}

const changelog = readFileSync('CHANGELOG.md', 'utf8');
const escapedVersion = version.replace(/\./g, '\\.');
const heading = new RegExp(`^##\\s*\\[${escapedVersion}\\][^\\n]*\\n`, 'm');
const headingMatch = heading.exec(changelog);
const section = headingMatch
  ? changelog.slice(headingMatch.index + headingMatch[0].length).split(/\r?\n##\s*\[/)[0]
  : '';
if (!section) throw new Error(`CHANGELOG has no section for ${version}`);

const sectionNames = [...section.matchAll(/^###\s+(.+)$/gm)].map((m) => m[1].trim());
if (!sectionNames.length || sectionNames.some((name) => !['新增', '修复', '优化', '提示'].includes(name))) {
  throw new Error(`${version} notes must use 新增 / 修复 / 优化 / 提示 sections`);
}

const bullets = [...section.matchAll(/^-\s+(.+)$/gm)].map((m) => m[1].replaceAll('**', '').trim());
if (bullets.length < 1 || bullets.length > 6) throw new Error(`${version} notes must contain 1-6 bullets`);
for (const bullet of bullets) {
  if ([...bullet].length > 40) throw new Error(`${version} note exceeds 40 characters: ${bullet}`);
}
