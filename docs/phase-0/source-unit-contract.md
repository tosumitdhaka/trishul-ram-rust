# P0-R2 — Source units, fan-out lineage and acknowledgement barrier

**Status:** Phase 0 approved/frozen source-unit obligation contract at `2cedaeac5657a8941fe9366f04029cd11b0cfd30`; P2+ production delivery implementation remains a later gate. P1 uses only [ephemeral semantics](p1-safety-boundary.md), but its test harness must exercise the *logical* lineage/obligation evaluator with simulated receipts and no destructive acknowledgement.

## 1. Immutable identity model

`SourceUnitId = H(source_namespace || normalized_source_identity || source_epoch || position)` is stable across retries *for the same source payload/generation*. The source plugin provides a canonical namespace and identity fields; a missing durable identity disqualifies the source from stronger replayable delivery claims. The source unit is **one ackable source position**, not one decoded record. A file read commonly produces N decoded rows under one SourceUnitId; Kafka can use a partition+offset, etc.

`InputRecordId = (SourceUnitId, decode_index)`. `OutputRecordId = (InputRecordId, transform_path, expansion_index_path)`, with deterministic expansion ordinal per input/transform. `BranchObligationId = (pipeline_revision, SourceUnitId, OutputRecordId, sink_slot_id, branch_version)`. A branch slot is the stable **configuration position/ID**, not just a connector type (two local sinks are different slots). `EffectId = H(BranchObligationId || output_projection_digest || output_frame_index)`. `ReceiptId = (EffectId, sink_commit_generation)`. Hash algorithms and encoding canonicals must be versioned; plan changes never silently reuse receipt keys.

Expansion 1:N inherits the full ancestry/source-unit set; filtered 1:0 yields a *recorded terminal filter outcome* for that input. Aggregates/window results may depend on multiple source units: output lineage contains a set of contributors and checkpoint frontier; each contributing unit remains pending until its relevant output/state obligations are durably resolved. Stateful aggregate release and output/ack must use one recovery-safe checkpoint protocol; never acknowledge inputs solely because an intermediate in-memory aggregate exists. P1 explicitly **rejects** all expansion and stateful operators; simulate these in abstract obligation tests for P2+.

## 2. Required obligation graph and decision predicate

On accepted decoding, allocate an obligation barrier **before** emitting records: record expected row count or increment a controlled open-child count, then **seal** once decoding and all transforms/branch enumeration complete. A source unit is not eligible for acknowledgement while any parent fan-out or expansion can still add obligations.

For each output record, evaluate each configured sink branch independently:
- predicate false => a terminal `filtered_for_sink` disposition with branch key (no effect);
- predicate true => create a concrete branch obligation, including each serialized frame/chunk and a required sink confirmation tier;
- transform error => `failed` or `dlq_pending` with lineage, not silent omission;
- zero decoded outputs => explicit `empty_source_unit` disposition (eligible only under an agreed zero-output policy, not by an empty set of receipts);
- zero after a deliberate global filter => explicit `filtered_global` disposition;
- implicit unexpected zero output => `failed` until diagnosed.

A SourceUnit is **DECIDED** iff: barrier sealed; every expected branch/effect and transformation disposition is terminal at the declared tier; every required retained state snapshot and output/checkpoint frontier is durably committed; no `pending`, `unknown`, `failed` or unresolved `dlq_pending` obligation exists. An agreed deliberate drop or durable DLQ terminal result may permit acknowledgement only under that pipeline's explicit loss policy. A successful write buffer flush without durable receipt does not close a durable obligation.

The source manager may call source `ack(position, decided_checkpoint_id)` **only after** a transactional durable DECIDED checkpoint. Acks occur monotonically for consecutive positions within the same ordered source partition/stream; a later completed unit cannot advance a gap. For non-ordered units/files, acks are identity-specific and never imply an unrelated unit has completed. Persist the acknowledgement intent/idempotency marker **before** issuing destructive finalization; after crash, retry idempotently and revalidate source identity at the effect boundary. Distinguish `decided`, `ack_requested` and `ack_confirmed`.

## 3. Durable transaction and replay

The manager authority ledger must atomically commit within one DB transaction: terminal branch receipts/dispositions and seal status, state revision/checkpoint ref, decided frontier, and source-ack intent, guarded by pipeline revision+attempt fence. **Never** require an impossible atomic transaction across an external sink and that DB: if sink effect succeeds but receipt persistence fails, the branch becomes `UNKNOWN`; replay may duplicate externally. Persist a replayable intent/manifest **before** first external write. Sink idempotency keys use EffectId when the sink supports it; if not, report the duplicate window.

Worker-local completion and outbox notification must be atomic. The manager stores each result idempotently under exact attempt/fence/checkpoint identity; loss of callback response results in retransmission of the **same** completed outcome, not another run. A partially confirmed fan-out persists the confirmed branch receipts and replays only genuinely uncommitted branches on retry, unless the receiver cannot establish whether a timed-out effect happened; those unknown branches need reconciliation or idempotent replay, never an invented failure or success. Confirmed branches with no replay-safety may still be externally duplicated after control DB crash; report the uncertainty honestly.

