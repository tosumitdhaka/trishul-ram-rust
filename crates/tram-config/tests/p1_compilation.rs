// SPDX-License-Identifier: Apache-2.0
//! Frozen Phase 0 P1 compiler oracle. No test touches the source/sink filesystem.

use std::collections::BTreeMap;
use tram_config::{compile_p1_builtin, substitute_env, ConfigError, Transform};

const GOLDEN: &str = include_str!("../../../docs/phase-0/fixtures/p1/pipeline.yaml");
const INPUT: &str = include_str!("../../../docs/phase-0/fixtures/p1/input.json");
const A: &str = include_str!("../../../docs/phase-0/fixtures/p1/expected-a.json");
const B: &str = include_str!("../../../docs/phase-0/fixtures/p1/expected-b.json");

// Pinned object IDs checked independently by GitHub Actions via git hash-object.
const FIXTURE_BLOBS: [(&str, &str); 4] = [
    (
        "docs/phase-0/fixtures/p1/pipeline.yaml",
        "4ca175df56a8bc86655da49f6713d8a5c64a0e41",
    ),
    (
        "docs/phase-0/fixtures/p1/input.json",
        "5e9127b5ecb429199f1e150726b8091ddf898430",
    ),
    (
        "docs/phase-0/fixtures/p1/expected-a.json",
        "72ef5ae522bc6cb032b18c49150ea6cadecce993",
    ),
    (
        "docs/phase-0/fixtures/p1/expected-b.json",
        "1dc44035a3fa51938f640d838562bee7d54baa16",
    ),
];
fn vars() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("TRAM_P1_INPUT_DIR".into(), "/isolated/in".into()),
        ("TRAM_P1_SCRATCH_A".into(), "/isolated/scratch/a".into()),
        ("TRAM_P1_SCRATCH_B".into(), "/isolated/scratch/b".into()),
    ])
}
fn compile(yaml: &str) -> Result<tram_config::ValidatedPlan, ConfigError> {
    compile_p1_builtin(yaml, &vars())
}
fn insert_source(field: &str) -> String {
    GOLDEN.replacen("  source:\n", &format!("  source:\n    {field}\n"), 1)
}
fn err_code(config: &str, code: &str) {
    let error = compile(config).expect_err("configuration must fail closed");
    assert_eq!(error.code(), code, "{config}");
}

