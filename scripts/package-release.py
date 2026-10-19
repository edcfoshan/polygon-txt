"""Collect signed desktop assets and a standalone CLI archive after validation."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import zipfile

parser = argparse.ArgumentParser()
parser.add_argument('--target', required=True)
parser.add_argument('--label', required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
version = json.loads((root / 'package.json').read_text('utf-8'))['version']
build = root / 'src-tauri/target' / args.target / 'release'
out = root / 'dist-release' / args.label
gui, cli = out / 'gui', out / 'cli'
gui.mkdir(parents=True, exist_ok=True)
cli.mkdir(parents=True, exist_ok=True)
prefix = f'JisigG_{version}'

def collect(pattern, name, signed=False):
    matches = list((build / 'bundle').glob(pattern))
    if len(matches) != 1:
        raise RuntimeError(f'Expected exactly one {pattern}, got {matches}')
    shutil.copy2(matches[0], gui / name)
    if signed:
        signature = Path(str(matches[0]) + '.sig').read_text('utf-8').strip()
        if not signature:
            raise RuntimeError(f'Missing signature: {name}')
        (gui / (name + '.sig')).write_text(signature, encoding='utf-8')
        return {'asset': name, 'signature': signature}

if args.label == 'windows-x64':
    updater = collect(f'nsis/*_{version}_x64-setup.exe', f'{prefix}_x64-setup.exe', True)
    shutil.copy2(build / 'jisig-bpoint-converter.exe', gui / f'{prefix}_x64-portable.exe')
    platform = 'windows-x86_64'
elif args.label == 'linux-x64':
    updater = collect(f'appimage/*_{version}_amd64.AppImage', f'{prefix}_amd64.AppImage', True)
    collect(f'deb/*_{version}_amd64.deb', f'{prefix}_amd64.deb')
    platform = 'linux-x86_64'
else:
    arch = 'aarch64' if args.label == 'macos-arm64' else 'x64'
    collect(f'dmg/*_{version}_{arch}.dmg', f'{prefix}_{arch}.dmg')
    updater = collect('macos/*.app.tar.gz', f'{prefix}_{arch}.app.tar.gz', True)
    platform = 'darwin-aarch64' if arch == 'aarch64' else 'darwin-x86_64'

exe = build / ('jisigg-cli.exe' if args.label == 'windows-x64' else 'jisigg-cli')
assert subprocess.check_output([str(exe), '--version'], text=True).strip() == f'jisigg-cli {version}'
subprocess.run([str(exe), 'config', 'validate', str(root / 'docs/cli/example.json'), '--json'], check=True)
folder = f'JisigG_CLI_{version}_{args.label}'
stage = out / folder
stage.mkdir(exist_ok=True)
shutil.copy2(exe, stage / exe.name)
shutil.copy2(root / 'docs/CLI.md', stage / 'README.md')
shutil.copy2(root / 'docs/cli/SKILL.md', stage / 'SKILL.md')
shutil.copy2(root / 'docs/cli/example.json', stage / 'example.json')
if args.label == 'windows-x64':
    archive = cli / (folder + '.zip')
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
        for file in stage.iterdir():
            z.write(file, f'{folder}/{file.name}')
else:
    archive = cli / (folder + '.tar.gz')
    with tarfile.open(archive, 'w:gz') as t:
        t.add(stage, arcname=folder)

for directory in [gui, cli]:
    checksums = [f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}'
                 for p in sorted(directory.iterdir()) if p.is_file()]
    (directory / f'SHA256SUMS-{args.label}.txt').write_text('\n'.join(checksums) + '\n', encoding='utf-8')
(out / 'manifest.json').write_text(json.dumps({
    'version': version, 'label': args.label, 'platform': platform, 'updater': updater,
}, indent=2), encoding='utf-8')
