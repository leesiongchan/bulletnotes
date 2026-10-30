#!/usr/bin/env python3
"""Build development installers with cargo-packager."""
import argparse
import contextlib
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
# mise.toml pins an unreleased cargo-packager commit, which still reports this version.
PACKAGER_VERSION = '0.11.8'
# Cargo.toml keeps this version on main; release tags are the source of truth.
PLACEHOLDER_VERSION = '0.0.0'
TARGETS = {
    'aarch64-apple-darwin': ('Darwin', 'arm64', ['app', 'dmg']),
    'x86_64-pc-windows-msvc': ('Windows', 'AMD64', ['nsis']),
    'aarch64-pc-windows-msvc': ('Windows', 'ARM64', ['nsis']),
    'x86_64-unknown-linux-gnu': ('Linux', 'x86_64', ['deb', 'appimage']),
    'aarch64-unknown-linux-gnu': ('Linux', 'aarch64', ['deb', 'appimage']),
}

# Winit and WGPU load these at runtime, so dpkg-shlibdeps cannot find them. The AppImage uses the host's copies.
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
    if not re.fullmatch(semver, version) or package_version not in (PLACEHOLDER_VERSION, version):
        raise ValueError('--version must be valid SemVer and match Cargo.toml or its ' + PLACEHOLDER_VERSION + ' placeholder')
    if ref is not None and ref != 'refs/tags/v' + version:
        raise ValueError('--release-ref must exactly equal refs/tags/v' + version)


def stamp_version(manifest, lock, version):
    # A text edit keeps Cargo.lock consistent without network access; --locked builds catch any drift.
    stamped = []
    for text, header in ((manifest, r'\[package\]'), (lock, r'\[\[package\]\]')):
        pattern = r'(?m)^(' + header + r'\r?\nname = "bulletnotes"\r?\nversion = )"' + re.escape(PLACEHOLDER_VERSION) + '"'
        text, count = re.subn(pattern, lambda match: match.group(1) + json.dumps(version), text)
        if count != 1:
            raise ValueError('Cargo.toml and Cargo.lock must hold the ' + PLACEHOLDER_VERSION + ' placeholder')
        stamped.append(text)
    return stamped


@contextlib.contextmanager
def stamped_version(root, version):
    """Build with a release version while leaving the checkout byte-for-byte unchanged."""
    paths = [root / 'Cargo.toml', root / 'Cargo.lock']
    originals = [path.read_bytes() for path in paths]
    stamped = stamp_version(*(data.decode() for data in originals), version)
    try:
        for path, text in zip(paths, stamped):
            path.write_bytes(text.encode())
        yield
    finally:
        for path, data in zip(paths, originals):
            path.write_bytes(data)


def development_environment(source):
    # cargo-packager notarizes automatically when Apple credentials are present.
    environment = {key: value for key, value in source.items()
                   if not key.startswith(('APPLE_', 'CARGO_PACKAGER_', 'BULLETNOTES_SIGNING_', 'BULLETNOTES_NOTARY_'))
                   and key != 'CARGO_TARGET_DIR'}
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


def write_packaging_inputs(inputs, target, binary, environment):
    """Generate the per-target files that Cargo.toml's packager config reads from target/packaging."""
    inputs.mkdir(parents=True, exist_ok=True)
    notices = inputs / 'ThirdPartyNotices.html'
    run([sys.executable, ROOT / 'scripts/generate-notices.py', '--target', target, '--output', notices], cwd=ROOT, env=environment)
    if 'linux' in target:
        with tempfile.TemporaryDirectory(prefix='bulletnotes-shlibdeps-') as temporary:
            # dpkg-shlibdeps requires a debian/control file in its working directory.
            (pathlib.Path(temporary) / 'debian').mkdir()
            (pathlib.Path(temporary) / 'debian/control').write_text('Source: bulletnotes\n\nPackage: bulletnotes\nArchitecture: any\nDescription: Desktop notes\n')
            dependencies = capture(['dpkg-shlibdeps', '-O', '-e' + str(binary)], cwd=temporary)
        if not dependencies.startswith('shlibs:Depends='):
            raise ValueError('dpkg-shlibdeps did not identify linked dependencies')
        depends = dependencies.removeprefix('shlibs:Depends=').split(', ') + list(LINUX_LIBRARIES.values())
        (inputs / 'deb-depends.txt').write_text('\n'.join(depends) + '\n')
    return notices


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