| Scenario | Safe decision |
|---|---|
| Input has 3 JSON objects and two selected sinks | 6 branch decisions plus any filtered predicates; ack only when all are terminal, barrier sealed and checkpoint committed |
| Sink A confirmed, sink B timed out | A receipt retained; B UNKNOWN; source unit is not ackable; replay uses EffectId and may duplicate B if sink lacks idempotency |
| 1:3 expand on one input | stable child ordinals 0..2, each with branch obligations; source unit remains open until all closed |
| All records deliberately filtered | terminal filter decisions recorded and barrier sealed; no fabricated output receipt |
| Decode error | source unit FAILED or durable DLQ result by explicit policy; not an empty successful file |
| Interrupted checkpoint after writes | recover from committed receipts or unknown effect classification; do not mark source processed from partial manager state |

## 4. Local-file source identity and finalization

Production local source canonical identity: normalized root + normalized *relative path* + content SHA-256 + observed source generation (stable size/mtime and, where available, inode/file-id). Symlink traversal is disallowed by default. Content hash distinguishes same-path replacement, and generation distinguishes change-in-progress; compare canonical open file identity and content hash again **immediately before** move/delete/mark processed. If changed, leave source untouched, mark `stale_source`, and start a distinct unit on rescan. A hash match is not sufficient when concurrent replacement is possible: use open-file identity and no-follow operations or fail closed on platforms without safe atomic primitive.

A source file may contain zero, one or many decoded records but is one SourceUnit. File finalization (move/delete/skip-processed mark) must happen **only** after all sink and checkpoint obligations are durably decided. Move or delete is a separate idempotent source effect with its own durable intent. No manual reprocessing skip solely on pathname.

## 5. Local sink staging and publish durability (P2+ only)

Write under a private per-attempt staging path on the **same filesystem** as destination. Persistent manifest binds `EffectId`, destination, staged checksum/length, immutable pipeline revision and expected prior-destination state. Flush file payload, then `fsync` file; atomically rename/link publish to final path (must not silently overwrite unrelated file); `fsync` parent directory before claiming the `fsynced_local` tier. Use no-follow/path confinement/atomic-no-replace operations as supported. If cross-device rename, directory fsync or atomic exclusivity is unavailable, reject requested durable tier or downgrade **only by explicit policy**; do not copy+delete and call it atomic. File/directory durability remains subject to filesystem/hardware guarantees and must be documented.

Recovery enumerates persistent manifests: staged-only is retryable, final matching `EffectId`+checksum is confirmed without republishing, final conflicting is blocked, and rename completed but journal write lost requires checksum/provenance reconciliation. A timestamp-derived filename alone is not an idempotency key. Never silently replace an already-published different `EffectId`. Claim completion only after receipt is durably recorded, then checkpoint/ack. Orphan staging cleanup happens after retention/recovery rules, not before.

## 6. Required regression identifiers (P1 simulator vs P2 real effects)

- `ACK-01`: 1 file → 3 records → two sinks, one filtered decision; barrier counts precisely and ack waits for last receipt.
- `ACK-02`: sink A confirmed / B unknown, with replay, exact receipts and no source ack; test both idempotent and non-idempotent sinks.
- `ACK-03`: expand 1:3, global filter zero, empty JSON array, invalid JSON; sealed barrier and no vacuous successes.
- `ACK-04`: checkpoint failure after sink success; retry with uncertainty and no premature ack.
- `FS-01`: same-path input replacement after read is rejected at finalize without deleting replacement.
- `FS-02`: kill before staged fsync, after fsync before rename, after rename before dir fsync, after dir fsync before receipt, after receipt before checkpoint, after checkpoint before ack; verify contents, manifests and unchanged source until decided.
- `FS-03`: conflicting destination, symlink, cross-device, disk-full, unsupported directory fsync, and orphan cleanup fail closed.

## Post-freeze proposed connector/revision exercise (NOT APPROVED)

See [ADR-016](proposals/adr-016-serialization-sources-persistence-gates.md) for a review proposal mapping this existing generic source-unit contract to streaming CDR files, Kafka, UDP SNMP traps and HTTP webhooks. The original identity formula binds `BranchObligationId` and `EffectId` to the immutable pipeline revision. Proposed [SRC-03](proposals/adr-016-serialization-sources-persistence-gates.md) additionally requires a durable **original-plan snapshot lookup** and a **cross-revision source-generation exclusion key independent of pipeline revision**, with manager CAS and blocked acknowledgement frontier. A newer revision cannot bypass an unresolved original A-confirmed/B-unknown guard by minting different EffectIds; the old receipt remains original and B remains uncertain/idempotently reconciled. These are reviewable **future P2 design/acceptance requirements**, not approved P1 connector coverage or relaxation of the frozen source ack predicate.
