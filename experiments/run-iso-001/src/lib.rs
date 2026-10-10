// SPDX-License-Identifier: Apache-2.0
//! Nonproduction RUN+ISO-001 contract model; **not** a completed 2x2 prototype.
//! No effect execution, source acknowledgement, external IPC, SQLite, or spawning.
//! The preparatory types intentionally make uncertain dispatch fail closed.
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;
pub const BRANCH_ITEMS: u32 = 64;
pub const BRANCH_BYTES: u64 = 4 * 1024 * 1024;
pub const AGGREGATE_ITEMS: u32 = 256;
pub const AGGREGATE_BYTES: u64 = 16 * 1024 * 1024;
pub const PER_RUN_CHARGE: u64 = 64 * 1024 * 1024;
pub const SHARED_CHARGE: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttemptId {
    pub run: u64,
    pub attempt: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authority {
    pub manager_id: u64,
    pub plan_digest: [u8; 32],
    pub revision: u64,
    pub attempt: AttemptId,
    pub worker_id: u64,
    pub session: u64,
    pub fence: u64,
    pub nonce: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionCommand {
    Prepare,
    Start(Authority),
    Status(AttemptId),
    Cancel(Authority),
    Drain { worker_id: u64, session: u64 },
    GetAttempt(AttemptId),
    ReplayCompletion(AttemptId),
    AckCompletion { attempt: AttemptId, digest: [u8; 32] },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionRequest {
    pub major: u16,
    pub minor: u16,
    pub request_id: u128,
    pub caller_manager: u64,
    pub command: ExecutionCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectCode {
    UnsupportedVersion,
    Unauthenticated,
    StaleAuthority,
    ReplayConflict,
    NoAuthority,
    NoDurableProof,
    ResourceLimit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionReply {
    Prepared,
    Admitted,
    CancelAcceptedGuardHeld,
    UnknownGuardHeld,
    Rejected(RejectCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportFault {
    MaybeSentDisconnected,
}

/// Shared conceptual typed service for future A/T and I/S adapters.
/// A transport fault is never a durable non-admission proof.
pub trait ExecutionService: Send + Sync {
    fn call(&self, request: ExecutionRequest)
        -> Pin<Box<dyn Future<Output = Result<ExecutionReply, TransportFault>> + Send + '_>>;
}

#[derive(Clone, Debug)]
struct Admission {
    authority: Authority,
    cancel_requested: bool,
    unknown: bool,
}

#[derive(Default)]
struct ModelState {
    attempts: HashMap<AttemptId, Admission>,
    replay: HashMap<u128, (ExecutionRequest, ExecutionReply)>,
    seen_nonces: HashSet<[u8; 32]>,
}

/// Test-only authority model. Deliberately never reports successful effects,
/// quiescence, completion or durable no-admission receipts.
#[derive(Clone)]
pub struct ContractModel {
    manager_id: u64,
    worker_id: u64,
    worker_session: u64,
    state: Arc<Mutex<ModelState>>,
}

impl ContractModel {
    pub fn new(manager_id: u64, worker_id: u64, worker_session: u64) -> Self {
        Self { manager_id, worker_id, worker_session,
            state: Arc::new(Mutex::new(ModelState::default())) }
    }

    pub fn inspect_guard(&self, attempt: &AttemptId) -> bool {
        let state = self.state.lock().expect("contract model lock poisoned");
        state.attempts.contains_key(attempt)
    }

    fn apply(&self, request: ExecutionRequest) -> ExecutionReply {
        if request.major != PROTOCOL_MAJOR || request.minor != PROTOCOL_MINOR {
            return ExecutionReply::Rejected(RejectCode::UnsupportedVersion);
        }
        if request.caller_manager != self.manager_id || request.request_id == 0 {
            return ExecutionReply::Rejected(RejectCode::Unauthenticated);
        }
        let mut state = self.state.lock().expect("contract model lock poisoned");
        if let Some((previous, response)) = state.replay.get(&request.request_id) {
            return if previous == &request { response.clone() }
                else { ExecutionReply::Rejected(RejectCode::ReplayConflict) };
        }
        let reply = match &request.command {
            ExecutionCommand::Prepare => ExecutionReply::Prepared,
            ExecutionCommand::Start(auth) => {
                if auth.manager_id != self.manager_id || auth.worker_id != self.worker_id
                    || auth.session != self.worker_session || auth.fence == 0
                    || auth.revision == 0 || auth.plan_digest == [0; 32]
                    || auth.nonce == [0; 32] {
                    ExecutionReply::Rejected(RejectCode::StaleAuthority)
                } else if let Some(existing) = state.attempts.get(&auth.attempt) {
                    if existing.authority == *auth {
                        ExecutionReply::Admitted // same-attempt, no second effect
                    } else {
                        ExecutionReply::Rejected(RejectCode::StaleAuthority)
                    }
                } else if state.seen_nonces.contains(&auth.nonce) {
                    ExecutionReply::Rejected(RejectCode::ReplayConflict)
                } else {
                    state.seen_nonces.insert(auth.nonce);
                    state.attempts.insert(auth.attempt.clone(), Admission {
                        authority: auth.clone(), cancel_requested: false, unknown: true });
                    ExecutionReply::Admitted // in-memory model only; NOT a durable receipt
                }
            }
            ExecutionCommand::Cancel(auth) => {
                match state.attempts.get_mut(&auth.attempt) {
                    Some(existing) if existing.authority == *auth => {
                        existing.cancel_requested = true;
                        existing.unknown = true;
                        ExecutionReply::CancelAcceptedGuardHeld
                    }
                    _ => ExecutionReply::Rejected(RejectCode::StaleAuthority),
                }
            }
            ExecutionCommand::Status(id) | ExecutionCommand::GetAttempt(id)
                | ExecutionCommand::ReplayCompletion(id) => {
                if state.attempts.get(id).is_some() {
                    ExecutionReply::UnknownGuardHeld
                } else { ExecutionReply::Rejected(RejectCode::NoAuthority) }
            }
            ExecutionCommand::AckCompletion { .. } => {
                ExecutionReply::Rejected(RejectCode::NoDurableProof)
            }
            ExecutionCommand::Drain { worker_id, session } => {
                if *worker_id != self.worker_id || *session != self.worker_session {
                    ExecutionReply::Rejected(RejectCode::StaleAuthority)
                } else { ExecutionReply::UnknownGuardHeld }
            }
        };
        state.replay.insert(request.request_id, (request, reply.clone()));
        reply
    }
}

impl ExecutionService for ContractModel {
    fn call(&self, request: ExecutionRequest)
        -> Pin<Box<dyn Future<Output = Result<ExecutionReply, TransportFault>> + Send + '_>> {
        Box::pin(async move { Ok(self.apply(request)) })
    }
}

/// Manager-facing loss-of-reply mapping: guard always remains UNKNOWN.
pub fn uncertain_dispatch(_fault: TransportFault) -> ExecutionReply {
    ExecutionReply::UnknownGuardHeld
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Charge {
    pub items: u32,
    pub payload_bytes: u64,
    pub live_bytes: u64,
}

#[derive(Default)]
pub struct QueueBudget {
    per_branch: HashMap<(u64, u8), Charge>,
    per_run: HashMap<u64, Charge>,
    total: Charge,
}

impl QueueBudget {
    /// Atomic A+B reservation: no partial branch enqueue on capacity denial.
    /// The caller must separately prove actual OS/process/IPC allocations.
    pub fn reserve_pair(&mut self, run: u64, items_each: u32,
                        bytes_each: u64, live_bytes: u64) -> Result<(), RejectCode> {
        let next_total = add_charge(self.total, Charge {
            items: items_each.checked_mul(2).ok_or(RejectCode::ResourceLimit)?,
            payload_bytes: bytes_each.checked_mul(2).ok_or(RejectCode::ResourceLimit)?,
            live_bytes,
        })?;
        if next_total.items > AGGREGATE_ITEMS || next_total.payload_bytes > AGGREGATE_BYTES
            || next_total.live_bytes > SHARED_CHARGE {
            return Err(RejectCode::ResourceLimit);
        }
        let per_run = add_charge(*self.per_run.get(&run).unwrap_or(&Charge::default()),
                                 Charge { items: items_each * 2,
                                          payload_bytes: bytes_each * 2, live_bytes })?;
        if per_run.live_bytes > PER_RUN_CHARGE { return Err(RejectCode::ResourceLimit); }
        let mut next_branches = Vec::new();
        for branch in [0_u8, 1] {
            let key = (run, branch);
            let next = add_charge(*self.per_branch.get(&key).unwrap_or(&Charge::default()),
                                  Charge { items: items_each, payload_bytes: bytes_each,
                                           live_bytes: 0 })?;
            if next.items > BRANCH_ITEMS || next.payload_bytes > BRANCH_BYTES {
                return Err(RejectCode::ResourceLimit);
            }
            next_branches.push((key, next));
        }
        self.total = next_total;
        self.per_run.insert(run, per_run);
        for (key, next) in next_branches { self.per_branch.insert(key, next); }
        Ok(())
    }

    pub fn total(&self) -> Charge { self.total }
}

fn add_charge(a: Charge, b: Charge) -> Result<Charge, RejectCode> {
    Ok(Charge {
        items: a.items.checked_add(b.items).ok_or(RejectCode::ResourceLimit)?,
        payload_bytes: a.payload_bytes.checked_add(b.payload_bytes)
            .ok_or(RejectCode::ResourceLimit)?,
        live_bytes: a.live_bytes.checked_add(b.live_bytes)
            .ok_or(RejectCode::ResourceLimit)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth() -> Authority {
        Authority { manager_id: 1, plan_digest: [7; 32], revision: 1,
            attempt: AttemptId { run: 100, attempt: 3 }, worker_id: 2,
            session: 4, fence: 5, nonce: [8; 32] }
    }
    fn call(model: &ContractModel, id: u128, cmd: ExecutionCommand) -> ExecutionReply {
        model.apply(ExecutionRequest { major: 1, minor: 0, request_id: id,
            caller_manager: 1, command: cmd })
    }

    #[test]
    fn rejects_version_and_session_before_admission() {
        let model = ContractModel::new(1, 2, 4);
        let mut bad = auth();
        bad.session = 99;
        assert_eq!(call(&model, 1, ExecutionCommand::Start(bad)),
            ExecutionReply::Rejected(RejectCode::StaleAuthority));
        assert!(!model.inspect_guard(&auth().attempt));
        let wrong_major = ExecutionRequest { major: 9, minor: 0, request_id: 2,
            caller_manager: 1, command: ExecutionCommand::Start(auth()) };
        assert_eq!(model.apply(wrong_major),
            ExecutionReply::Rejected(RejectCode::UnsupportedVersion));
    }

    #[test]
    fn retries_and_replays_cannot_replace_unknown_guard() {
        let model = ContractModel::new(1, 2, 4);
        let a = auth();
        assert_eq!(call(&model, 10, ExecutionCommand::Start(a.clone())), ExecutionReply::Admitted);
        assert_eq!(call(&model, 10, ExecutionCommand::Start(a.clone())), ExecutionReply::Admitted);
        assert_eq!(call(&model, 10, ExecutionCommand::Cancel(a.clone())),
            ExecutionReply::Rejected(RejectCode::ReplayConflict));
        let mut new_fence = a.clone();
        new_fence.fence = 6;
        assert_eq!(call(&model, 11, ExecutionCommand::Start(new_fence)),
            ExecutionReply::Rejected(RejectCode::StaleAuthority));
        assert_eq!(call(&model, 12, ExecutionCommand::Cancel(a.clone())),
            ExecutionReply::CancelAcceptedGuardHeld);
        assert_eq!(call(&model, 13, ExecutionCommand::AckCompletion {
            attempt: a.attempt.clone(), digest: [9; 32] }),
            ExecutionReply::Rejected(RejectCode::NoDurableProof));
        assert!(model.inspect_guard(&a.attempt));
        assert_eq!(uncertain_dispatch(TransportFault::MaybeSentDisconnected),
            ExecutionReply::UnknownGuardHeld);
    }

    #[test]
    fn pair_reservation_denies_atomically() {
        let mut b = QueueBudget::default();
        b.reserve_pair(1, 63, 1000, 1024).unwrap();
        let before = b.total();
        assert_eq!(b.reserve_pair(1, 2, 100, 32), Err(RejectCode::ResourceLimit));
        assert_eq!(b.total(), before);
        assert_eq!(b.reserve_pair(2, 8, 2 * 1024 * 1024 + 1, 0),
            Err(RejectCode::ResourceLimit));
        assert_eq!(b.total(), before);
    }
}
