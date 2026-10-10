# RUN+ISO-001 (PROPOSED) — Combined P2 runtime and standalone-isolation spike contract

**Status:** SPECIFICATION PROPOSAL ONLY. Independent design review and a separately recorded, exact-SHA ORCHESTRATOR **SPIKE_AUTHORIZED** decision are required **before execution**. No spike was performed or authorized by this document. **Owners:** DESIGN_OWNER (execution), Security/Operations (local IPC/process), EXECUTOR (independent measurements). **Depends on:** [ADR-013](adr-013-p2-runtime-concurrency.md), [ADR-014](adr-014-standalone-worker-failure-domain.md), frozen [attempt protocol](../attempt-protocol.md) and [source-unit contract](../source-unit-contract.md). Frozen P0 SHA: `2cedaeac5657a8941fe9366f04029cd11b0cfd30`.

## 1. Decision scope and prototype safety boundary

Evaluate **two runtime implementations** (A: bounded std-thread+bounded-channel control/dataflow; T: Tokio with bounded async stages plus dedicated bounded blocking I/O), each against **two local topologies** (I: same-process in-process adapter; S: supervised local worker process and private IPC). This **2×2 matrix** is the minimum viable comparison; if a cell is intentionally unavailable, record a separately approved exclusion and treat missing evidence as INCONCLUSIVE, not PASS. Profile C (both topologies) may be considered only after every advertised profile passes its own tests.

No candidate, including Tokio or supervised subprocess, is selected in advance. Reuse **one engine implementation per runtime candidate** for I and S; the same plan, source-unit graph and attempt/result interpretation apply. The spike uses only disposable test-root inputs, no real customers, brokers, credentials, external endpoints, production user paths, destructive source acknowledgements or network listener. All effects are isolated **scratch-only pseudo-effects** under a dedicated disposable root; any SQLite/file sync exercises use disposable databases and scratch files. A genuine SIGKILL, crash boundary and blocked-fsync simulation may be exercised **only there**, never against production resources. P1 is unchanged and its serial tests/capacity=2 configuration are **not** concurrency proof.

## 2. One exact conceptual typed ExecutionService signature (all four candidates)

The single shared, versioned API *shape* for the spike is:

```rust
// Design signature, not implementation or an ABI freeze:
trait ExecutionService {
    fn call(&self, request: ExecutionRequest)
        -> Future<Output = Result<ExecutionReply, TransportFault>>;
}
struct ExecutionRequest {
    protocol_major: u16,       // spike = 1; reject unknown major pre-effect
    protocol_minor: u16,       // declared feature negotiation
    request_id: RequestId,    // idempotent retry
    caller: AuthenticatedManager,
    command: ExecutionCommand,
}
enum ExecutionCommand {
    Prepare(PlanDigestAndCapabilities),
    Start(ExactAttemptAuthorityAndImmutablePlan),
    Status(ExactAttemptIdentity),
    Cancel(ExactAttemptAuthority),
    Drain(WorkerIdentityAndAuthority),
    GetAttempt(ExactAttemptIdentity),
    ReplayCompletion(ExactAttemptIdentity),
    AckCompletion(ExactAttemptIdentityAndResultDigest),
}
enum ExecutionReply {
    Prepared, Admitted(JournalReceipt), Running,
    CompletionCommitted(ResultAndOutboxReceipt),
    CallbackAcked(ManagerReceipt), CancelAccepted(TombstoneReceipt),
    Quiescent(DurableProof),
    Rejected(Code, Retryability, Option<DurableNoAdmissionReceipt>),
    Unknown(Reason, PreviousKnownState),
}
```

This is a **conceptual typed contract**, not compilable Rust syntax or permission to alter frozen enums. Every candidate must accept the **identical command/result and immutable authority fields** and pass byte-level contract vectors for local IPC and direct-call adapters. Existing [attempt protocol §4](../attempt-protocol.md) is normative if shorthand in this signature differs. **A transport error or timeout maps at the manager boundary to durable `unknown` (guard held) if the request may have reached the worker; never to proof-bearing `Rejected` without the exact authenticated journal tombstone.** Disconnect after effects or before reply cannot be classified as completed success. Direct in-process panics must not be converted into proven no-effects by exception mapping.

### Authority, persistence and transport ownership

