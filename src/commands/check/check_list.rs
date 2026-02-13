pub(super) fn check_list_cmd() -> anyhow::Result<()> {
    let rules: Vec<_> = tolk_linter::Linter::Tolk
        .all_rules()
        .map(|r| {
            serde_json::json!({
                "name": r.name(),
                "description": r.explanation().unwrap_or_default(),
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&rules)?);
    Ok(())
}
