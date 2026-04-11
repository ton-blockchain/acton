use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use ton_emulator::WorldStateSnapshot;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
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
    val target = net.randomAddress("snapshot-target");

    expect(net.balance(target)).toEqual(0);
    net.topUp(target, ton("3"));
    net.setNow(1700023001);

    expect(net.saveSnapshot("world-state.json")).toBeTrue();
}}
"#
    );

    let load_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot from disk`() {{
    val target = net.randomAddress("snapshot-target");

    expect(net.balance(target)).toEqual(0);
    expect(net.now()).toEqual(0);

    expect(net.loadSnapshot("world-state.json")).toBeTrue();

    expect(net.now()).toEqual(1700023001);
    expect(net.balance(target)).toEqual(ton("3"));
    expect(net.getShardAccount(target)).toBeNotNull();
    expect(net.saveSnapshot("roundtrip-world-state.json")).toBeTrue();
}}
"#
    );

    let replace_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot replaces current state`() {{
    val target = net.randomAddress("snapshot-target");

    net.topUp(target, ton("1"));
    net.setNow(99);
    expect(net.balance(target)).toEqual(ton("1"));
    expect(net.now()).toEqual(99);

    expect(net.loadSnapshot("world-state.json")).toBeTrue();

    expect(net.balance(target)).toEqual(ton("3"));
    expect(net.now()).toEqual(1700023001);
}}
"#
    );

    let invalid_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot invalid inputs`() {{
    expect(net.loadSnapshot("missing-world-state.json")).toBeFalse();
    expect(net.loadSnapshot("broken-world-state.json")).toBeFalse();
    expect(net.loadSnapshot("unsupported-world-state.json")).toBeFalse();
    expect(net.loadSnapshot("invalid-address-world-state.json")).toBeFalse();
    expect(net.loadSnapshot("invalid-config-world-state.json")).toBeFalse();
    expect(net.loadSnapshot("invalid-library-world-state.json")).toBeFalse();
    expect(net.loadSnapshot("duplicate-accounts-world-state.json")).toBeFalse();
}}
"#
    );

    let empty_save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save empty world state snapshot`() {{
    expect(net.now()).toEqual(0);
    expect(net.saveSnapshot("empty-world-state.json")).toBeTrue();
}}
"#
    );

    let empty_load_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load empty world state snapshot`() {{
    val target = net.randomAddress("snapshot-empty-target");

    net.topUp(target, ton("1"));
    net.setNow(77);

    expect(net.loadSnapshot("empty-world-state.json")).toBeTrue();

    expect(net.now()).toEqual(0);
    expect(net.balance(target)).toEqual(0);
}}
"#
    );

    let cache_only_save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot skips cached non existing accounts`() {{
    val target = net.randomAddress("snapshot-cache-only-target");

    expect(net.balance(target)).toEqual(0);
    expect(net.isDeployed(target)).toBeFalse();
    expect(net.saveSnapshot("cache-only-world-state.json")).toBeTrue();
}}
"#
    );

    let rich_save_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot rich state`() {{
    val primary = net.randomAddress("snapshot-rich-primary");
    val secondary = net.randomAddress("snapshot-rich-secondary");

    net.topUp(primary, ton("3"));
    net.topUp(secondary, ton("5"));
    net.setNow(1700023999);

    val libraryCode = build("simple");
    vm.registerLibrary(libraryCode);

    var config = net.getConfig();
    val targetVersion = GlobalVersion {{
        version: 424244,
        capabilities: 1099511640122,
    }};
    config.setGlobalVersion(targetVersion);
    expect(net.setConfig(config)).toBeTrue();

    expect(net.saveSnapshot("fixtures/../fixtures/rich-world-state.json")).toBeTrue();
}}
"#
    );

    let rich_load_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot rich state`() {{
    val primary = net.randomAddress("snapshot-rich-primary");
    val secondary = net.randomAddress("snapshot-rich-secondary");

    expect(net.loadSnapshot("./fixtures/rich-world-state.json")).toBeTrue();

    expect(net.now()).toEqual(1700023999);
    expect(net.balance(primary)).toEqual(ton("3"));
    expect(net.balance(secondary)).toEqual(ton("5"));

    val version = net.getConfig().getGlobalVersion();
    expect(version.version).toEqual(424244);
    expect(version.capabilities).toEqual(1099511640122);

    expect(net.saveSnapshot("fixtures/rich-world-state-roundtrip.json")).toBeTrue();
}}
"#
    );

    let rich_same_run_restore_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save mutate load world state snapshot in same run`() {{
    val primary = net.randomAddress("snapshot-same-run-primary");
    val secondary = net.randomAddress("snapshot-same-run-secondary");

    net.topUp(primary, ton("8"));
    net.topUp(secondary, ton("13"));
    net.setNow(1700024555);

    val libraryCode = build("simple");
    vm.registerLibrary(libraryCode);

    var config = net.getConfig();
    config.setGlobalVersion(GlobalVersion {{
        version: 515151,
        capabilities: 2222222222,
    }});
    expect(net.setConfig(config)).toBeTrue();

    expect(net.saveSnapshot("fixtures/same-run-before.json")).toBeTrue();

    net.setNow(1);

    var mutated = net.getConfig();
    mutated.setGlobalVersion(GlobalVersion {{
        version: 1,
        capabilities: 1,
    }});
    expect(net.setConfig(mutated)).toBeTrue();

    expect(net.loadSnapshot("fixtures/same-run-before.json")).toBeTrue();
    expect(net.now()).toEqual(1700024555);
    expect(net.balance(primary)).toEqual(ton("8"));
    expect(net.balance(secondary)).toEqual(ton("13"));

    val version = net.getConfig().getGlobalVersion();
    expect(version.version).toEqual(515151);
    expect(version.capabilities).toEqual(2222222222);

    expect(net.saveSnapshot("fixtures/same-run-after.json")).toBeTrue();
}}
"#
    );

    let replace_with_transient_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot drops transient state`() {{
    val transient = net.randomAddress("snapshot-transient-target");

    net.topUp(transient, ton("9"));
    net.setNow(777);
    expect(net.saveSnapshot("fixtures/transient-before-load.json")).toBeTrue();

    expect(net.loadSnapshot("fixtures/../world-state.json")).toBeTrue();
    expect(net.saveSnapshot("fixtures/after-load-world-state.json")).toBeTrue();
}}
"#
    );

    let failed_load_preserves_state_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test load world state snapshot failure keeps current state`() {{
    val target = net.randomAddress("snapshot-preserve-target");

    net.topUp(target, ton("4"));
    net.setNow(404);

    expect(net.loadSnapshot("duplicate-accounts-world-state.json")).toBeFalse();
    expect(net.balance(target)).toEqual(ton("4"));
    expect(net.now()).toEqual(404);

    expect(net.loadSnapshot("invalid-config-world-state.json")).toBeFalse();
    expect(net.balance(target)).toEqual(ton("4"));
    expect(net.now()).toEqual(404);
}}
"#
    );

    let save_failure_source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test save world state snapshot path failures`() {{
    expect(net.saveSnapshot("missing-dir/world-state.json")).toBeFalse();
    expect(net.saveSnapshot("fixtures")).toBeFalse();
    expect(net.saveSnapshot("fixtures/ok-world-state.json")).toBeTrue();
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
