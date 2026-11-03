use super::*;
use expect_test::expect;

#[test]
fn genesis_file_is_resolved_once_and_cannot_change_after_bootstrap() {
    let directory = tempfile::tempdir_in("/tmp").unwrap();
    let layout = Layout::new(directory.path().join("state"));
    layout.create_dirs().unwrap();
    let path = directory.path().join("genesis.toml");
    std::fs::write(
        &path,
        "global_id = -42\nvalidator_stake = 20000\nelected_for_seconds = 60\n[storage]\ncell_price = 123",
    )
    .unwrap();
    let args = BootstrapArgs {
        genesis: Some(path.clone()),
        block_time: Some(800),
        election_time: Some(40),
        ..Default::default()
    };
    let settings = prepare_settings(&layout, &args).unwrap();
    std::fs::write(&layout.manifest, "committed").unwrap();
    let resumed = prepare_settings(&layout, &BootstrapArgs::default()).unwrap();
    expect![["true -42 123 800 40 30 20000000000000"]].assert_eq(&format!(
        "{} {} {} {} {} {} {}",
        settings == resumed,
        resumed.network.global_id,
        resumed.network.storage.cell_price,
        resumed.network.simplex_target_rate_ms,
        resumed.network.elected_for_seconds,
        resumed.network.election_start_before_seconds,
        resumed.node.validator_stake_nano,
    ));
    let same = BootstrapArgs {
        genesis_json: Some(serde_json::to_string(&resumed.network).unwrap()),
        ..Default::default()
    };
    expect![["true"]].assert_eq(&prepare_settings(&layout, &same).is_ok().to_string());
    std::fs::write(&path, "global_id = -43").unwrap();
    let error = prepare_settings(&layout, &args).unwrap_err();
    expect![["genesis cannot change after network creation; create a new state directory"]]
        .assert_eq(&error.to_string());
    expect![["true"]]
        .assert_eq(&(Settings::load(&layout.settings).unwrap() == resumed).to_string());
}
