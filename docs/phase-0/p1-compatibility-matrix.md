# P0-R4 — Frozen-for-review P1 Python→Rust compatibility matrix

**Status:** DESIGN_OWNER proposal, **not** accepted architecture; no Python test has been executed in this remediation session. These are **expected** goldens inferred from pinned Python source/tests. P1 must execute the same fixture against the pinned Python reference before any claim of Python parity, record any observed differences, and return for design amendment where required.

**Python source pin:** `tosumitdhaka/trishul-ram@ff380725b86c9569901ea89ad6771623847cc4e2` (v1.8.0 released source + docs-only commit). **Baseline fixture pin:** the four Git blobs listed below, proposed in this PR. Exact SHA of the final Rust review commit will additionally lock all fixture content.

| Fixture | Git blob SHA in Rust design tree |
|---|---|
| `fixtures/p1/pipeline.yaml` | `4ca175df56a8bc86655da49f6713d8a5c64a0e41` |
| `fixtures/p1/input.json` | `5e9127b5ecb429199f1e150726b8091ddf898430` |
| `fixtures/p1/expected-a.json` | `72ef5ae522bc6cb032b18c49150ea6cadecce993` |
| `fixtures/p1/expected-b.json` | `1dc44035a3fa51938f640d838562bee7d54baa16` |

**P1 fixtures are semantic oracles, not evidence that the Rust implementation exists.** After fixture execution, compare typed JSON trees first; byte-by-byte JSON output comparison is required only where explicitly declared below. The P1 sink writes temporary scratch files and may use arbitrary scratch filenames, but their JSON payload must match the expected fixture after normalization of formatting/newline only.

## 1. Supported P1 config contract (precisely bounded)

Support *both* Python flat YAML and `pipeline:` wrapper; optional document `version: "1"` outside wrapper accepted for migration compatibility but is **not** interpreted as schema version. Empty/invalid YAML, unknown top-level pipeline keys, unknown discriminators, extra plugin fields and duplicated/ambiguous `sink`+`sinks` are rejected before any source is opened. Unknown options are **not silently discarded**, even when Python Pydantic has an implicit default.

**Allowed:** pipeline `name` (valid Python-style name), optional `description`, `enabled=true`, `schedule.type=manual`, `source.type=local` with required `path`, `file_pattern` default `*`, `recursive=false`; `serializer_in.type=json`, `serializer_out.type=json` or omitted (default JSON), `transforms` from `rename`, `add_field`, `filter`, `drop`; `sinks` length 1..2 (or singular `sink` as length 1) each `type=local`, `path`, `file_mode=single`, static literal `filename_template`, `overwrite=false`, optional simple condition, optional matching stateless sink transforms; `thread_workers=1`, `parallel_sinks=false`, `on_error=abort`, `retry_count=0`, `retry_delay_seconds=0`, `dlq=null`/absent, `delivery.contract=legacy`/absent. `name` regex and length must first be pinned to the Python validator; do not invent broader acceptance.

**Intentional P1 restrictions:** `file_mode=single` and `overwrite=false` required explicitly to avoid the Python local sink's default `append`/`overwrite=true`. P1 does **not** honor the Python `file_mode`, `max_records`, rolling files, format overrides, timestamps/part filenames, `skip_processed`, destructive source flags, `kubernetes`, `workers`, schedules other than manual, `on_error=continue/retry/dlq`, stream, batching/parallelism knobs, schema registry, configured source/sink retries, circuit breaker, external plugins, remote sink or stateful transforms. Any present unsupported field/value is rejected at **plan compile**, even if its numerical value looks harmless. In particular, `skip_processed` and `delete_after_read` MUST be absent from P1 source config even if set to `false`; `true` also rejects. The P1 harness itself enforces read-only/no-finalization behavior. Defaulted Python options that have no P1 effect are normalized only by a documented allowlist; no implicit permissive fallback.

For env substitution, support exact Python `${VAR}` and `${VAR:-default}` syntax **before YAML parsing** as in `tram/pipeline/loader.py` lines 16–35; missing variable without default => `CONFIG_MISSING_ENV`; empty-but-set value remains empty; no shell execution/secret echo. P1 source/sinks must resolve exclusively inside harness-provided input and scratch roots. Production config file paths cannot be used in P1 harness. Arbitrary `${...}` evaluation is forbidden. Later production secret references will need a distinct reviewed resolver.

## 2. Behavioral contract matrix

