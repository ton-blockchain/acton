use crate::commands::common::error_fmt;
use crate::config::ActonConfig;
use anyhow::anyhow;
use owo_colors::OwoColorize;
use std::process::{Command, Stdio};

pub fn run_cmd(script_name: &str, extra_args: &[String]) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let scripts = config
        .scripts
        .as_ref()
        .ok_or_else(|| anyhow!(error_fmt::no_scripts_section()))?;

    let script_command = scripts
        .get(script_name)
        .ok_or_else(|| anyhow!(error_fmt::script_not_found(&config, script_name)))?;

    let mut full_command = script_command.clone();
    if !extra_args.is_empty() {
        full_command.push(' ');
        full_command.push_str(&extra_args.join(" "));
    }

    println!("{}", full_command.bold());

    #[cfg(target_os = "windows")]
    let (shell, flag) = ("cmd", "/c");
    #[cfg(not(target_os = "windows"))]
    let (shell, flag) = ("/bin/sh", "-c");

    let status = Command::new(shell)
        .arg(flag)
        .arg(&full_command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    match status {
        Ok(child) => {
            let output = child.wait_with_output()?;
            let status = output.status;
            if status.success() {
                return Ok(());
            }

            if let Some(code) = status.code() {
                std::process::exit(code);
            } else {
                Err(anyhow!("Script '{}' terminated by signal", script_name))
            }
        }
        Err(e) => Err(anyhow!("Failed to execute script '{}': {}", script_name, e)),
    }
}
