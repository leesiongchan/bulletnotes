#!/usr/bin/env python3
"""Build development installers with Tauri's bundler, without a Tauri runtime."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

TOOLCHAIN = '1.99.1'
TAURI_VERSION = '2.12.4'
TARGETS = {
    'aarch64-apple-darwin': ('Darwin', 'arm64', ['app', 'dmg']),
    'x86_64-pc-windows-msvc': ('Windows', 'AMD64', ['nsis']),
    'aarch64-pc-windows-msvc': ('Windows', 'ARM64', ['nsis']),
    'x86_64-unknown-linux-gnu': ('Linux', 'x86_64', ['deb', 'appimage']),
    'aarch64-unknown-linux-gnu': ('Linux', 'aarch64', ['deb', 'appimage']),
}

# These libraries are loaded dynamically, so ELF dependency scanning cannot find them.
LINUX_LIBRARIES = {
    'libX11.so.6': 'libx11-6', 'libX11-xcb.so.1': 'libx11-xcb1',
    'libXcursor.so.1': 'libxcursor1', 'libXi.so.6': 'libxi6',
    'libxcb.so.1': 'libxcb1', 'libxkbcommon.so.0': 'libxkbcommon0',
    'libxkbcommon-x11.so.0': 'libxkbcommon-x11-0',
    'libwayland-client.so.0': 'libwayland-client0', 'libvulkan.so.1': 'libvulkan1',
}


def validate_release(version, package_version, ref):
    number = r'(?:0|[1-9][0-9]*)'
    identifier = r'(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)'
    semver = number + r'\.' + number + r'\.' + number
    semver += r'(?:-' + identifier + r'(?:\.' + identifier + r')*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?'
    if not re.fullmatch(semver, version) or version != package_version:
        raise ValueError('--version must be valid SemVer and match Cargo.toml')
    if ref is not None and ref != 'refs/tags/v' + version:
        raise ValueError('--release-ref must exactly equal refs/tags/v' + version)


def development_environment(source):
    # Tauri notarizes automatically when Apple credentials are present.
    environment = {key: value for key, value in source.items()
                   if not key.startswith(('APPLE_', 'TAURI_SIGNING_', 'BULLETNOTES_SIGNING_', 'BULLETNOTES_NOTARY_'))
                   and key not in ('CARGO_TARGET_DIR', 'TAURI_CONFIG')}
    environment['CI'] = 'true'
    if platform.system() == 'Darwin':
        environment['MACOSX_DEPLOYMENT_TARGET'] = '14.0'
    return environment


def run(command, **kwargs):
    print('+ ' + ' '.join(map(str, command)), flush=True)
    try:
        return subprocess.run(list(map(str, command)), check=True, **kwargs)
    except subprocess.CalledProcessError as error:
        for output in (error.stdout, error.stderr):
            if output:
                print(output, file=sys.stderr)
        raise


def capture(command, **kwargs):
    return run(command, capture_output=True, text=True, **kwargs).stdout.strip()


def sha(path):
    with pathlib.Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def write_fixture(fixture, root, version, target, notices, revision, dirty, binary):
    (fixture / 'frontend').mkdir()
    (fixture / 'frontend/index.html').write_text('<!doctype html><title>Packaging fixture</title>')
    # Cargo metadata identifies the real app; `tauri bundle` never compiles this fixture.
    manifest = '[package]\nname = "bulletnotes"\nversion = ' + json.dumps(version) + '\nedition = "2024"\n'
    manifest += '\n[[bin]]\nname = "bulletnotes"\npath = ' + json.dumps(str(root / 'src/main.rs')) + '\n'
    (fixture / 'Cargo.toml').write_text(manifest)
    config = {
        'productName': 'Bulletnotes', 'version': version, 'identifier': 'lol.harflix.bulletnotes',
        'build': {'frontendDist': './frontend'},
        'bundle': {
            'active': True, 'targets': TARGETS[target][2],
            'shortDescription': 'Bulletnotes desktop notes',
            'icon': ['icons/32x32.png', 'icons/128x128.png', 'icons/128x128@2x.png', 'icons/icon.icns', 'icons/icon.ico'],
            'resources': {str(notices): 'ThirdPartyNotices.html', str(root / 'packaging/Slint-License.md'): 'Slint-License.md'},
            'windows': {'webviewInstallMode': {'type': 'skip'}, 'nsis': {'installMode': 'currentUser'}},
        },
    }
    if target.endswith('apple-darwin'):
        info = plistlib.loads((root / 'packaging/Info.plist').read_bytes())
        info.update(CFBundleShortVersionString=version.split('-')[0].split('+')[0],
                    BulletnotesVersion=version, BulletnotesDistributionKind='development',
                    BulletnotesSourceRevision=revision + ('+dirty' if dirty else ''))
        (fixture / 'Info.plist').write_bytes(plistlib.dumps(info))
        config['bundle']['macOS'] = {
            'minimumSystemVersion': '14.0', 'signingIdentity': '-', 'hardenedRuntime': True,
            'entitlements': str(root / 'packaging/entitlements.plist'), 'infoPlist': str(fixture / 'Info.plist'),
        }
    if 'linux' in target:
        (fixture / 'debian').mkdir()
        (fixture / 'debian/control').write_text('Source: bulletnotes\n\nPackage: bulletnotes\nArchitecture: any\nDescription: Desktop notes\n')
        dependencies = capture(['dpkg-shlibdeps', '-O', '-e' + str(binary)], cwd=fixture)
        if not dependencies.startswith('shlibs:Depends='):
            raise ValueError('dpkg-shlibdeps did not identify linked dependencies')
        library_dir = pathlib.Path(capture(['pkg-config', '--variable=libdir', 'xkbcommon']))
        config['bundle']['linux'] = {
            'deb': {'depends': dependencies.removeprefix('shlibs:Depends=').split(', ') + list(LINUX_LIBRARIES.values())},
            'appimage': {'files': {
                **{'usr/lib/' + name: str(library_dir / name) for name in LINUX_LIBRARIES},
                **{'usr/share/doc/' + package + '/copyright': '/usr/share/doc/' + package + '/copyright'
                   for package in LINUX_LIBRARIES.values()},
                'usr/share/common-licenses': '/usr/share/common-licenses',
            }},
        }
    (fixture / 'tauri.conf.json').write_text(json.dumps(config, indent=2) + '\n')


def verify_binary_target(binary, target):
    # Read the platform's fixed machine field independently of the bundler configuration.
    with binary.open('rb') as stream:
        header = stream.read(64)
        if 'linux' in target:
            expected = 183 if target.startswith('aarch64') else 62
            valid = header[:6] == b'\x7fELF\x02\x01' and int.from_bytes(header[18:20], 'little') == expected
        elif 'windows' in target:
            stream.seek(int.from_bytes(header[60:64], 'little'))
            pe = stream.read(6)
            expected = 0xaa64 if target.startswith('aarch64') else 0x8664
            valid = header[:2] == b'MZ' and pe[:4] == b'PE\0\0' and int.from_bytes(pe[4:6], 'little') == expected
        else:
            return  # Mach-O is inspected with Apple's lipo/vtool below.
    if not valid:
        raise ValueError('Application binary architecture differs from ' + target)


def verify_tree(tree, version, target, notices, static_only, environment, revision, dirty):
    executable = 'bulletnotes.exe' if 'windows' in target else 'bulletnotes'
    matches = [path for path in tree.rglob(executable) if path.is_file()]
    if len(matches) != 1:
        raise ValueError('Expected one installed application binary in ' + str(tree))
    binary = matches[0]
    verify_binary_target(binary, target)
    for name, expected in [('ThirdPartyNotices.html', notices), ('Slint-License.md', ROOT / 'packaging/Slint-License.md')]:
        resources = list(tree.rglob(name))
        if len(resources) != 1 or sha(resources[0]) != sha(expected):
            raise ValueError('Missing or changed attribution resource: ' + name)
    launcher = tree / 'AppRun' if (tree / 'AppRun').is_file() else binary
    observed_version = capture([launcher, '--version'], env=environment, timeout=30)
    if observed_version not in ('bulletnotes ' + version, 'Bulletnotes ' + version):
        raise ValueError('Packaged binary version differs from release version: ' + observed_version)
    evidence = {'binary_sha256': sha(binary), 'version': observed_version,
                'launch': 'unobserved (static-only)', 'graphics_api': 'unobserved (static-only)'}
    if target.endswith('apple-darwin'):
        info = plistlib.loads((tree / 'Contents/Info.plist').read_bytes())
        expected_info = plistlib.loads((ROOT / 'packaging/Info.plist').read_bytes())
        expected_info.update(BulletnotesVersion=version, BulletnotesDistributionKind='development',
                             BulletnotesSourceRevision=revision + ('+dirty' if dirty else ''),
                             CFBundleShortVersionString=version.split('-')[0].split('+')[0])
        if any(info.get(key) != value for key, value in expected_info.items()):
            raise ValueError('Tauri bundle lost required Info.plist metadata')
        architecture = capture(['xcrun', 'lipo', '-archs', binary])
        if architecture != TARGETS[target][1]:
            raise ValueError('Bundle architecture differs from requested target')
        load = capture(['xcrun', 'vtool', '-show-build', binary])
        minimum = re.findall(r'^\s*minos\s+(\S+)', load, re.M)
        if not minimum or any(value not in ('14.0', '14.0.0') for value in minimum):
            raise ValueError('Mach-O deployment target must be 14.0')
        run(['codesign', '--verify', '--strict', '--deep', tree])
        signature = run(['codesign', '--display', '--verbose=4', tree], capture_output=True, text=True).stderr
        if 'Signature=adhoc' not in signature or not re.search(r'flags=.*runtime', signature):
            raise ValueError('Development app must have an ad-hoc hardened runtime signature')
        entitlements = capture(['codesign', '--display', '--entitlements', '-', '--xml', tree])
        if entitlements and plistlib.loads(entitlements.encode()) != {}:
            raise ValueError('Unexpected development app entitlements')
        evidence['signature'] = 'verified ad-hoc'
    if not static_only:
        observed = capture([launcher, '--verify-installation'], env=environment, timeout=60)
        if 'installation_check: true' not in observed:
            raise ValueError('Installed application did not acknowledge isolated installation check')
        graphics = re.search(r'^graphics_backend: (.+)$', observed, re.M)
        if not graphics or (target.endswith('apple-darwin') and graphics[1] != 'metal'):
            raise ValueError('Installation check did not identify the required renderer')
        evidence.update(launch='observed (isolated generated UI)', graphics_api=graphics[1])
    return evidence


def export_artifacts(artifacts, output, name, evidence):
    destinations = [(source, output / (name + source.suffix)) for source in artifacts]
    report = output / (name + '.verification.json')
    checksums = output / (name + '.sha256')
    for destination in [path for _, path in destinations] + [report, checksums]:
        if destination.exists() or destination.is_symlink():
            raise FileExistsError('Refusing to overwrite ' + str(destination))
    evidence = dict(evidence, artifacts={path.name: sha(source) for source, path in destinations})
    for source, destination in destinations:
        with source.open('rb') as incoming, destination.open('xb') as outgoing:
            shutil.copyfileobj(incoming, outgoing)
    with report.open('x') as stream:
        stream.write(json.dumps(evidence, indent=2) + '\n')
    with checksums.open('x') as stream:
        for filename, digest in evidence['artifacts'].items():
            stream.write(digest + '  ' + filename + '\n')
        stream.write(sha(report) + '  ' + report.name + '\n')
    print(json.dumps(evidence, indent=2), flush=True)


ROOT = pathlib.Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    parser.add_argument('--target', required=True, choices=TARGETS)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    parser.add_argument('--release-ref', help='Full tag ref, required to match Cargo version when supplied')
    parser.add_argument('--static-only', action='store_true', help='Record UI launch as unobserved; still check packaged --version')
    args = parser.parse_args()
    package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
    validate_release(args.version, package['version'], args.release_ref)
    system, machine, formats = TARGETS[args.target]
    if (platform.system(), platform.machine().lower()) != (system, machine.lower()):
        parser.error('Packaging requires a native ' + args.target + ' runner')
    args.output.mkdir(parents=True, exist_ok=True)
    output = args.output.resolve()
    name = 'Bulletnotes-' + args.version + '-' + args.target + '-development'
    if any(output.glob(name + '.*')):
        parser.error('Refusing to overwrite artifacts; choose a fresh output directory')
    environment = development_environment(os.environ)
    cargo = ['rustup', 'run', TOOLCHAIN, 'cargo']
    tauri = cargo + ['tauri']
    if capture(tauri + ['--version'], cwd=ROOT, env=environment) != 'tauri-cli ' + TAURI_VERSION:
        parser.error('Install the pinned tauri-cli ' + TAURI_VERSION)
    # Capture provenance before generating any packaging files.
    revision = capture(['git', 'rev-parse', 'HEAD'], cwd=ROOT)
    dirty = bool(capture(['git', 'status', '--porcelain'], cwd=ROOT))
    lock_hash = sha(ROOT / 'Cargo.lock')
    run(cargo + ['build', '--release', '--locked', '--bin', 'bulletnotes', '--target', args.target], cwd=ROOT, env=environment)
    metadata = json.loads(capture(cargo + ['metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT, env=environment))
    executable = 'bulletnotes.exe' if system == 'Windows' else 'bulletnotes'
    binary = pathlib.Path(metadata['target_directory']) / args.target / 'release' / executable
    verify_binary_target(binary, args.target)
    with tempfile.TemporaryDirectory(prefix='bulletnotes-package-') as temporary:
        fixture = pathlib.Path(temporary)
        notices = fixture / 'ThirdPartyNotices.html'
        run([sys.executable, ROOT / 'scripts/generate-notices.py', '--target', args.target, '--output', notices], cwd=ROOT, env=environment)
        write_fixture(fixture, ROOT, args.version, args.target, notices, revision, dirty, binary)
        build = fixture / 'build'
        staged = build / args.target / 'release'
        staged.mkdir(parents=True)
        shutil.copy2(binary, staged / executable)
        environment['CARGO_TARGET_DIR'] = str(build)
        run(tauri + ['icon', ROOT / 'assets/bulletnotes-logo.png', '--output', fixture / 'icons'], cwd=fixture, env=environment)
        command = tauri + ['bundle', '--target', args.target, '--ci', '--no-binary-patching']
        if system == 'Windows':
            command.append('--no-sign')
        run(command, cwd=fixture, env=environment)
        bundle = staged / 'bundle'
        artifacts = []
        observations = {}
        for kind in formats:
            suffix = {'app': '.app', 'dmg': '.dmg', 'nsis': '.exe', 'deb': '.deb', 'appimage': '.AppImage'}[kind]
            candidates = list((bundle / ('macos' if kind == 'app' else kind)).glob('*' + suffix))
            if len(candidates) != 1:
                raise ValueError('Expected one ' + kind + ' artifact')
            artifact = candidates[0]
            tree = artifact
            if kind == 'dmg':
                run(['hdiutil', 'verify', artifact])
                tree = fixture / 'mounted'
                tree.mkdir()
                run(['hdiutil', 'attach', '-readonly', '-nobrowse', '-mountpoint', tree, artifact])
                try:
                    observations[kind] = verify_tree(tree / 'Bulletnotes.app', args.version, args.target, notices, args.static_only, environment, revision, dirty)
                    if not (tree / 'Applications').is_symlink():
                        raise ValueError('DMG is missing Applications shortcut')
                finally:
                    run(['hdiutil', 'detach', tree])
            else:
                if kind == 'nsis':
                    tree = fixture / 'installed'
                    run([artifact, '/S', '/D=' + str(tree)], timeout=120, env=environment)
                elif kind == 'deb':
                    tree = fixture / 'deb-extracted'
                    dependencies = capture(['dpkg-deb', '-f', artifact, 'Depends'])
                    if not set(LINUX_LIBRARIES.values()).issubset(dependencies.split(', ')):
                        raise ValueError('Deb package is missing dynamically loaded libraries')
                    run(['dpkg-deb', '-x', artifact, tree])
                elif kind == 'appimage':
                    extracted = fixture / 'appimage-extracted'
                    extracted.mkdir()
                    run([artifact, '--appimage-extract'], cwd=extracted, capture_output=True, env=environment)
                    tree = extracted / 'squashfs-root'
                    if any(not (tree / 'usr/lib' / name).is_file() for name in LINUX_LIBRARIES):
                        raise ValueError('AppImage is missing dynamically loaded libraries')
                    if any(not (tree / 'usr/share/doc' / package / 'copyright').is_file()
                           for package in LINUX_LIBRARIES.values()) or not (tree / 'usr/share/common-licenses').is_dir():
                        raise ValueError('AppImage is missing native library license notices')
                observations[kind] = verify_tree(tree, args.version, args.target, notices, args.static_only, environment, revision, dirty)
            if kind == 'app':
                archive = fixture / 'Bulletnotes.zip'
                run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', artifact, archive])
                artifacts.append(archive)
            else:
                artifacts.append(artifact)
        export_artifacts(artifacts, output, name, {
            'kind': 'development', 'target': args.target, 'version': args.version,
            'source_revision': revision, 'source_dirty': dirty, 'cargo_lock_sha256': lock_hash,
            'source_binary_sha256': sha(binary), 'packager': 'tauri-cli ' + TAURI_VERSION,
            'launch': 'unobserved (static-only)' if args.static_only else 'observed (isolated generated UI)',
            'verification': observations,
        })


if __name__ == '__main__':
    main()
