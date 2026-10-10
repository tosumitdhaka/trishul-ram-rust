// SPDX-License-Identifier: Apache-2.0
//! Explicit test-created disposable roots. These tests use std::fs for fixture
//! setup and independent output inspection ONLY; the engine has cap-std handles.
#![cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tram_config::compile_p1_builtin;
use tram_engine::{
    budget::RAW_FILE_MAX,
    codec,
    harness::{
        BranchStatus, EphemeralStatus, HarnessError, InjectedFault, SourceDisposition, TestHarness,
    },
};
const YAML: &str = include_str!("../../../docs/phase-0/fixtures/p1/pipeline.yaml");
const INPUT: &[u8] = include_bytes!("../../../docs/phase-0/fixtures/p1/input.json");
const EXPECTED_A: &[u8] = include_bytes!("../../../docs/phase-0/fixtures/p1/expected-a.json");
const EXPECTED_B: &[u8] = include_bytes!("../../../docs/phase-0/fixtures/p1/expected-b.json");

struct DisposableRoot {
    root: PathBuf,
}
impl DisposableRoot {
    fn new() -> Self {
        let base = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = base.join(format!("tram-p1-test-{}-{nanos}", std::process::id()));
        fs::create_dir(&root).expect("create isolated test-root exclusively");
        fs::create_dir(root.join("in")).unwrap();
        fs::create_dir(root.join("scratch")).unwrap();
        Self { root }
    }
    fn file(&self, name: &str, bytes: &[u8]) {
        fs::write(self.root.join("in").join(name), bytes).unwrap();
    }
    fn plan(&self) -> tram_config::ValidatedPlan {
        plan_for(&self.root)
    }
}
fn plan_for(root: &Path) -> tram_config::ValidatedPlan {
    let vars = BTreeMap::from([
        (
            "TRAM_P1_INPUT_DIR".into(),
            root.join("in").to_string_lossy().into_owned(),
        ),
        (
            "TRAM_P1_SCRATCH_A".into(),
            root.join("scratch/a").to_string_lossy().into_owned(),
        ),
        (
            "TRAM_P1_SCRATCH_B".into(),
            root.join("scratch/b").to_string_lossy().into_owned(),
        ),
    ]);
    compile_p1_builtin(YAML, &vars).expect("golden P1 plan")
}
impl Drop for DisposableRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn inspect(
    out: &tram_engine::harness::RunOutcome,
    idx: usize,
) -> Vec<BTreeMap<String, tram_model::Datum>> {
    codec::decode(&fs::read(&out.scratch_paths[idx]).expect("exclusive scratch output")).unwrap()
}
fn source_snapshot(path: &Path) -> (Vec<u8>, std::fs::Metadata) {
    (fs::read(path).unwrap(), fs::metadata(path).unwrap())
}
#[test]
fn comp_01_golden_real_io_semantic_outputs_and_readonly_source() {
    let t = DisposableRoot::new();
    t.file("input.json", INPUT);
    let src = t.root.join("in/input.json");
    let (before, meta) = source_snapshot(&src);
    let cancel = AtomicBool::new(false);
    let out = TestHarness::start(&t.plan(), &t.root, &cancel).expect("admitted");
    assert_eq!(out.status, EphemeralStatus::Completed, "{:?}", out.error);
    assert_eq!(
        out.outputs,
        vec![BranchStatus::ScratchWritten, BranchStatus::ScratchWritten]
    );
    assert_eq!(out.source_units.len(), 1);
    assert_eq!(out.source_units[0].record_count, 2);
    assert_eq!(out.source_units[0].filtered_global, 1);
    assert_eq!(
        out.source_units[0].disposition,
        SourceDisposition::HasRecords
    );
    assert!(!out.source_units[0].run_failed);
    assert_eq!(inspect(&out, 0), codec::decode(EXPECTED_A).unwrap());
    assert_eq!(inspect(&out, 1), codec::decode(EXPECTED_B).unwrap());
    let (after, metadata) = source_snapshot(&src);
    assert_eq!(before, after);
    assert_eq!(meta.modified().unwrap(), metadata.modified().unwrap());
    assert_eq!(meta.len(), metadata.len());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(meta.mode(), metadata.mode());
        assert_eq!(meta.ino(), metadata.ino());
    }
    assert!(out.peaks.raw_buffers <= 1);
    assert!(out.peaks.branch_bytes.iter().all(|n| *n <= 8 * 1024 * 1024));
}
#[test]
fn p1_fanout_04_fail_b_after_a_produces_partial_scratch_without_source_mutation() {
    let t = DisposableRoot::new();
    t.file("input.json", INPUT);
    let src = t.root.join("in/input.json");
    let (before, meta) = source_snapshot(&src);
    let cancel = AtomicBool::new(false);
    let out = TestHarness::start_with_fault(
        &t.plan(),
        &t.root,
        &cancel,
        InjectedFault::FailBeforeSink(1),
    )
    .expect("run created");
    assert_eq!(out.status, EphemeralStatus::Failed);
    assert_eq!(
        out.outputs,
        vec![BranchStatus::ScratchWritten, BranchStatus::Failed]
    );
    assert_eq!(inspect(&out, 0), codec::decode(EXPECTED_A).unwrap());
    assert_eq!(out.scratch_paths.len(), 1);
    let (after, metadata) = source_snapshot(&src);
    assert_eq!(before, after);
    assert_eq!(meta.modified().unwrap(), metadata.modified().unwrap());
}
#[test]
fn p1_ack_02_cancel_after_partial_scratch_keeps_source_immutable() {
    let t = DisposableRoot::new();
    t.file("input.json", INPUT);
    let path = t.root.join("in/input.json");
    let (before, meta) = source_snapshot(&path);
    let cancel = AtomicBool::new(false);
    let out = TestHarness::start_with_fault(
        &t.plan(),
        &t.root,
        &cancel,
        InjectedFault::CancelAfterFirstWrite,
    )
    .unwrap();
    assert_eq!(out.status, EphemeralStatus::Cancelled);
    assert!(cancel.load(Ordering::Acquire));
    assert_eq!(
        out.outputs,
        vec![BranchStatus::ScratchWritten, BranchStatus::NotStarted]
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        meta.modified().unwrap()
    );
}
#[test]
fn p1_admit_01_no_root_escape_or_effect_for_unsafe_paths() {
    let t = DisposableRoot::new();
    t.file("input.json", INPUT);
    let cancel = AtomicBool::new(false);
    let mut plan = t.plan();
    plan.source.path = "/etc".into();
    assert_eq!(
        TestHarness::start(&plan, &t.root, &cancel).unwrap_err(),
        HarnessError::UnsafePlan
    );
    assert_eq!(fs::read_dir(t.root.join("scratch")).unwrap().count(), 0);
    plan = t.plan();
    plan.sinks[0].path = "/tmp/external".into();
    assert_eq!(
        TestHarness::start(&plan, &t.root, &cancel).unwrap_err(),
        HarnessError::UnsafePlan
    );
    assert_eq!(fs::read_dir(t.root.join("scratch")).unwrap().count(), 0);
}
#[test]
fn res_01_oversize_source_rejected_before_scratch_side_effects() {
    let t = DisposableRoot::new();
    t.file("too-large.json", &vec![b' '; RAW_FILE_MAX + 1]);
    let cancel = AtomicBool::new(false);
    assert_eq!(
        TestHarness::start(&t.plan(), &t.root, &cancel).unwrap_err(),
        HarnessError::ResourceExhausted
    );
    assert_eq!(fs::read_dir(t.root.join("scratch")).unwrap().count(), 0);
}
#[cfg(unix)]
#[test]
fn p1_admit_symlink_source_or_ancestor_is_rejected() {
    use std::os::unix::fs::symlink;
    let t = DisposableRoot::new();
    t.file("input.json", INPUT);
    symlink("/etc/passwd", t.root.join("in/evil.json")).unwrap();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        TestHarness::start(&t.plan(), &t.root, &cancel).unwrap_err(),
        HarnessError::PathEscape
    );
    assert_eq!(fs::read_dir(t.root.join("scratch")).unwrap().count(), 0);
}
#[test]
fn comp_f04_empty_json_array_produces_explicit_empty_outputs() {
    let t = DisposableRoot::new();
    t.file("empty.json", b"[]");
    let cancel = AtomicBool::new(false);
    let out = TestHarness::start(&t.plan(), &t.root, &cancel).unwrap();
    assert_eq!(out.status, EphemeralStatus::Completed);
    assert_eq!(out.source_units[0].record_count, 0);
    assert_eq!(out.source_units[0].disposition, SourceDisposition::Empty);
    assert_eq!(inspect(&out, 0), codec::decode(b"[]").unwrap());
    assert_eq!(inspect(&out, 1), codec::decode(b"[]").unwrap());
}
#[test]
fn comp_f01_malformed_json_run_fails_without_scratch_artifact() {
    let t = DisposableRoot::new();
    t.file("bad.json", b"[{},12]");
    let cancel = AtomicBool::new(false);
    let out = TestHarness::start(&t.plan(), &t.root, &cancel).unwrap();
    assert_eq!(out.status, EphemeralStatus::Failed);
    assert_eq!(
        out.outputs,
        vec![BranchStatus::NotStarted, BranchStatus::NotStarted]
    );
    assert!(out.scratch_paths.is_empty());
    assert_eq!(out.source_units.len(), 1);
    assert_eq!(out.source_units[0].disposition, SourceDisposition::Failed);
    assert!(out.source_units[0].run_failed);
    assert_eq!(fs::read(t.root.join("in/bad.json")).unwrap(), b"[{},12]");
}

