# P0-R1 — Phase 1 execution safety boundary

**Review state:** DESIGN_OWNER remediation candidate; normative only if Phase 0 is subsequently accepted. **Scope:** P1 is an isolated functional test harness, not a deployable/production TRAM execution service.

## Decision: P1_EPHEMERAL_ONLY

P1 MUST NOT implement or advertise durable admission, source acknowledgement, confirmed publication, transactional state, crash recovery, persistent run history, "strict" or at-least-once delivery. P1 has **no manager, network agent or persistent worker journal**. The engine is exercised with actual files **only inside disposable test-controlled directories**. The P2 implementation gate is blocked until durable ledger/admission, local sink publication and replay semantics in [source-units.md](source-units.md) and [attempt-protocol.md](attempt-protocol.md) have conformance tests.

### Exact permitted side effects

1. **Run admission:** a P1 `TestHarness::start(plan, test_root)` capability, not a worker protocol/lease, grants one ephemeral in-process execution. Admission MUST succeed only if the plan passes P1 compatibility/schema/resource validation, source resolves inside read-only `test_root/in`, sink resolves to a fresh unique run-scoped `test_root/scratch/<run-id>` with exclusive creation, and all destinations are confirmed non-production. No manager or remote dispatch is exposed.
2. **Source:** file reads/stat and content hash only; **never** mark-processed, move, rename, unlink, truncate, chmod or otherwise finalize source input. `skip_processed=true`, `move_after_read`, `delete_after_read`, `file_done_suffix` or any destructive/acknowledgement feature is rejected at validation. Ignore neither these nor unknown settings.
3. **Sink:** may write *only* new, exclusively created scratch artifacts inside its allowed run directory. No append to preexisting file, overwrite, cross-filesystem targets, symlink traversal, external path/URL, inherited file handles or user-chosen arbitrary output destinations. Scratch artifact is never promoted to a user/production final path. After any run, scratch output may be inspected in tests; its existence is **not** a durable receipt.
4. **Result:** state labels are `ephemeral_completed`, `ephemeral_failed`, `ephemeral_cancelled` or `ephemeral_unknown`. They describe the local test, **never** `delivered`, `fsynced_local`, `committed`, `source_acked` or `durable_completion`. A successful write to a test file does not prove an end-to-end delivery guarantee.
5. **Crash:** scratch output may be incomplete, duplicated or orphaned; the input remains untouched. No automatic resume, dedupe, checkpoint, output replay or cleanup guarantee is claimed. A new test run uses a new run-scoped scratch directory; a stale one is inspectable and cleaned only by explicit test fixture teardown.
6. **Failure/cancel:** abort new reads/expansions, signal cancellation, resolve outstanding scratch write outcomes within test deadline, report the uncertain ones, and never claim source acknowledgement. The harness returns nonzero/failure on out-of-quota, malformed input or sink failure. Quota refusal occurs before external effects; see [resource-budgets.md](resource-budgets.md).

### P1 pipeline subset

The P1 plan compiler accepts **manual batch**, local read-only source, JSON serializer, four stateless transforms (`rename`, `add_field`, `filter`, `drop`), one or two **scratch-local** sinks with optional per-sink condition and per-sink stateless transform. No network listeners, streams, real external providers, stateful transforms, conditional source deletion, per-sink retry, durable DLQ or remote pipelines. Source failure/record failure is observable and does not silently count as delivered; `on_error=abort` is the first supported behavior; `continue` may enter P1 only if explicit per-record failure oracle passes. Full details and options are locked in [p1-compatibility-matrix.md](p1-compatibility-matrix.md).

### Required P1 proof

- `P1-ADMIT-01`: valid read-only input and exclusive scratch output permit one run and label it ephemeral; unsafe path, symlink, existing target, missing quota, unsupported setting or bad YAML are rejected **without creating input/output artifacts**.
- `P1-ACK-02`: intentional sink failure and cancelled run leave input byte-for-byte unchanged, including mtime and filename; no ack operation is available in P1 trait wiring.
- `P1-CRASH-03`: subprocess crash after partial scratch write leaves input intact; restart has no claimed recovery and produces independent scratch; never report `confirmed`.
- `P1-FANOUT-04`: two sinks and branch transforms cannot mutate one another's view; one failed sink makes run `ephemeral_failed/unknown`.
- `P1-BOUNDS-05`: the exact bound/charge scenarios in resource-budgets.md remain enforced.

### Re-entry: P2 production-capable local execution

P2 must introduce transactional owner/attempt admission before **any** writable publication, immutable plan identity, durable worker reservation, idempotent publish manifest, file+directory fsync where supported, receipt/checkpoint ledger, revocation, recovery, replay, journal-full fail-closed and actual process-kill tests. P2 may not claim production readiness merely by adding a SQLite file: it must demonstrate the whole obligation chain, with per-source-unit accounting and replay. **The P1 ephemeral relaxation is forbidden in P2/P3/P4 runtime feature declarations.**
