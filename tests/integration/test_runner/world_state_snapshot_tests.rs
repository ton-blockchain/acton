use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use ton_emulator::WorldStateSnapshot;
use ton_executor::DEFAULT_CONFIG;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const SNAPSHOT_DIR: &str = "integration/snapshots/test-runner/world_state_snapshot";

fn build_world_state_snapshot_project(project_name: &str) -> Project {
    let save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot`() {{
    val target = randomAddress("snapshot-target");

    expect(testing.getAccountBalance(target)).toEqual(0);
    testing.topUp(target, ton("3"));
    testing.setNow(1700023001);

    expect(testing.saveSnapshot("world-state.json")).toBeTrue();
}}
"#
    );

    let load_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot from disk`() {{
    val target = randomAddress("snapshot-target");

    expect(testing.getAccountBalance(target)).toEqual(0);
    expect(testing.getNow()).toEqual(0);

    expect(testing.loadSnapshot("world-state.json")).toBeTrue();

    expect(testing.getNow()).toEqual(1700023001);
    expect(testing.getAccountBalance(target)).toEqual(ton("3"));
    expect(testing.getShardAccount(target)).toBeNotNull();
    expect(testing.saveSnapshot("roundtrip-world-state.json")).toBeTrue();
}}
"#
    );

    let replace_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot replaces current state`() {{
    val target = randomAddress("snapshot-target");

    testing.topUp(target, ton("1"));
    testing.setNow(99);
    expect(testing.getAccountBalance(target)).toEqual(ton("1"));
    expect(testing.getNow()).toEqual(99);

    expect(testing.loadSnapshot("world-state.json")).toBeTrue();

    expect(testing.getAccountBalance(target)).toEqual(ton("3"));
    expect(testing.getNow()).toEqual(1700023001);
}}
"#
    );

    let invalid_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot invalid inputs`() {{
    expect(testing.loadSnapshot("missing-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("broken-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("unsupported-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("invalid-address-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("invalid-config-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("invalid-library-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("invalid-random-seed-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot("duplicate-accounts-world-state.json")).toBeFalse();
}}
"#
    );

    let empty_save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save empty world state snapshot`() {{
    expect(testing.getNow()).toEqual(0);
    expect(testing.saveSnapshot("empty-world-state.json")).toBeTrue();
}}
"#
    );

    let empty_load_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load empty world state snapshot`() {{
    val target = randomAddress("snapshot-empty-target");

    testing.topUp(target, ton("1"));
    testing.setNow(77);

    expect(testing.loadSnapshot("empty-world-state.json")).toBeTrue();

    expect(testing.getNow()).toEqual(0);
    expect(testing.getAccountBalance(target)).toEqual(0);
}}
"#
    );

    let cache_only_save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot skips cached non existing accounts`() {{
    val target = randomAddress("snapshot-cache-only-target");

    expect(testing.getAccountBalance(target)).toEqual(0);
    expect(testing.isDeployed(target)).toBeFalse();
    expect(testing.saveSnapshot("cache-only-world-state.json")).toBeTrue();
}}
"#
    );

    let rich_save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot rich state`() {{
    val primary = randomAddress("snapshot-rich-primary");
    val secondary = randomAddress("snapshot-rich-secondary");

    testing.topUp(primary, ton("3"));
    testing.topUp(secondary, ton("5"));
    testing.setNow(1700023999);

    val libraryCode = build("simple");
    testing.registerLibrary(libraryCode);

    var config = testing.getConfig();
    val targetVersion = GlobalVersion {{
        version: 424244,
        capabilities: 1099511640122,
    }};
    config.setGlobalVersion(targetVersion);
    expect(testing.setConfig(config)).toBeTrue();

    expect(testing.saveSnapshot("fixtures/../fixtures/rich-world-state.json")).toBeTrue();
}}
"#
    );

    let rich_load_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot rich state`() {{
    val primary = randomAddress("snapshot-rich-primary");
    val secondary = randomAddress("snapshot-rich-secondary");

    expect(testing.loadSnapshot("./fixtures/rich-world-state.json")).toBeTrue();

    expect(testing.getNow()).toEqual(1700023999);
    expect(testing.getAccountBalance(primary)).toEqual(ton("3"));
    expect(testing.getAccountBalance(secondary)).toEqual(ton("5"));

    val version = testing.getConfig().getGlobalVersion();
    expect(version.version).toEqual(424244);
    expect(version.capabilities).toEqual(1099511640122);

    expect(testing.saveSnapshot("fixtures/rich-world-state-roundtrip.json")).toBeTrue();
}}
"#
    );

    let rich_same_run_restore_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save mutate load world state snapshot in same run`() {{
    val primary = randomAddress("snapshot-same-run-primary");
    val secondary = randomAddress("snapshot-same-run-secondary");

    testing.topUp(primary, ton("8"));
    testing.topUp(secondary, ton("13"));
    testing.setNow(1700024555);

    val libraryCode = build("simple");
    testing.registerLibrary(libraryCode);

    var config = testing.getConfig();
    config.setGlobalVersion(GlobalVersion {{
        version: 515151,
        capabilities: 2222222222,
    }});
    expect(testing.setConfig(config)).toBeTrue();

    expect(testing.saveSnapshot("fixtures/same-run-before.json")).toBeTrue();

    testing.setNow(1);

    var mutated = testing.getConfig();
    mutated.setGlobalVersion(GlobalVersion {{
        version: 1,
        capabilities: 1,
    }});
    expect(testing.setConfig(mutated)).toBeTrue();

    expect(testing.loadSnapshot("fixtures/same-run-before.json")).toBeTrue();
    expect(testing.getNow()).toEqual(1700024555);
    expect(testing.getAccountBalance(primary)).toEqual(ton("8"));
    expect(testing.getAccountBalance(secondary)).toEqual(ton("13"));

    val version = testing.getConfig().getGlobalVersion();
    expect(version.version).toEqual(515151);
    expect(version.capabilities).toEqual(2222222222);

    expect(testing.saveSnapshot("fixtures/same-run-after.json")).toBeTrue();
}}
"#
    );

    let replace_with_transient_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot drops transient state`() {{
    val transient = randomAddress("snapshot-transient-target");

    testing.topUp(transient, ton("9"));
    testing.setNow(777);
    expect(testing.saveSnapshot("fixtures/transient-before-load.json")).toBeTrue();

    expect(testing.loadSnapshot("fixtures/../world-state.json")).toBeTrue();
    expect(testing.saveSnapshot("fixtures/after-load-world-state.json")).toBeTrue();
}}
"#
    );

    let failed_load_preserves_state_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot failure keeps current state`() {{
    val target = randomAddress("snapshot-preserve-target");

    testing.topUp(target, ton("4"));
    testing.setNow(404);

    expect(testing.loadSnapshot("duplicate-accounts-world-state.json")).toBeFalse();
    expect(testing.getAccountBalance(target)).toEqual(ton("4"));
    expect(testing.getNow()).toEqual(404);

    expect(testing.loadSnapshot("invalid-config-world-state.json")).toBeFalse();
    expect(testing.getAccountBalance(target)).toEqual(ton("4"));
    expect(testing.getNow()).toEqual(404);
}}
"#
    );

    let save_failure_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot path failures`() {{
    expect(testing.saveSnapshot("missing-dir/world-state.json")).toBeFalse();
    expect(testing.saveSnapshot("fixtures")).toBeFalse();
    expect(testing.saveSnapshot("fixtures/ok-world-state.json")).toBeTrue();
}}
"#
    );

    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("save_world_state_snapshot", &save_source)
        .test_file("load_world_state_snapshot", &load_source)
        .test_file("replace_world_state_snapshot", &replace_source)
        .test_file("invalid_world_state_snapshot", &invalid_source)
        .test_file("save_empty_world_state_snapshot", &empty_save_source)
        .test_file("load_empty_world_state_snapshot", &empty_load_source)
        .test_file(
            "save_cache_only_world_state_snapshot",
            &cache_only_save_source,
        )
        .test_file("rich_save_world_state_snapshot", &rich_save_source)
        .test_file("rich_load_world_state_snapshot", &rich_load_source)
        .test_file(
            "rich_same_run_restore_world_state_snapshot",
            &rich_same_run_restore_source,
        )
        .test_file(
            "replace_world_state_snapshot_with_transient_state",
            &replace_with_transient_source,
        )
        .test_file(
            "failed_load_preserves_world_state_snapshot",
            &failed_load_preserves_state_source,
        )
        .test_file("save_world_state_snapshot_failures", &save_failure_source)
        .raw_file("broken-world-state.json", "{ not valid json")
        .build()
}

fn read_world_state_snapshot(path: &std::path::Path) -> WorldStateSnapshot {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read snapshot {}: {err}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|err| panic!("failed to parse snapshot {}: {err}", path.display()))
}

fn write_world_state_snapshot(path: &std::path::Path, snapshot: &WorldStateSnapshot) {
    std::fs::write(
        path,
        serde_json::to_string_pretty(snapshot).expect("failed to serialize snapshot"),
    )
    .unwrap_or_else(|err| panic!("failed to write snapshot {}: {err}", path.display()));
}

fn run_snapshot_test(project: &Project, test_file_stem: &str, snapshot_name: &str) {
    let test_path = format!("tests/{test_file_stem}.test.tolk");
    let snapshot_path = format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt");

    project
        .acton()
        .test()
        .path(&test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(&snapshot_path);
}

fn to_tolk_string_literal(value: &str) -> String {
    serde_json::to_string(value).expect("string literal must serialize")
}

#[test]
fn world_state_snapshot_can_be_saved_and_loaded_across_test_runs() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-save-and-load-across-runs");

    run_snapshot_test(
        &project,
        "save_world_state_snapshot",
        "save_world_state_snapshot",
    );

    let snapshot_path = project.path().join("world-state.json");
    assert!(
        snapshot_path.exists(),
        "snapshot file should be created on disk"
    );

    let snapshot_content =
        std::fs::read_to_string(&snapshot_path).expect("failed to read saved snapshot");
    let snapshot: WorldStateSnapshot =
        serde_json::from_str(&snapshot_content).expect("saved snapshot should be valid JSON");
    assert_eq!(snapshot.current_now, 1700023001);
    assert!(
        !snapshot.accounts.is_empty(),
        "snapshot should contain at least one materialized account"
    );

    run_snapshot_test(
        &project,
        "load_world_state_snapshot",
        "load_world_state_snapshot",
    );
}

#[test]
fn world_state_snapshot_load_replaces_current_runner_state_and_rejects_invalid_inputs() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-replace-and-invalid-inputs");
    std::fs::create_dir_all(project.path().join("fixtures")).expect("failed to create fixtures");

    run_snapshot_test(
        &project,
        "save_world_state_snapshot",
        "save_world_state_snapshot",
    );

    let mut unsupported_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    unsupported_snapshot.version += 1;
    write_world_state_snapshot(
        &project.path().join("unsupported-world-state.json"),
        &unsupported_snapshot,
    );

    let mut invalid_address_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    invalid_address_snapshot.accounts[0].address = "not-an-address".to_owned();
    write_world_state_snapshot(
        &project.path().join("invalid-address-world-state.json"),
        &invalid_address_snapshot,
    );

    let mut invalid_config_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    invalid_config_snapshot.config_boc64 = "not-base64".to_owned();
    write_world_state_snapshot(
        &project.path().join("invalid-config-world-state.json"),
        &invalid_config_snapshot,
    );

    let mut invalid_library_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    invalid_library_snapshot.libraries_boc64 = vec!["not-base64".to_owned()];
    write_world_state_snapshot(
        &project.path().join("invalid-library-world-state.json"),
        &invalid_library_snapshot,
    );

    let mut invalid_random_seed_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    invalid_random_seed_snapshot.random_seed = Some("42".to_owned());
    write_world_state_snapshot(
        &project.path().join("invalid-random-seed-world-state.json"),
        &invalid_random_seed_snapshot,
    );

    let mut duplicate_accounts_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    let duplicate_entry = duplicate_accounts_snapshot.accounts[0].clone();
    duplicate_accounts_snapshot.accounts.push(duplicate_entry);
    write_world_state_snapshot(
        &project.path().join("duplicate-accounts-world-state.json"),
        &duplicate_accounts_snapshot,
    );

    run_snapshot_test(
        &project,
        "replace_world_state_snapshot",
        "replace_world_state_snapshot",
    );

    run_snapshot_test(
        &project,
        "invalid_world_state_snapshot",
        "invalid_world_state_snapshot",
    );
}

#[test]
fn world_state_snapshot_empty_state_can_be_saved_and_loaded() {
    let project = build_world_state_snapshot_project("r-lib-world-state-snapshot-empty-state");

    run_snapshot_test(
        &project,
        "save_empty_world_state_snapshot",
        "save_empty_world_state_snapshot",
    );

    let empty_snapshot = read_world_state_snapshot(&project.path().join("empty-world-state.json"));
    assert_eq!(empty_snapshot.current_lt, 0);
    assert_eq!(empty_snapshot.current_now, 0);
    assert!(empty_snapshot.accounts.is_empty());
    assert!(empty_snapshot.libraries_boc64.is_empty());

    run_snapshot_test(
        &project,
        "load_empty_world_state_snapshot",
        "load_empty_world_state_snapshot",
    );
}

#[test]
fn world_state_snapshot_skips_cached_non_existing_accounts() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-skips-cached-misses");

    run_snapshot_test(
        &project,
        "save_cache_only_world_state_snapshot",
        "save_cache_only_world_state_snapshot",
    );

    let snapshot = read_world_state_snapshot(&project.path().join("cache-only-world-state.json"));
    assert!(
        snapshot.accounts.is_empty(),
        "snapshot should ignore cache-only non-existing accounts"
    );
}

#[test]
fn world_state_snapshot_roundtrip_preserves_config_libraries_and_logical_time() {
    let project = build_world_state_snapshot_project("r-lib-world-state-snapshot-rich-roundtrip");
    std::fs::create_dir_all(project.path().join("fixtures")).expect("failed to create fixtures");

    run_snapshot_test(
        &project,
        "rich_save_world_state_snapshot",
        "rich_save_world_state_snapshot",
    );

    let rich_snapshot_path = project
        .path()
        .join("fixtures")
        .join("rich-world-state.json");
    let rich_snapshot = read_world_state_snapshot(&rich_snapshot_path);
    assert_eq!(rich_snapshot.current_now, 1700023999);
    assert!(
        rich_snapshot.current_lt > 0,
        "rich snapshot should capture non-zero logical time after transactions"
    );
    assert_eq!(
        rich_snapshot.libraries_boc64.len(),
        1,
        "rich snapshot should persist registered libraries"
    );

    run_snapshot_test(
        &project,
        "rich_load_world_state_snapshot",
        "rich_load_world_state_snapshot",
    );

    let rich_roundtrip_snapshot = read_world_state_snapshot(
        &project
            .path()
            .join("fixtures")
            .join("rich-world-state-roundtrip.json"),
    );
    assert_eq!(
        rich_roundtrip_snapshot, rich_snapshot,
        "loading and re-saving rich state should be lossless"
    );
}

#[test]
fn world_state_snapshot_same_run_restore_is_lossless_for_rich_state() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-same-run-rich-restore");
    std::fs::create_dir_all(project.path().join("fixtures")).expect("failed to create fixtures");

    run_snapshot_test(
        &project,
        "rich_same_run_restore_world_state_snapshot",
        "rich_same_run_restore_world_state_snapshot",
    );

    let before_snapshot =
        read_world_state_snapshot(&project.path().join("fixtures").join("same-run-before.json"));
    let after_snapshot =
        read_world_state_snapshot(&project.path().join("fixtures").join("same-run-after.json"));
    assert_eq!(after_snapshot, before_snapshot);
}

#[test]
fn world_state_snapshot_load_replaces_transient_accounts_with_exact_saved_state() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-replaces-transient-state");
    std::fs::create_dir_all(project.path().join("fixtures")).expect("failed to create fixtures");

    run_snapshot_test(
        &project,
        "save_world_state_snapshot",
        "save_world_state_snapshot",
    );

    let original_snapshot = read_world_state_snapshot(&project.path().join("world-state.json"));

    run_snapshot_test(
        &project,
        "replace_world_state_snapshot_with_transient_state",
        "replace_world_state_snapshot_with_transient_state",
    );

    let transient_snapshot = read_world_state_snapshot(
        &project
            .path()
            .join("fixtures")
            .join("transient-before-load.json"),
    );
    assert_ne!(
        transient_snapshot, original_snapshot,
        "transient pre-load state should differ from the saved baseline"
    );

    let restored_snapshot = read_world_state_snapshot(
        &project
            .path()
            .join("fixtures")
            .join("after-load-world-state.json"),
    );
    assert_eq!(
        restored_snapshot, original_snapshot,
        "loadSnapshot must replace existing state instead of merging into it"
    );
}

#[test]
fn world_state_snapshot_failed_loads_do_not_poison_existing_runner_state() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-failed-load-keeps-state");

    run_snapshot_test(
        &project,
        "save_world_state_snapshot",
        "save_world_state_snapshot",
    );

    let mut invalid_config_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    invalid_config_snapshot.config_boc64 = "not-base64".to_owned();
    write_world_state_snapshot(
        &project.path().join("invalid-config-world-state.json"),
        &invalid_config_snapshot,
    );

    let mut duplicate_accounts_snapshot =
        read_world_state_snapshot(&project.path().join("world-state.json"));
    let duplicate_entry = duplicate_accounts_snapshot.accounts[0].clone();
    duplicate_accounts_snapshot.accounts.push(duplicate_entry);
    write_world_state_snapshot(
        &project.path().join("duplicate-accounts-world-state.json"),
        &duplicate_accounts_snapshot,
    );

    run_snapshot_test(
        &project,
        "failed_load_preserves_world_state_snapshot",
        "failed_load_preserves_world_state_snapshot",
    );
}

#[test]
fn world_state_snapshot_save_reports_path_failures_without_poisoning_future_saves() {
    let project =
        build_world_state_snapshot_project("r-lib-world-state-snapshot-save-path-failures");
    std::fs::create_dir_all(project.path().join("fixtures")).expect("failed to create fixtures");

    run_snapshot_test(
        &project,
        "save_world_state_snapshot_failures",
        "save_world_state_snapshot_failures",
    );

    let valid_snapshot =
        read_world_state_snapshot(&project.path().join("fixtures").join("ok-world-state.json"));
    assert_eq!(valid_snapshot.version, 1);
    assert!(
        !project
            .path()
            .join("missing-dir")
            .join("world-state.json")
            .exists(),
        "failed save should not create files in missing directories"
    );
}

#[test]
fn world_state_snapshot_helpers_reject_absolute_and_parent_escape_paths() {
    let absolute_outside = tempfile::NamedTempFile::new().expect("failed to create outside file");
    let outside_snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 0,
        current_now: 123,
        random_seed: None,
        config_boc64: DEFAULT_CONFIG.to_owned(),
        libraries_boc64: Vec::new(),
        accounts: Vec::new(),
    };
    write_world_state_snapshot(absolute_outside.path(), &outside_snapshot);
    let absolute_outside_literal =
        to_tolk_string_literal(&absolute_outside.path().to_string_lossy());

    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test world state snapshot rejects path escapes`() {{
    val absoluteOutside = {absolute_outside_literal};

    testing.setNow(321);

    expect(testing.loadSnapshot("../outside-valid-world-state.json")).toBeFalse();
    expect(testing.loadSnapshot(absoluteOutside)).toBeFalse();
    expect(testing.getNow()).toEqual(321);

    expect(testing.saveSnapshot("../escaped-world-state.json")).toBeFalse();
    expect(testing.saveSnapshot(absoluteOutside)).toBeFalse();
}}
"#
    );

    let project = ProjectBuilder::new("r-lib-world-state-snapshot-rejects-path-escapes")
        .test_file("snapshot_path_sandbox", &source)
        .build();

    let project_parent = project
        .path()
        .parent()
        .expect("project must have parent directory");
    let parent_snapshot_path = project_parent.join("outside-valid-world-state.json");
    write_world_state_snapshot(&parent_snapshot_path, &outside_snapshot);

    run_snapshot_test(
        &project,
        "snapshot_path_sandbox",
        "world_state_snapshot_helpers_reject_absolute_and_parent_escape_paths",
    );

    let absolute_snapshot = read_world_state_snapshot(absolute_outside.path());
    assert_eq!(absolute_snapshot.current_now, 123);
    let parent_snapshot = read_world_state_snapshot(&parent_snapshot_path);
    assert_eq!(parent_snapshot.current_now, 123);
    assert!(!project_parent.join("escaped-world-state.json").exists());
}
