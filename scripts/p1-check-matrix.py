#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify R2 frozen coverage against actually executed CI test result lines."""
import json
import re
import sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
data=json.loads((root/"docs/phase-1/r2-test-matrix.json").read_text())
required={f"COMP-{n:02}" for n in range(1,16)}
required|={f"COMP-F{n:02}" for n in range(1,9)}
required|={f"RES-{n:02}" for n in range(1,8)}
required|={f"ACK-{n:02}" for n in range(1,5)}
required|={"P1-ADMIT-01","P1-ACK-02","P1-CRASH-03","P1-FANOUT-04","P1-BOUNDS-05"}
cases=data["cases"]
ids=[case["id"] for case in cases]
assert len(ids)==len(set(ids)), "duplicate matrix IDs"
assert set(ids)==required, f"missing={sorted(required-set(ids))} unexpected={sorted(set(ids)-required)}"
assert data["architecture_sha"]=="2cedaeac5657a8941fe9366f04029cd11b0cfd30"
assert len(sys.argv)==3 and sys.argv[1]=="--executed-log", "execution log mandatory; no symbol-only pass"
logs=Path(sys.argv[2]).read_text(encoding="utf-8")
assert "test result: ok." in logs, "no successful Rust test-run result"
passed=set()
pending=None
target=None
for line in logs.splitlines():
    # Cargo announces each test executable. Keep the whole Rust module path
    # and the executable identity, never accept bare-name collisions.
    running=re.search(r"Running .*\(target/debug/deps/([A-Za-z_][A-Za-z_0-9]*)-[a-f0-9]+\)",line)
    if running:
        target=running.group(1)
        pending=None
    found=re.search(r"^test ([\w:]+) \.\.\.(.*)$",line)
    if found:
        assert target is not None, f"test name without binary identity: {line}"
        name,suffix=found.group(1),found.group(2).strip()
        identity=f"{target}::{name}"
        if suffix=="ok":
            passed.add(identity)
            pending=None
        elif suffix.startswith("ignored") or suffix.startswith("FAILED"):
            pending=None
        else:
            pending=identity
    elif line.strip()=="ok" and pending is not None:
        passed.add(pending)
        pending=None
    elif line.strip() in {"FAILED","ignored"}:
        pending=None
count=0
for case in cases:
    assert case["status"] in {"EXERCISED","P2_SIMULATED","DEFERRED_P2"}
    assert case["tests"] or case["status"]=="DEFERRED_P2"
    if case["status"]=="DEFERRED_P2":continue
    for test in case["tests"]:
        assert test in passed, f"{case['id']}: mapped test not executed successfully: {test}"
        count+=1
print(f"P1_MATRIX_CASES={len(cases)}",flush=True)
print(f"P1_MATRIX_EXERCISED={sum(c['status']=='EXERCISED' for c in cases)}",flush=True)
print(f"P1_MATRIX_P2_SIMULATED={sum(c['status']=='P2_SIMULATED' for c in cases)}",flush=True)
print(f"P1_MATRIX_EXECUTED_MAPPINGS={count}",flush=True)
