# TRAM Rust — proposed migration plan and design review gate

**Status:** PROPOSED, not approved or frozen. **Purpose:** reviewable scope and measurable gates, not a promise of delivery dates.

## 1. Evidence pins and source hierarchy

Sources inspected for this proposal:
- Python repository `tosumitdhaka/trishul-ram@ff380725b86c9569901ea89ad6771623847cc4e2`, whose latest commit is **documentation only**. Released Python source is v1.8.0, with v1.8.1–v1.8.3 hardening plans and backlog inventory documented, not assumed implemented.
- Python docs `docs/architecture.md`, `docs/plans/v1.8.0-v18-01-contracts.md`, `docs/ideas/rust-migration-assessment-2026-10.md`, `docs/ideas/open-items-inventory-2026-10-10.md`, and the v1.8.1/1.8.2/1.8.3 planning documents.
- SNMP `tosumitdhaka/trishul-snmp-rust@bef2b7643dddd4c28326febe9e310575110b6f4b`: native `trishul-snmp` 0.1.1 API, including v1/v2c/v3-USM, manager, notifier/listener, compiled JSON MIB consumption. It does **not** compile MIB source files; keep a separate compiler or artifact-production boundary.
- User-confirmed intent: new architecture rather than copy/translation; standalone and distributed; registry-first with few plugins; eventual similar functionality with native Rust performance.

When sources disagree: current released *code/tests* are evidence of actual behavior; explicit accepted intended contracts govern what Rust should preserve; newer docs/backlogs identify concerns, **not automatically fixed behavior**; new Rust deviations require versioned, reviewed decisions. Do not hardcode historical incidental bugs as parity targets.

## 2. Parity categories / requirement traceability

For every selected feature maintain a matrix with: Python source/test/docs reference; supported user scenario; current behavior and confidence; accepted intended result; Rust design choice; plugin/capability gate; golden/fault/performance test; migration impact; status.

Four allowable outcomes:
- **PRESERVE:** stable externally observable accepted contract (e.g., supported YAML syntax, expected SNMP field representation, outputs, public API where explicitly committed).
- **CORRECT:** reproduced/reportable defect; record expected corrected behavior and a negative regression (e.g., false completion or early acknowledgement).
- **ENHANCE:** intentional new design, with feature flag/version and migration guidance (e.g., explicit admission budgets, better timezone scheduling).
- **DEFER:** unsupported initial plugin/option; validation rejects it up front and documentation names scope (not silent fallback or implicit emulation).

Do not imply all 24 source, 20 sink, 12 serializer and 29 transform plugins in Python are implemented by the base Rust milestone.

## 3. Phased work, bounded scope

| Gate | Target | Evidence to exit |
|---|---|---|
| **P0 / Design** | Architecture, identities, persistence, plugin traits, plan schema/validation, delivery contract, SNMP adapter, resource/security defaults and baseline matrix | Independent ARCHITECTURE_REVIEW_APPROVED (or CHANGES_REQUIRED); open blockers resolved; no code required |
| **P1 / Core proof** | Rust workspace, typed values, registry, compiler, engine, JSON, local source/sink, four simple transforms, branch fan-out | Build/clippy/fmt; deterministic fixtures, bad input, multi-sink isolation, failed sink, bounded queues |
| **P2 / Standalone** | Native CLI, scheduling, control ledger, embedded worker, health/API, journal, cancellation/drain, persistence | Restart/crash/replay and lifecycle tests in real OS processes; correct output and ack evidence |
| **P3 / SNMP** | Rust `trishul-snmp` adapter for polling, trap receive and MIB enrichment; trap sink as separate gated capability | v1/v2c/v3 live wire, credential rejection, malformed trap/Counter64, burst/drop and cancellation gates |
| **P4 / Distributed** | Same engine over versioned private transport, manager/worker placement, capability routing, fencing, journal/outbox, recovery and rollout | Mixed worker failure/partition tests; old attempt cannot win; no unsupported placement; resource-bounded canary |
| **P5 / Catalog growth** | Kafka, REST/webhook, SFTP, stateful transforms, formats, remaining prioritized connectors | One plugin/version per acceptance matrix and live failure profile; no false durability claims |
| **P6 / Product parity** | Broader CLI/API/UI/Helm/deploy/workflows and accepted Python functional surface | Full compatibility & operational test suite, upgrade/import/rollback, end-to-end performance campaign |

