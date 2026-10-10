# ADR-015 (PROPOSED) — Audited UNKNOWN holds and evidence-backed recovery

**Status:** DESIGN_REMEDIATION / PROPOSED, independent DCR review required; **no new terminal/force-release transition is proposed**. **Owner:** DESIGN_OWNER + Operations/Security. **Frozen baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. **Binding original authority:** [attempt protocol](../attempt-protocol.md) and [source-unit contract](../source-unit-contract.md). P2 implementation/pre-gate unauthorized; P1 acceptance unchanged.

## 1. Exact authority invariant (F1 correction)

Frozen `unknown` already has **evidence-backed exits**, and this proposal cannot alter them:

- Authenticated **committed completion and matching manager CAS** may terminalize an attempt.
- Authenticated, immutable journal **no-admission tombstone with exact no-effects proof** may support `dispatching/unknown -> terminal_rejected` under the frozen conditions.
- Durable revocation **plus independently demonstrated quiescence/fencing of the old worker, all descendants and effect-capable handles**, with exact authority identity and CAS, may support `unknown -> terminal_revoked` and safe new attempt authorization.
- An unresolved `unknown` **retains its existing execution/source-generation guard**. An expired lease, dead PID alone, new worker session, lost journal volume, RPC timeout/404, receiver idempotency key, compensating operation promise, duplicate-tolerance policy, or operator signature **does not revoke old external-effect authority**. It cannot release/reassign that slot or acknowledge/finalize its source generation while the old worker or any child may still emit.

**Removed from proposal:** `RISK_ACCEPTED_REPLACEMENT` and the alternative permitting a connector-specific duplicate-tolerance/compensation contract to replace an active unknown. Any such independent compensation/exclusion model is a **different future DCR**, with old/new attempt mutual exclusion, exact fenced CAS, duplicate/out-of-order external effect classification, retained source-unit acknowledgement frontier, execution/receiver authority and operator audit; its approval cannot be inferred here.

## 2. Chosen narrow durable state resolution (F2 / Option A)

**Do not introduce** `ABANDON_WITH_UNCERTAINTY` as a terminal scheduler state, a second durable disposition or a replacement for `terminal_revoked`. The existing durable manager attempt remains `unknown` with its exact guard and protected source-unit frontier held. The only proposed operator metadata is an **idempotent audit/hold annotation** on the existing state:

- `RECONCILE`: authorized request to replay journal receipts or obtain independently verified quiescence. Does not change owner/guard on request alone.
- `QUARANTINE_UNRESOLVED`: persist `hold=true` and `hold_reason=(lost_volume|partition|unavailable_worker|ambiguous_effect|manual_incident)` as revision-checked **annotations** on the still-`unknown` attempt. No transition into a new scheduler state; no new attempt, no resource/guard cleanup and no source ack.
- `EVIDENCE_ABANDONED`: optional idempotent audit annotation saying an operator has **stopped actively pursuing current reconciliation**, not that execution or effects are absent. It is logically a hold annotation and **does not retire the run's guarded scheduling slot**, change `unknown` to a terminal state or permit GC. If durable annotations are not supported by the active ledger, this action is **NOT_IMPLEMENTABLE until separately authorized schema work**; using an external ticket alone cannot weaken the exclusion barrier.
- **`attempt.abandon` API and new terminal abandonment transition are unsupported in P2 by this ADR.** No alias to `terminal_revoked`, no new success/failure classification and no source finalization. A later terminal-abandon semantics/API requires a standalone, independently reviewed schema/state-machine DCR.
- Journal loss remains an **unsupported persistence configuration** for promised durable worker acceptance. Admission must require proven stable journal/mount/space semantics. A missing/erased worker volume after a crash does not constitute a no-effects witness.

### Persisted annotation requirements (proposal only, no schema implementation)

