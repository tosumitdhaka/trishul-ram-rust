# ADR-016 (PROPOSED) — D03/D07 precedence, immutable replay and phase-specific evidence

**Status:** DESIGN_REMEDIATION / PROPOSED, **not an accepted amendment**. **Owners:** Data/Serializer, SNMP Adapter, Persistence, Migration/Performance. **Frozen baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. **Independent reviewer:** [PR #4 CHANGES_REQUIRED #5479961538](https://github.com/tosumitdhaka/trishul-ram-rust/pull/4#pullrequestreview-5479961538). No new P1/P2 capability authorized.

## 1. Exact D03/D07 amendment and normative precedence proposal (F5)

**Proposed superseding clause D03-SNMP-P3 (not active until approved):**

> The generic serializer conversion/wire mapping for **non-SNMP** binary bytes, arbitrary decimal values and timestamps remains **DEFERRED TO P5** under D03. The P1 typed Datum and proven JSON subset remain unchanged. **Exception restricted to P3 SNMP+JSON output:** the SNMP adapter may advertise or execute that capability only after a versioned, field-level, deterministic, binary-safe encoding for SNMP `OctetString` has been selected, independently reviewed and proven against the pinned Python reference `tosumitdhaka/trishul-ram@ff380725b86c9569901ea89ad6771623847cc4e2`. The proof must explicitly exercise empty bytes, non-UTF-8 bytes, embedded NUL, trap metadata and unsigned Counter64 through `2^64-1` without float conversion. Absent the exact schema/field mapping and source/oracle tests, **reject SNMP+JSON at capability admission before effects**; never guess per-record UTF-8/base64/hex or coerce to float. This D03-SNMP-P3 exception overrides D03's **P5 timing only for SNMP+JSON**, not for general JSON serializers.

**Proposed D07 acceptance addendum (not active until approved):**

> D07's SNMP polling/trap/compiled-MIB integration **does not itself** authorize the SNMP+JSON output capability. `DATA-02` is a mandatory **P3 capability-admission prerequisite** with reviewed schema version, field-level golden Python parity, type-specific encoding and round-trip/error fixtures including zero-length, invalid UTF-8, embedded NUL, v1/v2/v3 trap metadata and exact Counter64 maximum; disabled/rejected if unproven. `DATA-02` is **not a P2 implementation requirement and not a retroactive P1 requirement**. The rest of the D07 native-library/MIB/version boundary remains frozen.

**Proposed precedence once approved:** `D03-SNMP-P3` and the D07 addendum supersede **only** the corresponding D03 generic P5 deferral where SNMP+JSON output is requested. All other D01–D12 accepted semantics, D03 generic mapping P5 re-entry and P1 fail-closed bytes handling remain binding. An unapproved proposal row in the decision register cannot silently supersede D03/D07.

## 2. JSON fractional/exponent policy — P2 PRE-GATE, not a forced P1 feature

Current P1 `codec::number` rejects `.` and `e/E` numeric forms as precision loss rather than silently converting them; `Datum` contains `FiniteFloat` and exact decimal types. For P2's proposed **supported** JSON input capability, require an exact grammar/lexeme decision with sign, exponent, signed zero, high precision, large integer, overflow, encode round-trip, NaN/Infinity rejection and documented comparison semantics. Candidate representation is exact decimal lexeme or normalized coefficient+scale until explicitly conversion-safe. If a P2 source format cannot meet the declared parity gate, reject fractional/exponent tokens **before effects** and advertise the narrowed capability honestly; never claim general Python JSON compatibility. P2 pre-gate freezes policy and generated Python differential corpus (not necessarily P2 implementation of every number feature). `DATA-01` is this P2 policy/test artifact, not P1 rerun.

## 3. Source-unit contract is generic; connector consequences are separate

| Source / phase | Identity of one ackable unit | Ingress/replay and immutable acknowledgement conditions |
|---|---|---|
| Local CDR file / P2 | root + relative path + verified opened file generation/fingerprint; framed offsets within one parent SourceUnit unless separately approved resumable chunk protocol | bounded streaming decoding and source-generation recheck; one file finalization only after all branch/checkpoint obligations terminal, no 4 MiB *production* file-size implication from P1 |
| Kafka / P5 | namespace, topic, partition, offset, consumer-generation/epoch | ordered contiguous frontier; rebalances/cross-consumer ownership fenced, no offset commit while prior unit unknown |
| UDP SNMP trap / P3 | listener/session and available datagram/OID metadata, best-effort observation identity | no invented replay cursor/ack for UDP; explicit bounded loss/drop and optional durable ingress spool with independently proven guarantee |
| HTTP webhook / P5 | tenant/source, authenticated request idempotency key, immutable request generation | 2xx/202 promises durable intake **only if** a durable ingress record has committed; otherwise label weaker acceptance |

P1's 4 MiB cap is a safety limit for the ephemeral harness and does not determine production CDR size. `SRC-01` documents these capability tiers; `SRC-02` demonstrates P2 large-file streaming/finalization and crash behavior before durable P2 exit; future Kafka/trap/webhook connector tests belong to their own later gates.

## 4. Source-generation exclusion and original-plan replay (F6)

The approved [source-unit contract](../source-unit-contract.md) defines `BranchObligationId=(pipeline_revision,SourceUnitId,OutputRecordId,sink_slot_id,branch_version)`; `EffectId` derives from it. That revision is **immutable once an attempt is admitted**. A new config revision can produce new effect keys, but **must not** re-admit the *same protected source generation* while older work remains `unknown`.

**Proposed normative P2 rule, independent of implementation storage schema:**

1. The authoritative manager ledger durably stores an immutable, content-hashed **exact original plan/config snapshot**, including pipeline ID, revision, normalized plugin config, sink topology and versions, per-effect projection canonicals, source namespace/unit/generation identity, admission attempt/fence and digest. Before Start, the worker validates the immutable plan digest; recovery can retrieve by exact original attempt ID and verify hash. **Snapshot unavailable/corrupt ⇒ retain UNKNOWN/guard and reject replay under a reconstructed/new plan.**
2. Maintain a **cross-revision exclusion key independent of pipeline revision**, binding canonical logical pipeline/source namespace + normalized source identity + **same source generation/epoch + ackable source position**. For a file this includes rooted relative path + content digest + generation/file-ID semantics; a new revision cannot bypass it by changing its plan ID or sink layout. Guard acquisition for the new revision is an exact compare-and-set transaction against older protected attempt/source-generation records. Do not rely on `EffectId` equality or a per-revision ledger namespace for mutual exclusion.
3. Crash recovery of an older `UNKNOWN` unit loads **only** that original plan and replays under its original `BranchObligationId`s and original `EffectId`s. Original A-confirmed receipt is reused without reopening/writing A; original B-unknown idempotency key is used solely for authenticated reconciliation or a proven receiver-idempotent retry according to the frozen duplicate-risk rules. If B is non-idempotent and its outcome unprovable, keep UNKNOWN, hold source ack and block unsafe retry rather than assert exactly-once.
4. A configuration edit is effective for **new unprotected source generations/runs only** under the declared plan-change policy. It does **not** change old source-unit obligation membership, checkpoint/ack frontier or source-finalization guard. A newer revision cannot independently read+publish/finalize/re-emit that same guarded source generation while B is unknown; concurrent new-plan admission attempts must return an explicit protected-source refusal without side effects.
5. Even when old A is confirmed, **A-confirmed/B-unknown means no source ack**. Manager reboot or source rescan must rebuild old exclusion/ack-frontier intent before admitting new runs. Replacing/rehoming an unresolved source unit to a newer revision requires a distinct independently reviewed migration/compensation DCR, not a new `EffectId` calculation or operator signature. P2 schema/CAS implementation details need independent P2 schema design approval.

### SRC-03 — concrete multi-revision crash/replay acceptance script

| Step | Required observable/proof |
|---|---|
| 1. Admit revision R1 for fixed source generation G with sink branches A/B | Manager stores immutable plan snapshot H1, source-exclusion key G, attempt/fence E1 |
| 2. A commits/receipts, B performs an ambiguous write and times out | A receipt keyed to original EffectId A1, B UNKNOWN under original B1; source ack frontier blocked |
| 3. Edit to revision R2 with altered branch topology/projection so EffectIds differ | H2 distinct; no overwrite/reinterpretation of H1 or A1/B1 |
| 4. Crash/actually restart manager and worker, then restore ledger/journal | Old H1/H1 digest and guard G recovered **before** new scheduling; no start admitted based merely on new session |
| 5. Concurrent R2 run seeks same G/source position | Exact ledger CAS denies it, including sink A reemit/finalization and new EffectIds; no second owner/effects |
| 6. Reconcile old receipt A1 | Same receipt reused, **A write count remains one**, no fresh A effect |
| 7. Reconcile old unknown B1 under R1 | For fixture with idempotent receiver, retry uses original B1 and external effect count remains one; for no-idempotency fixture, UNKNOWN remains guarded until independently proven safe |
| 8. Attempt new R2 source acknowledgement/deletion while old barrier remains unresolved | Denied; `decided/ack_requested/ack_confirmed` frontier not advanced and source unchanged |
| 9. Lose original plan snapshot or journal during separate fault case | Fail closed UNKNOWN, guard held, **never reconstruct from R2** |
| 10. Complete R1 only with authenticated matching receipt/quiescence then deliberately replan | No lost old obligations/duplicate A; release and successor admission only by the frozen exact identity+fence CAS |

For the **idempotent fixture**, assert no data loss and no double A/B external effect. For the **non-idempotent ambiguous B fixture**, no unsafe replay/ack is permitted; do not promise exactly-once where the evidence cannot support it. Capture actual effect ledger, hashes, CAS conflicts and after-restart source metadata. `SRC-03` is a **P2 durable implementation/acceptance proof** after separately authorized P2 work, while its ledger/snapshot/exclusion decisions are P2 PRE-GATE design prerequisites.

## 5. D04 persistence choice and portable CAS

The accepted architecture has **P2 SQLite manager control ledger + separate SQLite worker admission/completion/outbox journal** and later **P4 PostgreSQL manager + worker-local SQLite**; HA is deferred to P6+. PostgreSQL is not a P2 requirement. At P2 schema freeze specify SQLite WAL/fsync/checkpoint/upgrade, server-/monotonic-time guard semantics, failure-atomic completion/outbox and source-exclusion CAS, and journal filesystem persistence. At P4 independently justify Postgres operating cost/benefit and run the identical attempt/source-generation CAS/recovery suite on SQLite and Postgres; do not assume differing DB semantics are identical because a trait is shared.

## 6. Early performance and compatibility evidence

At **P2 exit**, measure pinned Python and Rust on *matched* durability tier and real confirmed useful outputs/ack tier, CPU-seconds/confirmed record, p95/p99, high-water process RSS, memory distribution, duplicate/loss/unknown and journal I/O. Python and Rust versions/corpora/CPU/disk must be pinned. **If matched Python durable guarantees are unavailable, classify `PERF-INCONCLUSIVE`, not P2 performance PASS and not a P1 scratch-vs-P2 durable comparison**; retain safety proof and submit a bounded follow-up performance plan for Orchestrator decision. The >=2× CPU throughput **or** >=30% lower peak worker-memory at matched throughput is retained as the P6 product evaluation target, not a promised P2 gate result.

Add seeded generated Python-vs-Rust semantic differential corpus for precedence, missing/null, large integers, decimal/exponent policy, boolean semantics, escaping and invalid options; minimize mismatches and classify them as PRESERVE, CORRECT, ENHANCE or DEFER, never silently accept unintended divergence.

## 7. Phase-specific proof ownership and nonregression

| Proof | Must be ready by | Explicit exclusion |
|---|---|---|
| `DATA-01` JSON fractional policy/schema/corpus | P2 PRE-GATE; implemented subset tested before P2 exit | No P1 float mandate |
| `DATA-02` SNMP `OctetString` binary JSON/schema/Counter64 Python oracle | **Before P3 SNMP+JSON capability admission** | **Not P2 implementation / not retroactive P1** |
| `SRC-01` generic source model and acknowledged tiers | P2 PRE-GATE paper exercise | Future connector implementation remains P3/P5 |
| `SRC-02` streaming large CDR file/finalize/crash | P2 durable local file acceptance | Not P1 4 MiB increase |
| `SRC-03` old-plan snapshot cross-revision source exclusion and replay | P2 PRE-GATE model, P2 durable acceptance execution | No new EffectId from R2 on guarded R1 unit |
| `DB-01` SQLite journal/ledger crash/power-loss/fill | P2 durable acceptance | No fake fsync proof |
| `DB-02` PG/SQLite identical contract conformance | P4 before distributed admission | No P2 PG mandate |
| `PERF-01` matched guarantees performance pilot | P2 exit or `PERF-INCONCLUSIVE` with explicit decision | Never compare P1 scratch with P2 durable |
| `COMP-EXT-01` seeded differential | P2 supported subset; continue for later connectors | Not broad Python compatibility claim |

**Disposition:** Exact bounded D03/D07 proposed amendment and F6 cross-revision exclusion are submitted for **fresh independent DCR review**. Until accepted, the original P0 rules govern. This ADR changes no existing P1 artifact, code, fixture, production mode or authority.
