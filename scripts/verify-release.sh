#!/usr/bin/env bash
set -euo pipefail
exec python3 - "$@" <<'PYTHON'
import argparse, json, pathlib, plistlib, re, subprocess, sys
parser=argparse.ArgumentParser(description="Verify a Bulletnotes app; distribution requirements are the default")
parser.add_argument('--development',action='store_true',help='Explicitly check an ad-hoc development artifact')
parser.add_argument('--static-only',action='store_true',help='Check metadata/signature only; report native launch as unobserved')
parser.add_argument('app',type=pathlib.Path)
args=parser.parse_args()
def fail(message):
 print('Verification failed: '+message,file=sys.stderr);sys.exit(1)
def run(command,label):
 try:
  result=subprocess.run(command,capture_output=True,text=True,timeout=30)
 except (OSError,subprocess.TimeoutExpired): fail(label+' could not complete')
 if result.returncode: fail(label+' failed')
 return result
app=args.app.resolve();info=app/'Contents/Info.plist'
try:
 with info.open('rb') as stream: metadata=plistlib.load(stream)
except (OSError,plistlib.InvalidFileException):fail('bundle metadata is invalid')
if metadata.get('CFBundleIdentifier')!='lol.harflix.bulletnotes' or metadata.get('CFBundleExecutable')!='bulletnotes':fail('bundle identity is incorrect')
if metadata.get('LSMinimumSystemVersion')!='14.0':fail('deployment target in Info.plist must be 14.0')
binary=app/'Contents/MacOS/bulletnotes'
for name in ['ThirdPartyNotices.html','Slint-License.md']:
 resource=app/'Contents/Resources'/name
 if not resource.is_file() or resource.stat().st_size==0:fail('required attribution resource is missing')
architecture=run(['xcrun','lipo','-archs',str(binary)],'architecture check').stdout.strip().split()
if len(architecture)!=1 or architecture[0] not in ['arm64','x86_64']:fail('unsupported architecture')
load=run(['xcrun','vtool','-show-build',str(binary)],'deployment target inspection').stdout
minimum=re.findall(r'^\s*minos\s+(\S+)',load,re.M)
if not minimum or any(value not in ['14.0','14.0.0'] for value in minimum):fail('Mach-O deployment target must be 14.0')
run(['codesign','--verify','--strict','--deep',str(app)],'signature verification')
signature=run(['codesign','--display','--verbose=4',str(app)],'signature inspection').stderr
if not re.search(r'flags=.*runtime',signature):fail('signature must enable hardened runtime')
entitlements=run(['codesign','--display','--entitlements','-','--xml',str(app)],'entitlement inspection').stdout
try: permissions=plistlib.loads(entitlements.encode()) if entitlements.strip() else {}
except plistlib.InvalidFileException:fail('entitlements are invalid')
if permissions:fail('unexpected entitlements; minimum policy is an empty dictionary')
kind=metadata.get('BulletnotesDistributionKind')
if args.development:
 if kind!='development':fail('artifact is not marked as a development build')
else:
 if kind!='distribution':fail('development artifact is not a distribution release')
 if 'Signature=adhoc' in signature or 'Authority=Developer ID Application:' not in signature:fail('distribution signature must use Developer ID Application')
 if not re.search(r'^TeamIdentifier=[A-Z0-9]{10}$',signature,re.M):fail('distribution signature has no verified team')
 run(['xcrun','stapler','validate',str(app)],'notarization staple validation')
 run(['spctl','--assess','--type','execute',str(app)],'Gatekeeper assessment')
launch='unobserved (static-only)'
if not args.static_only:
 output=run([str(binary),'--verify-installation'],'isolated installation launch')
 if 'installation_check: true' not in output.stdout:fail('installation launch did not acknowledge its check')
 launch='passed (generated UI; no store, credentials, or network)'
print(json.dumps({'artifact':str(app),'kind':kind,'architecture':architecture[0],'minimum_macos':'14.0','signature':'verified','launch':launch},indent=2))
PYTHON
