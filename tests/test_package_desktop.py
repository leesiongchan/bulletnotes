"""Safety contracts for the development desktop packager (no compiler required)."""
import importlib.util
import json
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts/package-desktop.py'


class DesktopPackagingTests(unittest.TestCase):
    def setUp(self):
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
