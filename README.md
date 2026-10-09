# TRAM Rust — Trishul Real-time Aggregation & Mediation

Rust-native redesign of [TRAM (Python)](https://github.com/tosumitdhaka/trishul-ram), targeting equivalent user-facing functionality with a new, correctness-first execution architecture and measurable Rust-native efficiency.

> **Status: Phase 0 — architecture proposal, not yet approved.** There is no working Rust daemon or supported production deployment in this repository.

## Design direction

- One shared pipeline execution engine for standalone and distributed manager/worker deployments.
- Trait-based registry for sources, sinks, serializers and transforms; begin with local and SNMP plus JSON and a small transform set.
- Explicit delivery, backpressure, recovery, security and resource-budget contracts.
- Existing Python behavior is a reference for accepted user-facing requirements; documented defects and incidental implementation choices are not migration requirements.
- Native SNMP integration through [trishul-snmp-rust](https://github.com/tosumitdhaka/trishul-snmp-rust), without duplicating its protocol stack.
- Implement in vertical slices, with conformance tests and independent design review before expanding plugin breadth.

See the proposed Phase 0 documents on the design branch. The design is **not frozen** until an explicit review and approval.

## References

- [Python TRAM](https://github.com/tosumitdhaka/trishul-ram)
- [Existing Rust migration assessment](https://github.com/tosumitdhaka/trishul-ram/blob/main/docs/ideas/rust-migration-assessment-2026-10.md)
- [Native Rust SNMP stack](https://github.com/tosumitdhaka/trishul-snmp-rust)

## License

License selection and provenance of reused assets must be resolved as part of Phase 0 before importing code or UI assets.