| ID | Python pinned evidence | P1 accepted behavior and deliberate changes | Disposition | Required test |
|---|---|---|---|---|
| C01 YAML wrapper + env | `tram/pipeline/loader.py:16-35,39-111`; `tests/unit/test_loader.py` | Flat and wrapped `pipeline`; substitute `${VAR}`/`${VAR:-fallback}`; required missing fails. P1 extra unknowns strictly reject. | PRESERVE + CORRECT ambiguities | `COMP-01` parser golden/missing-env/unknown-field |
| C02 Local input | `tram/models/pipeline.py:78-92`, `tram/connectors/local/source.py:68-113`, `tests/integration/test_local_pipeline.py` | sorted file match, one file = one source unit, bytes immutable and read-only. Default pattern `*`. P1 rejects `recursive=true`, either present `skip_processed`/`delete_after_read` (including `false`), and all finalize flags. | PRESERVE basic + DEFER destructive | `COMP-02` two files, changing/oversize/symlink |
| C03 Sink configuration | `tram/models/pipeline.py:920-990`, `tram/connectors/local/sink.py:18-130`, `tests/unit/test_file_sink_common.py` | singular `sink` or `sinks` array; 1..2 local outputs; P1 exclusively new scratch artifacts (not file-mode append or durable finalization). Reject existing/non-scratch destination. | PRESERVE schema names + ENHANCE safe sandbox | `COMP-03` alias, two sinks, unsafe target |
| C04 JSON decode | `tram/serializers/json_serializer.py:18-36`, `tests/integration/test_local_pipeline.py` | UTF-8 JSON object becomes one record; array of objects becomes ordered records, `[]` explicitly empty. Top-level scalar or array element scalar fails as `SERIALIZER_SCHEMA`. Malformed/NaN/Infinity rejected. | PRESERVE main + CORRECT malformed/nonstandard | `COMP-04` dict/list/empty/null/scalar/bad utf8 |
| C05 JSON encode | `tram/serializers/json_serializer.py:34-38`, `tram/models/pipeline.py:1247-1250` | always encode an **array of records**; ensure_ascii=true default; indent absent. Semantic tree equality mandatory. UTF-8 and escape policy verified against Python prior to claiming bytes parity. | PRESERVE + ENHANCE explicit number handling | `COMP-05` expected-a/b and escaping |
| C06 Datum precision | Python JSON+dict semantics in `json_serializer.py`, `tram/models/pipeline.py` | missing ≠ null; integers (including >2^53 and Counter64) retain exact decimal value, never silently float-coerced; booleans distinct. `Datum::Bytes` cannot be emitted as standard JSON without a reviewed encoding policy and is rejected. | ENHANCE explicit number/binary types | `COMP-06` null vs absent, uint64 max, big integer, byte refusal |
| C07 Rename | `tram/transforms/rename.py:14-44`, `tests/unit/test_transforms.py:21-61` | top-level source→destination rename; absent fields remain absent; overlapping paths, dotted/nested renames DEFER until path semantics validated. Branches never share mutable records. | PRESERVE top-level + DEFER nested | `COMP-07` unknown fields, conflict, branch isolation |
| C08 Add_field | `tram/transforms/add_field.py:89-143`, `tests/unit/test_transforms.py:121-153` | compile deterministic subset: literal strings/booleans/integers, field variable refs, + - * integer arithmetic, comparison; sequential fields see earlier additions. Reject unknown functions, dotted accesses, time/random, floats not in proven subset, overflow and dynamic operations pre-effect where syntactically detectable. | PRESERVE tested subset + DEFER full simpleeval | `COMP-08` chain, unsafe expression, zero divide, overflow |
| C09 Filter | `tram/transforms/filter_rows.py:46-80`, `tests/unit/test_transforms.py:236-266`; `tests/unit/test_routing.py:11-41` | basic field-vs-literal numeric/string comparison and boolean logic with Python-compatible truthiness for admitted primitive types; absent field errors, not a false no-op; `filter` removes records with a recorded lineage disposition. Reject unsupported expressions at compile time (Python may defer parse errors to record time; intentional correction). | PRESERVE subset + CORRECT early validation | `COMP-09` comparisons, missing field, malformed condition |
| C10 Drop | `tram/transforms/drop.py:10-39`, `tests/unit/test_transforms.py:155-194` | list of top-level field names, missing names ignored. Conditional mapping and dotted paths DEFER. | PRESERVE top-level + DEFER conditional | `COMP-10` present/missing fields, reject nested |
| C11 Multi-sink isolation | `tram/pipeline/executor.py:1147-1205`, `tests/unit/test_routing.py`, `tests/unit/test_sink_transforms.py` | global transforms once per record; sink predicates and sink transforms independent per branch; failure in one sink cannot claim total success. In P1 output stays scratch and status remains ephemeral. | PRESERVE routing + ENHANCE explicit outcomes | `COMP-11` two goldens and branch mutation |
| C12 Error and DLQ | `tram/models/pipeline.py:1560-1645`, `tram/pipeline/executor.py`, `tram/interfaces/base_source.py:10-23` | P1 `on_error=abort`, fail run on malformed/failed record or sink; no automatic skip/drop/source ack; `continue/retry/dlq` and durable DLQ are rejected, not simulated. Future policy uses source-unit contract. | PRESERVE abort + DEFER other tiers | `COMP-12` config rejection, failed sink, no false ack |
| C13 Source identity/finalization | `tram/connectors/local/source.py:97-113,162-226`, `tram/interfaces/base_source.py:79-93` | content fingerprint used as logical identity; P1 never performs destructive finalize. P2 same-path replacement guard + durable manifest specified in source-unit-contract. | PRESERVE identity idea + CORRECT stronger guard | `COMP-13` same-path replacement simulated, P1 input intact |
| C14 Resource admission | Existing Python loader/executor; Rust P1 [resource-budgets](resource-budgets.md) | P1 has new explicit hard budgets, includes expanded/decoded bytes and branch queues; reject before violating limits. | ENHANCE | `RES-01..07` |
| C15 Schedule/worker/API | `tram/models/pipeline.py:20-45,1560+`; `tram/agent/server.py` | Only manual *test harness* P1. No production scheduling/agent/HTTP, persistent state, worker discovery, API compatibility claim. | DEFER | `COMP-15` unsupported compile rejected |

