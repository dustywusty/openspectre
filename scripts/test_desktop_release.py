#!/usr/bin/env python3
"""Exercise release failures before a workflow can publish public artifacts."""
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import desktop_release as release

VERSION = '0.1.0'
REVISION = 'a' * 40


def executable(row):
    data = bytearray(256)
    if row['os'] == 'macos':
        data[:4] = b'\xcf\xfa\xed\xfe'
        struct.pack_into('<I', data, 4, 0x100000c if row['arch'] == 'arm64' else 0x1000007)
    elif row['os'] == 'windows':
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 0x3c, 128)
        data[128:132] = b'PE\0\0'
        struct.pack_into('<H', data, 132, 0xaa64 if row['arch'] == 'arm64' else 0x8664)
    else:
        data[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', data, 18, 183 if row['arch'] == 'arm64' else 62)
    return bytes(data)


def packages(directory, revision=REVISION):
    for row in release.TARGETS:
        name = release.archive_name(VERSION, row['target'])
        info = json.dumps({'version': VERSION, 'target': row['target'], 'revision': revision}).encode()
        if row['os'] == 'macos':
            files = {'OpenSpectre.app/Contents/Resources/build-info.json': info,
                     'OpenSpectre.app/Contents/MacOS/spectre-client': executable(row)}
        else:
            extension = '.exe' if row['os'] == 'windows' else ''
            files = {'OpenSpectre/build-info.json': info, 'OpenSpectre/spectre-client' + extension: executable(row)}
        if row['extension'] == 'zip':
            with zipfile.ZipFile(directory / name, 'w') as archive:
                for path, data in files.items():
                    archive.writestr(path, data)
        else:
            with tarfile.open(directory / name, 'w:gz') as archive:
                for path, data in files.items():
                    member = tarfile.TarInfo(path)
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))


class Releases(unittest.TestCase):
    def test_tag_must_match_client_version_and_valid_semver(self):
        self.assertFalse(release.validate_tag('v0.1.0', VERSION))
        self.assertTrue(release.validate_tag('v0.2.0-rc.1', '0.2.0-rc.1'))
        for tag in ['0.1.0', 'v0.2.0', 'v01.1.0', 'v0.1.0/evil', 'v0.1.0-01']:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_tag(tag, VERSION)

    def test_six_unique_targets_and_archive_names(self):
        self.assertEqual(len(release.TARGETS), 6)
        self.assertEqual(len({release.archive_name(VERSION, row['target']) for row in release.TARGETS}), 6)
        for os_name in ['macos', 'linux', 'windows']:
            self.assertEqual({row['arch'] for row in release.TARGETS if row['os'] == os_name}, {'arm64', 'x86_64'})

    def test_executable_headers_prevent_mislabeled_architectures(self):
        for row in release.TARGETS:
            release.validate_binary(executable(row), row['target'])
            wrong = next(other for other in release.TARGETS if other['os'] == row['os'] and other['arch'] != row['arch'])
            with self.assertRaisesRegex(ValueError, 'Wrong executable'):
                release.validate_binary(executable(wrong), row['target'])
        with self.assertRaises(ValueError):
            release.binary_identity(b'not an executable')

    def test_checksums_require_six_packages_from_the_same_revision(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with self.assertRaisesRegex(ValueError, 'all six'):
                release.checksums(directory, VERSION, REVISION)
            packages(directory)
            files = release.checksums(directory, VERSION, REVISION)
            self.assertEqual(len(files), 7)
            for line in (directory / 'SHA256SUMS.txt').read_text().splitlines():
                digest, name = line.split('  ')
                self.assertEqual(digest, hashlib.sha256((directory / name).read_bytes()).hexdigest())
            with self.assertRaisesRegex(ValueError, 'identity mismatch'):
                release.checksums(directory, VERSION, 'b' * 40)
            files[0].unlink()
            with self.assertRaisesRegex(ValueError, 'all six'):
                release.checksums(directory, VERSION, REVISION)

    @patch.object(release, 'client_version', return_value=VERSION)
    @patch.object(release.subprocess, 'check_output', return_value=REVISION + '\n')
    def test_manual_builds_and_prs_do_not_publish(self, *_):
        for event in ['pull_request', 'workflow_dispatch', 'push']:
            result = release.prepare({'GITHUB_EVENT_NAME': event, 'GITHUB_REF_TYPE': 'branch'})
            self.assertEqual(result['publish'], 'false')
        with self.assertRaisesRegex(ValueError, 'existing version tag'):
            release.prepare({'GITHUB_EVENT_NAME': 'workflow_dispatch', 'PUBLISH_REQUESTED': 'true'})
        result = release.prepare({'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF_TYPE': 'tag', 'RELEASE_TAG': 'v0.1.0'})
        self.assertEqual(result['publish'], 'true')

    @patch.object(release, 'client_version', return_value=VERSION)
    def test_tag_must_point_to_the_checked_out_commit(self, *_):
        with patch.object(release.subprocess, 'check_output', side_effect=[REVISION, 'b' * 40]):
            with self.assertRaisesRegex(ValueError, 'checked-out commit'):
                release.prepare({'GITHUB_EVENT_NAME': 'release', 'RELEASE_TAG': 'v0.1.0'})

    @patch.object(release, 'client_version', return_value=VERSION)
    def test_publication_stages_a_draft_until_every_asset_is_uploaded(self, *_):
        calls = []
        def gh(command, **kwargs):
            calls.append(command)
            output = '[]'
            if command[1] == 'api' and '/tags/' in command[2]:
                output = json.dumps({'draft': True, 'assets': []})
            return subprocess.CompletedProcess(command, 0, stdout=output)
        with tempfile.TemporaryDirectory() as temporary, patch.object(release.subprocess, 'run', side_effect=gh):
            directory = Path(temporary)
            packages(directory)
            release.publish(directory, 'v0.1.0', REVISION, 'owner/repo')
        create = next(c for c in calls if c[1:3] == ['release', 'create'])
        self.assertIn('--draft', create)
        self.assertIn('--verify-tag', create)
        self.assertEqual(sum(c[1:3] == ['release', 'upload'] for c in calls), 7)
        self.assertEqual(calls[-1][1:3], ['release', 'edit'])
        self.assertIn('--draft=false', calls[-1])

    @patch.object(release, 'client_version', return_value=VERSION)
    def test_existing_public_assets_are_never_overwritten(self, *_):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            packages(directory)
            files = release.checksums(directory, VERSION, REVISION)
            assets = [{'name': f.name, 'digest': 'sha256:' + hashlib.sha256(f.read_bytes()).hexdigest()} for f in files]
            existing = {'tag_name': 'v0.1.0', 'draft': False, 'assets': assets}
            with patch.object(release.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stdout=json.dumps([[existing]]))) as gh:
                release.publish(directory, 'v0.1.0', REVISION, 'owner/repo')
                self.assertEqual(gh.call_count, 1, 'An identical rerun must not mutate a release')
            assets[0]['digest'] = 'sha256:' + '0' * 64
            with patch.object(release.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stdout=json.dumps([[existing]]))) as gh:
                with self.assertRaisesRegex(ValueError, 'different bytes'):
                    release.publish(directory, 'v0.1.0', REVISION, 'owner/repo')
                self.assertEqual(gh.call_count, 1)

    def test_missing_packages_cannot_call_github(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(release.subprocess, 'run') as gh:
            with self.assertRaises(ValueError):
                release.publish(Path(temporary), 'v' + release.client_version(), REVISION, 'owner/repo')
            gh.assert_not_called()


if __name__ == '__main__':
    unittest.main()
