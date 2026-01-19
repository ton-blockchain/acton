#![cfg(test)]

use anyhow::Ok;

use crate::WorldState;

#[test]
fn test_get_config() -> anyhow::Result<()> {
    let state = WorldState::new(
        crate::AccountsState::Local(crate::LocalAccountsState::new()),
        None,
    )?;

    let config = state.get_config();
    let version = config.get(8).expect("No version").expect("Has value");
    assert!(
        version
            .as_slice()
            .expect("Version cell corrupted")
            .load_u32()?
            >= 12
    );

    let root = config.root().clone().expect("Config has no root");
    assert!(!root.repr_hash().is_zero());

    Ok(())
}
