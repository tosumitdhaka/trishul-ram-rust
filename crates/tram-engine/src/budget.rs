// SPDX-License-Identifier: Apache-2.0
//! Pure, move-only reservation ledger. This cannot access a filesystem.
//! Every reservation is held until its owner drops it, including error paths.
use std::{cell::RefCell, rc::Rc};

pub const MIB: usize = 1_048_576;
pub const RAW_FILE_MAX: usize = 4 * MIB;
pub const RAW_PENDING_MAX: usize = 1;
pub const JSON_MAX_DEPTH: usize = 32;
pub const JSON_MAX_TOKENS: usize = 100_000;
pub const RECORDS_PER_SOURCE_MAX: usize = 4_096;
pub const DECODED_SOURCE_MAX: usize = 16 * MIB;
pub const ENCODED_RECORD_MAX: usize = MIB;
pub const BRANCH_COUNT_MAX: usize = 2;
pub const BRANCH_PENDING_MAX: usize = 8 * MIB;
pub const TOTAL_PENDING_MAX: usize = 16 * MIB;
pub const IO_CONCURRENCY_MAX: usize = 2;
pub const LIVE_TOTAL_MAX: usize = 64 * MIB;
pub const SCRATCH_RUN_MAX: usize = 64 * MIB;
pub const SCRATCH_ARTIFACT_MAX: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    RawBytes,
    RawBuffers,
    DecodedBytes,
    Records,
    BranchBytes(usize),
    SinkIo,
    LiveBytes,
    ScratchBytes,
    ScratchArtifacts,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub raw_bytes: usize,
    pub raw_buffers: usize,
    pub decoded_bytes: usize,
    pub records: usize,
    pub branch_bytes: [usize; BRANCH_COUNT_MAX],
    pub sink_io: usize,
    pub live_bytes: usize,
    pub scratch_bytes: usize,
    pub scratch_artifacts: usize,
}
impl Totals {
    pub fn value(&self, category: Category) -> Option<usize> {
        Some(match category {
            Category::RawBytes => self.raw_bytes,
            Category::RawBuffers => self.raw_buffers,
            Category::DecodedBytes => self.decoded_bytes,
            Category::Records => self.records,
            Category::BranchBytes(i) => *self.branch_bytes.get(i)?,
            Category::SinkIo => self.sink_io,
            Category::LiveBytes => self.live_bytes,
            Category::ScratchBytes => self.scratch_bytes,
            Category::ScratchArtifacts => self.scratch_artifacts,
        })
    }
    fn cell_mut(&mut self, category: Category) -> Option<&mut usize> {
        Some(match category {
            Category::RawBytes => &mut self.raw_bytes,
            Category::RawBuffers => &mut self.raw_buffers,
            Category::DecodedBytes => &mut self.decoded_bytes,
            Category::Records => &mut self.records,
            Category::BranchBytes(i) => self.branch_bytes.get_mut(i)?,
            Category::SinkIo => &mut self.sink_io,
            Category::LiveBytes => &mut self.live_bytes,
            Category::ScratchBytes => &mut self.scratch_bytes,
            Category::ScratchArtifacts => &mut self.scratch_artifacts,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetCaps {
    pub raw_bytes: usize,
    pub raw_buffers: usize,
    pub decoded_bytes: usize,
    pub records: usize,
    pub branch_bytes: usize,
    pub sink_io: usize,
    pub live_bytes: usize,
    pub total_pending: usize,
    pub scratch_bytes: usize,
    pub scratch_artifacts: usize,
}
impl Default for BudgetCaps {
    fn default() -> Self {
        Self {
            raw_bytes: RAW_FILE_MAX,
            raw_buffers: RAW_PENDING_MAX,
            decoded_bytes: DECODED_SOURCE_MAX,
            records: RECORDS_PER_SOURCE_MAX,
            branch_bytes: BRANCH_PENDING_MAX,
            sink_io: IO_CONCURRENCY_MAX,
            live_bytes: LIVE_TOTAL_MAX,
            total_pending: TOTAL_PENDING_MAX,
            scratch_bytes: SCRATCH_RUN_MAX,
            scratch_artifacts: SCRATCH_ARTIFACT_MAX,
        }
    }
}
impl BudgetCaps {
    pub fn lowered(self, lower: Self) -> Self {
        Self {
            raw_bytes: self.raw_bytes.min(lower.raw_bytes),
            raw_buffers: self.raw_buffers.min(lower.raw_buffers),
            decoded_bytes: self.decoded_bytes.min(lower.decoded_bytes),
            records: self.records.min(lower.records),
            branch_bytes: self.branch_bytes.min(lower.branch_bytes),
            sink_io: self.sink_io.min(lower.sink_io),
            live_bytes: self.live_bytes.min(lower.live_bytes),
            total_pending: self.total_pending.min(lower.total_pending),
            scratch_bytes: self.scratch_bytes.min(lower.scratch_bytes),
            scratch_artifacts: self.scratch_artifacts.min(lower.scratch_artifacts),
        }
    }
    pub fn cap(self, category: Category) -> Option<usize> {
        Some(match category {
            Category::RawBytes => self.raw_bytes,
            Category::RawBuffers => self.raw_buffers,
            Category::DecodedBytes => self.decoded_bytes,
            Category::Records => self.records,
            Category::BranchBytes(i) if i < BRANCH_COUNT_MAX => self.branch_bytes,
            Category::BranchBytes(_) => return None,
            Category::SinkIo => self.sink_io,
            Category::LiveBytes => self.live_bytes,
            Category::ScratchBytes => self.scratch_bytes,
            Category::ScratchArtifacts => self.scratch_artifacts,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    ResourceExhausted,
    ArithmeticOverflow,
    InvalidBranch,
}
struct Shared {
    current: Totals,
    peaks: Totals,
    caps: BudgetCaps,
}
#[derive(Clone)]
pub struct BudgetLedger(Rc<RefCell<Shared>>);
impl BudgetLedger {
    pub fn p1() -> Self {
        Self::new(BudgetCaps::default())
    }
    pub fn new(requested: BudgetCaps) -> Self {
        Self(Rc::new(RefCell::new(Shared {
            current: Totals::default(),
            peaks: Totals::default(),
            caps: BudgetCaps::default().lowered(requested),
        })))
    }
    pub fn current(&self) -> Totals {
        self.0.borrow().current
    }
    pub fn peaks(&self) -> Totals {
        self.0.borrow().peaks
    }
    pub fn caps(&self) -> BudgetCaps {
        self.0.borrow().caps
    }
    pub fn reserve(&self, category: Category, amount: usize) -> Result<Reservation, BudgetError> {
        let mut shared = self.0.borrow_mut();
        let cap = shared
            .caps
            .cap(category)
            .ok_or(BudgetError::InvalidBranch)?;
        let current = shared
            .current
            .value(category)
            .ok_or(BudgetError::InvalidBranch)?;
        let next = current
            .checked_add(amount)
            .ok_or(BudgetError::ArithmeticOverflow)?;
        if next > cap {
            return Err(BudgetError::ResourceExhausted);
        }
        if matches!(category, Category::BranchBytes(_)) {
            let pending = shared
                .current
                .branch_bytes
                .iter()
                .try_fold(0usize, |acc, x| acc.checked_add(*x))
                .ok_or(BudgetError::ArithmeticOverflow)?;
            if pending
                .checked_add(amount)
                .ok_or(BudgetError::ArithmeticOverflow)?
                > shared.caps.total_pending
            {
                return Err(BudgetError::ResourceExhausted);
            }
        }
        *shared
            .current
            .cell_mut(category)
            .ok_or(BudgetError::InvalidBranch)? = next;
        let peak = shared
            .peaks
            .cell_mut(category)
            .ok_or(BudgetError::InvalidBranch)?;
        *peak = (*peak).max(next);
        Ok(Reservation {
            ledger: self.clone(),
            category,
            amount,
        })
    }
    /// All-or-none branch queue capacity. If any reservation fails,
    /// already acquired reservations are dropped before returning.
    pub fn reserve_fanout(&self, branch_bytes: &[usize]) -> Result<Vec<Reservation>, BudgetError> {
        if branch_bytes.is_empty() || branch_bytes.len() > BRANCH_COUNT_MAX {
            return Err(BudgetError::InvalidBranch);
        }
        let mut reservations = Vec::with_capacity(branch_bytes.len());
        for (slot, bytes) in branch_bytes.iter().enumerate() {
            reservations.push(self.reserve(Category::BranchBytes(slot), *bytes)?);
        }
        Ok(reservations)
    }
}
/// No Clone: owner transfer or drop is the only way to discharge the resource.
/// This is run-local accounting, not a durable receipt.
pub struct Reservation {
    ledger: BudgetLedger,
    category: Category,
    amount: usize,
}
impl Reservation {
    pub fn amount(&self) -> usize {
        self.amount
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut shared = self.ledger.0.borrow_mut();
        let current = shared
            .current
            .cell_mut(self.category)
            .expect("category admitted when reservation created");
        *current = current
            .checked_sub(self.amount)
            .expect("balanced move-only reservation");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn res_ledger_raii_and_peak_accounting() {
        let ledger = BudgetLedger::p1();
        {
            let one = ledger.reserve(Category::RawBytes, 40).unwrap();
            let two = ledger.reserve(Category::RawBytes, 2).unwrap();
            assert_eq!(one.amount(), 40);
            assert_eq!(ledger.current().raw_bytes, 42);
            assert_eq!(ledger.peaks().raw_bytes, 42);
            drop(two);
            assert_eq!(ledger.current().raw_bytes, 40);
        }
        assert_eq!(ledger.current(), Totals::default());
        assert_eq!(ledger.peaks().raw_bytes, 42);
    }
    #[test]
    fn res_checked_overflow_and_limits_cannot_be_raised() {
        let ledger = BudgetLedger::new(BudgetCaps {
            raw_bytes: usize::MAX,
            ..BudgetCaps::default()
        });
        assert_eq!(ledger.caps().raw_bytes, RAW_FILE_MAX);
        assert!(matches!(
            ledger.reserve(Category::RawBytes, RAW_FILE_MAX + 1),
            Err(BudgetError::ResourceExhausted)
        ));
        assert!(matches!(
            ledger.reserve(Category::BranchBytes(2), 1),
            Err(BudgetError::InvalidBranch)
        ));
        let r = ledger
            .reserve(Category::ScratchBytes, SCRATCH_RUN_MAX)
            .unwrap();
        assert!(matches!(
            ledger.reserve(Category::ScratchBytes, usize::MAX),
            Err(BudgetError::ArithmeticOverflow)
        ));
        drop(r);
        assert_eq!(ledger.current(), Totals::default());
    }
    #[test]
    fn res_fanout_is_atomic_and_reserves_before_dispatch() {
        let ledger = BudgetLedger::new(BudgetCaps {
            branch_bytes: 10,
            total_pending: 14,
            ..BudgetCaps::default()
        });
        let a = ledger.reserve_fanout(&[8, 6]).unwrap();
        assert_eq!(ledger.current().branch_bytes, [8, 6]);
        assert!(matches!(
            ledger.reserve_fanout(&[3, 1]),
            Err(BudgetError::ResourceExhausted)
        ));
        assert_eq!(ledger.current().branch_bytes, [8, 6]);
        drop(a);
        assert_eq!(ledger.current(), Totals::default());
    }
    #[test]
    fn res_zero_source_and_record_limits() {
        let ledger = BudgetLedger::new(BudgetCaps {
            records: 2,
            decoded_bytes: 3,
            ..BudgetCaps::default()
        });
        let one = ledger.reserve(Category::Records, 2).unwrap();
        assert!(matches!(
            ledger.reserve(Category::Records, 1),
            Err(BudgetError::ResourceExhausted)
        ));
        let b = ledger.reserve(Category::DecodedBytes, 3).unwrap();
        assert!(ledger.reserve(Category::DecodedBytes, 1).is_err());
        drop((one, b));
        assert_eq!(ledger.current(), Totals::default());
    }
}
