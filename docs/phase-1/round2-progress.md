# Phase 1 Round 2 — Implementor evidence and open gates

**Status: PARTIAL / REMEDIATION_REQUIRED.** This is implementor evidence, not independent SOURCE_REVIEW, EXECUTOR verification, an ORCHESTRATOR acceptance, release authority, or permission to merge. Authorized at Issue #2 comment 6093717171, start ccc0c64094d6112ffd0245bc5fff72277c39a328, frozen P0 design 2cedaeac5657a8941fe9366f04029cd11b0cfd30.

## Scope implemented

- Linux-only disposable trusted-root TestHarness; cap-std directory-relative access to input and newly created run-scoped scratch, exact source/sink plan path admission, static filename checks, source read-only, symlink rejection tests, exclusive new scratch files, source-unit SHA256 and per-record lineage.
- Bounded JSON UTF-8 object/list/empty decoder with exact integer/u64/BigInteger preservation, checked depth and tokens, Python ensure_ascii-compatible array encoding, and unsupported wire shapes refused; four allowed stateless transforms and isolated sink branch cloning. No execution is offered outside harness.
- Move-only BudgetLedger reservations with exact frozen limit constants, fan-out pre-reservation, optional lowered test caps. Pure ACK-01..04 simulator with no actual source acknowledgement interface.
- Tests for golden A/B output, unchanged input bytes/metadata, 4MiB+1, 33 JSON nesting levels, >100000 tokens, 4097 records, >100 source entries, sorted sources, symlink/outside path, abort and cancel behavior, partial sink A / failed sink B, stalled sink cancellation and actual OS SIGKILL after scratch A followed by an independent run.

## Exact previously verified SHA evidence

Actions run 38025311706, Rust job 114134803461 and isolated Python job 114134803538, on ebd8c4b8e3dd8bb8e5eada16476b91686703f0da: SUCCESS. Rust 1.88.0; pinned four Phase 0 fixture hashes, cargo metadata --locked, cargo fmt --check, strict clippy --locked -- -D warnings, workspace **68/68**, focused config **11/11**, focused engine **37/37**, focused resource **9/9**, and isolated OS-process SIGKILL test **1/1** PASS. The focused tests are reruns of workspace cases, not additional unique tests.

Python oracle: exact reference tosumitdhaka/trishul-ram@ff380725b86c9569901ea89ad6771623847cc4e2, Python 3.13.16 in a disposable venv, status success; semantic A=[{"cell_id":"A","double_metric":24}] and B=[{"cell_id":"A","double_metric":24,"tag":"secondary"}] match Rust's golden trees. Input fingerprint SHA256 d6701e737f09ae3559f775a695fc8639b46d5b7f2d6b8cdd73dbe21a1b3a3353 and mtime unchanged. Only semantic golden parity was actually established, not full Python behavior or bytes parity.

OS process evidence emitted P1_CRASH_03_OS_KILL=SIGKILL_9, INPUT_UNCHANGED=true, INDEPENDENT_NEW_RUN=true. Scratch A can remain when child killed before sink B; no replay/deduplication/recovery/durable receipt inference.

Measured Linux /proc/self/status RSS for the **specific 128KiB branch-copy stress**: 6760 KiB before and 6860 KiB after; this is **not maximum process RSS**. Ledger peak counters for same test: raw_bytes=4194304, raw_buffers=1, decoded_bytes=16777216, records=0, branch_bytes=[8388608,8388608], sink_io=1, live_bytes=41943040, scratch_bytes=131139, scratch_artifacts=1. These numbers are reservation counters (several conservatively fixed) rather than observed allocation-by-allocation memory.

## Bounded remediation still required before Round 2 COMPLETE

1. Integrate actual hierarchical reservations before raw allocations, decoded Datum graph materialization, record/transform growth, full-fanout clone, encoding and scratch I/O; accurately account parent/branch occupancy and release on every error/cancel. The current fixed 40MiB live, 16MiB decoded and 8MiB per branch reservations do not prove total heap cap or backpressure. Record count peak 0 reveals that runtime records are not yet charged. Instrument peak process RSS during concurrent worst-case stress; before/after samples alone are insufficient.
2. Prove cap-root protection under adversarial concurrent directory/path swaps, final-component races, existing targets and cross-filesystem aliases. The current static symlink and out-of-root tests are useful but not complete OS authority/fault proof. Non-Linux platforms deliberately fail compilation rather than using unsafe fallback.
3. Complete full COMP-02..15 / COMP-F01..08 / RES-01..07 acceptance matrix (including true scratch disk full / 101st scratch artifact, simultaneous 16MiB decoded/64MiB total, queue stalls and source-unit failure disposition, all early return reservation release paths). Current coverage is representative and cannot be called exhaustive. RES-04 P2 expansion simulation remains unexecuted. Pure logical ACK simulator must not be mistaken for a delivery receipt.
4. Cargo.lock resolves 51 external registry crates after cap-std=3.4.5 and sha2=0.10.9. Version, registry checksum and SPDX inventory is in docs/phase-1/provenance-and-dependencies.md. Upstream per-crate LICENSE/NOTICE and advisories need independent review; unicode-ident has conjunctive Unicode-3.0 and version_check has legacy license string MIT/Apache-2.0.

No source ack/nack/finalization, filesystem production publication, journal/DB/outbox, durability, network service, SNMP, dynamic plugin loading, manager/worker, production release, merge or Round 2 gate acceptance. Keep PR #3 draft; next action REMEDIATION_REQUIRED and then independent SOURCE_REVIEW, separate EXECUTOR and explicit ORCHESTRATOR gate.
