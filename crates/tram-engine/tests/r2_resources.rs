// SPDX-License-Identifier: Apache-2.0
//! Additional bounded P1 resource, pre-effect and real-fault regressions.
//! Disposable fixture roots only; never operate on production paths.
#![cfg(target_os = "linux")]
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tram_config::{compile_p1_builtin, ConfigError};
use tram_engine::{
    budget::{BudgetCaps, BudgetError, BudgetLedger, Category, DECODED_SOURCE_MAX, RAW_FILE_MAX},
    codec::{self, JsonError},
    harness::{BranchStatus, EphemeralStatus, HarnessError, InjectedFault, TestHarness},
};
use tram_model::Datum;
const YAML: &str = include_str!("../../../docs/phase-0/fixtures/p1/pipeline.yaml");
const INPUT: &[u8] = include_bytes!("../../../docs/phase-0/fixtures/p1/input.json");
struct Root {
    path: PathBuf,
}
impl Root {
    fn new() -> Self {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tram-p1-bounds-{}-{n}", std::process::id()));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("in")).unwrap();
        fs::create_dir(path.join("scratch")).unwrap();
        Self { path }
    }
    fn add(&self, name: &str, bytes: &[u8]) {
        fs::write(self.path.join("in").join(name), bytes).unwrap();
    }
    fn plan(&self) -> tram_config::ValidatedPlan {
        plan(&self.path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn plan(root: &Path) -> tram_config::ValidatedPlan {
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
    compile_p1_builtin(YAML, &vars).unwrap()
}
fn scratch_runs(root: &Root) -> usize {
    fs::read_dir(root.path.join("scratch")).unwrap().count()
}
fn rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
#[test]
fn res_02_json_depth_and_token_count_fail_closed() {
    let deep = format!("{{\"a\":{}}}", "[".repeat(33) + "0" + &"]".repeat(33));
    assert_eq!(
        codec::decode(deep.as_bytes()),
        Err(JsonError::ResourceExhausted)
    );
    let many = format!(
        "{{\"a\":[{}]}}",
        std::iter::repeat_n("0", 100_001)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(many.len() < RAW_FILE_MAX);
    assert_eq!(
        codec::decode(many.as_bytes()),
        Err(JsonError::ResourceExhausted)
    );
}
#[test]
fn res_03_4097_decoded_records_and_decoded_allocation_cap() {
    let rows = format!(
        "[{}]",
        std::iter::repeat_n("{}", 4097)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        codec::decode(rows.as_bytes()),
        Err(JsonError::ResourceExhausted)
    );
    let ledger = BudgetLedger::p1();
    assert!(matches!(
        ledger.reserve(Category::DecodedBytes, DECODED_SOURCE_MAX + 1),
        Err(BudgetError::ResourceExhausted)
    ));
    assert_eq!(ledger.current().decoded_bytes, 0);
}
#[test]
fn res_07_101_source_artifact_candidates_rejected_pre_scratch() {
    let root = Root::new();
    for n in 0..101 {
        root.add(&format!("item-{n:03}.json"), b"[]");
    }
    let error = TestHarness::start(&root.plan(), &root.path, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(error, HarnessError::ResourceExhausted);
    assert_eq!(scratch_runs(&root), 0);
    assert_eq!(fs::read(root.path.join("in/item-000.json")).unwrap(), b"[]");
}
#[test]
fn p1_admit_01_config_finalization_negative_all_pre_effect() {
    let root = Root::new();
    root.add("input.json", INPUT);
    let vars = BTreeMap::from([
        (
            "TRAM_P1_INPUT_DIR".into(),
            root.path.join("in").to_string_lossy().into_owned(),
        ),
        (
            "TRAM_P1_SCRATCH_A".into(),
            root.path.join("scratch/a").to_string_lossy().into_owned(),
        ),
        (
            "TRAM_P1_SCRATCH_B".into(),
            root.path.join("scratch/b").to_string_lossy().into_owned(),
        ),
    ]);
    for field in [
        "skip_processed: true",
        "skip_processed: false",
        "delete_after_read: true",
        "delete_after_read: false",
        "unsupported_probe: false",
        "move_after_read: null",
    ] {
        let modified = YAML.replacen("  source:\n", &format!("  source:\n    {field}\n"), 1);
        assert!(
            matches!(
                compile_p1_builtin(&modified, &vars),
                Err(ConfigError::UnsupportedOption(_))
            ),
            "{field}"
        );
        assert_eq!(scratch_runs(&root), 0);
    }
    assert_eq!(fs::read(root.path.join("in/input.json")).unwrap(), INPUT);
}
#[test]
fn p1_admit_01_scratch_parent_symlink_is_rejected_without_outside_effects() {
    use std::os::unix::fs::symlink;
    let root = Root::new();
    root.add("input.json", INPUT);
    let outsider = Root::new();
    fs::remove_dir(root.path.join("scratch")).unwrap();
    symlink(outsider.path.join("scratch"), root.path.join("scratch")).unwrap();
    let fail = TestHarness::start(&root.plan(), &root.path, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(fail, HarnessError::PathEscape);
    assert_eq!(scratch_runs(&outsider), 0);
    assert_eq!(fs::read(root.path.join("in/input.json")).unwrap(), INPUT);
}
#[test]
fn comp_02_multiple_sources_are_sorted_and_one_file_is_one_unit() {
    let root = Root::new();
    root.add("b.json", br#"[{"old_id":"B","metric":12}]"#);
    root.add("a.json", br#"[{"old_id":"A","metric":12}]"#);
    let out = TestHarness::start(&root.plan(), &root.path, &AtomicBool::new(false)).unwrap();
    assert_eq!(out.status, EphemeralStatus::Completed, "{:?}", out.error);
    assert_eq!(out.source_units.len(), 2);
    assert_eq!(out.source_units[0].relative_name, "a.json");
    assert_eq!(out.source_units[1].relative_name, "b.json");
    let records = codec::decode(&fs::read(&out.scratch_paths[0]).unwrap()).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["cell_id"], Datum::String("A".into()));
    assert_eq!(records[1]["cell_id"], Datum::String("B".into()));
}
#[test]
fn res_05_blocked_sink_can_be_cancelled_without_source_mutation() {
    let root = Root::new();
    root.add("input.json", INPUT);
    let source = root.path.join("in/input.json");
    let bytes = fs::read(&source).unwrap();
    let mtime = fs::metadata(&source).unwrap().modified().unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let flag_child = Arc::clone(&flag);
    let plan = root.plan();
    let path = root.path.clone();
    let thread = std::thread::spawn(move || {
        TestHarness::start_with_fault(
            &plan,
            &path,
            &flag_child,
            InjectedFault::PauseAfterFirstWrite,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut observed = false;
    while Instant::now() < deadline {
        if let Ok(entries) = fs::read_dir(root.path.join("scratch")) {
            for entry in entries.flatten() {
                if entry.path().join("a/output-a.json").exists() {
                    observed = true;
                    break;
                }
            }
        }
        if observed {
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    assert!(
        observed,
        "blocked fault did not produce first scratch artifact"
    );
    flag.store(true, Ordering::Release);
    let out = thread.join().expect("bounded cancel").unwrap();
    assert_eq!(out.status, EphemeralStatus::Cancelled);
    assert_eq!(
        out.outputs,
        vec![BranchStatus::ScratchWritten, BranchStatus::NotStarted]
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), mtime);
}
#[test]
fn res_06_branch_clone_stress_and_rss_observations() {
    let root = Root::new();
    let payload = "x".repeat(128 * 1024);
    let source = format!(r#"[{{"old_id":"A","metric":12,"payload":"{payload}"}}]"#);
    root.add("input.json", source.as_bytes());
    let rss_before = rss_kib();
    let out = TestHarness::start(&root.plan(), &root.path, &AtomicBool::new(false)).unwrap();
    let rss_after = rss_kib();
    assert_eq!(out.status, EphemeralStatus::Completed, "{:?}", out.error);
    let a = codec::decode(&fs::read(&out.scratch_paths[0]).unwrap()).unwrap();
    let b = codec::decode(&fs::read(&out.scratch_paths[1]).unwrap()).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert!(!a[0].contains_key("tag"));
    assert_eq!(b[0]["tag"], Datum::String("secondary".into()));
    assert_eq!(a[0]["payload"], Datum::String(payload));
    assert_eq!(out.peaks.raw_buffers, 1);
    assert_eq!(out.peaks.sink_io, 1);
    assert!(out.peaks.branch_bytes.iter().all(|x| *x <= 8 * 1024 * 1024));
    println!("P1_RES06_RSS_KIB_BEFORE={rss_before:?}");
    println!("P1_RES06_RSS_KIB_AFTER={rss_after:?}");
    println!("P1_RES06_LEDGER_PEAKS={:?}", out.peaks);
}
#[test]
fn res_07_scratch_artifact_and_cumulative_accounting_refuse_over_budget() {
    let ledger = BudgetLedger::new(BudgetCaps {
        scratch_artifacts: 2,
        scratch_bytes: 12,
        ..BudgetCaps::default()
    });
    let first = ledger.reserve(Category::ScratchArtifacts, 2).unwrap();
    assert!(matches!(
        ledger.reserve(Category::ScratchArtifacts, 1),
        Err(BudgetError::ResourceExhausted)
    ));
    let bytes = ledger.reserve(Category::ScratchBytes, 12).unwrap();
    assert!(matches!(
        ledger.reserve(Category::ScratchBytes, 1),
        Err(BudgetError::ResourceExhausted)
    ));
    assert_eq!(ledger.current().scratch_artifacts, 2);
    drop((first, bytes));
    assert_eq!(ledger.current().scratch_artifacts, 0);
    assert_eq!(ledger.current().scratch_bytes, 0);
}

#[test]
fn r2_g1_runtime_actual_records_and_zero_leaked_reservations() {
    let t = Root::new();
    t.add("input.json", INPUT);
    let result = TestHarness::start(&t.plan(), &t.path, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        result.status,
        EphemeralStatus::Completed,
        "{:?}",
        result.error
    );
    assert_eq!(result.source_units[0].record_count, 2);
    assert!(
        result.peaks.records > 0,
        "actual decoded record count must be nonzero"
    );
    assert!(
        result.peaks.raw_bytes < RAW_FILE_MAX,
        "raw charge must use actual frame bytes not a fixed 4MiB"
    );
    assert!(
        result.peaks.decoded_bytes < DECODED_SOURCE_MAX,
        "decoder preflight must use frame-proportional ownership"
    );
    assert!(result.peaks.live_bytes > 0);
    assert!(result
        .peaks
        .branch_bytes
        .iter()
        .all(|x| *x > 0 && *x < 8 * 1024 * 1024));
    assert_eq!(result.peaks.scratch_artifacts, 2);
    assert_eq!(
        result.live_after_teardown,
        Default::default(),
        "all move-only guards released on terminal outcome"
    );
    println!("R2_G1_ACTUAL_PEAKS={:?}", result.peaks);
}
#[test]
fn r2_g1_injected_lower_caps_fail_closed_without_unbounded_output() {
    let t = Root::new();
    t.add("input.json", INPUT);
    let before = fs::read(t.path.join("in/input.json")).unwrap();
    let modified = fs::metadata(t.path.join("in/input.json"))
        .unwrap()
        .modified()
        .unwrap();
    let categories = [
        (
            "raw",
            BudgetCaps {
                raw_bytes: 1,
                ..BudgetCaps::default()
            },
        ),
        (
            "decoded",
            BudgetCaps {
                decoded_bytes: 1,
                ..BudgetCaps::default()
            },
        ),
        (
            "records",
            BudgetCaps {
                records: 1,
                ..BudgetCaps::default()
            },
        ),
        (
            "live",
            BudgetCaps {
                live_bytes: 1,
                ..BudgetCaps::default()
            },
        ),
        (
            "branch",
            BudgetCaps {
                branch_bytes: 1,
                ..BudgetCaps::default()
            },
        ),
        (
            "scratch_bytes",
            BudgetCaps {
                scratch_bytes: 1,
                ..BudgetCaps::default()
            },
        ),
        (
            "scratch_artifacts",
            BudgetCaps {
                scratch_artifacts: 1,
                ..BudgetCaps::default()
            },
        ),
    ];
    for (name, caps) in categories {
        let out = TestHarness::start_with_caps(&t.plan(), &t.path, &AtomicBool::new(false), caps)
            .unwrap();
        assert_eq!(
            out.status,
            EphemeralStatus::Failed,
            "category: {name} {:?}",
            out.error
        );
        assert_eq!(
            out.live_after_teardown,
            Default::default(),
            "category: {name}"
        );
        assert_eq!(fs::read(t.path.join("in/input.json")).unwrap(), before);
        assert_eq!(
            fs::metadata(t.path.join("in/input.json"))
                .unwrap()
                .modified()
                .unwrap(),
            modified
        );
        println!("R2_G1_LOWER_CAP_REJECTED={name} PEAK={:?}", out.peaks);
    }
}
#[test]
fn r2_g1_fault_during_second_scratch_write_is_uncertain_and_leak_free() {
    let t = Root::new();
    t.add("input.json", INPUT);
    let bytes = fs::read(t.path.join("in/input.json")).unwrap();
    let result = TestHarness::start_with_caps_and_fault(
        &t.plan(),
        &t.path,
        &AtomicBool::new(false),
        BudgetCaps::default(),
        InjectedFault::FailDuringSinkWrite(1),
    )
    .unwrap();
    assert_eq!(result.status, EphemeralStatus::Failed);
    assert_eq!(
        result.outputs,
        vec![BranchStatus::ScratchWritten, BranchStatus::Unknown]
    );
    assert_eq!(result.scratch_paths.len(), 2);
    assert_eq!(fs::read(&result.scratch_paths[1]).unwrap().len(), 11);
    assert_eq!(result.peaks.scratch_artifacts, 2);
    assert_eq!(result.live_after_teardown, Default::default());
    assert_eq!(fs::read(t.path.join("in/input.json")).unwrap(), bytes);
}

fn wait_for_run(root: &Root, slot: Option<&str>) -> PathBuf {
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Ok(entries) = fs::read_dir(root.path.join("scratch")) {
            for entry in entries.flatten() {
                let run = entry.path();
                let marker = slot.map(|x| run.join(x)).unwrap_or_else(|| run.clone());
                if marker.is_dir() {
                    return run;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "race test did not reach deterministic hook"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn r2_g2_concurrent_final_source_symlink_swap_denies_escape() {
    use std::os::unix::fs::symlink;
    let t = Root::new();
    t.add("input.json", INPUT);
    let outsider = Root::new();
    outsider.add("secret.json", b"{\"secret\":true}");
    let outside = outsider.path.join("in/secret.json");
    let original = fs::read(&outside).unwrap();
    let p = t.plan();
    let run_root = t.path.clone();
    let handle = std::thread::spawn(move || {
        TestHarness::start_with_fault(
            &p,
            &run_root,
            &AtomicBool::new(false),
            InjectedFault::PauseBeforeSourceOpen,
        )
    });
    let _ = wait_for_run(&t, None);
    std::thread::sleep(Duration::from_millis(35));
    fs::rename(t.path.join("in/input.json"), t.path.join("in/kept.json")).unwrap();
    symlink(&outside, t.path.join("in/input.json")).unwrap();
    let result = handle.join().unwrap();
    match result {
        Ok(ref out) => {
            assert_eq!(out.status, EphemeralStatus::Failed);
            assert!(out.scratch_paths.is_empty());
            assert_eq!(out.live_after_teardown, Default::default());
        }
        Err(e) => assert!(matches!(e, HarnessError::PathEscape | HarnessError::Io)),
    }
    assert_eq!(fs::read(&outside).unwrap(), original);
    assert_eq!(fs::read(t.path.join("in/kept.json")).unwrap(), INPUT);
}
#[test]
fn r2_g2_hardlink_alias_to_outside_inode_denied() {
    let t = Root::new();
    let outsider = Root::new();
    outsider.add("secret.json", INPUT);
    let secret = outsider.path.join("in/secret.json");
    let before = fs::read(&secret).unwrap();
    fs::hard_link(&secret, t.path.join("in/input.json")).unwrap();
    let result = TestHarness::start(&t.plan(), &t.path, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.status, EphemeralStatus::Failed);
    assert!(result.scratch_paths.is_empty());
    assert_eq!(result.live_after_teardown, Default::default());
    assert_eq!(fs::read(&secret).unwrap(), before);
}
#[test]
fn r2_g2_sink_parent_swap_to_external_symlink_refuses_outside_write() {
    use std::os::unix::fs::symlink;
    let t = Root::new();
    t.add("input.json", INPUT);
    let outsider = Root::new();
    let plan = t.plan();
    let run_root = t.path.clone();
    let handle = std::thread::spawn(move || {
        TestHarness::start_with_fault(
            &plan,
            &run_root,
            &AtomicBool::new(false),
            InjectedFault::PauseBeforeSinkOpen(0),
        )
    });
    let run = wait_for_run(&t, Some("a"));
    fs::rename(run.join("a"), run.join("a-original")).unwrap();
    symlink(outsider.path.join("scratch"), run.join("a")).unwrap();
    let out = handle.join().unwrap().unwrap();
    assert_eq!(out.status, EphemeralStatus::Failed);
    assert!(out.scratch_paths.is_empty());
    assert_eq!(out.live_after_teardown, Default::default());
    assert_eq!(scratch_runs(&outsider), 0);
    assert_eq!(fs::read(t.path.join("in/input.json")).unwrap(), INPUT);
}
#[test]
fn r2_g2_existing_destination_exclusive_creation_fails_without_overwrite() {
    let t = Root::new();
    t.add("input.json", INPUT);
    let plan = t.plan();
    let run_root = t.path.clone();
    let handle = std::thread::spawn(move || {
        TestHarness::start_with_fault(
            &plan,
            &run_root,
            &AtomicBool::new(false),
            InjectedFault::PauseBeforeSinkOpen(0),
        )
    });
    let run = wait_for_run(&t, Some("a"));
    let target = run.join("a/output-a.json");
    fs::write(&target, b"sentinel").unwrap();
    let out = handle.join().unwrap().unwrap();
    assert_eq!(out.status, EphemeralStatus::Failed);
    assert_eq!(fs::read(target).unwrap(), b"sentinel");
    assert_eq!(out.live_after_teardown, Default::default());
    assert_eq!(fs::read(t.path.join("in/input.json")).unwrap(), INPUT);
}

fn process_highwater_kib() -> u64 {
    fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn r2_g1_simultaneous_multifile_branch_rss_peak_and_zero_teardown() {
    let root = Root::new();
    let payload = "z".repeat(800 * 1024);
    let input = format!(r#"[{{"old_id":"A","metric":12,"payload":"{payload}"}}]"#);
    for n in 0..3 {
        root.add(&format!("file-{n:03}.json"), input.as_bytes());
    }
    let before = process_highwater_kib();
    let result = TestHarness::start(&root.plan(), &root.path, &AtomicBool::new(false)).unwrap();
    let after = process_highwater_kib();
    assert_eq!(
        result.status,
        EphemeralStatus::Completed,
        "{:?}",
        result.error
    );
    assert_eq!(result.source_units.len(), 3);
    assert_eq!(result.peaks.records, 1);
    assert!(result.peaks.raw_bytes >= 800 * 1024);
    assert!(result.peaks.decoded_bytes >= 800 * 1024);
    assert!(result
        .peaks
        .branch_bytes
        .iter()
        .all(|v| *v > 2 * 1024 * 1024));
    assert!(result.peaks.live_bytes < 64 * 1024 * 1024);
    assert!(result.peaks.scratch_bytes > 4 * 1024 * 1024);
    assert_eq!(result.peaks.scratch_artifacts, 2);
    assert_eq!(result.live_after_teardown, Default::default());
    assert!(after >= before);
    let first = codec::decode(&fs::read(&result.scratch_paths[0]).unwrap()).unwrap();
    let second = codec::decode(&fs::read(&result.scratch_paths[1]).unwrap()).unwrap();
    assert_eq!(first.len(), 3);
    assert_eq!(second.len(), 3);
    for n in 0..3 {
        assert_eq!(
            fs::read(root.path.join(format!("in/file-{n:03}.json"))).unwrap(),
            input.as_bytes()
        );
    }
    println!("R2_G1_MULTIFILE_RSS_VMHWM_KIB_BEFORE={before}");
    println!("R2_G1_MULTIFILE_RSS_VMHWM_KIB_AFTER={after}");
    println!("R2_G1_MULTIFILE_LEDGER_PEAKS={:?}", result.peaks);
    println!("R2_G1_MULTIFILE_RESERVATIONS_RELEASED=true");
}

#[test]
fn r2_g1_exact_four_mib_input_is_admissible_without_fifth_byte() {
    let t = Root::new();
    let minimal = format!(r#"[{{"old_id":"B","metric":4,"pad":"{}"}}]"#, "");
    let pad = "x".repeat(RAW_FILE_MAX - minimal.len());
    let content = format!(r#"[{{"old_id":"B","metric":4,"pad":"{pad}"}}]"#);
    assert_eq!(content.len(), RAW_FILE_MAX);
    t.add("input.json", content.as_bytes());
    let out = TestHarness::start(&t.plan(), &t.path, &AtomicBool::new(false)).unwrap();
    assert_eq!(out.status, EphemeralStatus::Completed, "{:?}", out.error);
    assert_eq!(out.source_units[0].byte_count, RAW_FILE_MAX);
    assert_eq!(out.source_units[0].filtered_global, 1);
    assert_eq!(out.peaks.raw_bytes, RAW_FILE_MAX);
    assert_eq!(out.live_after_teardown, Default::default());
    assert_eq!(
        fs::read(t.path.join("in/input.json")).unwrap(),
        content.as_bytes()
    );
    println!("R2_G1_RAW_EXACT_BOUND_ACCEPTED_4MIB=true");
}

#[test]
fn r2_f3_mutated_compiler_plan_rejected_pre_effect() {
    use tram_config::{Expression, Transform};
    let root = Root::new();
    root.add("input.json", INPUT);
    let mut forged = root.plan();
    assert!(forged.is_compiler_minted());
    forged.transforms.push(Transform::AddField(vec![(
        "nested.path".into(), Expression::Literal(Datum::Signed(1)),
    )]));
    assert!(!forged.is_compiler_minted());
    assert_eq!(
        TestHarness::start(&forged, &root.path.join("nonexistent"), &AtomicBool::new(false))
            .unwrap_err(), HarnessError::UnsafePlan
    );
    assert_eq!(
        TestHarness::start(&forged, &root.path, &AtomicBool::new(false)).unwrap_err(),
        HarnessError::UnsafePlan
    );
    assert_eq!(scratch_runs(&root), 0);
    assert_eq!(fs::read(root.path.join("in/input.json")).unwrap(), INPUT);
}
#[test]
fn r2_f3_unmodified_compiler_plan_and_clone_remain_admitted() {
    let root = Root::new();
    root.add("input.json", INPUT);
    let plan = root.plan();
    assert!(plan.is_compiler_minted());
    assert!(plan.clone().is_compiler_minted());
    let outcome = TestHarness::start(&plan, &root.path, &AtomicBool::new(false)).unwrap();
    assert_eq!(outcome.status, EphemeralStatus::Completed);
    assert_eq!(outcome.live_after_teardown, Default::default());
}

fn r2_f4_assert_bad_source_fails_without_effects(bytes: &[u8]) {
    use std::os::unix::fs::MetadataExt;
    use sha2::{Digest, Sha256};
    let root = Root::new();
    root.add("input.json", bytes);
    let source = root.path.join("in/input.json");
    let before = fs::metadata(&source).unwrap();
    let hash_before = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
    let outcome = TestHarness::start(&root.plan(), &root.path, &AtomicBool::new(false))
        .expect("failure after read must return an ephemeral outcome");
    assert_eq!(outcome.status, EphemeralStatus::Failed);
    assert!(outcome.error.as_deref() == Some("ResourceExhausted"), "{:?}",outcome.error);
    assert_eq!(outcome.source_units.len(),1);
    assert_eq!(outcome.source_units[0].disposition, tram_engine::harness::SourceDisposition::Failed);
    assert!(outcome.source_units[0].run_failed);
    assert_eq!(outcome.live_after_teardown, Default::default());
    assert!(outcome.scratch_paths.is_empty(), "no scratch artifacts");
    let run_dir = root.path.join("scratch").join(&outcome.run_id);
    assert_eq!(fs::read_dir(run_dir).unwrap().count(),0);
    let after = fs::metadata(&source).unwrap();
    assert_eq!(after.ino(), before.ino());
    assert_eq!(after.mode(), before.mode());
    assert_eq!(after.len(), before.len());
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    assert_eq!(format!("{:x}", Sha256::digest(fs::read(&source).unwrap())), hash_before);
}
#[test]
fn r2_f4_harness_33_depth_and_100001_tokens_fail_preserving_source() {
    let deep=format!("{{\"a\":{}}}", "[".repeat(33)+"0"+&"]".repeat(33));
    r2_f4_assert_bad_source_fails_without_effects(deep.as_bytes());
    let many=format!("{{\"a\":[{}]}}",std::iter::repeat_n("0",100_001).collect::<Vec<_>>().join(","));
    r2_f4_assert_bad_source_fails_without_effects(many.as_bytes());
}
#[test]
fn r2_f4_harness_4097_records_fail_preserving_source() {
    let records=format!("[{}]",std::iter::repeat_n("{}",4097).collect::<Vec<_>>().join(","));
    r2_f4_assert_bad_source_fails_without_effects(records.as_bytes());
}
#[test]
fn r2_f1_concurrent_threads_fail_closed_and_guard_releases_on_cancel() {
    let root = Root::new();
    root.add("input.json",INPUT);
    let flag=Arc::new(AtomicBool::new(false));
    let cancel=Arc::clone(&flag);
    let first_plan=root.plan();
    let path=root.path.clone();
    let thread=std::thread::spawn(move|| {
        TestHarness::start_with_fault(&first_plan,&path,&cancel,InjectedFault::PauseAfterFirstWrite)
    });
    let _first=wait_for_run(&root,Some("a"));
    let second=TestHarness::start(&root.plan(),&root.path,&AtomicBool::new(false));
    assert_eq!(second.unwrap_err(),HarnessError::ResourceExhausted);
    assert_eq!(scratch_runs(&root),1);
    flag.store(true,Ordering::Release);
    let finished=thread.join().unwrap().unwrap();
    assert_eq!(finished.status,EphemeralStatus::Cancelled);
    assert_eq!(finished.live_after_teardown,Default::default());
    let fresh=TestHarness::start(&root.plan(),&root.path,&AtomicBool::new(false)).unwrap();
    assert_eq!(fresh.status,EphemeralStatus::Completed);
    assert_eq!(scratch_runs(&root),2);
    assert_eq!(fs::read(root.path.join("in/input.json")).unwrap(),INPUT);
}
#[test]
fn r2_f1_cross_process_scratch_flock_rejects_overlap_and_releases_after_kill() {
    use std::process::{Command,Stdio};
    let root = Root::new();
    root.add("input.json",INPUT);
    let exe=std::env::current_exe().unwrap();
    let mut child=Command::new(exe)
        .arg("--exact").arg("r2_f1_child_holds_run_lock")
        .env("TRAM_P1_F1_CHILD_ROOT",root.path.to_str().unwrap())
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let _first=wait_for_run(&root,Some("a"));
    assert_eq!(
        TestHarness::start(&root.plan(),&root.path,&AtomicBool::new(false)).unwrap_err(),
        HarnessError::ResourceExhausted
    );
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    let next=TestHarness::start(&root.plan(),&root.path,&AtomicBool::new(false)).unwrap();
    assert_eq!(next.status,EphemeralStatus::Completed);
    assert_eq!(next.live_after_teardown,Default::default());
    assert_eq!(fs::read(root.path.join("in/input.json")).unwrap(),INPUT);
}
#[test]
fn r2_f1_child_holds_run_lock() {
    let Ok(root)=std::env::var("TRAM_P1_F1_CHILD_ROOT") else {return;};
    let root=PathBuf::from(root);
    let result=TestHarness::start_with_fault(
        &plan(&root),&root,&AtomicBool::new(false),InjectedFault::PauseAfterFirstWrite,
    );
    panic!("cross-process admission child unexpectedly returned: {result:?}");
}
#[test]
fn r2_f2_scratch_to_in_role_symlink_swap_is_denied_before_effects() {
    use std::os::unix::fs::{symlink,MetadataExt};
    let root=Root::new();
    root.add("input.json",INPUT);
    let before=fs::metadata(root.path.join("in/input.json")).unwrap();
    let p=root.plan();
    let path=root.path.clone();
    let thread=std::thread::spawn(move|| {
        TestHarness::start_with_fault(
            &p,&path,&AtomicBool::new(false),InjectedFault::PauseBeforeScratchOpen
        )
    });
    let deadline=Instant::now()+Duration::from_secs(5);
    while !tram_engine::harness::scratch_admission_gate_reached() {
        assert!(Instant::now()<deadline,"scratch swap gate never reached");
        std::thread::sleep(Duration::from_millis(2));
    }
    fs::rename(root.path.join("scratch"),root.path.join("scratch-original")).unwrap();
    symlink("in",root.path.join("scratch")).unwrap();
    assert_eq!(thread.join().unwrap().unwrap_err(),HarnessError::PathEscape);
    assert_eq!(fs::read_dir(root.path.join("in")).unwrap().count(),1);
    assert_eq!(fs::read(root.path.join("in/input.json")).unwrap(),INPUT);
    let after=fs::metadata(root.path.join("in/input.json")).unwrap();
    assert_eq!(before.mode(),after.mode());
    assert_eq!(before.modified().unwrap(),after.modified().unwrap());
    assert_eq!(before.ino(),after.ino());
    fs::remove_file(root.path.join("scratch")).unwrap();
    fs::rename(root.path.join("scratch-original"),root.path.join("scratch")).unwrap();
    assert_eq!(scratch_runs(&root),0);
}
