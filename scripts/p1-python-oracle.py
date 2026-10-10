# SPDX-License-Identifier: Apache-2.0
"""Run the exact pinned Python reference ONLY against disposable oracle inputs."""
from __future__ import annotations
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import sys
import tempfile

PIN = "ff380725b86c9569901ea89ad6771623847cc4e2"
ROOT = pathlib.Path(__file__).resolve().parents[1]
REF = ROOT / "python-reference"
FIXTURES = ROOT / "docs/phase-0/fixtures/p1"

def records(directory: pathlib.Path) -> list[dict]:
    found = sorted(directory.glob("*.json"))
    if not found:
        raise AssertionError(f"Python oracle produced no JSON artifacts in {directory.name}")
    result: list[dict] = []
    for path in found:
        data = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(data, list):
            result.extend(data)
        elif isinstance(data, dict):
            result.append(data)
        else:
            raise AssertionError("Python reference emitted unsupported JSON shape")
    return result

def main() -> None:
    sha = subprocess.check_output(["git", "-C", str(REF), "rev-parse", "HEAD"], text=True).strip()
    assert sha == PIN, f"Python oracle pin drift: {sha}"
    print("PYTHON_ORACLE_SHA", sha, flush=True)
    print("PYTHON_ORACLE_INTERPRETER", platform.python_version(), flush=True)
    from tram.pipeline.executor import PipelineExecutor
    from tram.pipeline.loader import load_pipeline_from_yaml
    with tempfile.TemporaryDirectory(prefix="tram-p1-python-oracle-") as dirname:
        root = pathlib.Path(dirname)
        src, dst_a, dst_b = root / "in", root / "scratch-a", root / "scratch-b"
        src.mkdir()
        dst_a.mkdir()
        dst_b.mkdir()
        input_path = src / "input.json"
        original = (FIXTURES / "input.json").read_bytes()
        input_path.write_bytes(original)
        before = input_path.stat()
        os.environ.update({
            "TRAM_P1_INPUT_DIR": str(src),
            "TRAM_P1_SCRATCH_A": str(dst_a),
            "TRAM_P1_SCRATCH_B": str(dst_b),
        })
        text = (FIXTURES / "pipeline.yaml").read_text(encoding="utf-8")
        plan = load_pipeline_from_yaml(text)
        run = PipelineExecutor().batch_run(plan)
        status = str(getattr(run.status, "value", run.status))
        print("PYTHON_ORACLE_STATUS", status, flush=True)
        if status != "success":
            raise AssertionError(f"Python oracle returned non-success: {status}")
        found_a, found_b = records(dst_a), records(dst_b)
        expect_a = json.loads((FIXTURES / "expected-a.json").read_text(encoding="utf-8"))
        expect_b = json.loads((FIXTURES / "expected-b.json").read_text(encoding="utf-8"))
        print("PYTHON_ORACLE_OUTPUT_A", json.dumps(found_a, sort_keys=True), flush=True)
        print("PYTHON_ORACLE_OUTPUT_B", json.dumps(found_b, sort_keys=True), flush=True)
        assert found_a == expect_a, "Python A semantic tree diverges"
        assert found_b == expect_b, "Python B semantic tree diverges"
        after = input_path.stat()
        assert input_path.read_bytes() == original, "Python oracle modified disposable input bytes"
        print("PYTHON_ORACLE_INPUT_SHA256", hashlib.sha256(original).hexdigest(), flush=True)
        print("PYTHON_ORACLE_INPUT_MTIME_UNCHANGED", before.st_mtime_ns == after.st_mtime_ns, flush=True)
        print("PYTHON_ORACLE_SEMANTIC_GOLDEN_PASS", flush=True)

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print("PYTHON_ORACLE_BLOCKED_OR_FAIL", type(exc).__name__, str(exc), flush=True)
        raise
