"""Keep model sources and all generated artifacts in separate directories."""
from pathlib import Path

MODEL_ROOT = Path(__file__).resolve().parent
REPO_ROOT = MODEL_ROOT.parents[1]
OUTPUT_ROOT = REPO_ROOT / "target" / "pack-model"


def output_path(name):
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    return OUTPUT_ROOT / name
