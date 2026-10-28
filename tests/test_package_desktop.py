"""Safety contracts for the development desktop packager (no compiler required)."""
import importlib.util
import json
import pathlib
import plistlib
import tempfile
import runpy
import subprocess
import sys
from unittest import mock
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts/package-desktop.py'


class DesktopPackagingTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(SCRIPT.is_file(), 'desktop packaging orchestration must exist')
        spec = importlib.util.spec_from_file_location('package_desktop', SCRIPT)
        self.package = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.package)

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

    def test_development_cannot_inherit_signing_or_notary_credentials(self):
        env = self.package.development_environment({
            'PATH': '/bin', 'APPLE_ID': 'secret', 'APPLE_API_KEY_PATH': 'secret',
            'APPLE_SIGNING_IDENTITY': 'secret', 'TAURI_SIGNING_PRIVATE_KEY': 'secret',
            'BULLETNOTES_NOTARY_PROFILE': 'secret', 'CARGO_TARGET_DIR': '/other',
        })
        self.assertEqual(env['PATH'], '/bin')
        self.assertFalse(any(key.startswith(('APPLE_', 'TAURI_SIGNING_', 'BULLETNOTES_NOTARY_')) for key in env))
        self.assertNotIn('CARGO_TARGET_DIR', env)

    def test_fixture_preserves_custom_metadata_and_only_packages_real_binary(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = pathlib.Path(temporary)
            notices = fixture / 'notices.html'
            notices.write_text('runtime licenses')
            self.package.write_fixture(fixture, ROOT, '0.1.0', 'aarch64-apple-darwin', notices, 'abc123', True, fixture / 'binary')
            config = json.loads((fixture / 'tauri.conf.json').read_text())
            info = plistlib.loads((fixture / 'Info.plist').read_bytes())
            self.assertEqual(info['NSPrincipalClass'], 'BulletnotesApplication')
            self.assertEqual(info['BulletnotesSourceRevision'], 'abc123+dirty')
            self.assertEqual(info['BulletnotesVersion'], '0.1.0')
            self.assertEqual(config['bundle']['macOS']['signingIdentity'], '-')
            self.assertIn(str(notices), config['bundle']['resources'])
            self.assertIn(str(ROOT / 'src/main.rs'), (fixture / 'Cargo.toml').read_text())
            self.assertFalse((fixture / 'src/main.rs').exists())

    def test_linux_installer_tracks_linked_and_dynamically_loaded_libraries(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = pathlib.Path(temporary)
            with mock.patch.object(self.package, 'capture', side_effect=['shlibs:Depends=libc6 (>= 2.39), libfontconfig1', '/usr/lib/x86_64-linux-gnu']) as capture:
                self.package.write_fixture(fixture, ROOT, '0.1.0', 'x86_64-unknown-linux-gnu', fixture / 'notices.html', 'abc', False, fixture / 'binary')
            config = json.loads((fixture / 'tauri.conf.json').read_text())['bundle']['linux']
            self.assertIn('libc6 (>= 2.39)', config['deb']['depends'])
            self.assertIn('libxkbcommon-x11-0', config['deb']['depends'])
            self.assertIn('libvulkan1', config['deb']['depends'])
            self.assertEqual(config['appimage']['files']['usr/lib/libxkbcommon-x11.so.0'], '/usr/lib/x86_64-linux-gnu/libxkbcommon-x11.so.0')
            self.assertEqual(config['appimage']['files']['usr/share/doc/libxkbcommon-x11-0/copyright'], '/usr/share/doc/libxkbcommon-x11-0/copyright')
            self.assertIn('dpkg-shlibdeps', capture.call_args_list[0].args[0])
            self.assertTrue((fixture / 'debian/control').is_file())

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

    def test_notices_filter_both_dependency_tools_without_writing_source(self):
        original = (ROOT / 'packaging/ThirdPartyNotices.html').read_bytes()
        with tempfile.TemporaryDirectory() as temporary:
            destination = pathlib.Path(temporary) / 'notices.html'
            commands = []

            def metadata(command, **kwargs):
                commands.append(command)
                return b'{"packages": []}'

            def about(command, **kwargs):
                commands.append(command)
                pathlib.Path(command[command.index('--output-file') + 1]).write_text('<html><p>许可 ©</p></html>', encoding='utf-8')
                return subprocess.CompletedProcess(command, 0, '', '')

            with mock.patch.object(sys, 'argv', ['generate-notices.py', '--target', 'aarch64-pc-windows-msvc', '--output', str(destination)]), mock.patch('subprocess.check_output', metadata), mock.patch('subprocess.run', about):
                runpy.run_path(str(ROOT / 'scripts/generate-notices.py'), run_name='__main__')
            self.assertEqual(commands[0][commands[0].index('--filter-platform') + 1], 'aarch64-pc-windows-msvc')
            self.assertEqual(commands[1][commands[1].index('--target') + 1], 'aarch64-pc-windows-msvc')
            self.assertEqual(destination.read_text(encoding='utf-8'), '<html><p>许可 ©</p></html>')
            self.assertEqual((ROOT / 'packaging/ThirdPartyNotices.html').read_bytes(), original)

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
