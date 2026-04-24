use acton_config::schema::ACTON_SCHEMA_JSON;
use anyhow::Result;
use std::io::{self, Write};

pub fn print_schema_cmd() -> Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(ACTON_SCHEMA_JSON.as_bytes())?;
    stdout.flush()?;
    Ok(())
}
