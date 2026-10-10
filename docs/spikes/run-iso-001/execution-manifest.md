# RUN+ISO-001 — pre-execution pin manifest (STAGE A PARTIAL, NOT EXECUTABLE)

**Gate:** `RUN_ISO_SPIKE_PREPARATION_AUTHORIZED` only (Issue #5, `tram-p2-run-iso-001-preparation-674be977-20261011`). **Do not run** measured 2×2 trials, process kills, OOM or blocked-fsync experiments on this manifest. `UNPINNED` is a mandatory block, never a default. The candidate code is only a contract model; the A/T × I/S prototype does **not** exist yet.

## Immutable authority and source pins

| Field | Value | Proof / state |
|---|---|---|
| Repository | `tosumitdhaka/trishul-ram-rust` | Live GitHub connected verification |
| Spike branch | `spike/p2-run-iso-001` | Rooted at accepted P1 SHA |
| Stage A assignment | `tram-p2-run-iso-001-preparation-674be977-20261011` | Issue #5, OPEN |
| Frozen Phase 0 | `2cedaeac5657a8941fe9366f04029cd11b0cfd30` | PR #1 OPEN/DRAFT; no amendments |
| Accepted P1 | `9757aaf29665d5f774e15583e1e4f23b140b9799` | PR #3 OPEN/DRAFT; not production |
| Reviewed spike specification (not frozen architecture) | `674be9779533bfe09117a05fbc3e83595f58afca` | PR #4 OPEN/DRAFT; independent SPEC PASS conditional spike |
| Final exact full prototype commit SHA | **UNPINNED** | Must be bound by immutable GitHub attestation once complete; cannot self-reference commit |
| A×I baseline exact SHA | **UNPINNED** | Must reference the runnable baseline in final candidate; not assume scaffold is baseline |
| Execution manifest Git blob SHA | **UNPINNED** | Capture after repository commit; bind externally to exact final code SHA |
| Dependency `Cargo.lock` SHA-256 | **UNPINNED** | Nested workspace lock not generated or built |
| Runtime choice | `NOT_DECIDED` | No Tokio or supervised profile selected |

## Fixture: reproducible but NOT an acceptance run

| Field | Value |
|---|---|
| Generator | `experiments/run-iso-001/tools/fixture.py` |
| Generator SHA-256 | `6558703928b0783ab5d5474c0605ae8759130b5223ca65dab3a68bf7f5177ab1` |
| Generator deterministic algorithm | SHAKE-256(payload-domain || big-endian seed/run/unit/record), binary source units; fixture root contents recursively checked |
| Fixture identity digest / SHA-256 | `42bd4fa7d739caee2c5498158c86437e4504cdc7ae4946c0e475574db78216ea` |
| Canonical digest algorithm | SHA-256 initialized with `TRAM-RUN-ISO-001-FIXTURE-v1\0`, then lexicographic generator entries `(path_len:u32be, path ASCII, content_len:u64be, raw file bytes)` |
| Workload | Seed `0x20261010`, 2 runs × 512 source units × 8 records = 8192 records total (4096/run), 75% 512-byte and 25% 4096-byte payloads |
| Schedule | `fixtures/schedule.json` in generated root: ascending source units, two-run round-robin, run 0 first, A then B branch, 8 records/sink batch |
| Sink B baseline / stalled / blocked | 50 ms / 250 ms **per batch** / indefinitely blocked, respectively; A 0 ms artificial delay |
| Clean acceptance | 3 trials for **each** A×I, A×S, T×I, T×S cell — all **NOT_RUN** |

## Environment: preparatory observation only, not a pinned execution runner

| Field | Value | Execution usability |
|---|---|---|
| Observed preparation OS | Debian GNU/Linux 13 (trixie), Linux x86_64 | Observation only |
| Observed preparation kernel | Linux `6.18.44` | Observation only |
| Observed CPU cgroup quota | `cpu.max = 400000 100000` (4 logical-core equivalent) | Stage B quota enforcement **UNPINNED** |
| Observed filesystem | container overlayfs with `fsync=volatile` upper mount | **UNSUITABLE** for durable fsync/restart proof |
| Exact Stage B OS/kernel/container build | **UNPINNED** | Required |
| Stage B rustc/cargo | **UNPINNED**; Rust target `1.88.0` | Required; not present here |
| Tokio version | **UNPINNED** | Required for T candidate |
| Cargo dependency versions/lock | **UNPINNED** | Required |
| Python version in preparation host | `3.13.5` | Runner Python **UNPINNED** |
| Stage B runner build/hash | **UNPINNED** | Required |
| Stage B actual CPU affinity/cgroup path/quota | **UNPINNED** | Required (4 logical cores) |
| Stage B RAM/FD limits and cgroup limits | **UNPINNED** | Required |
| Stage B filesystem type, device and mount options | **UNPINNED** | Required (must support honest durability experiment) |
| Stage B sandbox absolute disposable root | **UNPINNED** | Required |
| Stage B manager/worker processes and PID containment | **UNPINNED** | Required |
| Stage B network policy and peer credential proof | **UNPINNED** | Required; no TCP fallback |

## Planned measurement/event pin (NOT observed)

- Schema: `docs/spikes/run-iso-001/trace-schema.json`, `run-iso-001-trace-v1`, JSONL UTF-8, one object per line; **schema bytes SHA-256 UNPINNED** until final code/manifest attest; log SHA-256 is over exact bytes.
- Time: monotonic integer **nanoseconds** (`mono_ns`) with process clock identity. Source-unit p99 is admission → both required branch receipts committed, nearest-rank p99 on successful no-fault units; all incomplete/UNKNOWN/failed units included in correctness counts separately.
- 100 ms sampling plus fault edges: `/proc/<pid>/status` VmRSS/VmHWM, `/proc/<pid>/fd` count, per-process totals, summed process RSS and HWM, queue item/bytes, per-run/shared charge, active/queued blocking jobs. Collect Linux exit status/signal and stdout/stderr, source/effect/receipt/journal/output inventories before/after.
- Exact thresholds and per-cell matrix: see `case-matrix.md` and `measurement-plan.md`; require >=100 ms genuine overlap, <=64 messages/4 MiB each branch, <=256/16 MiB total, <=64 MiB/run and <=128 MiB shared live charge, <=4 running blocking ops/threads, <=16 waiting, >=20% two-run fairness per eligible 5 s window plus <=2 s progress, cancel <=2 s, blocked fsync UNKNOWN <=3 s, descendant quiescence <=5 s, summed HWM <=256 MiB, <=128 FD/process and <=256 total, throughput >=80%, p99 <=2×, summed HWM <=1.5× A×I baseline.

## Preparatory commands (allowed) and future command pins

```sh
# Allowed Stage A only; /tmp root is caller-created, owned, exclusive, mode 0700.
(cd experiments/run-iso-001/tools && python3 -m unittest -v test_fixture.py)
python3 experiments/run-iso-001/tools/fixture.py expected-digest
root="$(mktemp -d /tmp/tram-run-iso-001-XXXXXXXX)"
chmod 700 "$root"
python3 experiments/run-iso-001/tools/fixture.py generate --root "$root"
python3 experiments/run-iso-001/tools/fixture.py verify --root "$root"
# Also when toolchain becomes available: cargo test/fmt/clippy on isolated nested workspace.
```

**Stage B exact execution/kill commands, fixture location and cleanup allowlist:** **UNPINNED**. Stage A never runs fault injection, measured trial loops, process-tree kill, OOM, fsync block or provider effects. Once separately authorized, a cleanup allowlist must name only exact owned scratch subtrees, isolated test cgroup and recorded process PIDs plus start generations; wildcard process kill, host paths, production dirs and real customer endpoints are explicitly excluded.

## Binding/approval rule

This file cannot contain its own final commit SHA. Once prototype, generated fixtures, trace schema and execution environment are complete, publish immutable GitHub attestation **after commit** recording `(prototype full SHA, this manifest's Git blob SHA, generator hash, fixture SHA-256, schema hash, environment/runner attestation digest, A×I baseline SHA)`. No post-pin changes to code, fixture, environment or manifest without a new authorization. **This present incomplete manifest does not warrant a Stage B permit; every `UNPINNED` is a hard blocker**.
