#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""CI fails closed if an R2 frozen acceptance ID or named test disappears."""
import json
from pathlib import Path
import re
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
contents="\n".join(p.read_text(encoding="utf-8") for p in
    (root/"crates").rglob("*.rs"))
for case in cases:
    assert case["status"] in {"EXERCISED","P2_SIMULATED","DEFERRED_P2"}
    assert case["tests"] or case["status"]=="DEFERRED_P2"
    for test in case["tests"]:
        assert re.search(r"\bfn\s+"+re.escape(test)+r"\s*\(",contents),f"{case['id']}: missing {test}"
assert data["architecture_sha"]=="2cedaeac5657a8941fe9366f04029cd11b0cfd30"
print(f"P1_MATRIX_CASES={len(cases)}",flush=True)
print(f"P1_MATRIX_EXERCISED={sum(c['status']=='EXERCISED' for c in cases)}",flush=True)
print(f"P1_MATRIX_P2_SIMULATED={sum(c['status']=='P2_SIMULATED' for c in cases)}",flush=True)