#[test]
fn p1_crash_03_child_worker_after_a_write() {
    // Child runs only under explicitly selected test target and disposable root.
    let Ok(root) = std::env::var("TRAM_P1_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let cancelled = AtomicBool::new(false);
    let result = TestHarness::start_with_fault(
        &plan_for(&root),
        &root,
        &cancelled,
        InjectedFault::PauseAfterFirstWrite,
    );
    panic!("P1 subprocess unexpectedly returned instead of being killed: {result:?}");
}
#[test]
fn p1_crash_03_os_kill_after_scratch_a_and_independent_new_run() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let t = DisposableRoot::new();
    t.file("input.json", INPUT);
    let source = t.root.join("in/input.json");
    let (original, meta) = source_snapshot(&source);
    let mut child = Command::new(std::env::current_exe().expect("integration executable"))
        .arg("--exact")
        .arg("p1_crash_03_child_worker_after_a_write")
        .arg("--nocapture")
        .env("TRAM_P1_CHILD_ROOT", t.root.to_str().expect("utf8 root"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn worker");
    let deadline = Instant::now() + Duration::from_secs(12);
    let scratch = t.root.join("scratch");
    let mut partial: Option<PathBuf> = None;
    while Instant::now() < deadline {
        for run in fs::read_dir(&scratch).expect("run dirs") {
            let path = run.expect("entry").path().join("a/output-a.json");
            if let Ok(bytes) = fs::read(&path) {
                if codec::decode(&bytes).ok() == codec::decode(EXPECTED_A).ok() {
                    partial = Some(path);
                    break;
                }
            }
        }
        if partial.is_some() {
            break;
        }
        if let Ok(Some(exited)) = child.try_wait() {
            panic!("child exited before SIGKILL: {exited}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    if partial.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("child never reached partial scratch write");
    }
    child.kill().expect("OS kill");
    let status = child.wait().expect("OS wait");
    assert_eq!(status.signal(), Some(9), "required actual SIGKILL");
    let partial_file = partial.unwrap();
    assert_eq!(
        codec::decode(&fs::read(&partial_file).unwrap()).unwrap(),
        codec::decode(EXPECTED_A).unwrap()
    );
    assert_eq!(
        fs::read(&source).unwrap(),
        original,
        "kill must not mutate source bytes"
    );
    assert_eq!(
        fs::metadata(&source).unwrap().modified().unwrap(),
        meta.modified().unwrap()
    );
    let independent = TestHarness::start(&t.plan(), &t.root, &AtomicBool::new(false)).unwrap();
    assert_eq!(independent.status, EphemeralStatus::Completed);
    assert_ne!(independent.scratch_paths[0], partial_file);
    assert_eq!(inspect(&independent, 1), codec::decode(EXPECTED_B).unwrap());
    println!("P1_CRASH_03_OS_KILL=SIGKILL_9");
    println!("P1_CRASH_03_INPUT_UNCHANGED=true");
    println!("P1_CRASH_03_INDEPENDENT_NEW_RUN=true");
}
