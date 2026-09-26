"""Portable entry point for the independent Pack proposal checks."""
import argparse
import os
import subprocess
import sys

# Set this before importing local helpers; child interpreters also use -B.
sys.dont_write_bytecode = True

from paths import MODEL_ROOT, OUTPUT_ROOT, REPO_ROOT, output_path


def run_step(name, arguments):
    command = [sys.executable, "-B", *arguments]
    env = os.environ.copy()
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    print(f"Running {name}...", flush=True)
    result = subprocess.run(command, cwd=REPO_ROOT, env=env,
                            capture_output=True, text=True, timeout=300)
    output = result.stdout + result.stderr
    log = output_path(f"{name}.log")
    log.write_text(output, encoding="utf-8")
    print(output, end="" if output.endswith("\n") else "\n", flush=True)
    if result.returncode:
        print(f"{name} failed; log: {log}", file=sys.stderr)
        raise SystemExit(result.returncode)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--eval", metavar="EXPRESSION",
                        help="evaluate one proposal expression instead of running the suite")
    args = parser.parse_args()
    if sys.version_info < (3, 10):
        parser.error("Python 3.10 or newer is required")
    if args.eval is not None:
        run_step("expression", [str(MODEL_ROOT / "syntax.py"), args.eval])
        return
    run_step("unit", ["-m", "unittest", "discover", "-s", str(MODEL_ROOT),
                      "-p", "test_model.py", "-v"])
    run_step("exhaustive", [str(MODEL_ROOT / "exhaustive.py")])
    run_step("validation", [str(MODEL_ROOT / "validate.py")])
    print(f"All Pack proposal checks passed. Artifacts: {OUTPUT_ROOT}")


if __name__ == "__main__":
    main()
