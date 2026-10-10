# RUN+ISO-001 — Isolated pre-execution preparation

**Status: STAGE A PARTIAL. No 2×2 spike execution or acceptance is authorized.**

Authority: Issue #5, `tram-p2-run-iso-001-preparation-674be977-20261011`; frozen P0 `2cedaeac5657a8941fe9366f04029cd11b0cfd30`, accepted P1 `9757aaf29665d5f774e15583e1e4f23b140b9799`, proposed/reviewed-only RUN+ISO specification `674be9779533bfe09117a05fbc3e83595f58afca`. This nested Cargo workspace is not in the accepted root Cargo workspace.

## Included now

- `src/lib.rs` is a **pure in-memory typed authority/queue contract model**, with preparatory unit tests that reject unsupported protocol versions, stale sessions, replay conflict, premature receipt ACK and non-atomic A/B reservations. `Admitted` in this model is **not** a durable worker journal receipt; it has no external effects. `UnknownGuardHeld` is deliberately conservative.
- `tools/fixture.py` deterministically generates and validates 1024 source-unit binary files and the exact scheduling metadata under a *new, owned, private `/tmp/tram-run-iso-001-*`* scratch root. Each source unit holds 8 records; each fourth record is exactly 4096 bytes and other records exactly 512 bytes. The schedule alternates the two runs at ascending unit number and defines A/B batch delays. The generator neither acknowledges input nor creates sink artifacts.
- `tools/test_fixture.py` checks reproducibility and rejects preexisting output, changed input, arbitrary-root and symlink paths.
- `docs/spikes/run-iso-001/` records the trace schema, case matrix, and preparation manifest.

## Preparation commands — not experimental acceptance

```sh
# Python 3 standard library only; no network, faults, sinks, or benchmarks
cd experiments/run-iso-001/tools
python3 -m unittest -v test_fixture.py
python3 fixture.py expected-digest
root="$(mktemp -d /tmp/tram-run-iso-001-XXXXXXXX)"
chmod 700 "$root"
python3 fixture.py generate --root "$root"
python3 fixture.py verify --root "$root"
# Remove only this explicitly created scratch directory, after checking its identity.

# Only when Rust 1.88.0 + cargo are installed in an isolated environment:
cd experiments/run-iso-001
cargo test --manifest-path Cargo.toml
cargo fmt --check --manifest-path Cargo.toml
cargo clippy --manifest-path Cargo.toml --all-targets -- -D warnings
```

## Required before `READY_FOR_SPIKE_EXECUTION_REVIEW`

**Not yet implemented or evidenced:** real A (bounded std-thread/channel) versus T (bounded Tokio runtime) engines, I embedded versus S supervised UDS process, actual authenticated local IPC and binary contract vectors, separate SQLite manager/worker journal+outbox with atomic receipts, process-tree and old effect-handle quiescence, cancellation/blocked-fsync isolation, genuine monitored bounded blocking I/O, two-run fairness and truthful queue/IPC/OS memory charging, fail-closed UNKNOWN across manager/worker restarts. Nor have the 3×4 clean trials, fault cases or measurements been run (they are forbidden at this stage).

All non-Linux platforms remain unsupported for acceptance, and unsupported peer credential or journal guarantees must fail closed. The in-memory contract model is a specification aid only, **not a proof that the eventual engine enforces those rules**.

The manifest contains `UNPINNED` blockers and is **NOT VALID FOR EXECUTION AUTHORIZATION**. Only the Orchestrator can issue the separate exact-SHA RUN_ISO_SPIKE_EXECUTION_AUTHORIZED decision after the prototype and environment are pinned and independently reviewed.