- **Manager** exclusively owns the authoritative run/slot admission ledger, immutable original plan snapshot/revision lookup, CAS identity/fence/guard, durable source-unit/frontier and audit; only manager can decide whether a protected source generation may be admitted by another revision. Worker never directly modifies manager tables.
- **Worker** exclusively owns its separate durable admission/reservation/revocation/completion journal and atomic replayable result/outbox. The same independent roles are present in topology I even if the same process hosts both.
- **Local IPC in S** is a private OS-local channel (Linux UDS with checked peer UID/GID/process/workload identity and restrictive socket parent ownership/permissions; Windows named-pipe/ACL equivalent requires separate proof). No TCP, remotely exposed worker, unauthenticated ambient socket or plaintext remote fallback in P2. Bind every Start/Cancel/receipt to exact manager identity, attempt, immutable plan digest/revision, worker ID/session, fence generation and nonce. Replay and stale sessions are rejected before effects; a peer connection alone is not start authority.
- **Protocol versioning:** explicit major/minor + capability fields, reject unsupported major/mandatory fields and version downgrade before effects; prove direct/IPC reply enum equality and stable serialized request/response fixtures. D05 `tonic`/mTLS choice and remote worker deployment remain **P4 re-entry gates**; S does not imply P4 compliance.
- **Quotas across processes:** manager grants bounded, ledger-tracked per-run and shared run admission permits; worker enforces local byte+item queue ceilings and returns/reconciles durable consumption. IPC serialization buffers, kernel pending bytes, prefetch, pending requests and process RSS are charged/limited, not counted as free. On IPC loss or worker crash, do not optimistically release shared reservations/old guard until evidence from journal/reconciliation or quiescence; no double reservation budgets after worker restart.
- **Process tree:** manager supervision records PID/start generation, child descendants and effect-capable handles. Stopping/kill is not proof of quiescence until the old worker **and descendants** are demonstrably unable to write. No new attempt can gain the old protected slot merely on PID change, timeout, loss of journal or restart.

## 3. Precommitted numeric acceptance profile (spike-only; not frozen P2 production limits)

This profile is **binding on the future spike unless independently reviewed and amended *before* test execution**. The Orchestrator spike authorization must pin: candidate code SHA, OS/kernel, rustc/cargo and Tokio versions (Rust toolchain target **1.88.0**), CPU quota (4 logical cores), filesystem type/mount options, interpreter/tool versions, exact deterministic generated-fixture SHA-256, log format and a disposable filesystem root. First execution target: isolated Linux x86_64; any other OS remains **UNSUPPORTED/FAIL-CLOSED** unless a separate equivalent peer-auth, process-tree and filesystem test matrix is approved.

| Parameter | Exact spike threshold / workload |
|---|---|
| Input workload | seed `0x20261010`; **2 independent runs**, each **512 source units × 8 records = 4096 records/run**; 75% records 512-byte payload, 25% 4096-byte payload; 2 sink branches/run; deterministic 1:1/no transform expansion; original fixture digest locked in spike authorization |
| Trials | 3 clean runs **per 2×2 candidate cell** plus fault cases with log/trace digests; no warm-cache substitution or skipped failure |
| Overlapping work | 2 concurrently admitted runs; at least **2 outstanding sink I/O intervals overlap for >=100 ms** in every nonfault trial, measured via monotonic timestamped enter/complete hooks around actual operation |
| Sink distribution | A: no artificial delay; B: deterministic 50 ms per batch baseline; separate stalled-B phase with **250 ms per batch** and a fully blocked-B phase; same schedule across candidates |
| Bounded sink queues | max **64 pending messages and 4 MiB payload bytes per branch**, including IPC/send buffer; max **256 queued messages and 16 MiB** across four branches; fail if any sampled/observed high-water exceeds either bound |
| Scope budgets | per-run tracked live limit **64 MiB**, shared aggregate live limit **128 MiB** across the two runs (source, decoded, queues, clone, encode, tasks, IPC charge); zero negative/double release; no new protected-source admission on independent ledger alone |
| Blocking I/O | max **4 simultaneously running blocking operations**, max **16 queued blocking jobs**, max **4 dedicated blocking threads**. Non-blocking reactor wait must not block on synchronous fsync/SQLite I/O |
| Fairness | while both runs remain runnable (not blocked on a genuine per-source/sink effect) and have queued work, each receives **>=20% of sink dequeue opportunities in every complete 5-second measurement window**, and each makes one forward-progress dequeue at least every 2 seconds |
| Cancellation | signal-to-stopped-new-ingress/permit-wait-unblocked **<=2 seconds** in 3/3 trials; timeout/possible-effect remains **UNKNOWN with guard held**, never fake quiescence |
| Blocked fsync | within **<=3 seconds** of cancel, control returns a *pending/UNKNOWN* classification and remains responsive; **never** claim blocked thread/OS write has ended or release guard until proved |
| Child cleanup | after isolated kill request, old worker plus all effect-capable descendants must be proved stopped/fenced in **<=5 seconds** for a quiescent/replacement PASS; otherwise **FAIL for isolation/quiescence**, guard stays held |
| Memory | observe process `VmHWM`/equivalent per process **and sum** for manager and workers; total measured high-water **<=256 MiB**, plus all ledger/queue caps above; RSS observations are sampled system evidence, not a universal OS allocation guarantee |
| Handles | high-water **<=128 open FDs/handles per process**, **<=256 aggregate**; record values at 100 ms intervals and at fault boundaries; evidence cannot rely solely on an end-of-run sample |
| Nonregression | on identical workload, matching pseudo-effect confirmation tier and CPU/disk environment, candidate confirmed useful throughput **>=80%** of minimal bounded std-thread+channel baseline, p99 completion latency **<=2×** baseline and high-water aggregate RSS **<=1.5×** baseline; failing performance is **FAIL/CHANGES_REQUIRED**, not waived without reviewed exception |
| Provenance | monotonic nanosecond trace for dispatch, Start, journal commit, effect-start, write, receipt, ack barrier, cancellation, SIGKILL, manager/worker restart, child process exit, resume; stdout/stderr exit code, exact SHA, hash and before/after filesystem/snapshot/ledger fields |
| Safety invariant | after simulated A-confirmed/B-UNKNOWN there is **zero source ack**, no A re-emission if its receipt is durable, no release of unknown slot without authenticated evidence/quiescence; check after every crash/restart |

