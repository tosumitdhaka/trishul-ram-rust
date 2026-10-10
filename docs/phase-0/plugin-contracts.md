# TRAM Rust — proposed plugin, record and configuration contracts

**Status:** Historical Phase 0 proposed wording, frozen at approved `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. The optional post-P1 corrections in this draft amendment remain **unapproved**; interfaces below are conceptual, not production trait implementations.

## P1 scope clarification

P1 is an **isolated read-only-source/scratch-output, ephemeral-only test harness**, with no production output publication, source ack, durable result, external network or arbitrary filesystem access. The concrete supported YAML is [the P1 compatibility matrix](p1-compatibility-matrix.md), whose fixture is binding for proposed semantic test expectations. The example farther below is **illustrative architecture notation only**, not a supported YAML contract. Strict resource admission and typed failure semantics: [resource budgets](resource-budgets.md). Lineage and future durable receipts: [source unit contract](source-unit-contract.md).

## 1. Registry before catalog

Four public plugin *roles* are mandatory even if initial implementations are few:

- **Source**: batch pull, scheduled polling or long-running push/listen; exposes explicit acknowledgement capability.
- **Serializer**: raw bytes ⇄ typed records; schema-aware where supported; streaming/batch framing is declared.
- **Transform**: per-record or keyed-stateful record operation; may drop, expand or produce error dispositions.
- **Sink**: accepts encoded payload or structured batches; returns write, flush and durable-confirmation semantics.

A plugin manifest provides stable `category/name`, semantic implementation version, compatible engine contract version, config schema, supported operation modes, accepted/produced value kinds, acknowledgement/durability tier, ordering guarantees, batch-size limit, cancellation behavior, and required external permissions/secrets. Runtime instance factory is registered at build time initially.

**Do not confuse selection with hot loading.** New native plugins require a build/redeploy in v1. External-process/WASM adapters, stable ABI and third-party code sandboxing are independent future designs. Runtime YAML hot-reload can replace *plans* subject to execution fencing without loading arbitrary code.

### Conceptual lifecycle

| Role | Conceptual operations | Mandatory output |
|---|---|---|
| Source | validate → open → next/subscribe → ack/nack/checkpoint → close | bounded ingress items + stable source identity where available |
| Serializer | validate → decode/encode (possibly streaming) | typed records or bytes + deterministic error classification |
| Transform | validate → initialize → apply → snapshot/restore → close | zero/one/many records or explicit discard/error |
| Sink | validate → open → write → flush/confirm → close | per-item/partition outcome and confirmation tier |
| Registry | enumerate → schema → match capability → instantiate | deterministic plan selection or rejection |

Closed/failed plugins must not continue to produce writes after ownership/cancellation invalidation to the extent a protocol allows. Plugin panic must become a bounded run failure rather than corrupting the manager ledger; untrusted native code cannot be treated as process-isolated.

## 2. Canonical data model

Avoid using `serde_json::Value` as the *only* internal representation: the engine must preserve telecom-specific `u64` counters, byte arrays, timestamps, large integer/decimal values, missing vs null and metadata. Proposed conceptual types:

```text
RecordEnvelope {
  data: ordered map<string, Datum>,     # field order when format requires it
  metadata: {source, received_at, ...}, # namespaced immutable provenance
  origin: SourcePosition | None,        # replay/ack semantics
  schema_ref: SchemaIdentity | None,
}
Datum =
  Null | Bool | SignedInteger | UnsignedInteger | BigInteger |
  Decimal | Float | String | Bytes | Timestamp |
  Array<Datum> | Object<map<string, Datum>>
IngressItem =
  RawBytes(Bytes, Meta, SourcePosition?) |
  StructuredRecords(Batch<RecordEnvelope>, Meta, SourcePosition?)
