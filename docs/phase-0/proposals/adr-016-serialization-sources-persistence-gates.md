# ADR-016 (PROPOSED) — Data mappings, source identity, persistence and early evidence

**Status:** PROPOSED / DESIGN_CHANGE_REQUEST required for accepted D03/D07 re-entry timing; independent approval pending. **Owners:** Data/Serializer, SNMP Adapter, Persistence, Migration/Performance; **frozen baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. No new P1 plugin scope.

## 1. D03/D07: make the phase dependency explicit

Accepted D03 defers binary/decimal/timestamp JSON mapping to P5; D07 needs binary-safe `OctetString` representation in P3. **Proposed correction:** P3 SNMP's exact binary JSON mapping is decided and Python-oracle tested **before enabling its P3 output**, while full generic serializer coverage may remain P5. Preserve raw `Datum::Bytes`; forbid implicit UTF-8, float coercion or ambiguous base64/hex conventions. SNMP adapter owner chooses a versioned schema/field-level encoding only after comparing pinned Python outputs, including empty, invalid UTF-8 and embedded-NUL octets, Counter64, nested varbinds and trap metadata. Until then JSON output for unsupported bytes is rejected pre-effect or the plugin is not advertised.

## 2. JSON fractional/exponent numbers

P1 `codec::number` rejects decimals and exponent syntax as precision loss while `Datum::FiniteFloat` and exact decimals exist internally. This is honest P1-subset behavior, **not** general JSON numeric compatibility. At P2 PRE-GATE decide accepted source formats and exact JSON-number lexical parsing rules: finite numbers, scale/exponent, signed zero, overflow, high precision and round-trip encode. Proposed preferred representation is exact validated decimal lexeme/coefficient+scale when feasible, with explicitly declared opt-in `Float` conversion/rounding only where selected plugin semantics require it. P2 must either pass a Python differential corpus for supported fractional JSON or **refuse** that input with an explicit capability contract before effects. No silent lossy fallback, NaN/Infinity acceptance, or promise of broad parity without evidence.

## 3. Source-unit applicability (not file-only)

Frozen `SourceUnitId` is an ackable source position and `InputRecordId` carries a decode index. Maintain the generic obligation graph but require a short P2 pre-gate paper exercise and later connector-specific admission tests:

| Source | Identity/position | Replay or acknowledgement truth |
|---|---|---|
| Local CDR file (large/streaming) | stable file generation + relative path + verified fingerprint; framed chunk offsets are within one ackable unit unless an independently proven resumable chunk contract exists | delayed file finalization until every decoded part/branch/checkpoint closes; bounded streaming decode, TOCTOU and crash validation |
| Kafka (P5 candidate) | cluster/topic/partition/offset + generation/consumer-group authority | ordered committed offset only after contiguous decided source-unit frontier; partition rebalances fenced |
| SNMP UDP trap (P3) | listener/session time window + datagram/event digest/sequence when available; no invented durable source cursor | ingress may drop on overload; no upstream replay/ack guarantee unless durable local ingress spool actually commits |
| HTTP webhook (P5) | request idempotency key + authenticated tenant/source identity | 2xx/202 cannot promise durable acceptance until ingress journal commit; otherwise explicit weaker response tier |

P1's 4 MiB raw cap is a *harness safety bound*, not a promised production file-size limit. Before P2 finalization define the supported local file size policy, streaming codec/framing, disk and memory caps, partial decoder outcome, and source generation revalidation; no automatic unlimited-file acceptance.

## 4. EffectId and configuration changes

`BranchObligationId` deliberately contains the immutable **pipeline revision** and `EffectId` hashes its output projection; this prevents different plans from falsely sharing receipts. On crash recovery and in-progress retries, reload the **original durable plan snapshot/revision** and reuse the **original identity and EffectId**. A config edit only affects newly admitted runs/units as specified by immutable scheduling authority; it cannot silently rewrite an old unknown/unfinished unit's sink effects. Verify replay of old A-confirmed/B-unknown under a newer revision does not create new B idempotency keys or double-ack A. Explicit migration of still-active source units needs a separately approved reconciliation DCR.

## 5. D04 backend justification and portability

Retain accepted **P2 SQLite manager + separate SQLite worker journal**, single active manager; **P4 PostgreSQL manager + worker-local SQLite**. PostgreSQL is **not** a P2 dependency nor implicitly required by postponed HA. P4 must justify PostgreSQL via deployment, observability, transactional and operational needs and test *the same* ledger state/CAS/fencing/recovery properties on both backends. The common trait may not hide differing fsync/WAL/quorum/durability behavior; validate rollback, snapshot, schema migration, disk-full and power-loss cases independently. Re-enter D04 at P4 if measured cost/requirements favor a different topology.

## 6. Benchmarks and Python differential

At **P2 exit**, execute the first matched-tier Python/Rust throughput, CPU per confirmed record, p95/p99, peak RSS, duplicate/loss, and journal I/O comparison on pinned corpora and stated environment. Baseline P1 scratch throughput is **not** comparable to durable P2 source ack/fsync. The >=2x useful CPU throughput OR >=30% peak-memory improvement remains a **P6 business go/no-go target**; P2 must report values and identify regressions, not falsely promise the target. Expand differential expression tests (operator precedence, null/missing, integer/decimal boundaries, escaping, rejection) through a seeded generated corpus and minimized mismatches; distinguish Python observed behavior from deliberate Rust CORRECT/ENHANCE/DEFER choices.

## Required gate artifacts

`DATA-01` bounded fractional/exponent JSON corpus and explicit unsupported behavior; `DATA-02` P3 SNMP binary JSON decision and pinned Python proof; `SRC-01` source matrix above with safe ack tier; `SRC-02` large-file streaming/finalize/crash proof; `SRC-03` old revision crash/replay idempotency; `DB-01` real SQLite journal power-loss/fsync/rollback acceptance; `DB-02` P4 equivalent CAS suite SQLite/PostgreSQL; `PERF-01` P2 matched-guarantee pilot; `COMP-EXT-01` seeded expression/serializer differential suite.

Nothing in this ADR modifies frozen P1 caps, the accepted Python oracle pins, durable source-ack ordering or P2 authorization. A design reviewer must resolve the P2 scope decisions and D03/D07 phase amendment before the corresponding capabilities are admitted.
