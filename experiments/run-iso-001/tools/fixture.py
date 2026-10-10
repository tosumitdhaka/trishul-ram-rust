#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""RUN+ISO-001 deterministic, scratch-only fixture generator (not an experiment runner)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import tempfile

SEED = 0x20261010
RUNS = 2
UNITS_PER_RUN = 512
RECORDS_PER_UNIT = 8
MAGIC = b"RUNISO1\0"
DOMAIN = b"TRAM-RUN-ISO-001-FIXTURE-v1\0"


def safe_root(path: Path) -> Path:
    temp_parent = Path(tempfile.gettempdir()).resolve(strict=True)
    if path.is_symlink() or not path.is_dir():
        raise ValueError("scratch root must be an existing ordinary directory, not a symlink")
    resolved = path.resolve(strict=True)
    if resolved.parent != temp_parent or not resolved.name.startswith("tram-run-iso-001-"):
        raise ValueError("scratch root must be an immediately nested tram-run-iso-001-* temp directory")
    s = resolved.stat()
    if not stat.S_ISDIR(s.st_mode) or s.st_uid != os.getuid() or (stat.S_IMODE(s.st_mode) & 0o077):
        raise ValueError("scratch root must be owned by caller and mode 0700")
    return resolved


def records_bytes(run: int, unit: int) -> bytes:
    result = bytearray(MAGIC + struct.pack(">BHH", run, unit, RECORDS_PER_UNIT))
    for rec in range(RECORDS_PER_UNIT):
        size = 4096 if rec in (3, 7) else 512
        identity = struct.pack(">IBHH", SEED, run, unit, rec)
        payload = hashlib.shake_256(DOMAIN + identity).digest(size)
        result.extend(struct.pack(">HI", rec, size))
        result.extend(payload)
    return bytes(result)


def schedule_bytes() -> bytes:
    # Each run independently preserves ascending source-unit order; global interleave is fixed.
    schedule = {
        "schema": "run-iso-001-schedule-v1", "seed": "0x20261010",
        "runs": RUNS, "units_per_run": UNITS_PER_RUN,
        "records_per_unit": RECORDS_PER_UNIT,
        "sink_slots": ["A", "B"],
        "records_per_sink_batch": 8,
        "branch_order": ["A", "B"],
        "interleave": "round-robin-ascending-unit-run-0-first",
        "ordered_sources": [f"run-{run}/unit-{unit:04d}.bin"
                            for unit in range(UNITS_PER_RUN) for run in range(RUNS)],
        "delays_ms_per_batch": {"baseline": {"A": 0, "B": 50},
                                 "stalled_B": {"A": 0, "B": 250},
                                 "blocked_B": {"A": 0, "B": None}},
    }
    return (json.dumps(schedule, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def entries():
    yield "schedule.json", schedule_bytes()
    for run in range(RUNS):
        for unit in range(UNITS_PER_RUN):
            yield f"run-{run}/unit-{unit:04d}.bin", records_bytes(run, unit)


def aggregate_digest(rows):
    digest = hashlib.sha256(DOMAIN)
    for relative, content in rows:
        raw = relative.encode("ascii")
        digest.update(struct.pack(">I", len(raw)))
        digest.update(raw)
        digest.update(struct.pack(">Q", len(content)))
        digest.update(content)
    return digest.hexdigest()


def generate(root: Path) -> str:
    root = safe_root(root)
    if any(root.iterdir()):
        raise ValueError("root not empty; refuse partial overwrite")
    fixture_root = root / "fixtures"
    fixture_root.mkdir(mode=0o700)
    entries_list = list(entries())
    for rel, body in entries_list:
        dst = fixture_root / rel
        dst.parent.mkdir(mode=0o700, exist_ok=True)
        fd = os.open(dst, os.O_CREAT | os.O_EXCL | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(body)
    return aggregate_digest(entries_list)


def verify(root: Path) -> str:
    root = safe_root(root)
    fixture_root = root / "fixtures"
    if fixture_root.is_symlink() or not fixture_root.is_dir():
        raise ValueError("missing real fixtures directory")
    expected = list(entries())
    actual_paths = set()
    for base, dirs, files in os.walk(fixture_root, followlinks=False):
        for d in dirs:
            p = Path(base) / d
            if p.is_symlink():
                raise ValueError("symlink in fixtures")
        for name in files:
            p = Path(base) / name
            if p.is_symlink() or not p.is_file():
                raise ValueError("non-regular file in fixtures")
            actual_paths.add(p.relative_to(fixture_root).as_posix())
    if actual_paths != {k for k, _ in expected}:
        raise ValueError("unexpected or missing fixture files")
    for rel, original in expected:
        if (fixture_root / rel).read_bytes() != original:
            raise ValueError(f"fixture contents altered: {rel}")
    return aggregate_digest(expected)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("action", choices=("generate", "verify", "expected-digest"))
    p.add_argument("--root", type=Path)
    a = p.parse_args()
    if a.action != "expected-digest" and a.root is None:
        p.error("--root is required")
    value = (aggregate_digest(entries()) if a.action == "expected-digest"
             else generate(a.root) if a.action == "generate" else verify(a.root))
    print(json.dumps({"schema": "run-iso-001-fixture-digest-v1", "seed": "0x20261010",
                      "algorithm": "sha256-canonical-path-len-content-v1", "sha256": value}, sort_keys=True))


if __name__ == "__main__":
    main()
