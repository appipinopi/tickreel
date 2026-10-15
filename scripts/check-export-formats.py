"""Run the desktop-free, real FFmpeg export checks before publishing a payload."""
from pathlib import Path
import subprocess
import sys

result = subprocess.run([str(Path(sys.argv[1]).resolve()), '--export-self-test'], capture_output=True, text=True, encoding='utf-8', timeout=600)
print(result.stdout, result.stderr, flush=True)
assert result.returncode == 0 and '12 export formats' in result.stdout, 'Native export checks failed'
