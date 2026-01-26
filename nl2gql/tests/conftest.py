import sys
from pathlib import Path

# Ensure project root is on path for imports when running pytest from repo root
ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))



