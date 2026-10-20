#!/usr/bin/env bash
set -euo pipefail
app_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$app_root"
python3 scripts/prepare-slint.py
exec python3 - "$@" <<'PYTHON'
import argparse, hashlib, json, os, pathlib, platform, plistlib, re, shutil, subprocess, sys, tempfile
parser=argparse.ArgumentParser(description="Build locked Apple Silicon macOS 14 bundles; nothing is published")
parser.add_argument('--version',required=True)
parser.add_argument('--output',type=pathlib.Path,required=True)
parser.add_argument('--development',action='store_true',help='Explicit ad-hoc signing, no notary upload')
parser.add_argument('--notarize',action='store_true',help='Explicitly upload distribution archives to Apple notary service')
args=parser.parse_args()
if args.development and args.notarize:parser.error('development artifacts cannot be notarized')
def run(command,**kwargs):return subprocess.run(command,check=True,**kwargs)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
if platform.system()!='Darwin':parser.error('macOS with Xcode CLI tools is required')
root=pathlib.Path.cwd()
metadata=json.loads(subprocess.check_output(['rustup','run','1.99.1','cargo','metadata','--no-deps','--locked','--format-version','1']))
package=next(package for package in metadata['packages'] if package['name']=='bulletnotes')
if args.version!=package['version']:parser.error('--version must match Cargo.toml and the compiled About version')
match=re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?',args.version)
if not match:parser.error('invalid semantic version')
identity=os.environ.get('BULLETNOTES_SIGNING_IDENTITY')
profile=os.environ.get('BULLETNOTES_NOTARY_PROFILE')
if not args.development:
 if not identity or not profile:parser.error('distribution requires BULLETNOTES_SIGNING_IDENTITY and BULLETNOTES_NOTARY_PROFILE from the release environment')
 if not args.notarize:parser.error('distribution requires explicit --notarize (uploads to Apple only)')
build_number=os.environ.get('BULLETNOTES_BUILD_NUMBER','1')
if not re.fullmatch(r'[1-9][0-9]{0,3}',build_number):parser.error('BULLETNOTES_BUILD_NUMBER must be 1–9999')
args.output.mkdir(parents=True,exist_ok=True);output=args.output.resolve()
kind='development' if args.development else 'distribution'
for architecture in ['arm64']:
 for suffix in ['.app','.zip','.verification.json']:
  if (output/('Bulletnotes-'+args.version+'-'+architecture+'-'+kind+suffix)).exists():parser.error('refusing to overwrite an existing artifact; choose a fresh output directory')
# Attribution generation fails rather than quietly dropping an unaccepted license.
run(['python3','scripts/generate-notices.py'])
notices=root/'packaging/ThirdPartyNotices.html'
if not notices.is_file() or notices.stat().st_size==0:parser.error('runtime attribution generation produced no resource')
environment=os.environ.copy();environment['MACOSX_DEPLOYMENT_TARGET']='14.0'
# Use the selected Xcode SDK explicitly.
environment['SDKROOT']=subprocess.check_output(['xcrun','--sdk','macosx','--show-sdk-path'],text=True).strip()
source=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
source_dirty=bool(subprocess.check_output(['git','status','--porcelain'],text=True).strip())
if source_dirty and not args.development:parser.error('distribution requires a clean committed worktree')
lock_hash=sha(root/'Cargo.lock')
for target,architecture in [('aarch64-apple-darwin','arm64')]:
 run(['rustup','run','1.99.1','cargo','build','--release','--locked','--bin','bulletnotes','--target',target],env=environment)
 name='Bulletnotes-'+args.version+'-'+architecture+'-'+kind
 with tempfile.TemporaryDirectory(prefix='.bulletnotes-package-',dir=output) as temporary:
  app=pathlib.Path(temporary)/(name+'.app');mac=app/'Contents/MacOS';resources=app/'Contents/Resources'
  mac.mkdir(parents=True);resources.mkdir()
  binary=pathlib.Path(metadata['target_directory'])/target/'release/bulletnotes'
  shutil.copy2(binary,mac/'bulletnotes')
  with (root/'packaging/Info.plist').open('rb') as stream:info=plistlib.load(stream)
  info['CFBundleShortVersionString']='.'.join(match.group(i) for i in [1,2,3]);info['CFBundleVersion']=build_number
  info['BulletnotesVersion']=args.version;info['BulletnotesDistributionKind']=kind;info['BulletnotesSourceRevision']=source+('+dirty' if source_dirty else '')
  with (app/'Contents/Info.plist').open('wb') as stream:plistlib.dump(info,stream)
  for filename in ['ThirdPartyNotices.html','Slint-License.md']:shutil.copy2(root/'packaging'/filename,resources/filename)
  signing=['codesign','--force','--sign','-' if args.development else identity,'--options','runtime','--entitlements','packaging/entitlements.plist']
  signing.append('--timestamp=none' if args.development else '--timestamp');run(signing+[str(app)])
  archive=pathlib.Path(temporary)/(name+'.zip')
  run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(app),str(archive)])
  if not args.development:
   submission=run(['xcrun','notarytool','submit',str(archive),'--keychain-profile',profile,'--wait','--output-format','json'],capture_output=True,text=True)
   status=json.loads(submission.stdout)
   if status.get('status')!='Accepted':sys.exit('Apple notarization did not accept the artifact; inspect its notary log before retrying')
   run(['xcrun','stapler','staple',str(app)]);archive.unlink()
   run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(app),str(archive)])
  command=[str(root/'scripts/verify-release.sh')]
  if args.development:command.append('--development')
  if platform.machine()!='arm64':command.append('--static-only')
  command.append(str(app))
  verified=run(command,capture_output=True,text=True);evidence=json.loads(verified.stdout)
  evidence.update({'source_revision':source,'source_dirty':source_dirty,'cargo_lock_sha256':lock_hash,'binary_sha256':sha(mac/'bulletnotes'),'archive_sha256':sha(archive)})
  final=output/app.name;shutil.move(str(app),final);shutil.move(str(archive),output/archive.name)
  evidence['artifact']=str(final)
  (output/(name+'.verification.json')).write_text(json.dumps(evidence,indent=2)+'\n')
  print(json.dumps(evidence,indent=2),flush=True)
print('Artifacts built locally. No publication or distribution was performed.')
PYTHON
