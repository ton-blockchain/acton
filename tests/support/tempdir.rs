use tempfile::TempDir;

pub(crate) fn create_tmp_dir() -> TempDir {
    let update_snapshots = std::env::var("DISABLE_TMP_DIR_CLEANUP_IN_TESTS").is_ok();

    let mut temp_dir = TempDir::with_prefix("acton-").expect("Failed to create temp dir");
    temp_dir.disable_cleanup(update_snapshots);
    temp_dir
}
