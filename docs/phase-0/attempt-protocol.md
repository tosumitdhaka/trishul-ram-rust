# P0-R3 — Normative attempt and execution authority protocol

**Status:** Phase 0 remediation proposal. P1 has no production attempts; this specifies the P2 standalone and P4 distributed control-plane behavior that must be accepted before implementation. Standalone uses the **same semantics** via an in-process adapter; no special bypass of durable checks.

## 1. Identities and fence

A control-plane `ExecutionAuthority` binds `pipeline_id`, immutable `plan_digest` and `plan_revision`, `run_id` (logical), `attempt_id` (distinct per retry), `slot_id`, `worker_id`, `worker_session_id` (changes on process boot), `fence_generation` (monotonic ledger), `auth_epoch`, `not_before`, `expires_at`, `action` and stable token nonce. Reject ambiguous empty/mismatched fields before side effects. The manager persists attempt+fence assignment via CAS in the ledger; the worker verifies token signer/short-lived validity against its registered manager and current session, persists reservation+tombstone checks atomically in journal, then starts work. TLS identity *alone* is not authorization.

`fence_generation` is a monotonic manager-issued number per pipeline/slot. Compare and swap on `(pipeline_id,slot_id,run_id,attempt_id,plan_revision,worker_session_id,fence_generation)` for every mutable ledger transition; only exact identity may commit checkpoint/release guard. Worker also compares token claim to request, manifest, journal reservation and actual worker session. A manager crash/reboot does not reset fence sequence or extend authorization expiry. Persist session epoch/tombstone GC high watermark; monotonic clock for local durations, trusted wall clock plus bounded skew for token windows, no replay after clock rollback.

## 2. Normative manager states and transitions

| Current | Event & prerequisite | Durable action | Next | Return/meaning |
|---|---|---|---|---|
| no intent | accepted start; validated plan and capacity | insert logical run intent | `pending` | accepted intent, not execution |
| pending | claim under free execution guard (CAS) | attempt+fence+guard committed | `claimed` | attempt created |
| claimed | send scheduled, dispatch intent persisted | record exact worker/session/nonce and dispatching | `dispatching` | can issue remote start |
| claimed | manager crashes before send is ever possible (outbox has no send intent) | reconcile ledger; prove never emitted | `terminal_rejected` or new claim | safe no-effect case |
| dispatching | worker returns journal-admitted with exact authority | mark acknowledged admission | `admitted` | durable reservation **not** run start |
| admitted | worker reports thread/task started under same token | mark running | `running` | effect window may have begun |
| dispatching/admitted/running | user stop/drain requested | record cancel intent; retain guard | `stopping` | cancellation **accepted**, not quiescent |
| dispatching/admitted/running/stopping | RPC deadline/connection lost; may have reached worker | record reason, hold guard | `unknown` | no blind redispatch |
| running/stopping/unknown | worker reports committed completion or reconciliation proves terminal | CAS final receipt/checkpoint & guard release | `terminal` | stable outcome |
| stopping/unknown | worker confirms durable tombstone **and** no active effects, or verified old session is permanently terminated/fenced at effect boundary | CAS quiescent proof+guard release | `terminal_revoked` | safe to reassign (external effects previously emitted still possible) |
| pending/claimed | invalid plan, explicit pre-send rejection, or expired unsent authorization with positive non-delivery proof | record reason; release exact guard | `terminal_rejected` | no effects proven |
| terminal* | any duplicate/stale update | no mutation | terminal* | return same outcome or authority error |

**Not allowed:** `unknown -> claimed` or `unknown -> new attempt` solely on timeout; `stopping -> terminal_revoked` upon stop request alone; `running -> claimed` without identity-fenced terminal/quiescence; release guard by `run_id` alone. `claimed` that was ever admitted into transport send-intent cannot be assumed unsent on manager restart; it must reconcile as dispatching/unknown.

Worker-local transitions: `absent -> reserved -> running -> completion_committed -> callback_acked`, or `absent/reserved/running -> tombstoned -> quiescent`, with `interrupted/unknown` on process crash. `completion_committed` persists result AND replayable outbox atomically before removing active work; `callback_acked` requires a manager durability receipt for exact attempt and checkpoint. A local `reserved` row after restart is **not** automatically resumed and is not proof that external effects were absent.

## 3. Worker authority and command matrix

