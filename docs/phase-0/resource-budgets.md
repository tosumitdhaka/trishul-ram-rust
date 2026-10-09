# P0-R5 — Enforceable resource and admission budget contract

**Status:** Design remediation; all P1 limits below are *normative proposed defaults* for the isolated test harness. They are not measured Rust runtime limits and no production performance claim is made. P2/P4 retain the same accounting interface and add durable-disk budgets.

## 1. Accounting is pre-effect, hierarchical and overflow-safe

Use a shared `BudgetLedger` with parent run, source unit, decoder, transform expansion and sink-branch scopes. Every in-flight item holds a move-only `Reservation` charging **allocated bytes**, **logical record count**, **serialized bytes**, **pending branch bytes** and **I/O concurrency**; a reservation is acquired **before** reading/allocating a frame, creating a decoded record, cloning data into a branch, staging a sink write or spawning an I/O task. Charges live until the buffer/effect is released or a terminal outcome is recorded. `checked_add` is mandatory; overflow -> quota rejection.

`charge_bytes(payload)` accounts for payload length **plus envelope/container overhead** (a measured or conservative hard cap per record/field). Shared `Bytes`/Arc payloads count backing allocation once at the run scope and an additional reference/branch occupancy charge for every queue/branch that retains them; detached/copy-on-write mutation charges new backing bytes. Encoders reserve output based on a provable upper bound or grow incrementally behind a bounded writer; never allocate an unbounded temporary JSON `String` and charge afterward. All variable-length paths, keys, strings and decoder depth/entries are bounded before allocation. Do not rely on bounded item count alone to claim bounded heap.

For each source unit `u` and sink branch `b`, a required reservation must satisfy `raw_live(u) + decoded_live(u) + scratch_live(u) + sum_b pending_bytes(u,b) + framework_overhead <= run_live_budget` (account physical shared buffers only once, without omitting reference/queue overhead). Large decoded value trees and expanded output must fit concurrently with prior pending branches; **no double-free and no negative charges**. All four categories have distinct telemetry.

## 2. Phase 1 fixed safety envelope

| Budget key | Default hard cap | Scope / admission point |
|---|---:|---|
| Active pipelines / runs | 1 / 1 | test harness start |
| Raw single-file size | 4 MiB | stat before open; bounded read checks actual size during read |
| Raw pending file buffers | 1 | source queue; no prefetch of full second file |
| JSON nesting depth / token count | 32 / 100,000 | streaming bounded parser, before recursive allocation |
| Decoded records per source unit | 4,096 | decoder yields records incrementally; reserve one before materialization |
| Decoded live bytes per source unit | 16 MiB | typed Datum allocations + envelopes before construction |
| Single decoded/output record encoded bound | 1 MiB | per-record admission, no truncate |
| Transform expansion children per input | 1 (P1); proposed P2 cap 64 | P1 rejects expansion operators; later each child reserves before emission |
| Configured sink branches | 2 | plan validation |
| Pending bytes per sink branch | 8 MiB | before branch enqueue/encode |
| Total pending sink output | 16 MiB | includes encoded buffers and staged write buffers |
| Concurrent sink I/O operations | 2 | acquire permit before I/O start |
| Total live engine allocation budget | 64 MiB | run-wide hierarchical reservations, including parsing/intermediate buffers |
| Cumulative scratch output per run | 64 MiB | reserve before write, includes temp manifests/metadata overhead |
| Parsed YAML bytes / mapping/sequence nesting | 256 KiB / 32 | bounded YAML ingestion before tokenization |
| Scratch artifacts per run | 100 | exclusive creation guard before each file |

All byte counts are binary MiB (`1 MiB = 1,048,576 bytes`). Caps can be lowered for tests, but **cannot be raised in P1** without re-review of test harness safety. A deployment may be subject to stricter OS/container memory limits; lack of OS memory should fail as a run error, never justify ignoring our limits.

## 3. Backpressure and failure behavior

- **Pull input (local files):** refuse oversized file before read; when decoder/branch budgets are full, **pause reads** and wait for reservations to release with a bounded cancellation-aware deadline. If capacity cannot be obtained before deadline, return `RESOURCE_EXHAUSTED` and terminate the ephemeral run as failed/unknown; never drop input silently.
- **Codec/transform:** reject on depth/node/record/cardinality/output budget BEFORE allocating growth. If a partial decode produced accepted records and the next element exceeds limits, fail the **entire source unit**, maintain the complete logical obligation barrier and do not convert partial decoded output to delivery success.
- **Fan-out:** a record routed to two branches obtains all required queue/byte reservations **atomically** (or suspends before enqueuing any branch) to avoid sink A retaining arbitrary unbounded records while B is blocked. A prior external write cannot be rolled back; record partial outcome honestly.
- **Slow sink:** pending bytes remain charged until write/flush/terminal disposition. Bounded async waiting cannot create additional unmetered tasks; on timeout, fail with `SINK_STALLED` and retain uncertainty if a write may have occurred. P1 scratch files are not durable receipts.
- **Cancel:** stop new admission promptly; pending waits unblock and release permits; already started writes conclude/timeout with explicit outcome. No cleanup thread may bypass quotas or delete durable undecided results.

**Proposed result classification:** pre-effect over-limit = `REJECTED_RESOURCE`, no side effects; after-effect capacity exhaustion = `RUN_RESOURCE_FAILED` with per-branch confirmed/uncertain outcomes. A fresh input is never silently dropped. Runtime logs contain exact resource category, requested/reserved/limit and run identity; metrics avoid file/path/secret labels.

## 4. Durable runtime extensions — P2/P4

- P2+ journal/outbox and persistent staging each have **byte quotas, reserved recovery headroom and separate capacity metrics**. Disk-full or corrupt journal closes **new** admission (`ADMISSION_CLOSED`, unhealthy readiness). Existing undecided/ack-unconfirmed results **cannot be evicted** for disk recovery. Bounded retry/retention may delete only safely acked entries with a persisted GC watermark.
- P2 staged writes reserve final size and per-attempt disk capacity before writing any bytes, and report `unknown` on failed/ambiguous fsync/rename. P4 additionally bounds RPC request/response payload and concurrent pending dispatch.
- Streaming and unbackpressureable UDP ingress require explicitly measured bounded drop policies, not P1 behavior nor a claim of durable input.

## 5. Acceptance tests and instrumentation

| Test | Expected evidence |
|---|---|
| `RES-01` 4 MiB + 1 raw byte | reject before full file allocation; input untouched, no scratch |
| `RES-02` deeply nested JSON (33) or >100k tokens | deterministic bounded parse error, measured peak within envelope |
| `RES-03` 4,097 records or decoded materialization >16 MiB | no out-of-limit allocation, no vacuous source-unit success |
| `RES-04` expansion 1:64 then 1:65 (P2 simulator) | 65th child not allocated; barrier pending/failure, not acked |
| `RES-05` sink A fast / B blocked forever | queue byte ceiling respected; upstream stops; cancellation returns; no runaway tasks |
| `RES-06` copy-on-write fan-out repeatedly modifies large values | correctly charges copies, refuses overflow, no cross-branch mutation |
| `RES-07` scratch disk full, 101st file, 64 MiB cumulative | fail loudly, no input finalization |
| `RES-08` P2 journal full with unacked outbox | readiness/admission closed, undelivered records retained across restart |

P1 evidence must include instrumentation for peak reserved counts/bytes, decoded records, branch queue occupancy and sink-task concurrency **plus observed process RSS** under repeatable stress. Resource ledger invariants are property-tested and checked under every early-return/error/cancellation path; run teardown has zero leaked reservations.
