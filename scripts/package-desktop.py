#!/usr/bin/env python3
"""Build a standalone desktop package for the current host. No server or asset folder is needed."""
import argparse
import json
import math
import os
from pathlib import Path
import platform
import plistlib
import shutil
import struct
import subprocess
import tarfile
import tempfile
import zipfile
import zlib
from desktop_release import archive_name, client_version, target_info, validate_binary

ROOT = Path(__file__).resolve().parents[1]


def run(*command):
    subprocess.run(command, cwd=ROOT, check=True)


def zip_folder(folder, archive):
    # Cargo license files can retain timestamps older than ZIP's 1980 lower bound.
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, strict_timestamps=False) as bundle:
        for file in sorted(folder.rglob('*')):
            if file.is_file():
                bundle.write(file, file.relative_to(folder.parent))


def icon_png(path, size):
    """Draw the app's vector-style radar mark into a PNG with only the standard library."""
    pixels = bytearray()
    for y in range(size):
        pixels.append(0)
        for x in range(size):
            px, py = (x + .5) / size * 128, (y + .5) / size * 128
            dx, dy = px - 64, py - 64
            rounded = max(abs(dx) - 40, 0) ** 2 + max(abs(dy) - 40, 0) ** 2
            if rounded > 22 ** 2:
                color = (0, 0, 0, 0)
            else:
                color = (11, 25, 36, 255)
                radius = math.hypot(dx, dy)
                if 42 < radius < 44 or 26 < radius < 27:
                    color = (40, 78, 85, 255)
                if abs(dx) < .5 or abs(dy) < .5:
                    color = (31, 58, 69, 255)
                # A forward-pointing tank silhouette and its two tracks.
                if (49 < px < 79 and 55 < py < 86) or (59 < px < 69 and 32 < py < 63):
                    color = (104, 236, 204, 255)
                if ((41 < px < 47) or (81 < px < 87)) and 55 < py < 88:
                    color = (63, 167, 157, 255)
                if math.hypot(px - 96, py - 36) < 5:
                    color = (255, 194, 114, 255)
            pixels.extend(color)
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0))
                     + chunk(b'IDAT', zlib.compress(bytes(pixels), 9)) + chunk(b'IEND', b''))


def notices(destination, version, target):
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / 'assets/open/LICENSE', destination / 'Signal-Frontier-LICENSE.txt')
    shutil.copy2(ROOT / 'crates/spectre-client/assets/SpaceGrotesk-OFL.txt', destination / 'SpaceGrotesk-OFL.txt')
    shutil.copy2(ROOT / 'docs/desktop-client.md', destination / 'README.md')
    shutil.copy2(ROOT / 'docs/desktop-releases.md', destination / 'desktop-releases.md')
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    (destination / 'build-info.json').write_text(json.dumps({
        'version': version, 'target': target, 'revision': revision,
    }, indent=2) + '\n')
    # Include the licenses shipped by each runtime dependency in Cargo's registry.
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=ROOT))
    packages = {package['id']: package for package in metadata['packages']}
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    client = next(p['id'] for p in metadata['packages'] if p['name'] == 'spectre-client')
    visited = set()
    def visit(ident):
        if ident in visited:
            return
        visited.add(ident)
        for dependency in nodes[ident]['deps']:
            if any(kind['kind'] != 'dev' for kind in dependency['dep_kinds']):
                visit(dependency['pkg'])
    visit(client)
    inventory = []
    for ident in sorted(visited):
        package = packages[ident]
        if not package['source']:
            continue
        inventory.append(f"{package['name']} {package['version']}: {package['license'] or 'See included license'}")
        source = Path(package['manifest_path']).parent
        license_dir = destination / 'licenses' / f"{package['name']}-{package['version']}"
        for file in source.iterdir():
            if file.is_file() and file.name.lower().startswith(('license', 'copying', 'notice', 'unlicense')):
                license_dir.mkdir(parents=True, exist_ok=True)
                shutil.copy2(file, license_dir / file.name)
    (destination / 'DEPENDENCIES.txt').write_text('\n'.join(inventory) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-build', action='store_true', help='Package an existing release binary')
    parser.add_argument('--target', help='Rust target triple; defaults to the compiler host')
    parser.add_argument('--out', type=Path, default=ROOT / 'artifacts/desktop')
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    compiler = subprocess.check_output(['rustc', '-vV'], text=True)
    target = args.target or next(line.split(': ', 1)[1] for line in compiler.splitlines() if line.startswith('host: '))
    target_data = target_info(target)
    host = platform.system()
    if target_data['os'] != {'Darwin': 'macos', 'Windows': 'windows', 'Linux': 'linux'}.get(host):
        raise SystemExit('Package on the target operating system so native app tools and libraries are available')
    if not args.no_build:
        run('cargo', 'build', '--locked', '--release', '-p', 'spectre-client', '--target', target)
    version = client_version()
    target_dir = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target_dir.is_absolute():
        target_dir = ROOT / target_dir
    binary = target_dir / target / 'release' / ('spectre-client.exe' if host == 'Windows' else 'spectre-client')
    if not binary.is_file():
        raise SystemExit(f'Release binary is missing: {binary}')
    validate_binary(binary.read_bytes(), target)
    archive = out / archive_name(version, target)
    with tempfile.TemporaryDirectory(prefix='spectre-package-') as temporary:
        stage = Path(temporary)
        if host == 'Darwin':
            app = stage / 'OpenSpectre.app'
            contents = app / 'Contents'
            (contents / 'MacOS').mkdir(parents=True)
            resources = contents / 'Resources'
            notices(resources, version, target)
            shutil.copy2(binary, contents / 'MacOS/spectre-client')
            (contents / 'MacOS/spectre-client').chmod(0o755)
            info = {'CFBundleName': 'OpenSpectre', 'CFBundleDisplayName': 'OpenSpectre',
                    'CFBundleIdentifier': 'org.openspectre.client', 'CFBundleExecutable': 'spectre-client',
                    'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': version.split('-')[0].split('+')[0],
                    'CFBundleVersion': version.split('-')[0].split('+')[0], 'CFBundleIconFile': 'OpenSpectre',
                    'NSHighResolutionCapable': True, 'LSMinimumSystemVersion': '11.0',
                    'LSApplicationCategoryType': 'public.app-category.action-games'}
            with (contents / 'Info.plist').open('wb') as stream:
                plistlib.dump(info, stream)
            iconset = stage / 'OpenSpectre.iconset'
            iconset.mkdir()
            for size in [16, 32, 128, 256, 512]:
                for scale in [1, 2]:
                    suffix = '@2x' if scale == 2 else ''
                    icon_png(iconset / f'icon_{size}x{size}{suffix}.png', size * scale)
            run('iconutil', '-c', 'icns', str(iconset), '-o', str(resources / 'OpenSpectre.icns'))
            run('codesign', '--force', '--sign', '-', str(app))
            # Replace only this script's named output, after staging a complete package.
            destination = out / app.name
            if destination.exists():
                shutil.rmtree(destination)
            shutil.copytree(app, destination)
            run('ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(destination), str(archive))
            print(f'App: {destination}')
        else:
            folder = stage / 'OpenSpectre'
            notices(folder, version, target)
            shutil.copy2(binary, folder / binary.name)
            if host == 'Windows':
                zip_folder(folder, archive)
            else:
                with tarfile.open(archive, 'w:gz') as bundle:
                    bundle.add(folder, arcname=folder.name)
        print(f'Archive: {archive}')


if __name__ == '__main__':
    main()
