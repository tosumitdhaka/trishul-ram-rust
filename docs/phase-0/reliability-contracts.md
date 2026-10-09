# TRAM Rust — proposed delivery, recovery, security and operational invariants

**Status:** PROPOSED; semantics are design constraints subject to review, not a claim of implemented functionality.

## Phase scope and normative dependency

This document describes the **P2+/P4 production architecture**. The initial P1 implementation is a strictly ephemeral test harness under [P1 safety](p1-safety-boundary.md), and does not claim durable admission, external-effect fencing, source ack/finalize or durable completion. Normative P2+ source-unit/receipt/checkpoint rules are in [source-unit-contract.md](source-unit-contract.md); exact manager/worker state transitions and result semantics are in [attempt-protocol.md](attempt-protocol.md). Bounds and resource accounting are in [resource-budgets.md](resource-budgets.md). Those detailed contracts supersede shorthand below.

## 1. Truthful outcomes and ownership

Separate **logical run** from execution attempts. Proposed identity tuple:
`pipeline_id, config_revision, run_id, attempt_id, placement_slot, worker_id, worker_session, fence_generation`. Different attempts for one run never share execution authority. All mutating messages and completions are checked against their identities; a mere `run_id` cannot release another attempt's claim.

Proposed lifecycle:

```text
planned -> admitted -> dispatched -> running -> stopping -> terminal
                        \-> unknown <-----------------/
admitted/dispatched may reject and terminate without running.
unknown -> terminal only after evidence/reconciliation; not by timeout alone.
```

Manager owns control-plane intent and attempt/fence ledger; worker owns local durable authorization/admission journal and replayable completion outbox. A cancellation revocation must be persisted before acknowledging its effect. If an old worker cannot be proven quiescent or fenced at the effect boundary, do not blindly redispatch. Storage updates use conditional identity/revision checks.

**External-effect limitation:** even a valid fencing design cannot revoke writes already accepted by an external destination. The claim is controlled ownership and honest duplicate/uncertainty reporting, **not** universal exactly-once delivery.

## 2. Record disposition and acknowledgement

A source unit is an explicit ackable position, **not** an output record. Completion requires a sealed child/branch obligation graph, every required sink receipt/filter/DLQ disposition terminal, the state/checkpoint persisted and a contiguous cursor barrier. Zero output is an explicit disposition, not vacuous success. Confirmed A/unknown B means the whole unit remains unackable. See [source-unit-contract.md](source-unit-contract.md).

Per-source unit and per-sink branch track **separate obligations**:

| Condition | Permitted disposition | Source ack/finalize? |
|---|---|---|
| All required sink effects confirmed at required tier | `delivered` | Yes, after any required state/checkpoint commit |
| Condition filtered intentionally | `filtered` | Yes when recorded as terminal by policy |
| Explicit configured drop | `dropped_by_policy` | Yes only if policy permits and records it |
| Record failed; DLQ durably confirmed | `dead_lettered` | Only if DLQ policy explicitly accepts this as final |
| Retry scheduled, sink buffering only, remote timeout/unknown | `pending` / `uncertain` | No |
| Sink/DLQ exhausted, state checkpoint missing or unconfirmed | `failed` / `uncertain` | No, unless a documented weaker loss contract was requested |

Multi-sink fan-out may be partially delivered; expose *each* sink's result and never say the whole record succeeded solely because one branch succeeded. Filtering a record for one branch must not mask obligations from other branches. Stateful checkpoint and external output obligations are conjunctive where relevant.

Delivery contracts are negotiated explicitly: best-effort, at-least-once with replay and confirmed sink, or constrained transactional semantics when supported by specific source/sink pairs. Do not imply stronger guarantees than the weakest participant. UDP SNMP traps are best-effort at ingress; HTTP webhook acceptance is not crash-durable until an actual ingress write is committed.

## 3. Crash/partition acceptance cases

Every state change requires exact attempt, worker session, plan and monotonic fence checks. Cancellation accepted is distinct from effect quiescence; lease expiry cannot release ownership. See [normative attempt state table](attempt-protocol.md).

