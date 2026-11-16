"""Docker-only verification; run from the repository root. No host GUI access."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[4]
os.chdir(root)
assert Path('/.dockerenv').is_file(), 'Docker sandbox required'
evidence = Path(__file__).resolve().parent
env = os.environ.copy()
env['PATH'] = '/opt/kestrel-rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:' + env['PATH']
env['CARGO_HOME'] = '/workspace/kestrel-swatch/cargo'
env['CARGO_TARGET_DIR'] = '/workspace/kestrel-swatch/target'
for kind in ['DATA', 'CONFIG', 'STATE']:
    env[f'XDG_{kind}_HOME'] = str(root / 'target/layout-revision' / kind.lower())
env['XDG_DATA_DIRS'] = str(root / 'target/layout-revision/system')
results = []

def run(name, args):
    result = subprocess.run(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    (evidence / (name + '.log')).write_text('$ ' + ' '.join(args) + '\n' + result.stdout + f'\nEXIT={result.returncode}\n')
    results.append({'name': name, 'command': args, 'exit': result.returncode})
    print(name, 'exit=', result.returncode, flush=True)
    return result.returncode

run('fmt-apply', ['cargo', 'fmt', '--all'])
run('fmt-check', ['cargo', 'fmt', '--all', '--', '--check'])
if run('build-final', ['cargo', 'build', '--offline', '--locked', '--workspace', '--all-targets']):
    (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    sys.exit(1)
# Rust 1.100 places these test binaries in build/<crate>/<hash>/out instead
# of deps; the existing rendering-helper search cannot ascend from that path.
# Copy only the already-built helper, never change shipped source for this.
target = Path(env['CARGO_TARGET_DIR']) / 'debug'
helper = target / 'swatch-board-theme-worker'
staged = []
for out in (target / 'build').glob('swatch-board*/**/out'):
    if out.is_dir() and any(p.is_file() and os.access(p, os.X_OK) and p.name.startswith(('swatch_board-', 'swatch_board_theme_core-')) for p in out.iterdir()):
        destination = out / helper.name
        shutil.copy2(helper, destination)
        staged.append(str(destination))
(evidence / 'helper-staging.json').write_text(json.dumps({'sha256': hashlib.sha256(helper.read_bytes()).hexdigest(), 'destinations': staged}, indent=2) + '\n')
run('regression-green', ['cargo', 'test', '--offline', '--locked', '-p', 'swatch-board', '--bin', 'swatch-board', 'layout_tests::', '--', '--nocapture', '--test-threads=1'])
run('workspace-tests', ['cargo', 'test', '--offline', '--locked', '--workspace', '--all-targets', '--no-fail-fast', '--', '--test-threads=1'])
run('clippy', ['cargo', 'clippy', '--offline', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings'])
run('diff-check', ['git', 'diff', '--check'])
(evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
print(json.dumps(results, indent=2))
sys.exit(any(item['exit'] for item in results))
