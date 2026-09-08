// 从 CHANGELOG.md 提取指定版本的更新说明段落
// 用法：node scripts/extract-changelog.js 3.3.0
// 输出到 stdout（供 gh release edit --notes-file 使用）
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const version = process.argv[2];
if (!version) {
  console.error('用法：node scripts/extract-changelog.js <X.Y.Z>');
  process.exit(1);
}

const cl = join(root, 'CHANGELOG.md');
let body = '';
if (existsSync(cl)) {
  const text = readFileSync(cl, 'utf8');
  const re = new RegExp(`##\\s*\\[${version.replace(/\./g, '\\.')}\\][^\\n]*\\n([\\s\\S]*?)(?=\\n##\\s*\\[|$)`);
  const m = text.match(re);
  body = m ? m[1].trim() : '';
}
if (!body) body = `版本 ${version} 更新`;

console.log(`${body}

---

### 下载说明
- **Windows**：\`.exe\` 安装包或 \`-portable.exe\` 便携版（更新推荐走应用内自动更新）
- **macOS**：\`.dmg\`（arm64 = Apple Silicon，x64 = Intel）
- **Linux**：\`.AppImage\` 或 \`.deb\`

### 系统要求
- Windows 10/11 (64位) · macOS 10.15+ · Linux (WebKit2GTK)
`);