#[test]
fn comp_01_golden_pinned_compiles_to_exact_p1_plan_without_effects() {
    let plan = compile(GOLDEN).expect("COMP-01-GOLDEN");
    assert_eq!(plan.contract_version, 1);
    assert!(plan.ephemeral_only);
    assert_eq!(plan.name, "p1-local-json");
    assert_eq!(plan.source.path, "/isolated/in");
    assert_eq!(plan.source.file_pattern, "*.json");
    assert_eq!(plan.transforms.len(), 4);
    assert!(matches!(plan.transforms[0], Transform::Rename(_)));
    assert!(matches!(plan.transforms[1], Transform::AddField(_)));
    assert!(matches!(plan.transforms[2], Transform::Filter(_)));
    assert!(matches!(plan.transforms[3], Transform::Drop(_)));
    assert_eq!(plan.sinks.len(), 2);
    assert_eq!(plan.sinks[0].filename_template, "output-a.json");
    assert_eq!(plan.sinks[1].filename_template, "output-b.json");
    assert_eq!(plan.sinks[1].transforms.len(), 1);
    assert_eq!(FIXTURE_BLOBS.len(), 4);
    assert!(INPUT.contains("old_id"));
    assert!(A.contains("double_metric"));
    assert!(B.contains("secondary"));
    assert!(FIXTURE_BLOBS
        .iter()
        .all(|(path, hash)| path.starts_with("docs/phase-0/")
            && hash.len() == 40
            && hash.bytes().all(|b| b.is_ascii_hexdigit())));
}
#[test]
fn comp_01_flags_present_true_and_false_reject_without_any_io() {
    for field in [
        "skip_processed: true",
        "skip_processed: false",
        "delete_after_read: true",
        "delete_after_read: false",
        "unsupported_probe: false",
        "move_after_read: null",
        "file_done_suffix: null",
        "file_stability_seconds: 0",
        "file_min_age_seconds: 0",
    ] {
        err_code(&insert_source(field), "CONFIG_UNSUPPORTED_OPTION");
    }
}
#[test]
fn comp_01_flat_and_singular_sink_alias() {
    let without_wrapper = GOLDEN.replacen("version: \"1\"\npipeline:\n", "", 1);
    let flat = without_wrapper
        .lines()
        .map(|line| line.strip_prefix("  ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(compile(&flat).expect("flat supported").sinks.len(), 2);
    let one = r#"
name: simple
schedule: {type: manual}
source: {type: local, path: /isolated/in}
serializer_in: {type: json}
sink: {type: local, path: /isolated/scratch/a, file_mode: single, overwrite: false, filename_template: out.json}
on_error: abort
retry_count: 0
retry_delay_seconds: 0
"#;
    assert_eq!(compile(one).expect("single sink").sinks.len(), 1);
}
#[test]
fn comp_01_environment_resolution_defaults_and_required() {
    assert_eq!(
        substitute_env(&format!("{}{{A:-default}}", '$'), &BTreeMap::new()).expect("fallback"),
        "default"
    );
    assert_eq!(
        substitute_env(
            &format!("{}{{A:-default}}", '$'),
            &BTreeMap::from([("A".into(), String::new())])
        )
        .expect("set empty"),
        ""
    );
    assert_eq!(
        substitute_env(&format!("{}{{A}}", '$'), &BTreeMap::new()),
        Err(ConfigError::MissingEnv("A".into()))
    );
    assert!(matches!(
        compile_p1_builtin(GOLDEN, &BTreeMap::new()),
        Err(ConfigError::MissingEnv(_))
    ));
    err_code(
        &GOLDEN.replace("type: manual", "type: cron"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
}
#[test]
fn comp_01_rejects_malformed_unknown_or_conflicting_config() {
    err_code("pipeline: [unclosed", "CONFIG_MALFORMED_YAML");
    err_code("null", "CONFIG_INVALID_VALUE");
    err_code(
        &GOLDEN.replace("  name:", "  unsupported_top: false\n  name:"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("    type: json", "    type: json\n    schema: ignored"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("  sinks:", "  sink: {type: local}\n  sinks:"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("  source:\n", "  source:\n    skip_processed: false\n"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("    recursive: false", "    recursive: true"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("  parallel_sinks: false", "  parallel_sinks: true"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("  retry_count: 0", "  retry_count: 2"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
}
#[test]
fn comp_01_rejects_unsupported_plugin_serializer_transforms_and_expressions() {
    err_code(
        &GOLDEN.replace("type: local", "type: sftp"),
        "CONFIG_UNSUPPORTED_PLUGIN",
    );
    err_code(
        &GOLDEN.replace("type: json", "type: xml"),
        "CONFIG_UNSUPPORTED_PLUGIN",
    );
    err_code(
        &GOLDEN.replace("type: rename", "type: exec"),
        "CONFIG_UNSUPPORTED_PLUGIN",
    );
    err_code(
        &GOLDEN.replace("metric * 2", "__import__('os')"),
        "CONFIG_UNSUPPORTED_EXPRESSION",
    );
    err_code(
        &GOLDEN.replace("metric >= 10", "metric.value >= 10"),
        "CONFIG_UNSUPPORTED_EXPRESSION",
    );
    err_code(
        &GOLDEN.replace("metric >= 10", "unknown_fn(metric)"),
        "CONFIG_UNSUPPORTED_EXPRESSION",
    );
    err_code(
        &GOLDEN.replace(
            "    - type: drop\n      fields: [metric]",
            "    - type: drop\n      fields: [metric.foo]",
        ),
        "CONFIG_UNSUPPORTED_OPTION",
    );
}
#[test]
fn comp_01_rejects_bad_sink_topology_and_unsafe_outputs() {
    let duplicate = GOLDEN
        .replace(
            &format!("{}{{TRAM_P1_SCRATCH_B}}", '$'),
            &format!("{}{{TRAM_P1_SCRATCH_A}}", '$'),
        )
        .replace("output-b.json", "output-a.json");
    err_code(&duplicate, "CONFIG_UNSUPPORTED_OPTION");
    err_code(
        &GOLDEN.replace("file_mode: single", "file_mode: append"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("overwrite: false", "overwrite: true"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("output-b.json", "../outside.json"),
        "CONFIG_UNSUPPORTED_OPTION",
    );
    err_code(
        &GOLDEN.replace("  sinks:\n", "  sinks: []\n"),
        "CONFIG_MALFORMED_YAML",
    );
    err_code(
        &GOLDEN.replacen("    - type: local", "    - type: kafka", 1),
        "CONFIG_UNSUPPORTED_PLUGIN",
    );
}
