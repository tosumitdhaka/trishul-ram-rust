// SPDX-License-Identifier: Apache-2.0
//! Harness-only capability-confined Linux P1 runner.
//! No arbitrary destination opening, source modification, production API or ack.
#[cfg(not(target_os = "linux"))]
compile_error!("P1 effectful test harness is supported only on validated Linux; fail closed.");
use crate::{
    budget::{BudgetLedger, Category, BRANCH_PENDING_MAX, RAW_FILE_MAX, RECORDS_PER_SOURCE_MAX},
    codec,
    transform::{self, RecordDisposition},
};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    sync::atomic::AtomicBool as RunAtomicBool,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tram_config::{Expression, Transform, ValidatedPlan};
use tram_model::{
    BranchId, Provenance, RecordEnvelope, RecordId, RunId, SourcePosition, SourceUnitId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HarnessError {
    UnsafePlan,
    UnsupportedPlatform,
    Io,
    MalformedSource,
    ResourceExhausted,
    PathEscape,
    InvalidSource,
    EmptyPlan,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EphemeralStatus {
    Completed,
    Failed,
    Cancelled,
    Unknown,
}
impl EphemeralStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "ephemeral_completed",
            Self::Failed => "ephemeral_failed",
            Self::Cancelled => "ephemeral_cancelled",
            Self::Unknown => "ephemeral_unknown",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchStatus {
    NotStarted,
    ScratchWritten,
    Failed,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InjectedFault {
    None,
    FailBeforeSink(usize),
    FailDuringSinkWrite(usize),
    PauseBeforeSourceOpen,
    PauseBeforeScratchOpen,
    PauseBeforeSinkOpen(usize),
    CancelAfterFirstWrite,
    /// Test-only pause after A write, allowing parent to issue SIGKILL.
    PauseAfterFirstWrite,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceDisposition {
    Pending,
    HasRecords,
    Empty,
    FilteredGlobal,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEvidence {
    pub relative_name: String,
    pub sha256: String,
    pub byte_count: usize,
    pub record_count: usize,
    pub filtered_global: usize,
    pub disposition: SourceDisposition,
    pub run_failed: bool,
}
#[derive(Clone, Debug)]
pub struct RunOutcome {
    pub status: EphemeralStatus,
    pub run_id: String,
    pub outputs: Vec<BranchStatus>,
    pub source_units: Vec<SourceEvidence>,
    pub scratch_paths: Vec<PathBuf>,
    pub peaks: crate::budget::Totals,
    pub live_after_teardown: crate::budget::Totals,
    pub error: Option<String>,
}
/// One admitted P1 run per process. The guard is acquired before filesystem
/// inspection and released on every return, cancellation, error or unwind.
static ACTIVE_P1_RUN: RunAtomicBool = RunAtomicBool::new(false);
static SCRATCH_ADMISSION_GATE: RunAtomicBool = RunAtomicBool::new(false);
/// Diagnostic signal for the bounded within-root scratch-swap regression.
#[must_use]
pub fn scratch_admission_gate_reached() -> bool {
    SCRATCH_ADMISSION_GATE.load(Ordering::Acquire)
}
struct ActiveP1Run;
impl ActiveP1Run {
    fn acquire() -> Result<Self, HarnessError> {
        ACTIVE_P1_RUN.compare_exchange(
            false, true, Ordering::AcqRel, Ordering::Acquire
        ).map_err(|_| HarnessError::ResourceExhausted)?;
        Ok(Self)
    }
}
impl Drop for ActiveP1Run {
    fn drop(&mut self) {
        ACTIVE_P1_RUN.store(false, Ordering::Release);
    }
}

/// Open a *single* named directory through a no-follow dirfd-relative syscall.
/// Comparing the preflight inode to the opened handle defeats scratch->in
/// symlink substitution and renamed-directory role confusion.
fn open_role_dir(
    root: &Dir,
    name: &str,
    expected: &cap_std::fs::Metadata,
) -> Result<Dir, HarnessError> {
    use cap_std::fs::MetadataExt;
    let fd = rustix::fs::openat(
        root,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    ).map_err(|_| HarnessError::PathEscape)?;
    let opened = rustix::fs::fstat(&fd).map_err(|_| HarnessError::PathEscape)?;
    if opened.st_dev != expected.dev() || opened.st_ino != expected.ino() {
        return Err(HarnessError::PathEscape);
    }
    // The path might have changed *after* open; the handle remains pinned.
    // This second lookup is a refusal check, not a substitute for fstat.
    let after = root.symlink_metadata(name).map_err(|_| HarnessError::PathEscape)?;
    if after.file_type().is_symlink() || after.dev() != opened.st_dev || after.ino() != opened.st_ino {
        return Err(HarnessError::PathEscape);
    }
    Ok(Dir::from(fd))
}

pub struct TestHarness;
impl TestHarness {
    /// The sole effect entry: test_root must be a disposable trusted directory
    /// with existing in/ and scratch/. Config strings are never OS capabilities.
    pub fn start(
        plan: &ValidatedPlan,
        test_root: &Path,
        cancelled: &AtomicBool,
    ) -> Result<RunOutcome, HarnessError> {
        Self::start_with_fault(plan, test_root, cancelled, InjectedFault::None)
    }
    pub fn start_with_fault(
        plan: &ValidatedPlan,
        test_root: &Path,
        cancelled: &AtomicBool,
        fault: InjectedFault,
    ) -> Result<RunOutcome, HarnessError> {
        Self::start_with_caps_and_fault(
            plan,
            test_root,
            cancelled,
            crate::budget::BudgetCaps::default(),
            fault,
        )
    }
    /// Lower-only, isolated deterministic limits for resource-pressure tests.
    /// Never raises frozen P1 caps.
    pub fn start_with_caps(
        plan: &ValidatedPlan,
        test_root: &Path,
        cancelled: &AtomicBool,
        caps: crate::budget::BudgetCaps,
    ) -> Result<RunOutcome, HarnessError> {
        Self::start_with_caps_and_fault(plan, test_root, cancelled, caps, InjectedFault::None)
    }
    pub fn start_with_caps_and_fault(
        plan: &ValidatedPlan,
        test_root: &Path,
        cancelled: &AtomicBool,
        caps: crate::budget::BudgetCaps,
        fault: InjectedFault,
    ) -> Result<RunOutcome, HarnessError> {
        let _active_run = ActiveP1Run::acquire()?;
        if !plan.is_compiler_minted()
            || !plan.ephemeral_only
            || plan.contract_version != 1
            || plan.sinks.is_empty()
            || plan.sinks.len() > 2
            || !test_root.is_absolute()
        {
            return Err(HarnessError::UnsafePlan);
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(HarnessError::UnsafePlan);
        }
        // Pre-effect structural admission, before even test_root metadata.
        if Path::new(&plan.source.path) != test_root.join("in")
            || !matches!(plan.source.file_pattern.as_str(), "*" | "*.json")
        {
            return Err(HarnessError::UnsafePlan);
        }
        let mut slots = BTreeSet::new();
        for s in &plan.sinks {
            let path = Path::new(&s.path);
            if path.parent() != Some(test_root.join("scratch").as_path()) {
                return Err(HarnessError::UnsafePlan);
            }
            let slot = path
                .file_name()
                .and_then(|x| x.to_str())
                .ok_or(HarnessError::UnsafePlan)?;
            if slot.is_empty()
                || slot.len() > 80
                || !slot
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !slots.insert(slot.to_owned())
            {
                return Err(HarnessError::UnsafePlan);
            }
            let f = &s.filename_template;
            if f.is_empty()
                || f == "."
                || f == ".."
                || f.contains('/')
                || f.contains('\\')
                || f.contains("..")
                || f.contains('{')
                || f.contains('}')
                || f.bytes().any(|b| b < 32 || b == 127)
            {
                return Err(HarnessError::UnsafePlan);
            }
        }
        if std::fs::symlink_metadata(test_root)
            .map_err(|_| HarnessError::Io)?
            .file_type()
            .is_symlink()
        {
            return Err(HarnessError::PathEscape);
        }
        // Parent-safe capability path resolution. cap-std cannot escape this
        // opened root via '..' or attacker-swapped directory symlinks.
        let root =
            Dir::open_ambient_dir(test_root, ambient_authority()).map_err(|_| HarnessError::Io)?;
        use cap_std::fs::MetadataExt;
        let input_preflight = root.symlink_metadata("in").map_err(|_| HarnessError::PathEscape)?;
        let scratch_preflight = root.symlink_metadata("scratch").map_err(|_| HarnessError::PathEscape)?;
        if input_preflight.file_type().is_symlink()
            || scratch_preflight.file_type().is_symlink()
            || !input_preflight.is_dir()
            || !scratch_preflight.is_dir()
            || input_preflight.dev() != scratch_preflight.dev()
            || (input_preflight.dev(), input_preflight.ino())
                == (scratch_preflight.dev(), scratch_preflight.ino())
        {
            return Err(HarnessError::PathEscape);
        }
        let input = open_role_dir(&root, "in", &input_preflight)?;
        // Gate solely for deterministic sandbox-race regressions; no effects
        // have occurred and neither a source read nor scratch create is allowed.
        if fault == InjectedFault::PauseBeforeScratchOpen {
            SCRATCH_ADMISSION_GATE.store(true, Ordering::Release);
            let paused = pause_for_adversarial_test(cancelled);
            SCRATCH_ADMISSION_GATE.store(false, Ordering::Release);
            paused?;
        }
        let scratch = open_role_dir(&root, "scratch", &scratch_preflight)?;
        // Linux advisory flock on the opened scratch *directory descriptor*:
        // shared-root contenders in other processes cannot have overlapping
        // live ledgers. The kernel releases it on SIGKILL, preserving P1 crash
        // semantics and avoiding a stale pathname lock file.
        rustix::fs::flock(&scratch, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        // All reservations below follow backing buffer/record lifetime.
        let ledger = BudgetLedger::new(caps);
        let mut paths = Vec::new();
        for entry in input.read_dir(".").map_err(|_| HarnessError::Io)? {
            let entry = entry.map_err(|_| HarnessError::Io)?;
            let file_name = entry.file_name();
            let name = file_name.to_str().ok_or(HarnessError::PathEscape)?;
            if name.len() > 512 || name == "." || name == ".." {
                return Err(HarnessError::PathEscape);
            }
            let meta = input.symlink_metadata(name).map_err(|_| HarnessError::Io)?;
            if meta.file_type().is_symlink() {
                return Err(HarnessError::PathEscape);
            }
            if !meta.is_file() {
                return Err(HarnessError::InvalidSource);
            }
            if meta.len() > RAW_FILE_MAX as u64 {
                return Err(HarnessError::ResourceExhausted);
            }
            if plan.source.file_pattern == "*" || name.ends_with(".json") {
                paths.push(name.to_owned());
            }
            if paths.len() > 100 {
                return Err(HarnessError::ResourceExhausted);
            }
        }
        paths.sort();
        // No scratch directory creation until plan authority + all input names
        // have been checked. Run ID is never accepted from YAML.
        let run_id = new_run_id(&scratch)?;
        let run_dir = scratch.open_dir(&run_id).map_err(|_| HarnessError::Io)?;
        let mut outcome = RunOutcome {
            status: EphemeralStatus::Failed,
            run_id: run_id.clone(),
            outputs: vec![BranchStatus::NotStarted; plan.sinks.len()],
            source_units: Vec::new(),
            scratch_paths: Vec::new(),
            peaks: ledger.peaks(),
            live_after_teardown: crate::budget::Totals::default(),
            error: None,
        };
        let inner = execute(
            ExecutionRequest {
                plan,
                input: &input,
                run_dir: &run_dir,
                paths: &paths,
                ledger: &ledger,
                cancelled,
                fault,
                root: test_root,
            },
            &mut outcome,
        );
        outcome.peaks = ledger.peaks();
        outcome.live_after_teardown = ledger.current();
        match inner {
            Ok(()) => outcome.status = EphemeralStatus::Completed,
            Err(e) => {
                for unit in &mut outcome.source_units {
                    if unit.disposition == SourceDisposition::Pending {
                        unit.disposition = SourceDisposition::Failed;
                    }
                    unit.run_failed = true;
                }
                outcome.status =
                    if cancelled.load(Ordering::Acquire) || matches!(e, HarnessError::UnsafePlan) {
                        EphemeralStatus::Cancelled
                    } else {
                        EphemeralStatus::Failed
                    };
                outcome.error = Some(format!("{e:?}"));
            }
        }
        Ok(outcome)
    }
}

fn source_link_is_unique(source: &cap_std::fs::Metadata) -> bool {
    use cap_std::fs::MetadataExt;
    source.nlink() == 1
}
fn pause_for_adversarial_test(cancelled: &AtomicBool) -> Result<(), HarnessError> {
    for _ in 0..25 {
        if cancelled.load(Ordering::Acquire) {
            return Err(HarnessError::UnsafePlan);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}
fn same_source_identity(a: &cap_std::fs::Metadata, b: &cap_std::fs::Metadata) -> bool {
    use cap_std::fs::MetadataExt;
    a.is_file()
        && b.is_file()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.modified().ok() == b.modified().ok()
}
/// Borrowed lexical preflight. Charge an upper bound on retained Datum/string
/// allocations before calling the in-memory parser. A conservative refusal is
/// preferable to a live-memory overrun.
/// Borrowed lexical preflight of *top-level* JSON object records. Nested
/// object values are not source records. The full decoder still rejects
/// malformed documents; this preflight only obtains pre-materialization quota.
fn preflight_root_record_count(input: &[u8]) -> Result<usize, HarnessError> {
    let root = input
        .iter()
        .copied()
        .find(|b| !matches!(*b, b' ' | b'\r' | b'\n' | b'\t'));
    let mut count = if root == Some(b'{') { 1usize } else { 0usize };
    let array_root = root == Some(b'[');
    let mut level = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for &b in input {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
            continue;
        }
        match b {
            b'"' => quoted = true,
            b'[' | b'{' => {
                if b == b'{' && array_root && level == 1 {
                    count = count
                        .checked_add(1)
                        .ok_or(HarnessError::ResourceExhausted)?;
                }
                level = level
                    .checked_add(1)
                    .ok_or(HarnessError::ResourceExhausted)?;
            }
            b']' | b'}' => level = level.saturating_sub(1),
            _ => {}
        }
    }
    if count > RECORDS_PER_SOURCE_MAX {
        return Err(HarnessError::ResourceExhausted);
    }
    Ok(count)
}
fn estimate_json_allocation_ceiling(input: &[u8]) -> Result<usize, HarnessError> {
    let mut tokens = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for &c in input {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                quoted = false;
            }
        } else if c == b'"' {
            quoted = true;
            tokens = tokens
                .checked_add(1)
                .ok_or(HarnessError::ResourceExhausted)?;
        } else if matches!(c, b'{' | b'}' | b'[' | b']' | b',' | b':') {
            tokens = tokens
                .checked_add(1)
                .ok_or(HarnessError::ResourceExhausted)?;
        }
    }
    let bytes = input
        .len()
        .checked_mul(2)
        .and_then(|n| tokens.checked_mul(144).and_then(|t| n.checked_add(t)))
        .and_then(|n| n.checked_add(1024))
        .ok_or(HarnessError::ResourceExhausted)?;
    if bytes > crate::budget::DECODED_SOURCE_MAX {
        return Err(HarnessError::ResourceExhausted);
    }
    Ok(bytes)
}
fn estimate_datum_owned(v: &tram_model::Datum) -> Result<usize, HarnessError> {
    use tram_model::Datum;
    let own = std::mem::size_of::<Datum>();
    let inner = match v {
        Datum::String(x) => x.len(),
        Datum::Bytes(x) => x.len(),
        Datum::BigInteger(x) => x.as_str().len(),
        Datum::Decimal(x) => x.as_str().len(),
        Datum::Array(xs) => xs
            .iter()
            .try_fold(0usize, |n, v| {
                estimate_datum_owned(v).ok().and_then(|v| n.checked_add(v))
            })
            .ok_or(HarnessError::ResourceExhausted)?,
        Datum::Object(xs) => xs
            .iter()
            .try_fold(0usize, |n, (k, v)| {
                estimate_datum_owned(v)
                    .ok()
                    .and_then(|v| v.checked_add(k.len() + 128))
                    .and_then(|v| n.checked_add(v))
            })
            .ok_or(HarnessError::ResourceExhausted)?,
        _ => 0,
    };
    own.checked_add(inner)
        .and_then(|n| n.checked_add(64))
        .ok_or(HarnessError::ResourceExhausted)
}
fn estimate_envelope_owned(record: &RecordEnvelope) -> Result<usize, HarnessError> {
    record
        .data
        .iter()
        .try_fold(4096usize, |n, (k, v)| {
            estimate_datum_owned(v)
                .ok()
                .and_then(|value| value.checked_add(k.len() + 128))
                .and_then(|value| n.checked_add(value))
        })
        .ok_or(HarnessError::ResourceExhausted)
}
/// Conservative pre-effect headroom for stateless transforms. Field values
/// may deep-clone an already admitted Datum for every added field.
fn transform_growth_upper(
    record: &RecordEnvelope,
    steps: &[Transform],
) -> Result<usize, HarnessError> {
    let largest = record
        .data
        .values()
        .try_fold(0usize, |max, v| {
            estimate_datum_owned(v).ok().map(|n| max.max(n))
        })
        .ok_or(HarnessError::ResourceExhausted)?;
    let mut cost = 0usize;
    for step in steps {
        match step {
            Transform::AddField(fields) => {
                for (key, expr) in fields {
                    let max_value = largest.max(largest_literal_owned(expr)?).max(256);
                    let bytes = max_value
                        .checked_add(key.len())
                        .and_then(|n| n.checked_add(192))
                        .ok_or(HarnessError::ResourceExhausted)?;
                    cost = cost
                        .checked_add(bytes)
                        .ok_or(HarnessError::ResourceExhausted)?;
                }
            }
            Transform::Rename(pairs) => {
                for (_from, to) in pairs {
                    cost = cost
                        .checked_add(to.len())
                        .and_then(|n| n.checked_add(192))
                        .ok_or(HarnessError::ResourceExhausted)?;
                }
            }
            Transform::Filter(_) | Transform::Drop(_) => {}
        }
    }
    Ok(cost)
}
fn largest_literal_owned(expr: &Expression) -> Result<usize, HarnessError> {
    match expr {
        Expression::Literal(v) => estimate_datum_owned(v),
        Expression::Field(_) => Ok(0),
        Expression::Unary { expr, .. } => largest_literal_owned(expr),
        Expression::Binary { left, right, .. } => {
            Ok(largest_literal_owned(left)?.max(largest_literal_owned(right)?))
        }
    }
}
/// No P1 background queues exist. Refusal blocks further source admission
/// for at most a small bounded deadline; cancellation is polled while waiting.
/// This is not a durable or async sink retry and never spawns new tasks.
fn reserve_fanout_cancellable(
    ledger: &BudgetLedger,
    bytes: &[usize],
    cancelled: &AtomicBool,
) -> Result<Vec<crate::budget::Reservation>, HarnessError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(HarnessError::UnsafePlan);
        }
        match ledger.reserve_fanout(bytes) {
            Ok(guards) => return Ok(guards),
            Err(crate::budget::BudgetError::ResourceExhausted) => {
                if std::time::Instant::now() >= deadline {
                    return Err(HarnessError::ResourceExhausted);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => return Err(HarnessError::ResourceExhausted),
        }
    }
}

fn new_run_id(scratch: &Dir) -> Result<String, HarnessError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HarnessError::Io)?
        .as_nanos();
    for attempt in 0..32 {
        let id = format!("run-{}-{nanos}-{attempt}", std::process::id());
        match scratch.create_dir(&id) {
            Ok(()) => return Ok(id),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(HarnessError::Io),
        }
    }
    Err(HarnessError::ResourceExhausted)
}
struct ExecutionRequest<'a> {
    plan: &'a ValidatedPlan,
    input: &'a Dir,
    run_dir: &'a Dir,
    paths: &'a [String],
    ledger: &'a BudgetLedger,
    cancelled: &'a AtomicBool,
    fault: InjectedFault,
    root: &'a Path,
}
fn execute(request: ExecutionRequest<'_>, out: &mut RunOutcome) -> Result<(), HarnessError> {
    let ExecutionRequest {
        plan,
        input,
        run_dir,
        paths,
        ledger,
        cancelled,
        fault,
        root,
    } = request;
    let mut branches = vec![Vec::<RecordEnvelope>::new(); plan.sinks.len()];
    // Guards cover the lifetime of all retained branch copies and encoded
    // payloads, rather than being created after the allocation.
    let mut branch_guards = Vec::new();
    let mut scratch_guards = Vec::new();
    for name in paths {
        if cancelled.load(Ordering::Acquire) {
            return Err(HarnessError::UnsafePlan);
        }
        let source = input.symlink_metadata(name).map_err(|_| HarnessError::Io)?;
        if !source.is_file() || source.len() > RAW_FILE_MAX as u64 {
            return Err(HarnessError::ResourceExhausted);
        }
        let _raw_buffers = ledger
            .reserve(Category::RawBuffers, 1)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        if fault == InjectedFault::PauseBeforeSourceOpen {
            pause_for_adversarial_test(cancelled)?;
        }
        let mut file = input.open(name).map_err(|_| HarnessError::PathEscape)?;
        let opened = file.metadata().map_err(|_| HarnessError::Io)?;
        if !opened.is_file() || opened.len() > RAW_FILE_MAX as u64 {
            return Err(HarnessError::ResourceExhausted);
        }
        if !source_link_is_unique(&opened) {
            return Err(HarnessError::PathEscape);
        }
        // Reject stale or swapped source generation even when the swapped
        // target is another regular file inside the capability root.
        if !same_source_identity(&source, &opened) {
            return Err(HarnessError::PathEscape);
        }
        // Reserve *exact* opened size. A separate one-byte stack read
        // detects growth without allocating an uncharged 4MiB+1 vector.
        let actual_capacity = opened.len() as usize;
        let _raw_charge = ledger
            .reserve_owned(Category::RawBytes, actual_capacity)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let mut bytes = Vec::with_capacity(actual_capacity);
        std::io::Read::by_ref(&mut file)
            .take(actual_capacity as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| HarnessError::Io)?;
        let mut overrun = [0u8; 1];
        let grew = file.read(&mut overrun).map_err(|_| HarnessError::Io)? != 0;
        if grew
            || bytes.len() != actual_capacity
            || !same_source_identity(&opened, &file.metadata().map_err(|_| HarnessError::Io)?)
            || !same_source_identity(
                &opened,
                &input
                    .symlink_metadata(name)
                    .map_err(|_| HarnessError::PathEscape)?,
            )
        {
            return Err(HarnessError::PathEscape);
        }
        let hash = Sha256::digest(&bytes);
        let digest = format!("{hash:x}");
        let unit_index = out.source_units.len();
        out.source_units.push(SourceEvidence {
            relative_name: name.clone(),
            sha256: digest.clone(),
            byte_count: bytes.len(),
            record_count: 0,
            filtered_global: 0,
            disposition: SourceDisposition::Pending,
            run_failed: false,
        });
        // The preflight walks the borrowed wire bytes. Its reservation is a
        // conservative ownership ceiling, acquired before Datum materializes.
        let budgeted_upper = estimate_json_allocation_ceiling(&bytes)?;
        let _decoded_charge = ledger
            .reserve_owned(Category::DecodedBytes, budgeted_upper)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let reserved_records = preflight_root_record_count(&bytes)?;
        let _record_count = ledger
            .reserve(Category::Records, reserved_records)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let rows = codec::decode(&bytes).map_err(|error| match error {
            codec::JsonError::ResourceExhausted => HarnessError::ResourceExhausted,
            _ => HarnessError::MalformedSource,
        })?;
        if rows.len() != reserved_records || rows.len() > RECORDS_PER_SOURCE_MAX {
            return Err(HarnessError::ResourceExhausted);
        }
        let unit_id = SourceUnitId::new(format!("{name}:{digest}"))
            .map_err(|_| HarnessError::InvalidSource)?;
        out.source_units[unit_index].record_count = rows.len();
        for (ordinal, data) in rows.into_iter().enumerate() {
            if cancelled.load(Ordering::Acquire) {
                return Err(HarnessError::UnsafePlan);
            }
            let run = RunId::new(out.run_id.clone()).map_err(|_| HarnessError::InvalidSource)?;
            let record = RecordId::new(format!("{name}:{ordinal}"))
                .map_err(|_| HarnessError::InvalidSource)?;
            let mut item = RecordEnvelope::new(Provenance {
                run_id: run,
                source_unit_id: unit_id.clone(),
                record_id: record,
                source_plugin: "local".into(),
                ingested_at: None,
            });
            item.source_position = Some(SourcePosition {
                opaque_cursor: name.as_bytes().to_vec(),
                ordinal: Some(ordinal as u64),
            });
            item.data = data;
            let transform_headroom = transform_growth_upper(&item, &plan.transforms)?;
            let _transform_guard = ledger
                .reserve_owned(Category::DecodedBytes, transform_headroom)
                .map_err(|_| HarnessError::ResourceExhausted)?;
            let disposition = transform::apply(&mut item, &plan.transforms)
                .map_err(|_| HarnessError::MalformedSource)?;
            if disposition == RecordDisposition::FilteredGlobal {
                out.source_units[unit_index].filtered_global += 1;
                continue;
            }
            // Every branch clone is independent; no cross-sink mutable alias.
            // Reserve all planned branch copies *before* cloning any of them.
            // Transform copies are covered by an explicit conservative bound.
            let mut requested = Vec::new();
            for sink in &plan.sinks {
                let base = estimate_envelope_owned(&item)?;
                let headroom = transform_growth_upper(&item, &sink.transforms)?;
                requested.push(
                    base.checked_add(headroom)
                        .and_then(|n| n.checked_add(1024))
                        .ok_or(HarnessError::ResourceExhausted)?,
                );
            }
            let guards = reserve_fanout_cancellable(ledger, &requested, cancelled)?;
            let mut ownership_guards = Vec::new();
            for bytes in &requested {
                ownership_guards.push(
                    ledger
                        .reserve(Category::LiveBytes, *bytes)
                        .map_err(|_| HarnessError::ResourceExhausted)?,
                );
            }
            // The branch guard set remains resident until output teardown.
            branch_guards.push((guards, ownership_guards));
            for (slot, sink) in plan.sinks.iter().enumerate() {
                let mut branch = item.fork_for_branch(
                    BranchId::new(format!("sink-{slot}"))
                        .map_err(|_| HarnessError::InvalidSource)?,
                );
                if transform::branch_condition(&branch, sink.condition.as_ref(), slot)
                    .map_err(|_| HarnessError::MalformedSource)?
                    != RecordDisposition::Retained
                {
                    continue;
                }
                transform::apply(&mut branch, &sink.transforms)
                    .map_err(|_| HarnessError::MalformedSource)?;
                branches[slot].push(branch);
            }
        }
        let unit = &mut out.source_units[unit_index];
        unit.disposition = if unit.record_count == 0 {
            SourceDisposition::Empty
        } else if unit.filtered_global == unit.record_count {
            SourceDisposition::FilteredGlobal
        } else {
            SourceDisposition::HasRecords
        };
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(HarnessError::UnsafePlan);
    }
    // Incrementally admitted encoded buffers cannot bypass the run-wide cap.
    let mut encoded = Vec::new();
    let mut encoded_guards = Vec::new();
    let mut pending_guards = Vec::new();
    for (slot, records) in branches.iter().enumerate() {
        // Exact output size and largest temporary per-record frame are
        // computed without allocating encoded buffers.
        let (length, max_frame) =
            codec::encoded_array_size(records).map_err(|_| HarnessError::ResourceExhausted)?;
        if length > BRANCH_PENDING_MAX {
            return Err(HarnessError::ResourceExhausted);
        }
        // Encoded output lives alongside still-retained branch records:
        // charge both pending branch occupancy and run-wide live capacity.
        let pending = ledger
            .reserve(Category::BranchBytes(slot), length)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let live = ledger
            .reserve(
                Category::LiveBytes,
                length
                    .checked_add(max_frame)
                    .ok_or(HarnessError::ResourceExhausted)?,
            )
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let bytes = codec::encode_array(records).map_err(|_| HarnessError::ResourceExhausted)?;
        if bytes.len() != length {
            return Err(HarnessError::ResourceExhausted);
        }
        encoded_guards.push(live);
        pending_guards.push(pending);
        encoded.push(bytes);
    }
    let _keep = (branch_guards, encoded_guards, pending_guards);
    for (slot, sink) in plan.sinks.iter().enumerate() {
        if cancelled.load(Ordering::Acquire) {
            return Err(HarnessError::UnsafePlan);
        }
        if fault == InjectedFault::FailBeforeSink(slot) {
            out.outputs[slot] = BranchStatus::Failed;
            return Err(HarnessError::Io);
        }
        let _io = ledger
            .reserve(Category::SinkIo, 1)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        // Written bytes and artifact count remain cumulatively charged for
        // the entire run, including after an ambiguous/partial write.
        scratch_guards.push(
            ledger
                .reserve_scratch(encoded[slot].len())
                .map_err(|_| HarnessError::ResourceExhausted)?,
        );
        let slot_name = Path::new(&sink.path)
            .file_name()
            .and_then(|x| x.to_str())
            .ok_or(HarnessError::UnsafePlan)?;
        run_dir
            .create_dir(slot_name)
            .map_err(|_| HarnessError::Io)?;
        if fault == InjectedFault::PauseBeforeSinkOpen(slot) {
            pause_for_adversarial_test(cancelled)?;
        }
        let slot_dir = run_dir.open_dir(slot_name).map_err(|_| HarnessError::Io)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = slot_dir
            .open_with(&sink.filename_template, &options)
            .map_err(|_| HarnessError::Io)?;
        out.scratch_paths.push(
            root.join("scratch")
                .join(&out.run_id)
                .join(slot_name)
                .join(&sink.filename_template),
        );
        if fault == InjectedFault::FailDuringSinkWrite(slot) {
            let partial = encoded[slot].len().min(11);
            let _ = file.write_all(&encoded[slot][..partial]);
            out.outputs[slot] = BranchStatus::Unknown;
            return Err(HarnessError::Io);
        }
        if file.write_all(&encoded[slot]).is_err() {
            out.outputs[slot] = BranchStatus::Unknown;
            return Err(HarnessError::Io);
        }
        out.outputs[slot] = BranchStatus::ScratchWritten;
        if fault == InjectedFault::CancelAfterFirstWrite && slot == 0 {
            cancelled.store(true, Ordering::Release);
            return Err(HarnessError::UnsafePlan);
        }
        if fault == InjectedFault::PauseAfterFirstWrite && slot == 0 {
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_secs(20) {
                if cancelled.load(Ordering::Acquire) {
                    return Err(HarnessError::UnsafePlan);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            // The watchdog bounds a missed-kill regression; no success claim.
            return Err(HarnessError::Io);
        }
    }
    Ok(())
}

#[cfg(test)]
mod bounded_backpressure_tests {
    use super::*;
    use crate::budget::{BudgetCaps, Totals};
    #[test]
    fn r2_g1_full_branch_stops_admission_before_fanout_and_cancel_unblocks() {
        let ledger = BudgetLedger::new(BudgetCaps {
            branch_bytes: 0,
            total_pending: 0,
            ..BudgetCaps::default()
        });
        let cancel = AtomicBool::new(false);
        let started = std::time::Instant::now();
        assert!(matches!(
            reserve_fanout_cancellable(&ledger, &[1, 1], &cancel),
            Err(HarnessError::ResourceExhausted)
        ));
        assert!(started.elapsed() >= std::time::Duration::from_millis(100));
        assert_eq!(ledger.current(), Totals::default());
        cancel.store(true, Ordering::Release);
        let cancelled = std::time::Instant::now();
        assert!(matches!(
            reserve_fanout_cancellable(&ledger, &[1, 1], &cancel),
            Err(HarnessError::UnsafePlan)
        ));
        assert!(cancelled.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(ledger.current(), Totals::default());
    }
}
