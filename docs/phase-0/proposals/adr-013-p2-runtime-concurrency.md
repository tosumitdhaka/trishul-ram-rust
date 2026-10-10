# ADR-013 (PROPOSED) — P2 runtime, concurrent dataflow and bounded blocking I/O

**Status:** PROPOSED / independent DESIGN_REVIEW required; not an amendment to the Phase 0 freeze until approved. **Owner:** DESIGN_OWNER (runtime); **gate:** P2 PRE-GATE, before P2 production-effect implementation. **Baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. **Evidence:** P1 `9757aaf29665d5f774e15583e1e4f23b140b9799`, [SOURCE_REVIEW_PASS](https://github.com/tosumitdhaka/trishul-ram-rust/pull/3#pullrequestreview-5479675831) and [EXECUTOR_PASS](https://github.com/tosumitdhaka/trishul-ram-rust/pull/3#issuecomment-6099635829).

## Problem and preserved contracts

The Phase 0 architecture requires bounded ingress, independent sink branches, resource pre-reservation and cancellation but leaves executor/task and blocking I/O scheduling unspecified. The P1 harness accumulates records, encodes per branch and writes sinks sequentially; its `BudgetLedger` is `Rc<RefCell>`. P1's accepted serial-only test profile is **not** evidence of concurrent queues, two simultaneous I/Os or production backpressure. Preserve P1's deliberately ephemeral, scratch-only, one-active-run guarantees; **do not retrofit an unreviewed runtime into P1**.

## Proposed decision (requires spike before ACCEPT)

1. Use **Tokio as the leading P2 runtime candidate**, with one supervised control-task group per run and bounded producer/decoder/transform/fan-out and sink-worker stages. This is a proposal, not an already frozen crate choice; compare a minimal threads+channels alternative in a spike. `tonic`/Axum are separate candidate transports, not mandated by this ADR.
2. Distinguish *allocation accounting*, *queue occupancy* and *I/O permits*. Every message, retained decoded batch, branch copy, pending encode buffer and spawned work item acquires a bounded byte/count permit **before** allocation/enqueue; receipt/checkpoint/journal and recovery headroom have separately enforceable quotas. Do not count the same physically shared buffer twice, and never omit owned copies/task overhead. A production budget must be shared across concurrent runs in its scope, unlike the P1 per-run shortcut.
3. Fan-out must reserve every required sink-branch slot atomically or wait without making a partial enqueue; branch-specific effects already issued remain reported independently. A slow B must exert upstream backpressure, not cause unbounded A buffering or unmetered tasks. Preserve source-unit obligation sealing and A-confirmed/B-unknown no-ack behavior.
4. Route blocking filesystem reads, synchronous SQLite transactions, `fsync`/directory sync, and blocking plugin calls through a **bounded dedicated blocking facility** or dedicated worker thread, never on core async reactor tasks. Bound waiting threads and queues as well as permits; `spawn_blocking` cancellation is not a kill/rollback guarantee. Journal commit and effects-boundary cancellation retain their own identity/fence protocol.
5. Cancellation closes ingress, awakens pending queue/permit waits, requests sink stop and waits for explicit task/effect quiescence with deadline. If an effect may have happened, return `unknown`; a timed-out task is not dead. Drop/abort of a Rust future does not certify termination of a blocking OS call or native plugin.
6. Lock ordering, readiness, worker supervision, fair per-pipeline budgeting, panic/fail-stop propagation, metric cardinality and shutdown order must be documented before P2 implementation. No dynamic untrusted native plugin isolation is claimed.

## Required pre-freeze concurrency spike

- **RUN-01:** produce + decode + transform + both sinks overlap; record measured *simultaneous* I/O activity >=2 when supported, not merely configured cap=2.
- **RUN-02:** slow B + fast A; saturate B's bounded queue and prove source read pauses, no unlimited tasks/allocations, correct A/B outcome and cancellation.
- **RUN-03:** cancel while waiting for each permit, SQLite lock and blocking `fsync`; no deadlock or false quiescent acknowledgement.
- **RUN-04:** panic/failure of one sink task and dropped async future; supervisor returns bounded truthful failure, maintains journal/guard invariants.
- **RUN-05:** multi-run admission and memory caps, early-release/leak checks, serialized or independent-source fairness; include peak RSS rather than before/after only.
- **RUN-06:** crash after external write but before receipt, after journal commit but before response, and during cancellation: results must obey existing attempt and source-unit contracts.
- **RUN-07:** benchmark Tokio candidate against a small synchronous bounded-channel alternative, recording CPU, confirmed useful outputs, peak RSS and tail latency at **matched delivery tier**.
- **RUN-08:** no regression to P1 4 MiB/4096/64 MiB limits or P1 serial-only profile; no P2 concurrency success inferred from P1 CI.

## Decision boundary

P2 PRE-GATE needs spike logs, benchmark traces, concrete thread/permit counts, ownership of each blocking API, proposed service signatures, and independent DESIGN_OWNER/REVIEWER agreement. Reject/runtime-ADR DCR if the candidate cannot meet RUN-01..08. No source code, Phase 1 scope, or production claim is authorized by this proposal.
