// SPDX-License-Identifier: Apache-2.0
//! Actual Linux tmpfs ENOSPC proof. Run explicitly on disposable 2 MiB mount.
#![cfg(target_os = "linux")]
use std::{collections::BTreeMap,fs,io::Write,path::PathBuf,sync::atomic::AtomicBool};
use tram_config::compile_p1_builtin;
use tram_engine::harness::{TestHarness, EphemeralStatus, BranchStatus};
const YAML:&str=include_str!("../../../docs/phase-0/fixtures/p1/pipeline.yaml");

#[test]
#[ignore = "requires private CI tmpfs; run using scripts/p1-enospc.sh"]
fn p1_real_enospc_scratch_is_unknown_and_source_immutable(){
    let root=PathBuf::from(std::env::var("TRAM_P1_ENOSPC_ROOT")
        .expect("isolated mount root required"));
    assert!(root.is_absolute());
    let mountinfo=fs::read_to_string("/proc/self/mountinfo").unwrap();
    assert!(mountinfo.lines().any(|line|
        line.contains(" - tmpfs ") &&
        line.split_whitespace().nth(4)==Some(root.to_str().unwrap())
    ),"dedicated tmpfs mount required");
    fs::create_dir(root.join("in")).unwrap();
    fs::create_dir(root.join("scratch")).unwrap();
    let payload="x".repeat(128*1024);
    let source= format!(r#"[{{"old_id":"A","metric":12,"payload":"{payload}"}}]"#);
    let path=root.join("in/input.json");
    fs::write(&path,source.as_bytes()).unwrap();
    let bytes=fs::read(&path).unwrap();
    let before=fs::metadata(&path).unwrap();
    let vars=BTreeMap::from([
        ("TRAM_P1_INPUT_DIR".into(),root.join("in").to_string_lossy().into_owned()),
        ("TRAM_P1_SCRATCH_A".into(),root.join("scratch/a").to_string_lossy().into_owned()),
        ("TRAM_P1_SCRATCH_B".into(),root.join("scratch/b").to_string_lossy().into_owned()),
    ]);
    let plan=compile_p1_builtin(YAML,&vars).unwrap();
    let mut filler=fs::File::create(root.join("scratch/filler")).unwrap();
    let chunk=[0x5au8;4096];
    let mut filled=0usize;
    let hit_enospc=loop {
        match filler.write_all(&chunk) {
            Ok(())=>{
                filled+=chunk.len();
                assert!(filled<2*1024*1024,"mount is not bounded 2 MiB");
            },
            Err(err)=>break err.raw_os_error()==Some(28),
        }
    };
    assert!(hit_enospc,"must observe kernel ENOSPC not injected failure");
    assert!(filled>128*1024);
    filler.set_len((filled-64*1024) as u64).unwrap();
    filler.sync_all().unwrap();
    let result=TestHarness::start(&plan,&root,&AtomicBool::new(false)).unwrap();
    assert_eq!(result.status,EphemeralStatus::Failed,"{:?}",result.error);
    assert!(result.outputs.iter().any(|s|*s!=BranchStatus::ScratchWritten));
    assert_eq!(result.live_after_teardown,Default::default());
    assert_eq!(fs::read(&path).unwrap(),bytes,"source content immutable");
    let after=fs::metadata(&path).unwrap();
    use std::os::unix::fs::MetadataExt;
    assert_eq!((before.ino(),before.mode(),before.len(),before.mtime_nsec(),before.mtime()),
        (after.ino(),after.mode(),after.len(),after.mtime_nsec(),after.mtime()));
    println!("P1_REAL_ENOSPC=28");
    println!("P1_REAL_ENOSPC_RUN_STATUS={}",result.status.as_str());
    println!("P1_REAL_ENOSPC_SINK_DISPOSITIONS={:?}",result.outputs);
    println!("P1_REAL_ENOSPC_SOURCE_IMMUTABLE=true");
    println!("P1_REAL_ENOSPC_LEDGER_RELEASED=true");
}
