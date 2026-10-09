# TRAM Rust — Trishul Real-time Aggregation & Mediation

Rust-native redesign of [TRAM (Python)](https://github.com/tosumitdhaka/trishul-ram), targeting equivalent user-facing capabilities through a new execution architecture rather than a mechanical port.

> **Status: Phase 0 / PROPOSED.** Architecture and contracts are under review. There is no running Rust TRAM product in this repository yet. No design is frozen and no implementation is authorized by these documents.

## Design principles

1. One reusable execution engine for standalone and distributed manager–worker operation.
2. Preserve accepted user-facing functionality; identify, test and correct documented defects instead of porting them.
3. Build source, sink, serializer and transform plugin contracts first, and add implementations incrementally.
4. Enforce bounded resources, truthful delivery outcomes, recovery, authentication and cancellation from the outset.
5. Adopt [trishul-snmp-rust](https://github.com/tosumitdhaka/trishul-snmp-rust) as the native SNMP implementation; do not duplicate its wire stack.
6. Measure complete-output throughput, CPU and memory against pinned Python baselines; do not assume language alone provides a speedup.

## Phase 0 documents

- [System architecture](docs/phase-0/architecture.md)
- [Plugin and data model contracts](docs/phase-0/plugin-contracts.md)
- [Reliability, delivery, security and operations](docs/phase-0/reliability-contracts.md)
- [Migration scope, gates and open decisions](docs/phase-0/migration-plan.md)

**Proposed initial vertical slice:** local source + local sink + JSON serializer + a few stateless transforms, exercising the full registry, pipeline engine and failure semantics. Next add native SNMP polling, walks and traps. Then complete standalone orchestration and manager–worker deployment through the same engine.

## References

- [Python TRAM](https://github.com/tosumitdhaka/trishul-ram)
- [Python Rust-migration assessment (2026-10-06)](https://github.com/tosumitdhaka/trishul-ram/blob/main/docs/ideas/rust-migration-assessment-2026-10.md)
- [Python v1.8.x hardening inventory](https://github.com/tosumitdhaka/trishul-ram/blob/main/docs/ideas/open-items-inventory-2026-10-10.md)
- [Native Rust SNMP toolkit](https://github.com/tosumitdhaka/trishul-snmp-rust)

## Provenance and license

This is independently authored design material informed by the reference implementation. Code and assets are **not** imported by this bootstrap. The repository license, UI asset reuse and third-party dependency provenance require explicit decisions before importing anything.
