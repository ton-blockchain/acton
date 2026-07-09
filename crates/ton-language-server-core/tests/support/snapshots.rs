use std::fs;
use std::path::Path;

pub(crate) fn assert_file_snapshot(path: &str, actual: &str) -> anyhow::Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(path);
    let update_snapshots = std::env::var("UPDATE_SNAPSHOTS").is_ok_and(|value| !value.is_empty());
    if update_snapshots {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, actual)?;
    } else {
        let expected = fs::read_to_string(&path)?;
        assert_eq!(actual, expected, "snapshot mismatch for {}", path.display());
    }
    Ok(())
}
