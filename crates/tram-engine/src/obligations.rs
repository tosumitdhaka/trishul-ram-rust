// SPDX-License-Identifier: Apache-2.0
//! Pure P2+ obligation/receipt simulator, NEVER a P1 source acknowledgement.
//! In-memory-only logical states cannot mint a receipt or source checkpoint.
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Pending,
    Filtered,
    Confirmed,
    Unknown,
    Failed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObligationError {
    Sealed,
    NotSealed,
    Duplicate,
    OutOfBounds,
    Undecided,
    NoCheckpoint,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordDisposition {
    Retained,
    FilteredGlobal,
    EmptySourceUnit,
    Invalid,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulatedBarrier {
    sealed: bool,
    decoded_count: usize,
    expected_slots: usize,
    record_dispositions: Vec<RecordDisposition>,
    branches: BTreeMap<(usize, usize), Disposition>,
    checkpoint_committed: bool,
}
impl SimulatedBarrier {
    pub fn new() -> Self {
        Self::with_slots(1)
    }
    pub fn with_slots(slots: usize) -> Self {
        assert!(
            (1..=2).contains(&slots),
            "P1 test simulator has one or two sink slots"
        );
        Self {
            sealed: false,
            decoded_count: 0,
            expected_slots: slots,
            record_dispositions: Vec::new(),
            branches: BTreeMap::new(),
            checkpoint_committed: false,
        }
    }
    pub fn open_record(&mut self) -> Result<usize, ObligationError> {
        if self.sealed {
            return Err(ObligationError::Sealed);
        }
        let index = self.decoded_count;
        self.decoded_count = self
            .decoded_count
            .checked_add(1)
            .ok_or(ObligationError::OutOfBounds)?;
        self.record_dispositions.push(RecordDisposition::Retained);
        Ok(index)
    }
    pub fn mark_global_filtered(&mut self, index: usize) -> Result<(), ObligationError> {
        if self.sealed {
            return Err(ObligationError::Sealed);
        }
        let existing = self
            .record_dispositions
            .get_mut(index)
            .ok_or(ObligationError::OutOfBounds)?;
        *existing = RecordDisposition::FilteredGlobal;
        Ok(())
    }
    pub fn branch(
        &mut self,
        index: usize,
        slot: usize,
        disposition: Disposition,
    ) -> Result<(), ObligationError> {
        if self.sealed {
            return Err(ObligationError::Sealed);
        }
        if slot >= self.expected_slots
            || self.record_dispositions.get(index) != Some(&RecordDisposition::Retained)
        {
            return Err(ObligationError::OutOfBounds);
        }
        if self.branches.contains_key(&(index, slot)) {
            return Err(ObligationError::Duplicate);
        }
        self.branches.insert((index, slot), disposition);
        Ok(())
    }
    pub fn seal(&mut self) -> Result<(), ObligationError> {
        if self.sealed {
            return Err(ObligationError::Sealed);
        }
        if self.decoded_count == 0 {
            self.record_dispositions
                .push(RecordDisposition::EmptySourceUnit);
        }
        self.sealed = true;
        Ok(())
    }
    /// One transition is a simulated receipt and cannot be used as production authority.
    pub fn simulated_receipt(
        &mut self,
        index: usize,
        slot: usize,
        result: Disposition,
    ) -> Result<(), ObligationError> {
        if !self.sealed {
            return Err(ObligationError::NotSealed);
        }
        let before = self
            .branches
            .get_mut(&(index, slot))
            .ok_or(ObligationError::OutOfBounds)?;
        if *before != Disposition::Pending && *before != Disposition::Unknown {
            return Err(ObligationError::Duplicate);
        }
        *before = result;
        Ok(())
    }
    pub fn logical_decided(&self) -> bool {
        self.sealed
            && self
                .record_dispositions
                .iter()
                .all(|d| *d != RecordDisposition::Invalid)
            && self
                .branches
                .values()
                .all(|d| matches!(d, Disposition::Filtered | Disposition::Confirmed))
            && self.record_dispositions.iter().enumerate().all(|(i, d)| {
                *d != RecordDisposition::Retained
                    || (0..self.expected_slots).all(|slot| self.branches.contains_key(&(i, slot)))
            })
    }
    /// No P1 engine invokes or has access to a destructive ack API.
    /// This predicate models what a future durable checkpoint would require.
    pub fn checkpoint_eligible(&self) -> bool {
        self.logical_decided() && self.checkpoint_committed
    }
    pub fn simulate_checkpoint(&mut self, success: bool) -> Result<(), ObligationError> {
        if !self.logical_decided() {
            return Err(ObligationError::Undecided);
        }
        if !success {
            return Err(ObligationError::NoCheckpoint);
        }
        self.checkpoint_committed = true;
        Ok(())
    }
    pub fn pending_unknown(&self) -> usize {
        self.branches
            .values()
            .filter(|d| {
                matches!(
                    d,
                    Disposition::Pending | Disposition::Unknown | Disposition::Failed
                )
            })
            .count()
    }
    pub fn decoded_count(&self) -> usize {
        self.decoded_count
    }
}
impl Default for SimulatedBarrier {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ack_01_seal_two_branches_and_one_filtered() {
        let mut barrier = SimulatedBarrier::with_slots(2);
        for _ in 0..3 {
            barrier.open_record().unwrap();
        }
        barrier.mark_global_filtered(1).unwrap();
        for index in [0, 2] {
            for slot in 0..2 {
                barrier.branch(index, slot, Disposition::Pending).unwrap();
            }
        }
        assert!(!barrier.checkpoint_eligible());
        barrier.seal().unwrap();
        assert!(barrier
            .simulated_receipt(0, 0, Disposition::Confirmed)
            .is_ok());
        assert!(!barrier.logical_decided());
        for (i, slot) in [(0, 1), (2, 0), (2, 1)] {
            barrier
                .simulated_receipt(i, slot, Disposition::Confirmed)
                .unwrap();
        }
        assert!(barrier.logical_decided());
        assert!(!barrier.checkpoint_eligible());
        barrier.simulate_checkpoint(true).unwrap();
        assert!(barrier.checkpoint_eligible());
    }
    #[test]
    fn ack_02_branch_a_confirmed_b_unknown_replay_does_not_ack() {
        let mut barrier = SimulatedBarrier::with_slots(2);
        let row = barrier.open_record().unwrap();
        barrier.branch(row, 0, Disposition::Pending).unwrap();
        barrier.branch(row, 1, Disposition::Pending).unwrap();
        barrier.seal().unwrap();
        barrier
            .simulated_receipt(row, 0, Disposition::Confirmed)
            .unwrap();
        barrier
            .simulated_receipt(row, 1, Disposition::Unknown)
            .unwrap();
        assert_eq!(barrier.pending_unknown(), 1);
        assert!(!barrier.logical_decided());
        assert_eq!(
            barrier.simulate_checkpoint(true),
            Err(ObligationError::Undecided)
        );
        barrier
            .simulated_receipt(row, 1, Disposition::Confirmed)
            .unwrap();
        assert!(barrier.logical_decided());
    }
    #[test]
    fn ack_03_empty_and_deliberate_filter_are_not_vacuous_pending() {
        let mut empty = SimulatedBarrier::new();
        assert!(!empty.logical_decided());
        empty.seal().unwrap();
        assert!(empty.logical_decided());
        assert!(!empty.checkpoint_eligible());
        let mut filtered = SimulatedBarrier::new();
        let row = filtered.open_record().unwrap();
        filtered.mark_global_filtered(row).unwrap();
        filtered.seal().unwrap();
        assert!(filtered.logical_decided());
        assert_eq!(filtered.open_record(), Err(ObligationError::Sealed));
        let mut unexpected = SimulatedBarrier::new();
        unexpected.open_record().unwrap();
        unexpected.seal().unwrap();
        assert!(!unexpected.logical_decided());
    }
    #[test]
    fn ack_01_missing_branch_and_duplicate_insertion_cannot_be_silent() {
        let mut barrier = SimulatedBarrier::with_slots(2);
        let row = barrier.open_record().unwrap();
        barrier.branch(row, 0, Disposition::Pending).unwrap();
        assert_eq!(
            barrier.branch(row, 0, Disposition::Confirmed),
            Err(ObligationError::Duplicate)
        );
        barrier.seal().unwrap();
        barrier
            .simulated_receipt(row, 0, Disposition::Confirmed)
            .unwrap();
        assert!(
            !barrier.logical_decided(),
            "missing B cannot be treated as decided"
        );
    }
    #[test]
    fn ack_04_checkpoint_failure_prevents_ack_eligibility() {
        let mut b = SimulatedBarrier::new();
        let row = b.open_record().unwrap();
        b.branch(row, 0, Disposition::Pending).unwrap();
        assert_eq!(
            b.simulated_receipt(row, 0, Disposition::Confirmed),
            Err(ObligationError::NotSealed)
        );
        b.seal().unwrap();
        b.simulated_receipt(row, 0, Disposition::Confirmed).unwrap();
        assert_eq!(
            b.simulate_checkpoint(false),
            Err(ObligationError::NoCheckpoint)
        );
        assert!(!b.checkpoint_eligible());
        b.simulate_checkpoint(true).unwrap();
        assert!(b.checkpoint_eligible());
    }
    #[test]
    fn res_04_p2_expansion_64_allowed_65_refused_without_logical_ack() {
        use crate::budget::{BudgetCaps, BudgetLedger, Category};
        // P2-only expansion budget simulator: no expand operator is admitted by
        // the frozen P1 compiler and no child is emitted by this test.
        let ledger = BudgetLedger::new(BudgetCaps {
            records: 64, ..BudgetCaps::default()
        });
        let children = ledger.reserve(Category::Records, 64).unwrap();
        assert_eq!(ledger.current().records, 64);
        assert!(ledger.reserve(Category::Records, 1).is_err());
        let mut barrier=SimulatedBarrier::new();
        let r=barrier.open_record().unwrap();
        barrier.branch(r,0,Disposition::Pending).unwrap();
        barrier.seal().unwrap();
        assert!(!barrier.logical_decided());
        assert!(!barrier.checkpoint_eligible());
        drop(children);
        assert_eq!(ledger.current().records, 0);
    }

}
