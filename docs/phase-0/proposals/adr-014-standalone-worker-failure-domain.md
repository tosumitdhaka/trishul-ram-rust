# ADR-014 (PROPOSED) — P2 standalone worker failure domain and local transport

**Status:** CONDITIONAL_SPIKE_REQUIRED / NOT APPROVED; no production topology selected. **Owners:** DESIGN_OWNER + Security/Operations. **Baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. **Joint spike contract:** [RUN+ISO-001](run-iso-combined-spike-contract.md). P2 production-effect implementation and any new IPC worker remain unauthorized.

## 1. Alternatives (no preselection)

| Candidate | Boundary | Approval prerequisite |
|---|---|---|
| **I — Embedded** | Manager and worker logic in one process, typed direct adapter | Same semantic engine/quotas; document that native panic/abort/OOM can terminate the manager. Production eligibility requires an explicit availability policy accepting that failure domain |
| **S — Supervised local subprocess** | Distinct worker process on private OS-local authenticated IPC | Prove peer/workload identity, exact attempt/fence, journal ownership, child-tree cleanup, crash/restart, bounded cross-process queues and resource limits |
| **C — Both** | Explicit development/production support matrix | Independently pass/waive required cases for *each* advertised profile; no implicit fallback from S to I |

S is **a candidate, not a preferred or chosen production profile**. Runtime choices T (Tokio) and A (bounded std threads) from [ADR-013](adr-013-p2-runtime-concurrency.md) are also not selected. A single separately authorized **combined** RUN+ISO-001 test will compare the viable 2×2 runtime/topology matrix. Profiling can inform the selection, but no topology can silently weaken frozen execution guard or source ack rules.

## 2. Shared ExecutionService and ownership contract

**One typed service signature for every candidate** is specified in [RUN+ISO-001 §2](run-iso-combined-spike-contract.md): `ExecutionService::call(ExecutionRequest) -> Future<Result<ExecutionReply, TransportFault>>`. Commands: Prepare, Start, Status, Cancel, Drain, GetAttempt, ReplayCompletion and AckCompletion; result enums and exact immutable authority are common for I/S and T/A. The signature is a non-compilable design/prototype API, not permission for a new P1 service or a frozen P4 wire ABI. Protocol major/minor compatibility is checked **before effects**.

- **Manager ledger ownership:** only manager persists logical run intent, slot/source-generation guards, exact revision/fence CAS, immutable original plan identity, checkpoints/frontier and operator audit. Do not write manager tables from the worker. A manager callback stores the exact worker result under guarded identity.
- **Worker-local journal/outbox:** only worker persists admission/reservation, session, tombstone/revocation, exact completion and replayable callback outbox atomically. In I these remain separate *logical and storage* roles; in S the worker controls its own store with reviewed persistence/mount ownership.
- **Private P2 local IPC:** S binds a Linux UDS with strict root/parent directory permissions, verified SO_PEERCRED (UID/GID/PID/workload membership) and explicit Start/Cancel credentials bound to manager, plan revision/digest, worker identity/session, attempt, fence generation, nonce. A privileged local login or socket connect alone is not authorization. For other OS (Windows named-pipe/ACL identity), demonstrate an equivalent approved mapping; if absent, fail closed as UNSUPPORTED without a TCP or plaintext remote fallback. Framing/mandatory feature negotiation must reject incompatible major version, unknown mandatory fields, replayed/stale tokens and mixed plan revisions.
- **No P2 remote worker:** no remotely exposed worker endpoint, gRPC/mTLS shortcut or D05/P4 protocol approval. D05 remote `tonic`/mTLS and PKI/bootstrap remain an independently reviewed P4 re-entry.
- **Quota ownership across process boundary:** manager budgets outstanding runs and source generations; each worker charges local message/count/byte, IPC buffers, journal capacity and open handles. Any lease or IPC dropout is **UNKNOWN** if send/effects might have occurred: do not reclaim guard or let fresh worker mint an independent quota on a protected slot without durable proof.
- **Process cleanup:** supervisor maintains worker+descendant identity and effect-capable handles, proves whole process tree ended or effects are fenced before declaring quiescence/reassignment. A SIGKILL request, new PID, reboot, expired token, retryable connector or idempotent receiver is **not** itself that proof. An orphan child still writing after manager thinks the worker died is a blocking failure.

