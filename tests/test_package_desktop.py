"""Safety contracts for the development desktop packager (no compiler required)."""
import html
import importlib.util
import io
import json
import pathlib
import plistlib
import runpy
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from contextlib import redirect_stdout
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts/package-desktop.py'


class DesktopPackagingTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location('package_desktop', SCRIPT)
        self.package = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.package)

    def generate_notices(self, bundled_license, source_license):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / 'scripts').mkdir()
            (root / 'packaging').mkdir()
            source = root / 'dependency/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md'
            source.parent.mkdir(parents=True)
            source.write_bytes(source_license)
            (root / 'packaging/Slint-License.md').write_bytes(bundled_license)
            shutil.copy(ROOT / 'packaging/about.toml', root / 'packaging/about.toml')
            font_dir = root / 'site/assets/fonts'
            font_dir.mkdir(parents=True)
            for name in ('dm-mono-OFL.txt', 'dm-sans-OFL.txt'):
                shutil.copy(ROOT / 'site/assets/fonts' / name, font_dir / name)
            script = root / 'scripts/generate-notices.py'
            shutil.copy(ROOT / 'scripts/generate-notices.py', script)
            output = root / 'notices.html'
            metadata = {'packages': [{
                'name': 'slint', 'version': '1.18.1',
                'license': 'LicenseRef-Slint-Royalty-free-2.0',
                'manifest_path': str(root / 'dependency/Cargo.toml'),
            }]}

            def cargo_about(command, **kwargs):
                pathlib.Path(command[command.index('--output-file') + 1]).write_text('<html></html>')
                return subprocess.CompletedProcess(command, 0, stderr=
                    "WARN LicenseRef-Slint-Royalty-free-2.0 has no license file for crate 'slint 1.18.1'\n")

            with mock.patch.object(sys, 'argv', [str(script), '--output', str(output)]), \
                    mock.patch('subprocess.check_output', return_value=json.dumps(metadata).encode()), \
                    mock.patch('subprocess.run', side_effect=cargo_about), \
                    redirect_stdout(io.StringIO()):
                runpy.run_path(str(script), run_name='__main__')
            return output.read_text(encoding='utf-8')

    def test_notices_accept_windows_line_endings(self):
        license_text = (ROOT / 'packaging/Slint-License.md').read_bytes().replace(b'\r\n', b'\n')
        for bundled_eol in (b'\n', b'\r\n'):
            for source_eol in (b'\n', b'\r\n'):
                with self.subTest(bundled_eol=bundled_eol, source_eol=source_eol):
                    rendered = self.generate_notices(
                        license_text.replace(b'\n', bundled_eol),
                        license_text.replace(b'\n', source_eol))
                    self.assertIn('Slint Royalty-free License 2.0', rendered)
                    self.assertIn('slint 1.18.1', rendered)
                    for name in ('dm-mono-OFL.txt', 'dm-sans-OFL.txt'):
                        font_license = (ROOT / 'site/assets/fonts' / name).read_text(encoding='utf-8')
                        expected = '\n'.join(line.rstrip() for line in html.escape(font_license).splitlines()).strip()
                        self.assertIn(expected, rendered)

    def test_notices_reject_changed_license_text(self):
        license_text = (ROOT / 'packaging/Slint-License.md').read_bytes()
        changed = license_text + b'Changed license terms\n'
        for bundled, source in [(license_text, changed), (changed, license_text), (changed, changed)]:
            with self.subTest(bundled_changed=bundled == changed, source_changed=source == changed), \
                    self.assertRaisesRegex(SystemExit, 'Slint license text changed'):
                self.generate_notices(bundled, source)

    def test_release_inputs_reject_injection_and_wrong_tags(self):
        for version in ['1.2.3;touch bad', '../1.2.3', '01.2.3', '1.2.3-01', '1.2.3-']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                self.package.validate_release(version, version, None)
        for ref in ['refs/tags/v1.2.4', 'v1.2.3', 'refs/tags/v1.2.3\n']:
            with self.subTest(ref=ref), self.assertRaises(ValueError):
                self.package.validate_release('1.2.3', '1.2.3', ref)
        self.package.validate_release('1.2.3-dev.1', '1.2.3-dev.1', 'refs/tags/v1.2.3-dev.1')
        with self.assertRaises(ValueError):
            self.package.validate_release('1.2.3', '1.2.4', None)

    def test_release_version_accepts_cargo_placeholder(self):
        self.package.validate_release('1.2.3', '0.0.0', None)
        self.package.validate_release('1.2.3', '0.0.0', 'refs/tags/v1.2.3')
        self.package.validate_release('0.0.0', '0.0.0', None)
        with self.assertRaises(ValueError):
            self.package.validate_release('1.2.3', '0.0.1', None)

    def test_stamp_version_rewrites_only_the_root_package(self):
        manifest = '[package]\r\nname = "bulletnotes"\r\nversion = "0.0.0"\r\n\r\n[dependencies]\r\nother = "0.0.0"\r\n'
        lock = ('[[package]]\r\nname = "aaa"\r\nversion = "0.0.0"\r\n\r\n'
                '[[package]]\r\nname = "bulletnotes"\r\nversion = "0.0.0"\r\ndependencies = [\r\n "aaa",\r\n]\r\n')
        stamped_manifest, stamped_lock = self.package.stamp_version(manifest, lock, '1.2.3')
        self.assertEqual(stamped_manifest, manifest.replace('version = "0.0.0"', 'version = "1.2.3"', 1))
        self.assertEqual(stamped_lock, lock.replace('name = "bulletnotes"\r\nversion = "0.0.0"',
                                                    'name = "bulletnotes"\r\nversion = "1.2.3"'))
        with self.assertRaises(ValueError):
            self.package.stamp_version(stamped_manifest, lock, '1.2.3')
        with self.assertRaises(ValueError):
            self.package.stamp_version(manifest, stamped_lock, '1.2.3')

    def test_repository_cargo_files_hold_the_placeholder(self):
        self.package.stamp_version((ROOT / 'Cargo.toml').read_text(), (ROOT / 'Cargo.lock').read_text(), '1.2.3')

    def test_stamped_version_restores_the_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            manifest = b'[package]\nname = "bulletnotes"\nversion = "0.0.0"\n'
            lock = b'[[package]]\nname = "bulletnotes"\nversion = "0.0.0"\n'
            (root / 'Cargo.toml').write_bytes(manifest)
            (root / 'Cargo.lock').write_bytes(lock)
            with self.assertRaises(RuntimeError):
                with self.package.stamped_version(root, '1.2.3'):
                    self.assertIn(b'version = "1.2.3"', (root / 'Cargo.toml').read_bytes())
                    self.assertIn(b'version = "1.2.3"', (root / 'Cargo.lock').read_bytes())
                    raise RuntimeError('build failed')
            self.assertEqual((root / 'Cargo.toml').read_bytes(), manifest)
            self.assertEqual((root / 'Cargo.lock').read_bytes(), lock)

    def test_development_cannot_inherit_signing_or_notary_credentials(self):
        env = self.package.development_environment({
            'PATH': '/bin', 'APPLE_ID': 'secret', 'APPLE_API_KEY_PATH': 'secret',
            'APPLE_SIGNING_IDENTITY': 'secret', 'CARGO_PACKAGER_SIGN_PRIVATE_KEY': 'secret',
            'BULLETNOTES_NOTARY_PROFILE': 'secret', 'CARGO_TARGET_DIR': '/other',
        })
        self.assertEqual(env['PATH'], '/bin')
        self.assertFalse(any(key.startswith(('APPLE_', 'CARGO_PACKAGER_', 'BULLETNOTES_NOTARY_')) for key in env))
        self.assertNotIn('CARGO_TARGET_DIR', env)

    def packager_config(self):
        return tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['metadata']['packager']

    def test_packager_config_reads_committed_files_and_generated_inputs(self):
        config = self.packager_config()
        sources = [resource['src'] for resource in config['resources']] + config['icons'] + [
            config['macos']['entitlements'], config['macos']['info-plist-path'], config['nsis']['installer-icon']]
        generated = {'target/packaging/ThirdPartyNotices.html', config['deb']['depends']}
        self.assertEqual(generated, {'target/packaging/ThirdPartyNotices.html', 'target/packaging/deb-depends.txt'})
        for source in sources:
            with self.subTest(source=source):
                self.assertTrue(source in generated or (ROOT / source).is_file())
        self.assertEqual({resource['target'] for resource in config['resources']},
                         {'ThirdPartyNotices.html', 'Slint-License.md'})

    def test_macos_icon_matches_info_plist_and_version_comes_from_cargo(self):
        info = plistlib.loads((ROOT / self.packager_config()['macos']['info-plist-path']).read_bytes())
        self.assertIn(info['CFBundleIconFile'], [pathlib.Path(icon).name for icon in self.packager_config()['icons']])
        self.assertNotIn('CFBundleShortVersionString', info)

    def test_linux_inputs_declare_dynamically_loaded_libraries(self):
        with tempfile.TemporaryDirectory() as temporary:
            inputs = pathlib.Path(temporary) / 'packaging'
            with mock.patch.object(self.package, 'run'), \
                    mock.patch.object(self.package, 'capture', return_value='shlibs:Depends=libc6 (>= 2.39), libgcc-s1'):
                notices = self.package.write_packaging_inputs(inputs, 'x86_64-unknown-linux-gnu', pathlib.Path('bulletnotes'), {})
            self.assertEqual(notices, inputs / 'ThirdPartyNotices.html')
            depends = (inputs / 'deb-depends.txt').read_text().splitlines()
            self.assertEqual(depends[:2], ['libc6 (>= 2.39)', 'libgcc-s1'])
            self.assertTrue(set(self.package.LINUX_LIBRARIES.values()).issubset(depends))

    def test_binary_architecture_is_checked_independently_of_bundle_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = pathlib.Path(temporary) / 'binary'
            elf = bytearray(64)
            elf[:6] = b'\x7fELF\x02\x01'
            elf[18:20] = (183).to_bytes(2, 'little')
            binary.write_bytes(elf)
            self.package.verify_binary_target(binary, 'aarch64-unknown-linux-gnu')
            with self.assertRaises(ValueError):
                self.package.verify_binary_target(binary, 'x86_64-unknown-linux-gnu')
            pe = bytearray(256)
            pe[:2] = b'MZ'
            pe[60:64] = (128).to_bytes(4, 'little')
            pe[128:134] = b'PE\0\0' + (0x8664).to_bytes(2, 'little')
            binary.write_bytes(pe)
            self.package.verify_binary_target(binary, 'x86_64-pc-windows-msvc')
            with self.assertRaises(ValueError):
                self.package.verify_binary_target(binary, 'aarch64-pc-windows-msvc')

    def test_artifact_export_refuses_overwrite_and_hashes_every_download(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            artifact = root / 'Bulletnotes_1.2.3_x64-setup.exe'
            artifact.write_bytes(b'installer')
            output = root / 'out'
            output.mkdir()
            self.package.export_artifacts([artifact], output, 'Bulletnotes-1.2.3-x86_64-pc-windows-msvc-development', {'launch': 'unobserved (static-only)'})
            report = json.loads(next(output.glob('*.verification.json')).read_text())
            self.assertEqual(list(report['artifacts']), ['Bulletnotes-1.2.3-x86_64-pc-windows-msvc-development.exe'])
            self.assertEqual(next(iter(report['artifacts'].values())), self.package.sha(artifact))
            with self.assertRaises(FileExistsError):
                self.package.export_artifacts([artifact], output, 'Bulletnotes-1.2.3-x86_64-pc-windows-msvc-development', {})


if __name__ == '__main__':
    unittest.main()
