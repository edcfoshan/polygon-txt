import fs from 'node:fs';
import path from 'node:path';

const repoRoot = path.resolve(process.cwd());
const tauriConfigPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
const capabilityPath = path.join(repoRoot, 'src-tauri', 'capabilities', 'default.json');
const indexPath = path.join(repoRoot, 'index.html');

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const tauriConfig = readJson(tauriConfigPath);
const capability = readJson(capabilityPath);
const indexHtml = fs.readFileSync(indexPath, 'utf8');

const mainWindow = tauriConfig.app?.windows?.[0];
assert(mainWindow, 'tauri.conf.json 缺少主窗口配置');
assert(mainWindow.decorations === false, '主窗口未启用自绘标题栏（decorations 应为 false）');

const permissions = capability.permissions ?? [];
assert(
  permissions.includes('core:window:default'),
  'capability 缺少 core:window:default，窗口最小化/最大化/关闭 API 可能被拒绝'
);
assert(
  permissions.includes('core:window:allow-start-dragging'),
  'capability 缺少 core:window:allow-start-dragging，自绘标题栏拖拽区不完整'
);

for (const id of ['btnWinMin', 'btnWinMax', 'btnWinClose']) {
  assert(indexHtml.includes(`id="${id}"`), `index.html 缺少窗口按钮 ${id}`);
  const buttonTag = indexHtml.match(new RegExp(`<button[^>]*id="${id}"[^>]*>`, 'i'))?.[0] ?? '';
  assert(buttonTag, `index.html 未找到窗口按钮 ${id} 的标签`);
  assert(
    !buttonTag.includes('data-tauri-drag-region'),
    `${id} 不应带 data-tauri-drag-region 属性，存在该属性会被当成拖拽区`
  );
}

console.log('Titlebar config OK');