```

These are candidates, **not** final Rust API or ABI. Choose allocation/copy strategy (borrowed buffers, `Bytes`, COW/arena or owned `Arc` batches) using profiling; correctness must not depend on memory aliasing. Bytes must not be silently UTF-8-coerced. JSON adapters explicitly define encoding conventions for bytes, timestamps, non-JSON numbers and `NaN` values.

Record identity and source checkpoint are distinct. A batch can share source position while containing multiple records. Traps and other unacknowledgeable push events declare that limitation. Preserve per-sink mutation isolation.

## 3. Plan and config validation

**Canonical P1 config:** use the [exact pinned P1 YAML fixture](fixtures/p1/pipeline.yaml) and the [P1 compatibility matrix](p1-compatibility-matrix.md), not a schematic snippet with unsupported keys or arbitrary external paths. The sample previously shown here mixed conceptual/unsupported keys and could be mistaken for a valid P1 plan. Future production plugin YAML examples require their own accepted versioned schema and capability gate.

- Existing Python YAML should be accepted for *supported semantics*, with a declared compatibility subset. Do not claim universal config parity in Phase 1.
- Unknown plugin, misspelled option, unsupported operation, ambiguous schema, invalid env reference and unavailable secret fail **before opening a source**. New engine-specific config changes must be versioned with migration guidance.
- Compile transforms and predicates once per plan; parameter values and schema references are validated at compile/load time where feasible.
- Compatibility tests must cover default values, field aliases, null vs absent, integer bounds, expression truthiness, error reporting and sensitive-config redaction.
- Plugin capability is a function of **plugin version + mode + options**, not merely a connector name. For example, polling vs traps, v2c vs v3, or ackable vs non-ackable matters.

## 4. Delivery-relevant capabilities

```text
SourceCapabilities:
  mode: batch | stream | both
  replayable: bool
  ack_mode: none | per_record | per_batch | checkpoint
  ordering: unordered | partition_ordered | total_ordered

SinkCapabilities:
  input: bytes | structured | both
  confirm: none | accepted | durable | transactional
  idempotency_key: unsupported | optional | required
  ordering: unordered | partition_ordered | total_ordered

TransformCapabilities:
  state: stateless | keyed | global
  cardinality: one_to_one | filter | expand | aggregate
  deterministic: bool
```

Terms are provisional and need precise codec-specific interpretations. The planner rejects any requested delivery contract that no end-to-end path can support. A sink returning `accepted` into a buffer is not automatically `confirmed`.

## 5. Initial plugin implementations and test strategy

The P1 local sink is **scratch-only** and cannot implement `fsynced_local`/durable confirmation; the P2 local sink adds idempotent persistent staging+publication+fsync and source checkpoint barrier after architecture-specific tests. The P1 parser must reject unsupported Python options before any I/O, not silently accept default production sink settings.

**First vertical slice (engine proof, not feature-complete product):**
- **P1:** read-only local file source and exclusive **scratch-only** local sink, with no durable publication, acknowledgement or destructive source finalization. **P2 only after a separate durability gate:** idempotent staging/publish/fsync and guarded source finalization.
- JSON serializer (including NDJSON as separate framing capability only if proven).
- Stateless `rename`, `add_field`, `filter`, `drop` transforms.
- One multi-sink route and one explicit invalid-config/record failure path.

**Next vertical slice (telecom proof):**
- `snmp_poll` and `snmp_trap` source operations; optional `snmp_trap` sink after receiver/conformance validation.
- Native `trishul-snmp` manager/listener/notifier; compiled JSON MIB bundle consumed via library. TRAM-specific mapping translates varbind OID/type/value into stable records.
- Live v1/v2c/v3 tests, community checks, USM wrong-key/privacy handling, duplicate/replay, Counter64 and malformed frames; traps never claim replayable acknowledgement.

**After foundation:** Kafka, REST/webhook, SFTP, other serializers and stateful transforms, each behind its own capability-specific test suite. Priority from production value and risk, not Python source-file order.

## 6. Test seam and versioning contract

- The registry must allow deterministic **test plugin** implementations for failures, timeouts, duplicate data, slow sinks and cancellation.
- Include plan compile tests, plugin config schema tests, descriptor honesty assertions, cross-version schema compatibility tests and negative capability cases.
- Every plugin release adds golden input/output fixtures, failure/recovery cases, overload/cancellation evidence and performance where relevant.
- A plugin cannot be considered supported until its manifest, docs and executed tests agree.

**Post-P1 proposals (not approved):** [runtime/dataflow](proposals/adr-013-p2-runtime-concurrency.md), [standalone failure domain](proposals/adr-014-standalone-worker-failure-domain.md), [uncertain attempts](proposals/adr-015-unknown-recovery-operations.md), [JSON/SNMP/source compatibility](proposals/adr-016-serialization-sources-persistence-gates.md). Existing P1 fixture and validation semantics are not changed by this editorial clarification.
