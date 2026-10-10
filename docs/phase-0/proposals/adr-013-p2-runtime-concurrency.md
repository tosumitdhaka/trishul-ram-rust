# ADR-013 (PROPOSED) — P2 runtime, concurrent dataflow and bounded blocking I/O

**Status:** CONDITIONAL_SPIKE_REQUIRED / NOT APPROVED. **Owner:** DESIGN_OWNER (runtime); **gate:** independent DCR review -> separately authorized combined RUN+ISO spike -> new reviewed architecture freeze -> P2 PRE-GATE. **Baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. **Governing specification:** [RUN+ISO-001](run-iso-combined-spike-contract.md). No Tokio/runtime decision or P2 source authority is implied.

## 1. Problem and immutable P1 boundary

The frozen design requires bounded ingress and fan-out, backpressure, independent sink obligations and truthful cancellation, but did not choose a scheduling runtime or isolation for blocking SQLite/fsync work. Accepted P1 `9757aaf29665d5f774e15583e1e4f23b140b9799` collects records, encodes branch arrays and writes sinks sequentially. Its `Rc<RefCell>` ledger and **mandatory `RUST_TEST_THREADS=1` serial acceptance** do not prove concurrent I/O. [R2 Orchestrator acceptance](https://github.com/tosumitdhaka/trishul-ram-rust/issues/2#issuecomment-6099860467) covers only ephemeral source-read/scratch-output behavior and known limitations. **P1 cannot be expanded, reinterpreted as a producer/consumer proof or reopened by this proposed DCR.**

## 2. Runtime alternatives (no preferred winner)

Candidate T: Tokio for async bounded producer/decoder/transform/fan-out/sink stages, plus a separately limited blocking facility for sync SQLite/fsync/file/plugin operations.

Candidate A: bounded dedicated standard threads with bounded channels and explicit worker supervision; must execute the same typed `ExecutionService` commands and result taxonomy, and the same durable authority protocol.

These alternatives **must use** the same engine semantic inputs/outputs and be compared in **both** in-process topology I and supervised local-process topology S. The mandatory 2×2 candidate matrix, exact common conceptual `ExecutionService::call(ExecutionRequest) -> Future<Result<ExecutionReply, TransportFault>>`, manager ledger vs worker journal/outbox ownership, peer-authenticated local IPC and version/identity/fence rules are frozen **for spike evaluation only** in [RUN+ISO-001 §1–2](run-iso-combined-spike-contract.md). This specification does not choose `tonic`, Axum, a remote listener or a P4 transport profile.

## 3. Proposed invariants for review and experiment

1. **Pre-allocation bounds:** charge byte/count for decoded envelopes, clones, queues, task overhead, encoder buffers and IPC before ownership transfer. Track shared physical buffers once and separately charge each owned copy. Pre-reserve all required branch positions atomically or block without partial enqueue. Independently constrain per-run and shared multi-run budgets; a second process cannot simply multiply the quota.
2. **True backpressure:** a blocked sink B must bound queue bytes/items, stop upstream production, and preserve A's truthful independent receipt; A confirmed/B unknown never permits source ack. Resource overload is a typed pre-effect rejection or explicit failed/unknown run, never drop-as-success.
3. **Blocking APIs:** bounded blocking threads/queue, no unmetered `spawn_blocking` fan-out. A cancelled async future cannot terminate an OS file write; blocked SQLite lock or fsync may leave effects unknown. Control tasks must remain responsive and record the held guard until actual proof.
4. **Cancellation and supervision:** close ingress, wake every permit waiter, signal workers, observe completion/quiescence with deadline, retain receipts/outbox and exact fence. Panic/abort/OOM evidence must be classified by the process topology; no exception becomes proof of no effects. Fairness and no reservation leak are measurable requirements.
5. **Persisted authority:** manager owns exact source/run/fence CAS and original plan; worker owns durable local admission, revocation, result/outbox. IPC disconnect after maybe-sent dispatch creates UNKNOWN. The same rules apply in both topologies.
6. **Nonproduction spike:** real process-kill/restart and simulated durable file/SQLite crash seams may run only in an isolated disposable spike test root after separate authorization; no production filesystem, network, source ack or P2 runtime implementation is approved.

## 4. Numeric RUN acceptance, bound to combined spike

**All numeric pass/fail thresholds are in [RUN+ISO-001 §3–4](run-iso-combined-spike-contract.md) and are part of this proposed ADR**, not optional suggestions. In particular, the spike must test 2 concurrent runs of 4096 records each, 2 sinks/run, 3 trials/candidate, measured outstanding I/O overlap >=100 ms, branch queues <=64 items/4 MiB, shared live charge <=128 MiB, <=4 blocking threads/16 queued jobs, 20% fairness in complete runnable 5-s windows, cancellation <=2 s, blocked-fsync unknown classification <=3 s, child-tree quiescence <=5 s, combined process VmHWM <=256 MiB and bounded FDs, and matched-tier throughput/latency/RSS nonregression against the minimal thread baseline. Do not infer any pass from configured concurrency=2 or serialized P1 tests.

| Case | Required objective |
|---|---|
| RUN-01 | Real >1 outstanding sink I/O plus exact adapter semantic equivalence |
| RUN-02 | Slow/blocked B enforces queue bounds and producer pause |
| RUN-03 | Cancel every type of permit wait, SQLite lock and blocked fsync with held guard on uncertainty |
| RUN-04 | Panic/abort, dropped future and supervisor truthfulness |
| RUN-05 | Multi-run fair shared budgets, zero unauthorized releases |
| RUN-06 | **Isolated nonproduction** mock effect/journal crash-before-receipt and commit-before-response actual process restart |
| RUN-07 | Three-trial controlled alt benchmark with fixed thresholds, including peak RSS/fd count |
| RUN-08 | Preserve frozen P1 acceptance and no retroactive runtime guarantee |

## 5. Decision, dependencies and gate

**CONDITIONAL_SPIKE_REQUIRED.** An independent REVIEWER first approves the **specification**, not a Tokio selection. ORCHESTRATOR may separately authorize the disposable RUN+ISO spike at an exact SHA. REVIEWER and EXECUTOR then check code, traces and objective acceptance; finally the DESIGN_OWNER recommends a runtime and the ORCHESTRATOR must explicitly approve an immutable DCR/precedence map before P2 code. Any unmet numeric/safety threshold is FAIL or documented INCONCLUSIVE, not a passing design. ADR-014 process selection is inseparable from this evidence. D01 single semantic engine, D05 P4 remote mTLS re-entry and all frozen unknown/ack invariants remain authoritative until approved amendment.