Atomic ledger compare-and-set over **exact run/slot/attempt/plan revision/session/fence plus expected manager-ledger revision**, with authenticated operator identity, command-id idempotency key, incident/reason, evidence digest, old/new annotation version and audit entry persisted together. No audit commit -> no hold/annotation success claim; duplicate command -> same result; conflicting revision -> reject without releasing guard. On manager restart, replay the persisted `unknown` guard, source-unit frontier and hold annotation **before** opening newer admissions, even if worker journal volume is permanently lost. Persistent exclusion cannot depend solely on an in-memory process-local mutex or removable worker journal. Retain tombstones, old receipts, hold/audit and blocked-ack frontier through recovery; GC is forbidden while undecided. P2 ledger interface details require their normal separate P2 schema review, and this ADR does not write migration SQL.

## 3. Operators and authority

Proposed permissions are `attempt.inspect`, `attempt.reconcile` and `attempt.quarantine`; `attempt.annotate_evidence_abandoned` is a conditional annotation permission **only after the corresponding audit metadata is reviewed**. There is **no** `attempt.risk_replace` or force-complete permission under this ADR. Actor authentication, incident ID, evidence provenance, expected-revision CAS and immutable audit trail are mandatory for an annotation. The operator can request reconciliation, not waive external-effect authority through intent/signature. Deny when audit persistence fails.

## 4. Acceptance proofs — all must be independently executed in future authorized P2 fault harness

| ID | Precondition/action | Exact required outcome |
|---|---|---|
| **UNK-01** | Maybe-sent start, lost refusal reply, matching durable no-admission tombstone | Authenticated `unknown -> terminal_rejected` only by exact-identity CAS; duplicate reply idempotent |
| **UNK-02** | Admission committed then durable `PRESTART_FAILED` completion/outbox, response lost | Receipt-backed terminal **failure** without source ack/effects, same result after reboot |
| **UNK-03** | Lost journal volume, 404, lease expiry, timeout and manager restart | `unknown` and guard survive restart; held source generation cannot be newly admitted by any plan revision, no source ack/finalize, no false quiescence |
| **UNK-04** | Operator records hold/`EVIDENCE_ABANDONED`, including permanently absent worker volume | Annotation idempotent with revision CAS; underlying `unknown`, execution guard, source obligations and newer-admission exclusion persist through restart; `attempt.abandon` terminal API unsupported |
| **UNK-05** | Authenticated terminal receipt, or durable revocation **plus proved quiescence/fence** of old worker **and every descendant/effect handle** | Exact manager CAS releases guard once and only then permits new higher-fence attempt; stale old authority cannot emit, old source ack still waits on all terminal obligations |
| **UNK-06** | Prior worker/descendant still able to write to scratch sink; operator force, duplicate-tolerance or receiver idempotency offered as replacement proof | Replacement DENIED and guard retained; deliberately let old descendant write *after the attempted override* to prove no unsafe acceptance; no duplicate second owner/effect or source ack |
| **UNK-07** | Manager restart races a late callback, audit annotation, worker volume deletion and a new-revision admission | Serializable CAS; persisted `unknown` owner/hold/guard and blocked frontier reconstructed first; new admission denied, no GC or ack |
| **UNK-08** | Unauthorized actor, old expected revision, conflicted identity, missing journal/audit, repeated command | Auth/fence/expected-revision failure refuses mutation; retry idempotent; guard/newer-admission block/source-ack prohibition unaffected |

**UNK-05 and UNK-06 are consistent:** operator desire alone is never evidence. UNK-05 passes only after the **existing frozen proofs** actually establish the old attempt cannot write; UNK-06 fails safe whenever that fact is unproven, including an observed live descendant.

## 5. Deferred separate DCR for any material new disposition

An eventual `ABANDONED_BUT_FENCED` terminal-like state or any compensation-based unsafe replay would require an explicit separately approved ledger/state-machine DCR defining durable retained exclusion fence, source-generation identities and protected source-unit frontier, old/new CAS and scheduling arbitration, actor + expected-revision permissions, journal-loss/restart recovery, retained tombstones/GC, read/API projection, actual external-effect/duplicate classifications and independent fault proofs. **No part of this proposal approves such a state or transition.**

**Disposition:** Narrow Option A specified; terminal abandonment and risk-accepted replacement **WITHHELD**; candidate must receive fresh independent DCR review. No production/P2 implementation or retroactive P1 gate change is implied.
