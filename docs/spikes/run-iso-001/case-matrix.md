# RUN+ISO-001 — precommitted case and negative-vector matrix

**Status:** definition/preparation only. Every cell is **NOT_RUN**. `A` = std-thread bounded channel; `T` = Tokio bounded async stages; `I` = embedded; `S` = supervised authenticated local worker. Shared typed request/reply, manager ledger / worker journal boundaries, confirmation tier and fixture identity shall be identical in all four cells. Runtime/topology selection remains undecided.

| Case | A×I | A×S | T×I | T×S | Evidence required at future Stage B |
|---|---|---|---|---|---|
| RUN-01 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Two concurrently admitted runs; overlap of two *actual* I/O intervals >=100 ms; identical command and branch receipts |
| RUN-02 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Block B; exact queue item+byte HWM <=64/4 MiB per branch and <=256/16 MiB aggregate, upstream pause before overflow, source not acked |
| RUN-03 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Cancel while waiting byte permit, SQLite lock, blocked fsync; <=2 s cancel and <=3 s responsive UNKNOWN; held guard |
| RUN-04 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Panic/abort/OOM; S manager survives; I accurately reports larger fatal failure domain |
| RUN-05 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Two-run shared 128 MiB and per-run 64 MiB charge, fairness >=20% in each 5s eligible window and <=2s forward progress; no permit leaks |
| RUN-06 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Real process restart + kill after mock write before receipt and commit-outbox before response; no false no-effect proof/ack |
| RUN-07 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Three clean trials/cell; throughput >=80% A×I baseline, p99 <=2x, summed VmHWM <=1.5x and <=256 MiB, FD <=128/process and <=256 total |
| RUN-08 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | P1 serial/scratch-only semantics/source fixtures untouched; no attempt to retrofit P1 concurrency |
| ISO-01 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Worker fatal process boundary, manager continuity only in S; unknown ownership held |
| ISO-02 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Mock manager/worker journals independently recovered, identical fenced original plan and worker outbox replay |
| ISO-03 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Old child retains scratch FD until verified quiescent; <=5 s S kill/prove-fenced; never release while old writes possible |
| ISO-04 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Spoof/wrong UID, version downgrade, bad nonce, stale fence and cross-run token reject before effects |
| ISO-05 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Identical byte-level direct vs local IPC request/reply vectors and interpretation |
| ISO-06 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | Linux UDS peer UID/GID/PID and owned parent perms; no TCP fallback; unsupported OS rejects |

## Negative authority vectors (not run)

`V01` wrong protocol major; `V02` mandatory minor/feature downgrade; `V03` wrong manager ID; `V04` wrong Linux UID/GID/PID process generation; `V05` stale worker session; `V06` stale fence; `V07` re-used nonce; `V08` request_id reused with a different command; `V09` exact replay that must be idempotent; `V10` cross-run token; `V11` old plan digest/revision after update; `V12` maybe-sent IPC disconnect mapped to durable UNKNOWN; `V13` old descendant writes after parent exits; `V14` forged no-admission tombstone; `V15` journal/volume loss cannot release guard or source ack; `V16` A confirmed/B UNKNOWN (A receipt never rewritten, B reconciled only under original authority). The preparatory Rust contract model tests only a subset of V01/V05/V06/V08/V09/V12, not full system proof.

## Deferred fault seams, only after explicit Stage B authorization

`F01` crash-before-manager-admission; `F02` crash after worker admission before effect; `F03` write-before-receipt; `F04` receipt+outbox committed-before-response; `F05` manager commits callback-before-reply; `F06` old descendant effect-capable after parent SIGKILL; `F07` SQLite lock and blocked `fsync`; `F08` disk full and journal corrupt/lost. Before/after snapshots must include manager source-generation CAS, immutable original plan revision/digest, worker journal/outbox, per-branch EffectId/receipt and scratch file hashes. All cases require exact exit code, stdout/stderr, event trace hash, PID/start generation and descendant inventory.

The source acknowledgement predicate stays **false** if barrier is unsealed, B is pending/unknown, checkpoint is missing, or any old effect-capable process remains; no operator risk override or receiver idempotency bypasses the original source-generation guard. No experiment result, pass, or timeout classification may be inferred from these planned cases.
