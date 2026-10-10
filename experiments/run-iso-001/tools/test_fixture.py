#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Preparation-only contract checks: do not run RUN/ISO acceptance cases."""
import os
from pathlib import Path
import tempfile
import unittest
from fixture import generate, verify, records_bytes, aggregate_digest, entries, safe_root


class FixtureContractTest(unittest.TestCase):
    def test_counts_and_lengths(self):
        self.assertEqual(len(records_bytes(0, 0)), 8 + 5 + 8 * 6 + 6 * 512 + 2 * 4096)
        self.assertEqual(sum(1 for _ in entries()), 1025)
        self.assertEqual(aggregate_digest(entries()), aggregate_digest(entries()))

    def test_full_generation_and_reverification(self):
        with tempfile.TemporaryDirectory(prefix="tram-run-iso-001-") as raw:
            root = Path(raw)
            digest = generate(root)
            self.assertEqual(digest, verify(root))
            with self.assertRaises(ValueError):
                generate(root)
            path = root / "fixtures/run-0/unit-0000.bin"
            path.write_bytes(path.read_bytes() + b"X")
            with self.assertRaisesRegex(ValueError, "altered"):
                verify(root)

    def test_rejects_symlink_and_non_temp_root(self):
        with tempfile.TemporaryDirectory(prefix="tram-run-iso-001-") as raw:
            bad = Path(raw) / "link"
            bad.symlink_to(raw)
            with self.assertRaises(ValueError):
                safe_root(bad)
        with self.assertRaises(ValueError):
            safe_root(Path("/mnt/data"))


if __name__ == "__main__":
    unittest.main()