| Boundary | Required recovery result |
|---|---|
| Crash before worker admission transaction commits | No work started; manager may safely decide another attempt after proof |
| Crash after admission commits, before start | Interrupted reservation discoverable; no blind execution restart |
| Crash after external sink writes, before confirmation/checkpoint | Uncertain outcome preserved; possible duplicate after controlled recovery |
| Crash between completion persistence and manager notification | Durable completion/outbox replay on worker restart |
| Manager commits callback then HTTP response is lost | Worker retries; manager identity/idempotency check prevents duplicate terminalization |
| Manager unavailable while worker runs | Continue only within valid policy/lease; retain completion and stop on required bounds |
| Worker partitioned with potentially active effects | Keep ownership unknown or fence/prove quiescence; never assume timeout = death |
| Repeated authorized start/cancel or stale message | Idempotent replay or explicit rejection, no second side effect for same attempt |

**Completion and outbox entries must be stored atomically** within one worker transaction. Manager ledger and worker journal are separate consistency domains: no distributed transaction is assumed. The protocol must be replayable/idempotent. Clock skew, replay windows, secret rotation and token expiry require deterministic tests.

## 4. Bounded memory, concurrency and overload

Define hard or configurable limits for: pipelines/runs/slots; incoming queue items *and bytes*; decoder partial frames; per-sink pending buffers; inflight I/O; transform state (including key cardinality); journal size; outstanding outbox results; DLQ/error samples; API request size; metrics labels; shutdown timeouts.

- Quota exhaustion rejects or backpressures **before** silently consuming beyond configured bounds.
- Never evict unacknowledged durable outcomes merely to reclaim disk. If reserved durable capacity is exhausted, stop new admission, report unhealthy readiness and retain evidence.
- Push/UDP sources that cannot backpressure must report bounded drops, with no false promise of reliable delivery.
- Slow sinks may not create unbounded source accumulation; cancellation/drain is bounded, and failure remains observable.
- Prefer explicit scheduling fairness and per-pipeline resource budgets over a single global semaphore.

## 5. Authentication and secret boundaries

- Authenticated/authorized public management API; authenticated machine-to-machine worker control; encrypted remote transport by default.
- A start token is short-lived and binds action, manager authority, worker session, pipeline revision, attempt identity and time window.
- Journal preserves replay barriers across restart and credible clock rollback. Handshake/rotation must not allow a stale session to regain authority.
- SNMP community/v3 USM credentials are secret references; redact in logs, traces, failed-plan messages and process arguments. Auth/priv protocol selection requires fail-closed validation.
- No arbitrary untrusted shared libraries or user code within the worker process in v1. Runtime configuration and connector inputs are untrusted and must have bounds and parsing validation.
- Audit plan/run modifications and security-relevant decisions without unbounded high-cardinality metrics or raw secret values.

## 6. Operability and quality gates

- `live` and `ready` are distinct; readiness depends on journal health, admission eligibility and required dependencies, not a process responding to HTTP.
- Expose pipeline/attempt/outbox/checkpoint health, per-sink confirmed/failed/filtered/uncertain counts, bounded-queue use, memory/CPU, SNMP decode/auth drops, retry/duplicate estimates and actionable failure reasons.
- Graceful drain closes new admission, stops listeners, resolves/records existing effects within deadlines and reports any unknown ownership.
- Changes to on-disk formats carry forward/rollback migrations with backup and compatibility checks. A new release does not overwrite state it cannot read safely.

## 7. Minimum fault-injection test gate

Deterministic tests must cover each crash boundary above, stale/colliding attempt identities, duplicate/out-of-order callback, journal full/corrupt, manager outage, slow sink, cancellation race, malformed configs, SNMP credential failure and restart state hydration. Integration tests exercise actual process death/restart and actual data output checks, not mocked counters alone.

Tests must assert **correct contents and source acknowledgements**, not just `200`, `202`, job completion, or produced count. Shared golden corpora can compare Python behavior where it is accepted, but a known-bug oracle must be replaced with the documented corrected contract.
