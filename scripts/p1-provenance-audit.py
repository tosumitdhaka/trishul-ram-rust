#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Exact Cargo.lock checksum/license-text inventory and advisory-candidate scan.

Engineering evidence ONLY, never SPDX/legal clearance or vulnerability
non-affectability assertion. Accepts only downloaded crate archives from
Cargo's registry cache; no execution/extraction of upstream code.
"""
import datetime
import glob
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "p1-dependency-evidence.json"
DB = Path("/tmp/tram-p1-rustsec-advisory-db")


def advisories_for(names: set[str]):
    result = {name: [] for name in names}
    if not DB.is_dir():
        return {"status": "UNAVAILABLE", "sha": None, "candidates": result}
    sha = subprocess.check_output(["git", "-C", str(DB), "rev-parse", "HEAD"], text=True).strip()
    for path in sorted((DB / "crates").rglob("*.md")):
        raw = path.read_text(encoding="utf-8")
        if not raw.startswith("+++\n"):
            continue
        end = raw.find("\n+++", 4)
        if end < 0:
            continue
        try:
            body = tomllib.loads(raw[4:end])
        except (ValueError, TypeError):
            continue
        advisory = body.get("advisory", {})
        pkg = advisory.get("package", path.parent.name)
        if pkg in names:
            result[pkg].append({
                "id": advisory.get("id", path.stem),
                "informational": advisory.get("informational"),
                "withdrawn": advisory.get("withdrawn"),
                "patched": body.get("versions", {}).get("patched", []),
                "unaffected": body.get("versions", {}).get("unaffected", []),
                "note": "Candidate by crate name only; patched ranges require independent RustSec-aware evaluation",
            })
    return {"status": "SNAPSHOT_CANDIDATES_NOT_AFFECTABILITY", "sha": sha, "candidates": result}


def main():
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    metadata = json.loads(subprocess.check_output([
        "cargo", "+1.88.0", "metadata", "--locked", "--format-version", "1"
    ], text=True))
    declared = {(p["name"], p["version"]): p for p in metadata["packages"] if p["source"]}
    crates = []
    critical = []
    for pkg in lock["package"]:
        if not pkg.get("source"):
            continue
        name, ver = pkg["name"], pkg["version"]
        meta = declared.get((name, ver))
        if not meta:
            critical.append(f"missing_metadata:{name}:{ver}")
            continue
        archives = sorted(glob.glob(str(Path.home() / ".cargo/registry/cache/*" / f"{name}-{ver}.crate")))
        if len(archives) != 1:
            critical.append(f"missing_or_ambiguous_archive:{name}:{ver}:{len(archives)}")
            crates.append({"name": name, "version": ver, "archive": "UNAVAILABLE"})
            continue
        digest = hashlib.sha256(Path(archives[0]).read_bytes()).hexdigest()
        if digest != pkg["checksum"]:
            critical.append(f"checksum_mismatch:{name}:{ver}")
        licenses = []
        with tarfile.open(archives[0], "r:gz") as archive:
            for member in archive.getmembers():
                rel = member.name.rsplit("/", 1)[-1]
                if not member.isfile() or member.size > 1_000_000:
                    continue
                if re.match(r"^(LICENSE|COPYING|NOTICE|COPYRIGHT)(?:[-_.].*)?$", rel, re.I):
                    content = archive.extractfile(member)
                    if content is not None:
                        licenses.append({
                            "path": member.name,
                            "sha256": hashlib.sha256(content.read()).hexdigest(),
                            "size": member.size,
                        })
        crates.append({
            "name": name, "version": ver, "registry": pkg["source"],
            "archive_sha256": digest, "lock_sha256": pkg["checksum"],
            "checksum_match": digest == pkg["checksum"],
            "license_expression": meta.get("license"),
            "declared_license_file": meta.get("license_file"),
            "license_notice_files": licenses,
            "missing_license_or_notice": len(licenses) == 0,
        })
    rustsec = advisories_for({pkg["name"] for pkg in crates})
    issues = [
        {"crate": x["name"], "version": x["version"],
         "issue": "License notice archive requires independent approval"}
        for x in crates if x.get("missing_license_or_notice")
    ]
    issues += [
        {"crate": x["name"], "version": x["version"],
         "issue": "Non-canonical/compound SPDX expression requires independent interpretation"}
        for x in crates if x.get("license_expression") and
            ("/" in x["license_expression"] or "Unicode-3.0" in x["license_expression"])
    ]
    issues += [
        {"crate": name, "version": "*",
         "issue": "RustSec advisory candidate; evaluate version/patched ranges independently"}
        for name, records in rustsec["candidates"].items() if records
    ]
    evidence = {
        "generated_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "rust_toolchain": "1.88.0",
        "scope": "P1 non-distributable harness; not legal, redistribution or security approval",
        "status": "INDEPENDENT_DEPENDENCY_REVIEW_REQUIRED",
        "locked_packages": len(crates),
        "critical_validation_errors": critical,
        "crates": crates,
        "rustsec_advisory_db": rustsec,
        "open_issues": issues,
    }
    REPORT.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    print("P1_PROVENANCE_SNAPSHOT_UTC", evidence["generated_utc"])
    print("P1_PROVENANCE_PACKAGES", len(crates))
    print("P1_PROVENANCE_CHECKSUM_FAILURES", len(critical))
    print("P1_PROVENANCE_MISSING_LICENSE_TEXTS", sum(x.get("missing_license_or_notice", False) for x in crates))
    print("P1_PROVENANCE_RUSTSEC_DB_SHA", rustsec["sha"])
    print("P1_PROVENANCE_ADVISORY_CANDIDATES", sum(map(len, rustsec["candidates"].values())))
    print("P1_PROVENANCE_INDEPENDENT_REVIEW_REQUIRED", len(issues))
    if critical:
        raise SystemExit("Fail-closed incomplete/mismatched registry archive inventory")


if __name__ == "__main__":
    main()
