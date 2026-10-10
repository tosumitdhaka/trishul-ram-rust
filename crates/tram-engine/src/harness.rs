// SPDX-License-Identifier: Apache-2.0
//! Harness-only capability-confined Linux P1 runner.
//! No arbitrary destination opening, source modification, production API or ack.
#[cfg(not(target_os = "linux"))]
compile_error!("P1 effectful test harness is supported only on validated Linux; fail closed.");
use crate::{
    budget::{
        BudgetLedger, Category, BRANCH_PENDING_MAX, ENCODED_RECORD_MAX, RAW_FILE_MAX,
        RECORDS_PER_SOURCE_MAX,
    },
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
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tram_config::ValidatedPlan;
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
    CancelAfterFirstWrite,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEvidence {
    pub relative_name: String,
    pub sha256: String,
    pub byte_count: usize,
    pub record_count: usize,
    pub filtered_global: usize,
}
#[derive(Clone, Debug)]
pub struct RunOutcome {
    pub status: EphemeralStatus,
    pub run_id: String,
    pub outputs: Vec<BranchStatus>,
    pub source_units: Vec<SourceEvidence>,
    pub scratch_paths: Vec<PathBuf>,
    pub peaks: crate::budget::Totals,
    pub error: Option<String>,
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
        if !plan.ephemeral_only
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
        for sub in ["in", "scratch"] {
            let meta = root.symlink_metadata(sub).map_err(|_| HarnessError::Io)?;
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(HarnessError::PathEscape);
            }
        }
        let input = root.open_dir("in").map_err(|_| HarnessError::PathEscape)?;
        let scratch = root
            .open_dir("scratch")
            .map_err(|_| HarnessError::PathEscape)?;
        // Fixed reservations are deliberately conservative and acquired before
        // discovery/stat/read. Some P1 budget categories still require live
        // stress instrumentation for Round 2 acceptance.
        let ledger = BudgetLedger::p1();
        let _live = ledger
            .reserve(Category::LiveBytes, 40 * 1024 * 1024)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let _decoded = ledger
            .reserve(Category::DecodedBytes, 16 * 1024 * 1024)
            .map_err(|_| HarnessError::ResourceExhausted)?;
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
        match inner {
            Ok(()) => outcome.status = EphemeralStatus::Completed,
            Err(e) => {
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
    for name in paths {
        if cancelled.load(Ordering::Acquire) {
            return Err(HarnessError::UnsafePlan);
        }
        let _raw_buffers = ledger
            .reserve(Category::RawBuffers, 1)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let _raw_bytes = ledger
            .reserve(Category::RawBytes, RAW_FILE_MAX)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let source = input.symlink_metadata(name).map_err(|_| HarnessError::Io)?;
        if !source.is_file() || source.len() > RAW_FILE_MAX as u64 {
            return Err(HarnessError::ResourceExhausted);
        }
        let mut file = input.open(name).map_err(|_| HarnessError::PathEscape)?;
        let opened = file.metadata().map_err(|_| HarnessError::Io)?;
        if !opened.is_file() || opened.len() > RAW_FILE_MAX as u64 {
            return Err(HarnessError::ResourceExhausted);
        }
        let mut bytes = Vec::with_capacity(RAW_FILE_MAX.min(opened.len() as usize + 1));
        std::io::Read::by_ref(&mut file)
            .take((RAW_FILE_MAX + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| HarnessError::Io)?;
        if bytes.len() > RAW_FILE_MAX {
            return Err(HarnessError::ResourceExhausted);
        }
        let hash = Sha256::digest(&bytes);
        let digest = format!("{hash:x}");
        let rows = codec::decode(&bytes).map_err(|_| HarnessError::MalformedSource)?;
        if rows.len() > RECORDS_PER_SOURCE_MAX {
            return Err(HarnessError::ResourceExhausted);
        }
        let unit_id = SourceUnitId::new(format!("{name}:{digest}"))
            .map_err(|_| HarnessError::InvalidSource)?;
        let mut src = SourceEvidence {
            relative_name: name.clone(),
            sha256: digest,
            byte_count: bytes.len(),
            record_count: rows.len(),
            filtered_global: 0,
        };
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
            let disposition = transform::apply(&mut item, &plan.transforms)
                .map_err(|_| HarnessError::MalformedSource)?;
            if disposition == RecordDisposition::FilteredGlobal {
                src.filtered_global += 1;
                continue;
            }
            // Every branch clone is independent; no cross-sink mutable alias.
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
        out.source_units.push(src);
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(HarnessError::UnsafePlan);
    }
    // All-branch reservation precedes *any* sink output. Actual output bytes
    // are hard capped to the reserved budget below.
    let reservations = ledger
        .reserve_fanout(&vec![BRANCH_PENDING_MAX; plan.sinks.len()])
        .map_err(|_| HarnessError::ResourceExhausted)?;
    let mut encoded = Vec::new();
    for records in &branches {
        let bytes = codec::encode_array(records).map_err(|_| HarnessError::ResourceExhausted)?;
        if bytes.len() > BRANCH_PENDING_MAX {
            return Err(HarnessError::ResourceExhausted);
        }
        // Explicitly checked per-record output before any sink file creation.
        for item in records {
            if codec::encode_one(&item.data)
                .map_err(|_| HarnessError::ResourceExhausted)?
                .len()
                > ENCODED_RECORD_MAX
            {
                return Err(HarnessError::ResourceExhausted);
            }
        }
        encoded.push(bytes);
    }
    let _keep = reservations;
    let mut total_scratch = 0usize;
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
        let _artifact = ledger
            .reserve(Category::ScratchArtifacts, 1)
            .map_err(|_| HarnessError::ResourceExhausted)?;
        let next = total_scratch
            .checked_add(encoded[slot].len())
            .ok_or(HarnessError::ResourceExhausted)?;
        let _scratch = ledger
            .reserve(Category::ScratchBytes, encoded[slot].len())
            .map_err(|_| HarnessError::ResourceExhausted)?;
        total_scratch = next;
        let slot_name = Path::new(&sink.path)
            .file_name()
            .and_then(|x| x.to_str())
            .ok_or(HarnessError::UnsafePlan)?;
        run_dir
            .create_dir(slot_name)
            .map_err(|_| HarnessError::Io)?;
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
        if file.write_all(&encoded[slot]).is_err() {
            out.outputs[slot] = BranchStatus::Unknown;
            return Err(HarnessError::Io);
        }
        out.outputs[slot] = BranchStatus::ScratchWritten;
        if fault == InjectedFault::CancelAfterFirstWrite && slot == 0 {
            cancelled.store(true, Ordering::Release);
            return Err(HarnessError::UnsafePlan);
        }
    }
    Ok(())
}
