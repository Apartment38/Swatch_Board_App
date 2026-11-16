"""Save exact source diff and computed verification totals, without Git writes."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[4]
evidence = Path(__file__).resolve().parent
tracked = ['rust/board/src/main.rs', 'rust/board/src/mixer.rs', 'rust/board/src/palette.rs']
diff = subprocess.check_output(['git', 'diff', '--', *tracked], cwd=root)
test_diff = subprocess.run(['git', 'diff', '--no-index', '--', '/dev/null', 'rust/board/src/layout_tests.rs'], cwd=root, stdout=subprocess.PIPE, check=False)
assert test_diff.returncode == 1
diff += test_diff.stdout
(evidence / 'candidate-source.diff').write_bytes(diff)
results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (evidence / 'workspace-tests.log').read_text())
summary = {
    'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'source_diff_definition': 'git diff of main.rs, mixer.rs, palette.rs against HEAD followed by git diff --no-index /dev/null rust/board/src/layout_tests.rs; includes inherited uncommitted swatch-actions work',
    'candidate_source_diff_sha256': hashlib.sha256(diff).hexdigest(),
    'workspace_tests': {key: sum(int(row[index]) for row in results) for index, key in enumerate(['passed', 'failed', 'ignored'])},
    'binary': '/workspace/kestrel-swatch/target/debug/swatch-board',
    'binary_sha256': hashlib.sha256(Path('/workspace/kestrel-swatch/target/debug/swatch-board').read_bytes()).hexdigest(),
    'checks': json.loads((evidence / 'results.json').read_text()),
}
(evidence / 'manifest.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
