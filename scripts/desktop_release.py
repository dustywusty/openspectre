#!/usr/bin/env python3
"""Shared desktop target matrix, release validation, checksums, and publication."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = [
    {'target': 'aarch64-apple-darwin', 'runner': 'macos-15', 'os': 'macos', 'arch': 'arm64', 'extension': 'zip'},
    {'target': 'x86_64-apple-darwin', 'runner': 'macos-15-intel', 'os': 'macos', 'arch': 'x86_64', 'extension': 'zip'},
    {'target': 'x86_64-pc-windows-msvc', 'runner': 'windows-2025', 'os': 'windows', 'arch': 'x86_64', 'extension': 'zip'},
    {'target': 'aarch64-pc-windows-msvc', 'runner': 'windows-11-arm', 'os': 'windows', 'arch': 'arm64', 'extension': 'zip'},
    {'target': 'x86_64-unknown-linux-gnu', 'runner': 'ubuntu-22.04', 'os': 'linux', 'arch': 'x86_64', 'extension': 'tar.gz'},
    {'target': 'aarch64-unknown-linux-gnu', 'runner': 'ubuntu-22.04-arm', 'os': 'linux', 'arch': 'arm64', 'extension': 'tar.gz'},
]
SEMVER = re.compile(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')


def client_version():
    manifest = (ROOT / 'crates/spectre-client/Cargo.toml').read_text()
    return re.search(r'^version = "([^"]+)"', manifest, re.M)[1]


def validate_tag(tag, version):
    match = SEMVER.fullmatch(tag[1:]) if tag.startswith('v') else None
    if not match or (match[4] and any(p.isdigit() and len(p) > 1 and p[0] == '0' for p in match[4].split('.'))):
        raise ValueError('Use a version tag such as v0.1.0 or v0.2.0-rc.1')
    if tag != f'v{version}':
        raise ValueError(f'Tag {tag} does not match client version {version}; update crates/spectre-client/Cargo.toml and Cargo.lock first')
    return match[4] is not None


def target_info(target):
    return next((row for row in TARGETS if row['target'] == target), None) or _unsupported(target)


def _unsupported(target):
    raise ValueError(f'Unsupported desktop target: {target}')


def archive_name(version, target):
    row = target_info(target)
    return f"OpenSpectre-{version}-{row['os']}-{row['arch']}.{row['extension']}"


def binary_identity(data):
    if data[:4] == b'\x7fELF' and data[4] == 2:
        endian = '<' if data[5] == 1 else '>'
        machine = struct.unpack_from(endian + 'H', data, 18)[0]
        return 'linux', {62: 'x86_64', 183: 'arm64'}.get(machine)
    if data[:4] == b'\xcf\xfa\xed\xfe':
        machine = struct.unpack_from('<I', data, 4)[0]
        return 'macos', {0x1000007: 'x86_64', 0x100000c: 'arm64'}.get(machine)
    if data[:2] == b'MZ':
        offset = struct.unpack_from('<I', data, 0x3c)[0]
        if data[offset:offset + 4] == b'PE\0\0':
            machine = struct.unpack_from('<H', data, offset + 4)[0]
            return 'windows', {0x8664: 'x86_64', 0xaa64: 'arm64'}.get(machine)
    raise ValueError('The package does not contain a supported 64-bit executable')


def validate_binary(data, target):
    row = target_info(target)
    identity = binary_identity(data)
    if identity != (row['os'], row['arch']):
        raise ValueError(f"Wrong executable for {target}: found {identity}")


def verify_archive(path, version, target, revision):
    row = target_info(target)
    if row['os'] == 'macos':
        info_name = 'OpenSpectre.app/Contents/Resources/build-info.json'
        binary_name = 'OpenSpectre.app/Contents/MacOS/spectre-client'
    else:
        info_name = 'OpenSpectre/build-info.json'
        binary_name = 'OpenSpectre/spectre-client' + ('.exe' if row['os'] == 'windows' else '')
    if row['extension'] == 'zip':
        with zipfile.ZipFile(path) as archive:
            info = json.loads(archive.read(info_name))
            binary = archive.read(binary_name)
    else:
        with tarfile.open(path, 'r:gz') as archive:
            info = json.load(archive.extractfile(info_name))
            binary = archive.extractfile(binary_name).read()
    expected = {'version': version, 'target': target, 'revision': revision}
    if info != expected:
        raise ValueError(f'{path.name}: build identity mismatch: expected {expected}, got {info}')
    validate_binary(binary, target)


def checksums(directory, version, revision):
    expected = {archive_name(version, row['target']) for row in TARGETS}
    found = {file.name for file in directory.iterdir() if file.name.endswith(('.zip', '.tar.gz'))}
    if expected != found:
        raise ValueError(f'Release requires all six packages. Missing: {sorted(expected - found)}; unexpected: {sorted(found - expected)}')
    for row in TARGETS:
        verify_archive(directory / archive_name(version, row['target']), version, row['target'], revision)
    lines = [f'{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}' for name in sorted(expected)]
    manifest = directory / 'SHA256SUMS.txt'
    manifest.write_text('\n'.join(lines) + '\n')
    return [directory / name for name in sorted(expected)] + [manifest]


def prepare(env):
    event = env.get('GITHUB_EVENT_NAME', '')
    tag = env.get('RELEASE_TAG', '')
    requested = env.get('PUBLISH_REQUESTED') == 'true'
    publish = event == 'release' or (event == 'push' and env.get('GITHUB_REF_TYPE') == 'tag') or (event == 'workflow_dispatch' and requested)
    version = client_version()
    if publish and not tag:
        raise ValueError('Publishing requires an existing version tag')
    prerelease = validate_tag(tag, version) if tag else False
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if tag:
        tagged = subprocess.check_output(['git', 'rev-parse', f'refs/tags/{tag}^{{commit}}'], cwd=ROOT, text=True).strip()
        if tagged != revision:
            raise ValueError('The checked-out commit does not match the release tag')
    return {'version': version, 'tag': tag, 'revision': revision,
            'prerelease': str(prerelease).lower(), 'publish': str(publish).lower(),
            'matrix': json.dumps({'include': TARGETS}, separators=(',', ':'))}


def publish(directory, tag, revision, repo):
    version = client_version()
    prerelease = validate_tag(tag, version)
    files = checksums(directory, version, revision)
    def gh(*args, **kwargs):
        return subprocess.run(['gh', *args], check=True, text=True, **kwargs)
    # Distinguish a missing release from authentication/network failures.
    releases = json.loads(gh('api', f'repos/{repo}/releases', '--paginate', '--slurp', capture_output=True).stdout)
    existing = next((release for page in releases for release in page if release['tag_name'] == tag), None)
    if existing is None:
        args = ['release', 'create', tag, '--repo', repo, '--verify-tag', '--draft',
                '--title', f'OpenSpectre {version}', '--generate-notes']
        if prerelease:
            args.append('--prerelease')
        gh(*args)
        existing = json.loads(gh('api', f'repos/{repo}/releases/tags/{tag}', capture_output=True).stdout)
    assets = {asset['name']: asset for asset in existing['assets']}
    for file in files:
        if file.name in assets:
            # Reruns can fill missing assets, but must never replace different published bytes.
            digest = assets[file.name].get('digest')
            if not digest:
                with tempfile.TemporaryDirectory() as temporary:
                    downloaded = Path(temporary) / file.name
                    gh('release', 'download', tag, '--repo', repo, '--pattern', file.name, '--output', str(downloaded))
                    digest = 'sha256:' + hashlib.sha256(downloaded.read_bytes()).hexdigest()
            if digest != 'sha256:' + hashlib.sha256(file.read_bytes()).hexdigest():
                raise ValueError(f'{file.name} already exists with different bytes. Publish a new version instead of replacing it.')
        else:
            gh('release', 'upload', tag, str(file), '--repo', repo)
    if existing['draft']:
        args = ['release', 'edit', tag, '--repo', repo, '--draft=false']
        if prerelease:
            args.append('--prerelease')
        gh(*args)
    return f'https://github.com/{repo}/releases/tag/{tag}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('prepare')
    sub.add_parser('matrix')
    for command in ['checksums', 'publish']:
        p = sub.add_parser(command)
        p.add_argument('--directory', type=Path, required=True)
        p.add_argument('--revision', required=True)
        if command == 'publish':
            p.add_argument('--tag', required=True)
            p.add_argument('--repo', required=True)
    args = parser.parse_args()
    if args.command == 'prepare':
        values = prepare(os.environ)
        text = ''.join(f'{key}={value}\n' for key, value in values.items())
        if os.environ.get('GITHUB_OUTPUT'):
            with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
                output.write(text)
        print(text, end='')
    elif args.command == 'matrix':
        print(json.dumps({'include': TARGETS}))
    elif args.command == 'checksums':
        for file in checksums(args.directory, client_version(), args.revision):
            print(file)
    else:
        print(publish(args.directory, args.tag, args.revision, args.repo))


if __name__ == '__main__':
    main()
