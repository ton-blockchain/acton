use std::str::FromStr;
use tolk_linter::Tolk;

pub(super) fn check_explain_cmd(code: &String) -> anyhow::Result<()> {
    if let Ok(tolk_rules) = Tolk::from_str(code)
        && let Some(rule) = tolk_rules.rules().next()
    {
        if let Some(explanation) = rule.explanation() {
            println!("{explanation}");
        } else {
            println!("No explanation available for rule {code}");
        }
    } else {
        anyhow::bail!("Unknown rule code: {code}");
    }
    Ok(())
}