The percentages/limits above are deliberate *spike acceptance values* and need no production backing claim. Tests must validate **admission denial** under saturation rather than shrinking workloads mid-run; any environment outside the authorized hardware/toolchain/fs envelope is INCONCLUSIVE pending reviewed re-pinning.

## 4. Required cases, all with explicit PASS/FAIL evidence

- **RUN-01 / ISO-05:** all four candidates run identical plan/typed call vectors; prove sink I/O overlap and direct-vs-IPC result equivalence, never count a configured permit as an outstanding I/O.
- **RUN-02:** blocked B saturates both item/byte limits with producer stopped before overflow, A remains within its budget, source barrier remains open and cancellation unblocks waits within 2 s.
- **RUN-03:** cancel while on a byte permit, SQLite lock and simulated blocked fsync; return promptly as pending/UNKNOWN, retain exclusion until actual quiescence.
- **RUN-04 / ISO-01:** sink panic/abort or disposable OOM/fatal exit; S must retain manager/API and ledger availability with UNKNOWN; I must record its larger fatal failure domain honestly. No production fault injection.
- **RUN-05:** concurrent-run quota and fairness, post-error permit release only when effects are quiescent/receipts established, no zero-cost cross-process queue inflation.
- **RUN-06 / ISO-02:** in **disposable non-production** worker journal/manager ledger, inject SIGKILL after mock write before receipt, and after worker completion+outbox commit before manager response; restart actual manager and worker processes. Check durable proof, no false no-effect/tombstone, A confirmed/B unknown no ack, original plan snapshot/replay receipts and exact fence.
- **RUN-07:** record full matched-guarantee alternate comparison for throughput, p99, CPU-seconds/confirmed result, peak RSS and handles; all thresholds above must PASS.
- **RUN-08:** old P1 serial suite/4 MiB/4096/64 MiB scope remains untouched; no retroactive P1 concurrency proof.
- **ISO-03:** kill worker and descendants while effect-capable child holds a scratch FD; observe child ability to continue writing. Deny replacement/guard release until verified kill/quiescence; any observed descendant write after accepted replacement is a **FAIL**.
- **ISO-04:** spoofed local caller, wrong UID/pipe ACL, malformed version, stale generation/nonce, replayed and cross-run token all DENIED pre-effect; timeout after uncertain dispatch -> UNKNOWN and same guard.
- **ISO-06:** Linux peer identity/root ownership test and explicit unsupported-platform test: no insecure fallback / silent TCP listener; stable child-exit and trace collection.

## 5. Interpretation and post-spike approval

Each case has an independent measured **PASS**, **FAIL** or **INCONCLUSIVE** for each applicable candidate cell. Any violated safety invariant, missed quota or unsupported auth/fencing result is FAIL. A flaky/unreproducible environment or absent code/trace is INCONCLUSIVE, never PASS. Only a fresh REVIEWER audit of the exact executed SHA and independent EXECUTOR evidence can support a DESIGN_OWNER selection recommendation. The Orchestrator must **separately** approve the chosen runtime and production process boundary, any explicit exceptions, P2 PRE-GATE and subsequent implementation authorization. No part of this document retroactively approves the combined spike or changes the Phase 0 frozen contract.

If S is selected later, the independent DCR must explicitly supersede frozen [architecture](../architecture.md) diagram (§2 embedded standalone worker) and §2's "standalone invokes in-process" sentence, plus any direct-method assumptions in §5 transport, **only** for the separately accepted production standalone profile. D01 shared semantics and D05 P4 remote mTLS re-entry continue unchanged.