| Command | Required caller/guard | Atomic worker operation | Success result |
|---|---|---|---|
| `hello/handshake` | authenticated manager identity, pinned protocol capability, current worker session | register/rotate authorized token signer with overlap bounds; no attempt admission | session/capability/journal health; **not** start authorization |
| `prepare(plan)` | compatible plan+caps and read-only validation | no side effects, validate checksum and quota | `prepared` |
| `start(authority)` | valid matching token+TLS peer, exact session/revision/attempt/fence, no tombstone, budget available | reserve attempt and quota in one durable transaction **before** source/sink open | `admitted` (never use `completed` here) |
| `status(attempt)` | authorized manager inspection | stable journal snapshot | absent / reserved / running / completed / interrupted / tombstoned / unknown; `absent` is not quiescence proof |
| `cancel(authority)` | valid cancel scope of same-or-newer fence; exact slot and worker session | commit durable tombstone/stop intent even if start not yet arrived | `cancel_accepted`; quiescence separately reported |
| `drain(worker)` | authenticated administrative authority | persist admission closed, block new attempts, signal active work | `drain_started`; not all-work-finished |
| `get_attempt` / `replay_completion` | authenticated manager / exact attempt | read journal and stable receipts; replay same completion | `completion_committed` or explicit absence |
| `ack_completion` | manager ack with same attempt/fence/result checksum | persist ack, retain tombstone until replay retention expires | `callback_acked` |

Duplicate `start` with same exact authority and identical plan: return existing journal state (`admitted/running/completed`) with **no second task**. Same `attempt_id` with different plan, run, fence or session => `CONFLICT` and no effects. Stale fence/session/expired token or missing authority => `UNAUTHORIZED`/`STALE_AUTHORITY`, fail closed. Worker journal/disk unavailable => `ADMISSION_CLOSED`. Unknown transport reply => `TRANSPORT_UNKNOWN`, retained manager guard. A canceled/tombstoned identity => `REVOKED`. Unsupported plugin/capability => `UNSUPPORTED_PLAN` before admission.

## 4. One semantic result taxonomy for local and remote adapters

Adapters return typed variants, independent of transport:
- `Prepared(plan_digest)`;
- `Admitted(attempt_identity, journal_receipt)`;
- `Running(attempt_identity, started_at)`;
- `CompletionCommitted(attempt_identity, result_digest, per_sink_outcomes, checkpoint_refs)`;
- `CallbackAcked(attempt_identity, manager_receipt)`;
- `CancelAccepted(attempt_identity, tombstone_receipt)`;
- `Quiescent(attempt_identity, durable_proof)`;
- `Rejected(code, retryability, admission_side_effects=false)`;
- `Unknown(reason, previous_known_state)`.

`Rejected` guarantees no new effects **only when worker admission was provably refused**; transport failures never imply `Rejected`. In-process exceptions and remote HTTP/gRPC codes map to the **same** typed result; neither adapter translates a timeout to `Rejected` or cancellation accepted to quiescent. Private protobuf/RPC field numbers and status-code wire mapping are frozen at P4 (D05); semantic outcomes are binding before P2.

## 5. Restart, expired lease and recovery

- Leases/authorizations have bounded validity for **new admissions** only. Lease expiry does not terminate running effects, release guards, erase tombstones or grant automatic replacement. Manager must obtain a completion receipt, durable revocation + quiescence, or proof old process/session can no longer write (including any active side-effect handles).
- Manager crash **before** durable dispatch intent: safe to replan; after intent: treat as maybe-sent and query worker's journal after restart, even if the local process died before receiving HTTP response.
- Worker reboot increments persistent session epoch; attempts from previous session are interrupted. Previous session termination is proven only when its execution process and possible child effects are conclusively terminated; a newly observed session_id alone is **not** proof that a previous process cannot write, particularly on partitions or identity reuse.
- A transport partition yields `unknown` and holds ownership. Explicit operator hold/repair can be required; never evict unknown or undecided journal outcomes to satisfy retention. A manager must not treat a worker's 404 as proof that old work was not accepted.
- Remote credentials rotated by worker/admin are bound to current session and overlap only for known in-flight authorization TTL; expired credentials may still authenticate replay queries using renewed secure channel, without minting new work for old authority.

## 6. Required protocol test IDs

`OWN-01`: crash before durable dispatch/send intent vs after send intent; reconcile without blind duplication. `OWN-02`: identical start replay vs conflicting attempt/revision/fence. `OWN-03`: cancellation before delayed start creates a persistent tombstone, rejected after retry/restart. `OWN-04`: stop accepted but effects remain in flight, status not quiescent. `OWN-05`: lease expired while task running, retain guard. `OWN-06`: timeout/partition unknown, do not reassign. `OWN-07`: worker reboot, old worker side-effect process alive, do not infer quiescence from new session. `OWN-08`: manager callback commit then response lost, worker retries identical completion; one terminal outcome. `OWN-09`: in-process and authenticated RPC adapters return same semantic classification for every fixture.

**Gate:** P2 may ship only once the required local authority/journal subset passes real restart tests. P4 requires authenticated remote protocol, isolation, rollout, and partition proof. P1 harness is not allowed to implement a weaker version while advertising production-capable features.
