// Worker 护栏自测：node --test cloudflare/update-download/worker.test.mjs
// 覆盖：路径/方法校验、无密钥回落 GitHub、R2 命中、对象缺失、R2 异常、限流、超大体量拒绝。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import worker from './worker.mjs';

const KEY = 'k'.repeat(32);
const GITHUB = 'https://github.com/edcfoshan/polygon-txt/releases/download/v4.5.0/JisigG_4.5.0_x64-setup.exe';

function env({ object, throwOnGet = false, limitOk = true } = {}) {
  return {
    TICKET_KEY: KEY,
    GITHUB_REPO: 'edcfoshan/polygon-txt',
    DOWNLOAD_LIMIT: { limit: async () => ({ success: limitOk }) },
    UPDATES: {
      get: async () => {
        if (throwOnGet) throw new Error('r2 down');
        return object ?? null;
      },
    },
  };
}

function req(path = '/download/4.5.0', headers = {}, method = 'GET') {
  return new Request(`https://updates.example.workers.dev${path}`, { method, headers });
}

const payload = new Uint8Array([1, 2, 3, 4]);

test('未带客户端密钥 → 302 回落 GitHub（不读 R2）', async () => {
  let read = false;
  const e = env({ object: { size: 4, body: payload } });
  e.UPDATES.get = async () => { read = true; return null; };
  const res = await worker.fetch(req(), e);
  assert.equal(res.status, 302);
  assert.equal(res.headers.get('location'), GITHUB);
  assert.equal(read, false, '无密钥请求不得触发 R2 读取');
});

test('带密钥且对象存在 → 200，附带字节与文件名', async () => {
  const res = await worker.fetch(req('/download/4.5.0', { 'X-Client-Key': KEY }), env({ object: { size: 4, body: payload } }));
  assert.equal(res.status, 200);
  assert.equal(res.headers.get('content-length'), '4');
  assert.equal(res.headers.get('content-disposition'), 'attachment; filename="JisigG_4.5.0_x64-setup.exe"');
  assert.deepEqual(new Uint8Array(await res.arrayBuffer()), payload);
});

test('对象缺失 → 302 回落 GitHub', async () => {
  const res = await worker.fetch(req('/download/4.5.0', { 'X-Client-Key': KEY }), env({ object: null }));
  assert.equal(res.status, 302);
  assert.equal(res.headers.get('location'), GITHUB);
});

test('R2 抛错 → 302 回落 GitHub，不向用户报失败', async () => {
  const res = await worker.fetch(req('/download/4.5.0', { 'X-Client-Key': KEY }), env({ throwOnGet: true }));
  assert.equal(res.status, 302);
});

test('超过每 IP 限流 → 429', async () => {
  const res = await worker.fetch(req('/download/4.5.0', { 'X-Client-Key': KEY }), env({ object: { size: 4, body: payload }, limitOk: false }));
  assert.equal(res.status, 429);
});

test('对象超过 64MB 上限 → 413 且关闭流', async () => {
  let cancelled = false;
  const res = await worker.fetch(
    req('/download/4.5.0', { 'X-Client-Key': KEY }),
    env({ object: { size: 64 * 1024 * 1024 + 1, body: { cancel: async () => { cancelled = true; } } } }),
  );
  assert.equal(res.status, 413);
  assert.equal(cancelled, true);
});

test('路径/方法非法 → 404，且不读 R2', async () => {
  let read = false;
  const e = env({ object: { size: 4, body: payload } });
  e.UPDATES.get = async () => { read = true; return null; };
  for (const r of [
    req('/download/4.5.0/extra', { 'X-Client-Key': KEY }),
    req('/download/abc', { 'X-Client-Key': KEY }),
    req('/download/4.5.0?x=1', { 'X-Client-Key': KEY }),
    req('/download/4.5.0', { 'X-Client-Key': KEY }, 'POST'),
  ]) {
    assert.equal((await worker.fetch(r, e)).status, 404);
  }
  assert.equal(read, false);
});

test('GITHUB_REPO 未配置 → 503（不读 R2）', async () => {
  const e = env({ object: { size: 4, body: payload } });
  delete e.GITHUB_REPO;
  assert.equal((await worker.fetch(req('/download/4.5.0', { 'X-Client-Key': KEY }), e)).status, 503);
});