**Review note:** The JSON encode/config ranges in C05 and filter class/apply range in C09 have been corrected against the pinned Python SHA. The other cited source/test paths and line boundaries were audited for existence and bounds (broader class sections provide context, not executable test evidence). The expected Rust fixture remains a **proposed design oracle**, not an executed Python comparison.

## 3. Golden suite — exact expected contents

`fixtures/p1/pipeline.yaml` reads `input.json` with two records `A:12` and `B:4`, renames `old_id` to `cell_id`, creates `double_metric`, filters `metric>=10`, drops `metric`, and routes the surviving record to two local sinks. Sink B adds `tag='secondary'`.

- Expected sink A tree: `[{"cell_id":"A","double_metric":24}]`.
- Expected sink B tree: `[{"cell_id":"A","double_metric":24,"tag":"secondary"}]`.
- Source input remains byte-for-byte unchanged, including filename, file mode, length and mtime.
- `TRAM_P1_INPUT_DIR` resolves within test `in/`; `TRAM_P1_SCRATCH_A/B` resolve to fresh isolated run-scoped scratch directories; neither may be redirected to an external path.
- No source acknowledgement and no durable/final-delivery receipt are produced; successful exit is `ephemeral_completed`.

### Binding plan-compiler regressions (P1 test specifications)

Derive **each** input below from the exact `fixtures/p1/pipeline.yaml` blob pinned above. For a negative case insert only the named YAML field under `pipeline.source`; do not change the other three pinned input/output blobs. Reject **before** opening/statting/reading any source, creating scratch output, reserving a run or invoking a plugin. Inspect source bytes/mtime, scratch tree and admission count; no changes are permitted on rejection.

| Case | Golden delta | Required compile/admission result |
|---|---|---|
| `COMP-01-GOLDEN` | No mutation; bind the `TRAM_P1_*` roots within fresh isolated harness directories | **Compiles** and admits only an ephemeral P1 run; no source-finalization options; allowed read-only input and exclusive scratch output |
| `COMP-01-SKIP-TRUE` | Insert `skip_processed: true` | `CONFIG_UNSUPPORTED_OPTION`, pre-effect rejection |
| `COMP-01-DELETE-TRUE` | Insert `delete_after_read: true` | `CONFIG_UNSUPPORTED_OPTION`, pre-effect rejection |
| `COMP-01-SKIP-FALSE` | Insert `skip_processed: false` | `CONFIG_UNSUPPORTED_OPTION`, pre-effect rejection even for `false` |
| `COMP-01-DELETE-FALSE` | Insert `delete_after_read: false` | `CONFIG_UNSUPPORTED_OPTION`, pre-effect rejection even for `false` |
| `COMP-01-UNKNOWN` | Insert `unsupported_probe: false` | `CONFIG_UNSUPPORTED_OPTION`, no unknown-option fallback |

The positive golden has only `ephemeral_completed` as a successful runtime status; it does not generate delivery or source-ack receipts. These cases are **future deterministic validation requirements**; they have not been executed.

`COMP-F01`–`COMP-F08` additional fault/golden cases: malformed JSON; duplicate path; array with scalar; empty list; two sinks one failed; oversize input; missing env; unknown transform; together with the `ACK-01..04` simulator cases they are mandatory before claiming P1 acceptance.

## 4. Gate

Design review can accept these as **proposed** bounded behavioral decisions without Rust code. Phase 1 cannot exit until the pinned Python oracle and Rust output are compared, deliberate differences are cataloged as CORRECT/ENHANCE/DEFER, and a regression suite records exact hashes/expected typed values. Unsupported Python options fail **before external effects**. No Python state/legacy schema migration is promised by this P1 subset.
