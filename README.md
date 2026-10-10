# TRAM Rust — Trishul Real-time Aggregation & Mediation

Rust-native redesign of [TRAM (Python)](https://github.com/tosumitdhaka/trishul-ram), targeting equivalent user-facing capabilities through a new execution architecture rather than a mechanical port.

> **Status (2026-10-10): Phase 0 approved at `2cedaeac5657a8941fe9366f04029cd11b0cfd30`.** Phase 1 Round 1 implementation is authorized on `phase/p1-ephemeral-core` but remains INCOMPLETE/UNVALIDATED. The Rust workspace includes an owned datum/record model and a data-only static plugin registry. Strict YAML plan compilation, a reproducible lockfile and actual Rust toolchain verification are pending. There is no production runtime, source/sink I/O, or durability.

## Design principles

1. One reusable execution engine for standalone and distributed manager–worker operation.
2. Preserve accepted user-facing functionality; identify, test and correct documented defects instead of porting them.
3. Build source, sink, serializer and transform plugin contracts first, and add implementations incrementally.
4. Enforce bounded resources, truthful delivery outcomes, recovery, authentication and cancellation from the outset.
5. Adopt [trishul-snmp-rust](https://github.com/tosumitdhaka/trishul-snmp-rust) as the native SNMP implementation; do not duplicate its wire stack.
6. Measure complete-output throughput, CPU and memory against pinned Python baselines; do not assume language alone provides a speedup.

## Phase 0 documents

**Independent review:** `CHANGES_REQUIRED` (review ID 5473153676) on original `96e11006e4784d2e70a7b0663c598c16376b04f7`. The documentation below is a **remediation candidate**, pending exact-SHA re-review; neither architecture approval nor Phase 1 code authorization has been granted. **P1 is an isolated ephemeral test harness, not a production-delivery milestone.**


- [System architecture](docs/phase-0/architecture.md)
- [Plugin and data model contracts](docs/phase-0/plugin-contracts.md)
- [Reliability, delivery, security and operations](docs/phase-0/reliability-contracts.md)
- [Migration scope, gates and review record](docs/phase-0/migration-plan.md)
- [P1 ephemeral safety boundary](docs/phase-0/p1-safety-boundary.md)
- [Source-unit lineage and acknowledgement](docs/phase-0/source-unit-contract.md)
- [Attempt/worker authority state machine](docs/phase-0/attempt-protocol.md)
- [P1 Python compatibility matrix + fixtures](docs/phase-0/p1-compatibility-matrix.md)
- [P1 resource accounting](docs/phase-0/resource-budgets.md)
- [Architecture decision register D01–D12](docs/phase-0/decision-register.md)
- [Independent-review remediation map](docs/phase-0/review-remediation.md)

**Proposed initial vertical slice:** ephemeral read-only local source + scratch-only local sink + JSON serializer + a few stateless transforms, exercising the registry and engine in a test harness without durable source acknowledgement or production publication. Next add native SNMP polling, walks and traps. Then complete standalone orchestration and manager–worker deployment through the same engine.

## References

- [Python TRAM](https://github.com/tosumitdhaka/trishul-ram)
- [Python Rust-migration assessment (2026-10-06)](https://github.com/tosumitdhaka/trishul-ram/blob/main/docs/ideas/rust-migration-assessment-2026-10.md)
- [Python v1.8.x hardening inventory](https://github.com/tosumitdhaka/trishul-ram/blob/main/docs/ideas/open-items-inventory-2026-10-10.md)
- [Native Rust SNMP toolkit](https://github.com/tosumitdhaka/trishul-snmp-rust)

## Provenance and license

This is independently authored design material informed by the reference implementation. Code and assets are **not** imported by this bootstrap. The repository license, UI asset reuse and third-party dependency provenance require explicit decisions before importing anything.
