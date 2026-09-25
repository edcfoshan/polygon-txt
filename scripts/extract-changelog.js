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
// 发布资产名必须 ASCII（GitHub 服务端会吃掉中文字符）：polygon-txt_{maj.min}_x64-*，与 gen-latest-json.js 一致
const shortVer = version.split('.').slice(0, 2).join('.');

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

### 下载与安装

| 平台 | 文件 | 说明 |
| --- | --- | --- |
| Windows | \`polygon-txt_${shortVer}_x64-setup.exe\` | **安装版（推荐）**：双击安装，写入开始菜单/桌面快捷方式，支持应用内自动更新 |
| Windows | \`polygon-txt_${shortVer}_x64-portable.exe\` | **便携版**：免安装，双击即用；换机器直接拷走 |
| macOS | \`*.dmg\` | Intel 芯片选 \`x64\`，Apple Silicon 选 \`aarch64\`（资产名中文前缀会被 GitHub 剥除，按后缀与架构认文件） |
| Linux | \`*.AppImage\` / \`*.deb\` | AppImage 免安装（\`chmod +x\` 后直接运行），deb 用系统包管理器安装 |

**Windows 安装步骤**（首次运行可能被 SmartScreen 拦下）：
1. 下载 \`polygon-txt_${shortVer}_x64-setup.exe\`
2. 双击运行；若出现「Windows 已保护你的电脑」，点「更多信息」→「仍要运行」
3. 按向导完成安装（勾选项含创建桌面快捷方式）

**更新**：各平台均支持应用内自动更新——启动后静默检查，标题栏出现绿色箭头即为有新版本，点它按提示「立即更新」。自动更新失败时，可按弹窗内的百度云链接手动下载。
国内下载慢时也可用百度云：https://pan.baidu.com/s/1xyW3-hyZrFDDG9ijYOf46g?pwd=e8vy （提取码 e8vy）

### 系统要求

- **Windows 10 / 11（64 位）**：需要 WebView2 运行时（Win11 与已更新的 Win10 已内置，缺失时安装程序会提示安装）
- **macOS 10.15+**（Intel / Apple Silicon 均有独立产物）
- **Linux**：需 WebKit2GTK（主流发行版仓库自带）

> ⚠️ 本工具完全免费开源。**任何以「激活 / 解锁 / 代下载」为名收费的渠道均为假冒**，请只从本仓库 Releases 或上方百度云链接获取。
`);
