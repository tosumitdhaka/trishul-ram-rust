# Phase 0 independent-review remediation map — design candidate

**Review:** `CHANGES_REQUIRED`, GitHub review `5473153676`, anchored to `96e11006e4784d2e70a7b0663c598c16376b04f7`. **Base:** `0e97f24e3936d923abd77875038494dd960039bb`. **Scope:** documentation, proposed decision register and test fixtures **only**. There is no architecture approval, no implementation authorization, no production code and no executed Rust/Python test evidence. The new candidate SHA is recorded on the PR review handoff comment after the single commit.

## Finding-by-finding resolution — precise file/section/line anchors

| Review ID | Remedy / explicit policy | Verification files and lines (within revised proposal) |
|---|---|---|
| **P0-R1** | P1 is **non-production, ephemeral-only**: read-only input, exclusively new scratch-local artifacts, no source-finalization, no durable/strict claim; P2 requires durable admission/ledger/journal and staged file receipt before any production effect. Crash behavior and exact P1 test cases are explicit. | `p1-safety-boundary.md:5–33`; `migration-plan.md` P1/P2 gate table; `architecture.md` phase-boundary section |
| **P0-R2** | Stable SourceUnitId / child output lineage / per-sink EffectId and ReceiptId; sealed obligation barrier, explicit empty/filtered and expansion cases, contiguous checkpoint frontier, partial A-confirmed/B-unknown replay, staged fsync/rename and same-path replacement guard, crash tests. | `source-unit-contract.md:5–65`; `reliability-contracts.md` §2; `plugin-contracts.md` P1 clarification |
| **P0-R3** | Normative manager transition table and worker command matrix; CAS exact authority binding, durable cancel tombstones, admission vs running vs completion vs callback ack, unknown/lease/session/manager-crash handling and in-process/remote result taxonomy. | `attempt-protocol.md:5–75`; `architecture.md` §5; `reliability-contracts.md` §1/§3 |
| **P0-R4** | Pinned Python v1.8.0 reference, explicit 15-row PRESERVE/CORRECT/ENHANCE/DEFER matrix, bounded YAML/options/env subset, input/output fixture blobs, JSON and four transforms, local file restrictions, branch isolation, DLQ/error deferrals. | `p1-compatibility-matrix.md:1–63`, `fixtures/p1/{pipeline.yaml,input.json,expected-a.json,expected-b.json}` |
| **P0-R5** | Hierarchical preallocation charge/reservation interface, fixed count/byte/depth/cardinality/queue/I/O/scratch quotas, backpressure or fail-closed classification, cancellation and disk-full/no-eviction rules, observable capacity and eight acceptance cases. | `resource-budgets.md:5–66`; `p1-safety-boundary.md` test IDs |
| **P0-R6** | All D01–D12 received explicit ACCEPT or bounded DEFER recommendations, owner, phase applicability and re-entry. Chosen Apache-2.0 repository licensing/import rules, secure loopback development, remote mutual TLS/token design, SNMP MIB producer/provenance and Counter64/varbind mapping requirements, no undocumented Python migration or mixed-worker guarantee. | `decision-register.md:1–37`; `migration-plan.md` §5 |

Line ranges are within these proposed Markdown files and are provided as navigation anchors, not claims of executed tests. Reviewers should inspect the exact new PR commit and its diff against **the prior reviewed commit**, not only against `main`.

## Reviewer focus and blockers that remain until re-review

1. Verify P1 scratch-only restrictions do not leak a production delivery claim through the existing design text or fixture defaults.
2. Validate that branch receipt/checkpoint accounting remains correct for replayable vs non-idempotent sink effects and that impossible cross-system atomicity is not claimed.
3. Validate restart/session rollover and cancellation-quiescence criteria (especially whether a new session actually proves old effects ended).
4. Check Python matrix against **actual** pinned code/tests: fixture expected results are source-derived design expectations and must be run as a Python oracle in P1, not reported as verified here.
5. Decide whether recommended D01–D12 ACCEPT/DEFER dispositions, chosen license and re-entry gates suffice for Phase 0 freeze.
6. Request additional narrow documentation changes if assumptions remain underdefined. Do **not** infer Phase 1 authorization or approval from this commit or a closed checklist.

## Governance next action

1. DESIGN_OWNER commits this documentation-only candidate on `design/phase-0-architecture`, publishes exact full SHA and this finding map to PR #1.
2. Independent REVIEWER compares `96e11006e4784d2e70a7b0663c598c16376b04f7` → new candidate at live PR head; returns PASS or actionable CHANGES_REQUIRED against exact SHA.
3. ORCHESTRATOR/Project Owner records `PHASE_0_ARCHITECTURE_APPROVED` only if independent review passes, with SHA and any bounded conditions; then separately authorizes P1 code scope and gates.
4. Before approval, PR remains draft. Do not merge, change Python reference/SNMP repo, or begin Rust implementation.

## R2 targeted remediation — review 5473428568 (successor candidate)

**Governance:** the R2 review on `d128df98b13c4f9316773cd66e2bb965850127b2` remains `CHANGES_REQUIRED`; Phase 0 remains a proposal and Phase 1 is **WITHHELD**. This section documents the bounded DESIGN_OWNER response, **not** independent review approval.

| R2 finding | Before | Proposed after / verification |
|---|---|---|
| R2-F1 — HIGH | Golden YAML included `skip_processed:false` and `delete_after_read:false` although P1 rejects both options | Remove both keys, repin exact new golden Git blob in `p1-compatibility-matrix.md`, require `COMP-01-GOLDEN` compile and `COMP-01-SKIP/DELETE-{TRUE,FALSE}` plus unknown-option pre-effect rejections; `p1-safety-boundary.md` explicitly states absent-only policy |
| R2-F2 — HIGH | `dispatching` had no journal-proven rejection transition; `admitted` could not terminalize a proven pre-start failure | `attempt-protocol.md` defines exact-authority refusal tombstone and guarded `terminal_rejected`, atomic pre-start failure/outbox and terminal failed receipt, guard/retry/replay/unknown handling, and `OWN-10..14` acceptance scenarios |
| R2-F3 — LOW | C05 and C09 citations extended beyond the pinned Python file lengths | C05 now cites `json_serializer.py:34-38` and `pipeline.py:1247-1250` for the serializer config; C09 cites `filter_rows.py:46-80` and its existing test class `test_transforms.py:236-266` |

**Validation scope:** fixture/source citation pinning and static Markdown/YAML contract inspection; **no** runtime, Python differential, Rust build or restart test is implied. Preserve closed P0-R1/R2/R5/R6 contracts unchanged. Independent exact-SHA re-review of R2-F1/F2/F3 is required before any architecture gate decision.