## 3. Joint numeric evidence and fault matrix

[RUN+ISO-001 §3](run-iso-combined-spike-contract.md) freezes exact numeric acceptance: 2 simultaneous runs, 2 sinks/run, 3 clean trials per candidate cell, branch <=64 items and 4 MiB, worker total reservations <=128 MiB, <=4 blocking threads, cancellation <=2 s, timeout from blocked fsync into UNKNOWN <=3 s, process-tree kill/quiescence <=5 s, high-water aggregate RSS <=256 MiB, <=128 handles/process, monitored overlapping I/O >=100 ms, fairness and throughput/p99/RSS nonregression. Both topology alternatives must run the same deterministic workload and error/trace vectors.

| Case | Verifiable requirement |
|---|---|
| ISO-01 | In S, isolated worker abort/OOM does not destroy manager ledger/API; manager observes UNKNOWN and preserves owner guard. I exposes its admitted failure domain |
| ISO-02 | Durable worker admission/result/outbox replay after local crash/restart; lost response cannot cause a duplicate start |
| ISO-03 | OS process/descendant with an effect-capable scratch handle survives attempted operator release: **replacement denied** until quiescent/fenced; measure child cleanup by 5-s threshold |
| ISO-04 | Forged local caller, stale session/fence, version downgrade, cross-run nonce and unauthorized Start/Cancel denied pre-effect; uncertain disconnect maps UNKNOWN |
| ISO-05 | Same plan/config, exact `ExecutionService` types, receipts and cancel/timeout semantics for I and S, identical source-unit decisions |
| ISO-06 | Linux peer-auth, permissions, symlinked local IPC parent, process-tree/root lifetime and explicit unsupported-platform fail-closed; no remote P2 listener |

**Fault tooling restriction:** crash/SIGKILL and disposable journal/fsync simulations are permitted only in a **separately authorized nonproduction spike** with isolated roots. The existing Phase 1 ephemeral harness and CI are not modified by this ADR.

## 4. Exact frozen clauses potentially superseded *only if S is later approved*

An eventual DCR acceptance of S for a specified production standalone profile would replace **only** the frozen [architecture](../architecture.md) §2 diagram's label "embedded worker runtime" within STANDALONE PROCESS with a supervised *local* worker subprocess; the §2 sentence "Standalone invokes the same service interface in-process" would become "Standalone uses the same service interface via an authenticated private local worker IPC for the approved isolated production profile"; §5 "In-process adapter sends typed calls on bounded channels or direct methods" would describe the **I/development profile only** (if retained) and add S's private local adapter. The frozen D01 single-engine semantics, D04 manager/worker independent journals, D05 no remote P2 endpoint and P4 mTLS decision, D06 source-unit obligations and all old-worker quiescence/fencing rules **are not superseded**.

No text actually takes precedence until a separately reviewed DCR names exact frozen clause anchors, test proof and Orchestrator-approved commit. A spike result alone does not decide, freeze, merge, release or authorize P2 runtime changes.

## 5. Decision boundary

**CONDITIONAL_SPIKE_REQUIRED.** First review this spec and joint acceptance. Only then ORCHESTRATOR may authorize execution of RUN+ISO-001 by a separate implementor/executor at exact candidate SHA in a disposable test environment. Independent REVIEWER+EXECUTOR proof, then an explicit DCR decision, choose I/S/C and T/A. Until that decision, neither Tokio nor subprocess B nor a process API is the accepted P2 topology.
