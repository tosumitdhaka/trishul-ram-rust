# ADR-014 (PROPOSED) — Standalone worker isolation and failure domains

**Status:** PROPOSED; **Owner:** DESIGN_OWNER + Security/Operations; **re-entry:** P2 PRE-GATE for process-boundary decision, and before any production-enabled native plugin. **Frozen baseline:** `2cedaeac5657a8941fe9366f04029cd11b0cfd30`.

## Motivation and constraints

Frozen D01 defines one shared engine and `ExecutionService` semantics; current architecture permits standalone in-process calls. A native plugin can panic, abort, hang or exhaust address-space memory, taking a colocated manager/API down. Rust panic catching cannot contain OOM abort, process abort or memory unsafety. Phase 0 already says native plugins are trusted and not process-isolated. Do not misrepresent `OWN-09` adapter equivalence as process fault isolation.

## Alternatives to review

| Option | Advantage | Limitation |
|---|---|---|
| A. In-process embedded worker | Smallest P2 topology, no IPC/extra deployment | Manager and worker share fatal-failure domain; plugin crash can terminate control plane |
| B. Supervised local worker subprocess with private local IPC | Separate address spaces, clear OS kill/quiescence and restart boundary, reuse same `ExecutionService` results | IPC authentication, cross-platform transports, child lifecycle, resource quotas, journal ownership, crash recovery and packaging |
| C. Both profiles with explicit production support matrix | Local development simplicity and production isolation | Doubles conformance matrix; profile parity and failure semantics must remain coherent |

**Preferred design for independent review:** B as the *production standalone target*, with A allowed as an explicitly labeled development/test profile until B is verified. This is a **proposed decision**, not a claim that P2 already implements out-of-process transport. Avoid prematurely selecting gRPC/mTLS for local P2: Unix-domain socket peer identity on Linux and a reviewed Windows equivalent may suffice; P4 remote protocol and PKI remain D05 re-entry decisions. No unauthenticated ambient local socket.

## Invariants for any accepted option

- Exactly one executable engine and plan compiler; local and remote adapters translate to identical typed attempt/result states. No second execution implementation.
- Control manager journal and worker admission/completion/outbox journal remain separate authority boundaries; IPC failure is `unknown` where effects are possible.
- Worker restart never releases old attempt guard on process disappearance alone. Prove old process *and any external effect-capable children* are dead or fenced; retain replayable journal, exact attempt/session/fence, and immutable plan.
- Persist decision and restart/ownership audit; prohibit stale worker resurrection and dual active sessions after manager/worker restart.
- Bind socket/pipe to intended user/workload identity, no arbitrary local caller starts or revokes an attempt.
- Enforce process-memory and file descriptor caps in a portable profile. Shutdown/cancel/kill produce different semantics and evidence.

## Required isolation spike and tests

`ISO-01` native worker fatal exit (abort and OOM if safely isolated) cannot take down manager in B; manager reports loss, preserves unknown/guard.
`ISO-02` recover journal after child restart, callback lost and crash boundary; no duplicate worker admission for exact authority.
`ISO-03` process kill while plugin holds a file/sink handle; quiescence requires OS+child proof, not merely new session epoch.
`ISO-04` local IPC spoof/stale-fence/other-user denial; version negotiation and replay-fence rejection.
`ISO-05` A/B same input/plan/receipt/timeout equivalence and measured latency/RSS; no adapter maps timeout to rejection.
`ISO-06` adverse packaging/test root and platform matrix (Linux mandatory for first production profile; Windows explicit support or fail-closed unsupported).

**Acceptance:** Orchestrator records chosen supported profiles, deployment/operational limits and exact review SHA before P2 production effects. Preserve P1 scratch-only and serial test constraints; no retroactive P1 scope expansion.
