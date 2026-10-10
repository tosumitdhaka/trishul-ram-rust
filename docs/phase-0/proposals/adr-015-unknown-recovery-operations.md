# ADR-015 (PROPOSED) — Operator recovery for permanently uncertain attempts

**Status:** PROPOSED; **Owner:** DESIGN_OWNER + Operations/Security; **gate:** P2 PRE-GATE and before production delivery. **Frozen protocol:** [attempt-protocol.md](../attempt-protocol.md), [source-unit-contract.md](../source-unit-contract.md), baseline `2cedaeac5657a8941fe9366f04029cd11b0cfd30`.

## Clarification: unknown is not irreversible

Frozen P0 permits `unknown -> terminal` with authenticated persisted completion, `unknown -> terminal_rejected` with exact journal-backed refusal/no-admission tombstone and no-effects proof, or `unknown -> terminal_revoked` with durable revocation **and** proven quiescence. The original review's phrase "no way out" is inaccurate. The gap is **lost or irrecoverable evidence** (destroyed journal volume, permanently partitioned worker, uncontrolled child process).

## Proposed operational policy

1. `RECONCILE`: privileged operator requests exact-authority journal/receipt reconciliation; only the existing proof-guarded terminal transitions can release execution ownership. Repeat-safe, immutable audit records for actor, time, case ID, attempt/session/fence, evidence and decision.
2. `QUARANTINE_UNRESOLVED`: persist a manual hold with reason/evidence-loss classification, no new admission for the guarded execution slot and no source acknowledgement. This is **not** a terminal success/failure and does not free its guard. Expose durable inspection, age/retention, alert and recovery runbook.
3. `ABANDON_WITH_UNCERTAINTY` (administrative bookkeeping only): retire scheduling intent under a distinct audited unresolved disposition **without** asserting external effects were absent, without releasing an effect-authority guard into an unsafe replacement, and without source acknowledgement. If the manager cannot represent abandonment without freeing guarded ownership, this operation is prohibited until a separate schema ADR supplies a second retained exclusion fence.
4. `RISK_ACCEPTED_REPLACEMENT` is **not** a universal force-terminal bypass. Allow only after authoritative quiescence/fencing across **every** effect-capable old process/child, or after a specific connector's separately accepted duplicate-tolerant/compensation contract explicitly permits replay. Maintain last fence, old attempt tombstone, possible-duplicate marker, original `EffectId`, audit and source-ack barrier. The operator's signature does **not** itself prove quiescence. No unconditional "force success", "force failed", "expire unknown on TTL", or delete-undecided-journal.
5. Journal loss (e.g. ephemeral Kubernetes volume) is an **unsupported durability topology** for a worker claiming durable admission. Preflight persistent volume/path, mount/fsync/space guarantees, backup/recovery and restart fault tests. Fail closed if unavailable; never silently turn `404`, expiry or reboot into no-effect proof.

## Permission and audit envelope (proposal)

Separate `attempt.inspect`, `attempt.reconcile`, `attempt.quarantine`, `attempt.abandon` and `attempt.risk_replace` permissions; require actor/workload identity, reason, incident reference, old/new fence, immutable evidence digests, double-authorization for risk replacement if policy requires it, and retention of before/after ledger state. Deny concurrent edits through exact revision CAS; preserve idempotent operator command IDs; redact secrets. An unavailable audit journal denies risky operations.

## Required proofs

`UNK-01` lost response after definitive tombstoned refusal → evidence-backed terminal rejection, idempotent.
`UNK-02` lost pre-start failed completion/outbox → terminal **failed**, no ack/no effects, never success.
`UNK-03` lost volume / 404 / timeout / lease expiry → quarantine, guard retained, no blind retry.
`UNK-04` operator abandons evidence-free run → no invented terminal result or owner release.
`UNK-05` independently proved worker+child quiescence → deliberate guarded replacement with higher fence; stale attempt cannot emit.
`UNK-06` replacement while old effects possible → DENIED, including an administrative override request.
`UNK-07` all actions survive manager restart and serialize with worker callback, preserve sealed obligation status and durable audit.
`UNK-08` operator API negative auth, missing journal, conflicting proof, stale revision and repeated command.

**Open review question:** the additional ledger state needed to represent abandoned-but-still-fenced attempts may require a schema DCR; do not implement `ABANDON_WITH_UNCERTAINTY` as an alias of existing terminal states. Reviewer must verify the invariant is actually representable before approval.
