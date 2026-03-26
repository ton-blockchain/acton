use anyhow::Result;
use flate2::read::GzDecoder;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::process::Command;
use tar::Archive;
use tempfile::Builder;

const COMPRESSED_MAN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/man.tgz"));

pub fn print_command_manual(command: &str) -> Result<bool> {
    if try_render_with_pager(command)? {
        return Ok(true);
    }

    let Some(plain_text) = extract_manual(command, "txt") else {
        return Ok(false);
    };

    let mut stdout = io::stdout().lock();
    stdout.write_all(&plain_text)?;
    Ok(true)
}

fn extract_manual(command: &str, extension: &str) -> Option<Vec<u8>> {
    let file_name = format!("acton-{command}.{extension}");
    let gz = GzDecoder::new(COMPRESSED_MAN);
    let mut archive = Archive::new(gz);

    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        let path = entry.path().ok()?;
        if path.file_name()?.to_str()? != file_name {
            continue;
        }

        let mut result = Vec::new();
        entry.read_to_end(&mut result).ok()?;
        return Some(result);
    }

    None
}

fn try_render_with_pager(command: &str) -> Result<bool> {
    if !io::stdout().is_terminal() {
        return Ok(false);
    }

    if let Some(man_page) = extract_manual(command, "1")
        && write_and_spawn(command, &man_page, "man")?
    {
        return Ok(true);
    }

    if let Some(text_page) = extract_manual(command, "txt") {
        if write_and_spawn(command, &text_page, "less")? {
            return Ok(true);
        }
        if write_and_spawn(command, &text_page, "more")? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn write_and_spawn(name: &str, contents: &[u8], command: &str) -> Result<bool> {
    let prefix = format!("acton-{name}.");
    let mut temp_file = Builder::new().prefix(&prefix).tempfile()?;
    let file = temp_file.as_file_mut();
    file.write_all(contents)?;
    file.flush()?;

    let path = temp_file.path();
    let relative_name = Path::new(".").join(path.file_name().expect("temp manual file name"));
    let status = Command::new(command)
        .arg(relative_name)
        .current_dir(path.parent().expect("temp manual directory"))
        .status();

    match status {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
