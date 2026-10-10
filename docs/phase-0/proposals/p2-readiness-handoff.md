# Design Owner handoff — Post-P1 P2-readiness architecture amendments

**Status:** DESIGN_SUBMITTED / PROPOSED, **NOT** an approved DCR, Phase 0 replacement freeze, P2 implementation authorization, review pass or merge request. **Repository:** `tosumitdhaka/trishul-ram-rust`. **Immutable accepted Phase 0 reference:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`. **Proposed branch:** `design/p2-readiness-architecture-amendments`. The associated draft PR supplies the exact review SHA.

## Evidence and current status verified when proposed (2026-10-10)

- Phase 0 `PHASE_0_ARCHITECTURE_APPROVED` against baseline above: [Orchestrator decision](https://github.com/tosumitdhaka/trishul-ram-rust/pull/1#issuecomment-6089485009).
- P1 R2 [independent SOURCE_REVIEW_PASS](https://github.com/tosumitdhaka/trishul-ram-rust/pull/3#pullrequestreview-5479675831) and [independent EXECUTOR_PASS](https://github.com/tosumitdhaka/trishul-ram-rust/pull/3#issuecomment-6099635829) at exact `9757aaf29665d5f774e15583e1e4f23b140b9799`. Executor: 100 serialized workspace tests passed, 1 ignored tmpfs-only; default parallel diagnostics fail; serial-only support limitation explicitly retained. Python golden parity limited to selected fixture.
- **No independently verified Orchestrator P1 R2 acceptance yet** when submitted. Do not interpret passing review/execution as acceptance, production release or P2 authorization.
- External technical design review was provided as user-supplied narrative (not an exact-SHA GitHub review or executed proof). Its findings are investigated here, not automatically accepted.

## DCR package and reviewer decisions requested

| Candidate | Proposed scope / decision | Gate |
|---|---|---|
| [ADR-013](adr-013-p2-runtime-concurrency.md) | Tokio-vs-threaded spike, concurrent pipelines, bounded blocking I/O, cancellation and shared permits | P2 PRE-GATE |
| [ADR-014](adr-014-standalone-worker-failure-domain.md) | in-process vs supervised local worker, blast radius, authenticated local IPC, restart proofs | P2 PRE-GATE and production profile |
| [ADR-015](adr-015-unknown-recovery-operations.md) | audited reconcile/quarantine, evidence-lost abandonment representation, no force-success/unsafe guard release | P2 PRE-GATE |
| [ADR-016](adr-016-serialization-sources-persistence-gates.md) | D03/D07 P3 binary JSON re-entry, P2 JSON numbers, immutable replay across config change, source models, DB backend rationale, P2 performance pilot | P2/P3/P4 as named |

## Required Orchestrator handling

1. Verify live draft amendment PR head/base and current P1 R2 gate, independently. If R2 acceptance is still pending, decide it separately against exact P1 SHA; neither this DCR nor executor PASS substitutes for acceptance.
2. Authorize a bounded **READ-ONLY DESIGN_REVIEW** of the proposed amendment SHA with owners for runtime, security/operations, codec/SNMP and persistence. Require explicit review of compatibility with D01–D12 and a line-specific verdict (`DCR_APPROVED` or `CHANGES_REQUIRED`).
3. For ADR-013/014 choices requiring real spike evidence, authorize a separate bounded **pre-P2 proof-of-concept** after design pre-gate. This is **not** permission for production P2 runtime code or frozen-contract replacement. Record candidate SHA and actual commands/metrics before freezing those decisions.
4. On independent approval, record a new immutable architecture-amendment SHA and an explicit precedence map: the accepted Phase 0 core invariants remain binding; only enumerated clauses are amended. Revise the canonical docs/gates under authorized scope; do not rewrite history/alter the pinned P1 fixture.
5. After an explicit P2 PRE-GATE and separate Orchestrator authorization, start P2 implementation on the accepted merged/selected design baseline. No PR #1/#3 merge/retarget and no P2 work is implied here.

## Non-regression invariants

Preserve: immutable plan/authority, exact attempt/session/fence journal CAS, refusal/no-effects receipts, guard held while effects uncertain, ACK sealed source-unit barrier, A confirmed/B unknown no ack, no exactly-once claim, fail-closed capabilities, filesystem confinement and P1 ephemeral-only contract. Source code, tests, Rust/Python/SNMP dependencies and original P1 golden blobs are untouched by the docs-only proposal.

**DCR outcome pending independent reviewer + Orchestrator.**
