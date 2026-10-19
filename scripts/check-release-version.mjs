import { readFileSync } from 'node:fs';
const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1];
if (tauri !== version || cargo !== version || process.env.GITHUB_REF_NAME !== `v${version}`) {
  throw new Error(`Version mismatch: package=${version}, desktop=${tauri}, Rust=${cargo}, tag=${process.env.GITHUB_REF_NAME}`);
}