def verify_tree(tree, version, target, notices, static_only, environment):
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
        expected_info.update(CFBundleShortVersionString=version)
        if any(info.get(key) != value for key, value in expected_info.items()):
            raise ValueError('App bundle lost required Info.plist metadata')
        if not (tree / 'Contents/Resources' / info['CFBundleIconFile']).is_file():
            raise ValueError('App bundle is missing its Info.plist icon')
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
    packager = cargo + ['packager']
    if capture(packager + ['--version'], cwd=ROOT, env=environment) != 'cargo-packager ' + PACKAGER_VERSION:
        parser.error('Install the pinned cargo-packager ' + PACKAGER_VERSION)
    # Capture provenance before generating any packaging files.
    revision = capture(['git', 'rev-parse', 'HEAD'], cwd=ROOT)
    dirty = bool(capture(['git', 'status', '--porcelain'], cwd=ROOT))
    lock_hash = sha(ROOT / 'Cargo.lock')
    stamping = package['version'] == PLACEHOLDER_VERSION != args.version
    if stamping:
        print('Stamping Cargo version ' + args.version + ' over the ' + PLACEHOLDER_VERSION + ' placeholder', flush=True)
    executable = 'bulletnotes.exe' if system == 'Windows' else 'bulletnotes'
    with tempfile.TemporaryDirectory(prefix='bulletnotes-package-') as temporary:
        staging = pathlib.Path(temporary)
        bundle = staging / 'packages'
        # cargo-packager reads the stamped version from Cargo.toml, so it runs inside the stamp too.
        with stamped_version(ROOT, args.version) if stamping else contextlib.nullcontext():
            run(cargo + ['build', '--release', '--locked', '--bin', 'bulletnotes', '--target', args.target], cwd=ROOT, env=environment)
            metadata = json.loads(capture(cargo + ['metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT, env=environment))
            binary = pathlib.Path(metadata['target_directory']) / args.target / 'release' / executable
            verify_binary_target(binary, args.target)
            notices = write_packaging_inputs(ROOT / 'target/packaging', args.target, binary, environment)
            # cargo-packager also packages any packager.{json,toml} under its working directory, so run it from an empty one.
            run(packager + ['--release', '--target', args.target, '--formats', ','.join(formats), '--out-dir', bundle,
                            '--manifest-path', ROOT / 'Cargo.toml'], cwd=staging, env=environment)
        artifacts = []
        observations = {}
        for kind in formats:
            suffix = {'app': '.app', 'dmg': '.dmg', 'nsis': '.exe', 'deb': '.deb', 'appimage': '.AppImage'}[kind]
            candidates = list(bundle.glob('*' + suffix))
            if len(candidates) != 1:
                raise ValueError('Expected one ' + kind + ' artifact')
            artifact = candidates[0]
            tree = artifact
            if kind == 'dmg':
                run(['hdiutil', 'verify', artifact])
                tree = staging / 'mounted'
                tree.mkdir()
                run(['hdiutil', 'attach', '-readonly', '-nobrowse', '-mountpoint', tree, artifact])
                try:
                    observations[kind] = verify_tree(tree / 'Bulletnotes.app', args.version, args.target, notices, args.static_only, environment)
                    if not (tree / 'Applications').is_symlink():
                        raise ValueError('DMG is missing Applications shortcut')
                finally:
                    run(['hdiutil', 'detach', tree])
            else:
                if kind == 'nsis':
                    tree = staging / 'installed'
                    run([artifact, '/S', '/D=' + str(tree)], timeout=120, env=environment)
                elif kind == 'deb':
                    tree = staging / 'deb-extracted'
                    dependencies = capture(['dpkg-deb', '-f', artifact, 'Depends'])
                    if not set(LINUX_LIBRARIES.values()).issubset(dependencies.split(', ')):
                        raise ValueError('Deb package is missing dynamically loaded libraries')
                    run(['dpkg-deb', '-x', artifact, tree])
                elif kind == 'appimage':
                    extracted = staging / 'appimage-extracted'
                    extracted.mkdir()
                    run([artifact, '--appimage-extract'], cwd=extracted, capture_output=True, env=environment)
                    tree = extracted / 'squashfs-root'
                observations[kind] = verify_tree(tree, args.version, args.target, notices, args.static_only, environment)
            if kind == 'app':
                archive = staging / 'Bulletnotes.zip'
                run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', artifact, archive])
                artifacts.append(archive)
            else:
                artifacts.append(artifact)
        export_artifacts(artifacts, output, name, {
            'kind': 'development', 'target': args.target, 'version': args.version,
            'source_revision': revision, 'source_dirty': dirty, 'cargo_lock_sha256': lock_hash,
            'source_binary_sha256': sha(binary), 'packager': 'cargo-packager ' + PACKAGER_VERSION,
            'launch': 'unobserved (static-only)' if args.static_only else 'observed (isolated generated UI)',
            'verification': observations,
        })


if __name__ == '__main__':
    main()
