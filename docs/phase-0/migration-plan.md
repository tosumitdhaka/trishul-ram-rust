# TRAM Rust — proposed migration plan and design review gate

**Status:** Historical Phase 0 plan (frozen at `2cedaeac5657a8941fe9366f04029cd11b0cfd30` after independent approval). Proposed post-P1 P2-readiness changes below require **separate** design review and explicit Orchestrator acceptance; no delivery dates are promised.

## Independent review remediation in this revision

**Historical pre-freeze record:** the original `96e11006e4784d2e70a7b0663c598c16376b04f7` received `CHANGES_REQUIRED` (review `5473153676`, P0-R1 through P0-R6); these were subsequently remediated, independently reviewed and frozen at `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. This historical section is not a current gate denial. The **new D13–D16** overlay requires separate approval. Binding details are linked in the [finding-to-file map](review-remediation.md).

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
| **P1 / Ephemeral core proof** | Registry, typed values, strict compiler, read-only local source + scratch-only local sink, JSON, four transforms and fan-out. **No durable admission, ack, final publication, network listener or production delivery claim**. | Python-oracle compare + pinned fixtures; build/clippy/fmt; read-only/scratch path confinement, crash leaves input untouched, branch isolation, P1 resource bounds |
| **P2 / Durable standalone** | Native control ledger + separate durable worker admission/completion/outbox journal, identity-fenced attempts, idempotent local staged publication, checkpoint/ack frontier, cancellation/drain, scheduling/API | Real OS crash/restart/fault tests before any production-enabled effect; confirmed publication, journal/outbox atomicity and source-finalize order |
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

## 4A. Post-P1 proposed pre-P2 architecture gate (NOT YET APPROVED)

This is a **proposed** supplemental gate on top of the accepted P0–P6 plan, not a retroactive P1 requirement. P1 R2 `9757aaf29665d5f774e15583e1e4f23b140b9799` **was formally accepted** by [Orchestrator PHASE_1_R2_ACCEPTED](https://github.com/tosumitdhaka/trishul-ram-rust/issues/2#issuecomment-6099860467) after independent SOURCE_REVIEW_PASS and EXECUTOR_PASS, solely for the isolated ephemeral scratch-only scope. P1 is still scratch-only, not a durable or concurrently proven production runtime. The executor's `RUST_TEST_THREADS=1` limitation is an explicitly accepted constraint, not evidence of P2 concurrency.

**P2 PRE-GATE entry checks (design review before production implementation):**

1. [ADR-013 runtime/concurrency](proposals/adr-013-p2-runtime-concurrency.md) + [ADR-014 isolation](proposals/adr-014-standalone-worker-failure-domain.md): independently review the **joint numeric [RUN+ISO-001 contract](proposals/run-iso-combined-spike-contract.md)**, and require a **separately Orchestrator-authorized, disposable nonproduction spike** before selecting either Tokio/std-thread runtime or in-process/supervised standalone topology. Freeze one typed ExecutionService, ledger/journal/IPC authority, 2 concurrent runs, measured I/O overlap, queue/thread/memory/FD/child limits, cancellation/crash deadlines, repeatable workload and performance nonregression.
2. **Joint selection gate:** after reviewed RUN-01..08 / ISO-01..06 spike evidence and independent review, explicitly choose the runtime and standalone process boundary, identify superseded frozen in-process clauses *if* S is selected, preserve D01 one-engine semantics and D05 P4 remote mTLS re-entry. The design review alone does **not** authorize running the spike, P2 source code or the eventual production profile.
3. [ADR-015 operator UNKNOWN recovery](proposals/adr-015-unknown-recovery-operations.md): only idempotent **annotations on durable UNKNOWN with guard and source-unit exclusion held**; no new terminal abandonment state, no `attempt.abandon` transition, no connector-specific duplicate-tolerant replacement without verified old-worker/child quiescence. Require restart, lost-volume and attempted override while descendant writes evidence (`UNK-01..08`).
4. [ADR-016 data/source/replay/persistence](proposals/adr-016-serialization-sources-persistence-gates.md): propose **exact D03-SNMP-P3/D07 precedence text**: P5 generic mapping retained, `DATA-02` only **before P3 SNMP+JSON admission**, not P2/P1; P2 JSON numeric scope, original durable plan snapshot, **cross-revision source-generation exclusion**, old A-confirmed/B-unknown original EffectIds after crash/edit, P2 SQLite fault and P4 dual-backend gates.
5. Pre-freeze acceptance package: exact source/fixture refs, independent DESIGN_REVIEW on immutable amendment candidate, proposed ADR decisions with rationale and open items, no P2 source mutation until separate Orchestrator authorization. Deferred tests must have *their own re-entry phase*; none may be silently counted as P2 passed.

**P2 EXIT supplemental measurements (proposed):** first **matched-guarantee** Python-vs-Rust CPU per confirmed output, confirmed throughput, p95/p99, high-water RSS, disk/journal overhead and duplicate/unknown accounting (`PERF-01`), plus a seeded generated differential corpus for supported expressions and JSON mapping (`COMP-EXT-01`). If the Python comparator lacks equivalent durable guarantees, record **PERF-INCONCLUSIVE** and request explicit follow-up rather than comparing the P1 scratch harness to a P2 durable sink. The existing >=2x CPU throughput or >=30% peak-memory objective stays a P6 overall go/no-go criterion, **not** a promised automatic P2 PASS. P2 must report comparison and regressions, not substitute fast scratch-only P1 throughput for durable P2 output.

**Future connector gates:** P3 SNMP binary JSON/Counter64/trap loss policy verified before advertisement; P4 transport+PostgreSQL CAS conformance; P5 Kafka, webhooks and other connectors advertise truthful replay/ack tiers, streaming sizes and failure semantics. Active source-unit effects keep the original immutable plan revision across replays. [Complete design review handoff](proposals/p2-readiness-handoff.md).

## 5. Deliberate decisions to review before implementation

**Historical:** D01–D12 originally had recommended ACCEPT/DEFER dispositions, but **those decisions are now independently approved and frozen at `2cedaeac5657a8941fe9366f04029cd11b0cfd30`**. D13–D16 are separately proposed/unapproved [decision-register additions](decision-register.md). The earlier question table below remains a historical checklist and **is not** a second unresolved decision authority.

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
- [ ] Independent review of proposed [P1 compatibility matrix and pinned goldens](p1-compatibility-matrix.md), intentional P1 subset/deviations and deferred imports.
- [ ] Registry traits, [lineage/ack barrier](source-unit-contract.md), [attempt authority table](attempt-protocol.md) and the [P1 ephemeral slice](p1-safety-boundary.md) reviewed.
- [ ] Standalone/distributed same-engine design and failure boundaries accepted.
- [ ] Typed value, secrets/security and resource-budget policy accepted.
- [ ] SNMP library version/MIB boundary and test plan accepted.
- [ ] Persistence, ownership, replay and cancellation failure model accepted.
- [ ] License/dependency supply-chain plan accepted.
- [ ] Only then record explicit **PHASE_0_ARCHITECTURE_APPROVED** and authorize implementation.

A **draft PR alone is not design approval**. Freeze an immutable reviewed commit for P1 and track deviations through deliberate architecture-decision records.