These are dependency/acceptance stages, **not** a release timeline. SNMP adapter prototyping can run before full standalone delivery, but must not bypass core delivery contract testing.

## 4. Performance and test policy

- Freeze exact Python/Rust revisions, dependency versions, CPU quota, memory, input corpora, sinks, durability tier and batch sizes before comparisons.
- Use genuine end-to-end **confirmed** throughput, CPU-seconds/confirmed record, p95/p99 completion latency, working/peak RSS, allocations, lost/duplicate output and disk/journal growth. Intake-only throughput is misleading.
- Compare Python released baseline, optimized Python where available and Rust on equivalent workload. Reuse corrected Python perf studies; historical ratios are not TRAM Rust forecasts.
- Initial **go/no-go criterion** from the Python migration assessment: selected CPU-bound cohort achieves >=2x useful throughput per CPU at matched guarantees **or** >=30% lower peak worker memory at matched throughput. Treat as an evaluation target; re-evaluate after the pilot, not a guaranteed deliverable.
- Differential tests: accepted YAML defaults, source/sink records, expressions, null/missing, large values, SNMP MIB enrichment, serialization/wire bytes where required. Property/fuzz tests: decoders, configs, state transitions, replay and overflow.
- Reliability: synthetic + real broker/agent faults, worker/manager process kill, restart, overload, slow sinks, journal corruption/fill, TLS/auth and credential rejection. Runtime tests must assert effects, not solely responses.
- Validate with real standalone and distributed deployment topology, Linux containers and Kubernetes profiles as scope matures.

## 5. Deliberate decisions to review before implementation

| ID | Proposed default | Review question |
|---|---|---|
| D01 | One engine and `ExecutionService` abstraction, separate control plane | Is standalone in-process call path strict enough to exercise distributed semantics? |
| D02 | Compile-time registered native plugins; later WASM/process plugins | Which third-party plugin use cases mandate an early stable out-of-process ABI? |
| D03 | Typed record envelope, raw bytes path, explicit numeric types | Which Python YAML/JSON representation conventions require exact compatibility and which can be versioned? |
| D04 | Manager authoritative ledger + worker durable SQLite journal | SQLite vs PostgreSQL product matrix; worker state mount, journaling and migration obligations? |
| D05 | Versioned authenticated remote control protocol (candidate `tonic`) | Final protocol and error/timeout mapping; any short-term Python manager interop requirement? |
| D06 | At-least-once when replayable + confirmable; explicit weaker tiers | Which source/sink pairs need transactional contracts; what is the policy on unknown outcomes? |
| D07 | Native SNMP direct dependency pinned; compiled-JSON MIB consumption | Who owns producing/validating MIB bundles and the Python parity mapping? |
| D08 | Retain/reuse existing JavaScript UI only after an API/assets review | Compatibility goals and licensing/provenance of reused UI code/assets? |
| D09 | Start single-active-manager, design identity/fencing for future HA | Is manager HA a v1 release requirement or a separately admitted feature? |
| D10 | No blanket Python data/schema migration promise | Which persisted pipelines, users, histories, secrets and state need supported import/rollback? |
| D11 | Reject unsupported pipeline options at validation/placement | Is a mixed Python/Rust deployment required or merely temporary migration tooling? |
| D12 | Security by default, secret references and authenticated worker RPC | Development mode safeguards, mTLS/bootstrap identity and key rotation strategy? |

## 6. Phase 0 sign-off checklist

- [ ] Independent architecture review performed against pinned references.
- [ ] Source-derived contracts separated from proposed Rust changes.
- [ ] Agreement on external compatibility matrix, deprecations and import strategy.
- [ ] Registry traits, state/ack identity semantics and the first vertical slice reviewed.
- [ ] Standalone/distributed same-engine design and failure boundaries accepted.
- [ ] Typed value, secrets/security and resource-budget policy accepted.
- [ ] SNMP library version/MIB boundary and test plan accepted.
- [ ] Persistence, ownership, replay and cancellation failure model accepted.
- [ ] License/dependency supply-chain plan accepted.
- [ ] Only then record explicit **PHASE_0_ARCHITECTURE_APPROVED** and authorize implementation.

A **draft PR alone is not design approval**. Freeze an immutable reviewed commit for P1 and track deviations through deliberate architecture-decision records.
