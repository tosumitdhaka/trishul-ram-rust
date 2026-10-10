# TRAM Rust — proposed system architecture

**Status:** Historical Phase 0 proposal text, independently approved at immutable `2cedaeac5657a8941fe9366f04029cd11b0cfd30` (Orchestrator [decision](https://github.com/tosumitdhaka/trishul-ram-rust/pull/1#issuecomment-6089485009)). The post-P1 proposals referenced below are **not frozen or approved**.
**Reference pins for this assessment:** Python `tosumitdhaka/trishul-ram@ff380725b86c9569901ea89ad6771623847cc4e2` (documentation-only successor to released v1.8.0); Rust SNMP `tosumitdhaka/trishul-snmp-rust@bef2b7643dddd4c28326febe9e310575110b6f4b` (`trishul-snmp` 0.1.1). Revalidate live heads before subsequent gates.

## Normative phase boundary and review precedence

The original P0 review returned `CHANGES_REQUIRED`. This revised package remains **PROPOSED**. Normative constraints are fully enumerated in [P1 safety](p1-safety-boundary.md), [source-unit obligations](source-unit-contract.md), [attempt authority](attempt-protocol.md), [compatibility matrix](p1-compatibility-matrix.md), [resource budgets](resource-budgets.md) and [decision register](decision-register.md). Where an earlier generic architecture sentence appears to require a worker journal for P1, **P1 is expressly non-production ephemeral-only** and cannot perform final publication/source acknowledgement. The durable-before-effect invariant becomes mandatory for P2+.

## 1. Purpose, principles, exclusions

Build an idiomatic, secure, resource-bounded, observable Rust pipeline system with **equivalent intended telecom mediation features** to Python TRAM over time. This is **not** a line-by-line translation, an obligation to preserve internal Python behavior, or a mandate to ship all connectors before the core. Existing documentation, tests and production workflows are *evidence* and candidates for compatibility oracles, not permission to propagate documented defects.

- **Accepted external contracts** (YAML behavior for supported configurations, public REST responses/routes where committed, record/wire representation, CLI workflows, delivery claims) are to be preserved or explicitly versioned and migrated.
- **Internal choices** (threads, Python objects, in-process decorators, callback implementation, internal agent transport, private DB layout) may change.
- **Known issues** become failure scenarios and negative acceptance tests, not porting requirements; historical plans are not evidence that every named issue is reproduced on the latest code.
- Scope first: one supported end-to-end pipeline path. No promises of immediate plugin catalog parity, native dynamic shared-library plugins, manager HA, exactly-once external output, universal custom code execution, or every existing Python API endpoint.

## 2. System boundaries

```text
                         External users / CLI / UI
                                    |
                         Public REST API + auth
                                    |
                   +-------------------------------+
                   |      CONTROL PLANE            |
                   | config/version | scheduler    |
                   | capability plan | placement    |
                   | execution ledger | operations  |
                   +-------------------------------+
                         | ExecutionService trait
                         |  local in-process OR
                         |  authenticated RPC
            +------------+------------------+
            |                               |
    STANDALONE PROCESS               DISTRIBUTED WORKERS
    embedded worker runtime          native worker processes
    local dispatch                   remote dispatch
            |                               |
            +--------------+----------------+
                           |
                 SHARED EXECUTION ENGINE
    source -> deserialize -> global transforms -> per-sink
               predicates/transforms -> serialize -> sinks
                   |            |
                checkpoints    DLQ
                           |
            plugin registry / budgets / telemetry
```

**Key invariant:** there is **one** engine and **one** execution-plan semantics. Standalone and distributed are topology adapters, not separate processing implementations. Standalone invokes the same service interface in-process; distributed uses an authenticated, versioned request/response protocol. No record-by-record RPC is required for a local pipeline. In distributed mode manager does not relay the normal data path.

### Control plane

- Own pipeline definitions, revisions, schedules, deployment intent, run requests, worker membership, capability-aware placement, authoritative attempt ownership, durable audit and desired/observed status.
- Compiles pipeline YAML into a validated, versioned execution plan; fail closed if a required plugin, operation, version or option is unsupported on the selected runtime.
- Schedule persistence and timezone/misfire semantics must be explicit; do not reproduce Python's documented UTC-only/misfire limitations by accident.
- Start with **one active manager** in distributed mode; durability and explicit fencing lay groundwork for later manager HA, but do not imply leader election is implemented.
- Separate public REST/CLI/API compatibility from private worker protocol. Public compatibility is an explicit matrix, not implied by matching endpoint names.

### Execution plane

- Loads immutable plans and plugin instances; manages bounded batch/stream ingress, decoding, per-record transform isolation, independent sink branches, acknowledgements, checkpoints, cancellation, and lifecycle.
- Stateless and stateful transforms share a pipeline contract; stateful keys run sequentially initially. Ordered parallelism/sharding is separately designed and tested.
- Native `trishul-snmp` owns SNMP transport, codec, USM, manager operations, notification lifecycle and compiled-JSON MIB enrichment. A TRAM adapter owns user config, event-to-record mapping, source identity and pipeline admission.
- Instrument sources, transforms and sinks using common delivery/outcome and resource-accounting types.

### Persistence boundary (proposal)

- **Standalone:** SQLite-backed authoritative control ledger **and** a logically separate local worker journal; two store roles may have separate files. Define power-loss durability settings and backup/migration procedures before release.
- **Distributed:** PostgreSQL authoritative control ledger for manager; worker-local durable journal (SQLite acceptable) for admission, revocation, result/outbox and recovery. Worker code does not directly mutate manager tables.
- Storage interfaces should be explicit Rust traits with transactional methods, not generic CRUD used indiscriminately by the engine.
- No schema migration from Python is implicitly guaranteed. Preserve import/export and test any mixed-version or state-migration promises before exposing them.

## 3. Proposed code boundaries

A **candidate** workspace, to be revised only by design review:

| Crate/app | Primary ownership |
|---|---|
| `tram-model` | Canonical typed values, records, identity, errors, disposition and outcome |
| `tram-config` | YAML parsing, env/secret references, schema and semantic validation |
| `tram-registry` | Static factory registration, manifests and capability negotiation |
| `tram-engine` | Execution planner, queues, fan-out, transforms, retries, cancellation, DLQ |
| `tram-plugins` | Initial local/JSON/stateless implementations and later connector adapters |
| `tram-snmp` | Narrow TRAM adapter over pinned `trishul-snmp` |
| `tram-control` | Scheduler, admission/placement, run ledger and reconciliation |
| `tram-storage` | Manager/worker journal implementations, migrations and transactions |
| `tram-transport` | In-process ExecutionService and authenticated versioned remote transport |
| `tram-api` | Public management REST API, auth, metrics/health and asset serving |
| `apps/tram`, `apps/tram-worker` | Standalone/manager CLI and worker binaries |
| `tram-testkit` | Shared golden fixtures, synthetic plugins, failure/clock controls |

Names and exact crate splits are proposed, not frozen. Avoid excessive micro-crates before compilation/testing proves the boundaries.

## 4. Processing lifecycle

1. Config is parsed into a **versioned, normalized plan**; plugin-specific configuration validates before any effects.
2. Registry resolves exact plugin names/versions and capabilities; admission checks supported formats, stream/batch mode, acknowledgement tiers, ordering, durable state and resource budgets.
3. **P2+ only:** Control plane durably records run intent/attempt identity and obtains an execution fence before dispatch. P1 instead validates its read-only source/scratch-only sink and creates a unique ephemeral test-run capability, with no durable or external authority.
4. **P2+ only:** Worker durably reserves an authorized attempt before side effects, with exact fence/session/plan checks and idempotent replay. P1 permits only isolated scratch effects, with no remote admission or durability claim.
5. Source emits bounded `IngressItem`s (raw bytes or native structured events) with metadata and an optional acknowledgement cursor.
6. Decoder yields typed records. Global transforms run with per-record failure isolation; sink branches use **independent logical record views** (copy-on-write or equivalent).
7. Sink predicates, branch transforms, serializer and sink write/flush produce explicit per-sink outcomes. Confirmed output, filter, explicit drop and successfully retained DLQ are distinguishable.
8. **P2+ only:** Source acknowledgement/finalization occurs only after the applicable outcome/checkpoint obligations are met. **P1 never acknowledges or destructively finalizes.** A timeout or unknown sink response remains **uncertain**, never silently a success.
9. **P2+ only:** Completion and replayable outbox receipt are persisted together; manager reconciles by exact attempt identity. P1 statuses are ephemeral and have no replay/outbox.
10. Stop/drain coordinates admission closure, source cancellation, in-flight work, flush deadlines and truthful terminal state.

**Do not claim exactly-once delivery** across arbitrary external sources/sinks. In particular, a fencing token cannot undo a previously executed external write, and UDP notifications offer no sender acknowledgement.

## 5. Execution/transport separation

- `ExecutionService`: `prepare`, `start`, `status`, `cancel`, `drain`, `get_attempt`, `replay_completion` semantics. Distinguish request accepted, run started, run terminated, result durably recorded and callback acknowledged.
- In-process adapter sends typed calls on bounded channels or direct methods; remote adapter uses versioned authenticated RPC with deadlines and stable error codes. Candidate RPC stack: `tonic`/Protobuf; freeze selection after interop/error-mapping review. Public API candidate: Axum HTTP; no Python runtime required.
- Worker advertises only capabilities that passed conformance gates. Any incomplete plugin or recovery feature cannot be selected by capability routing. Unknown/unsupported capabilities fail closed.
- Drain and rolling upgrade remain designed even for initial single-process execution; manager/worker process/session identity is included in remote requests.

## 6. Resource/performance model

- All work admission, channels, buffers, pending requests, transform state, per-sink output, journal/outbox, DLQ and telemetry samples have explicit count/byte/concurrency or retention quotas.
- Quotas operate *before* allocating large payloads; bounded queues must backpressure upstream or surface an explicit overload outcome. Webhook 202 cannot imply durable persistence unless an ingress journal actually commits it.
- Prefer streaming/batched codecs and owned bytes; avoid a mandatory JSON stringify/parse cycle between every stage. Typed records preserve u64 counters, binary payloads and large integers without silent float coercion.
- State-key ordering and source partition ordering constrain parallelism. Optimize only after profiling and differential correctness tests.
- Performance is measured as **confirmed useful outputs per CPU-second**, with memory, tail latency, duplicates, loss, resource limits and fairness included.

## 7. Security model

- Secure-by-default management and worker ingress; explicit machine identity and encrypted authenticated remote transport. Config secrets are references, not echoed in execution plans or logs.
- Authorization binds pipeline revision, logical run/attempt, worker session, expiry and action; replay/stale attempts are rejected before side effects. Use server-side decision time and durable admission watermark where applicable.
- Source/auth failures and malformed SNMP notifications are observable but may never leak credentials, raw passphrases or private payloads.
- Local development may have an explicit loopback-only unsafe mode; never make it a default for externally reachable deployments.
- Plugin boundaries are **trusted native code** at first, not arbitrary untrusted user code; multi-tenant sandboxes/WASM are later designs.

## 8. Architecture review gates

The blocking review (P0-R1..R6) is addressed by the [finding map](review-remediation.md); none of these edits constitute an approved freeze. D01–D12 disposition proposals are now in [decision-register.md](decision-register.md), with explicit phase boundaries and re-entry gates.

Design approval requires review of: shared-topology invariants; execution lifecycle; ack/commit contract; journal/fencing/crash recovery; plugin capabilities; source/record/serializer compatibility; auth; bounded memory and overload behavior; native SNMP role; deployment upgrade/rollback. No crate implementation is authorized merely because this proposal was committed.

Open issues and gate plan: [migration-plan.md](migration-plan.md). Detailed interfaces: [plugin-contracts.md](plugin-contracts.md), [reliability-contracts.md](reliability-contracts.md).

## 8. Post-P1 amendment proposals (not part of the accepted Phase 0 freeze)

The following **proposed** post-freeze amendments require independent DCR/ADR review and an explicit new Orchestrator approval before taking precedence over D01–D12 or authorizing P2 runtime work:

- [ADR-013: runtime/concurrency](proposals/adr-013-p2-runtime-concurrency.md) — candidate Tokio execution, bounded channels/permits, isolated blocking I/O and cancellation/quiescence proof. P1's single-thread/serialized test profile cannot prove P2 simultaneous sink I/O.
- [ADR-014: standalone worker failure domain](proposals/adr-014-standalone-worker-failure-domain.md) — in-process vs supervised local process, manager survival on native plugin fatal failure and same `ExecutionService` semantics.
- [ADR-015: UNKNOWN operations](proposals/adr-015-unknown-recovery-operations.md) — reconciliation, quarantine and audited unresolved evidence loss without a fake terminal success or unsafe guard release.
- [ADR-016: data/source/backend/performance re-entry](proposals/adr-016-serialization-sources-persistence-gates.md) — reconcile D03 P5 JSON mapping with D07 P3 SNMP requirement; decide P2 fractional JSON scope, old-revision EffectId replay, non-file source models, SQLite/P4 PostgreSQL rationale, and early matched-guarantee performance evidence.

[Review package / Orchestrator handoff](proposals/p2-readiness-handoff.md). The frozen P1 fixture, safety caps, source-unit obligation predicate, journal/CAS fencing, ambiguity semantics and D01 shared-engine invariant remain unchanged. **No P2 implementation authorization or Phase 1 R2 acceptance is claimed by this proposed documentation delta.**
