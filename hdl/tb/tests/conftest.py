"""Lets the tests in tb/tests import from tb/ (the runner and models/)."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
