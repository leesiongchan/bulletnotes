"""Safety contracts for the development desktop packager (no compiler required)."""
import importlib.util
import io
import json
import pathlib
import runpy
import shutil
import subprocess
import sys
import tempfile
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

    def test_development_cannot_inherit_signing_or_notary_credentials(self):
        env = self.package.development_environment({
            'PATH': '/bin', 'APPLE_ID': 'secret', 'APPLE_API_KEY_PATH': 'secret',
            'APPLE_SIGNING_IDENTITY': 'secret', 'TAURI_SIGNING_PRIVATE_KEY': 'secret',
            'BULLETNOTES_NOTARY_PROFILE': 'secret', 'CARGO_TARGET_DIR': '/other',
        })
        self.assertEqual(env['PATH'], '/bin')
        self.assertFalse(any(key.startswith(('APPLE_', 'TAURI_SIGNING_', 'BULLETNOTES_NOTARY_')) for key in env))
        self.assertNotIn('CARGO_TARGET_DIR', env)

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
