use crate::commands::common::error_fmt;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::anyhow;
use std::process::{Command, Stdio};

pub fn run_cmd(script_name: &str, extra_args: &[String]) -> anyhow::Result<()> {
    let config = ActonConfig::load()?;

    let scripts = config
        .scripts
        .as_ref()
        .ok_or_else(|| anyhow!(error_fmt::no_scripts_section()))?;

    let script_command = scripts
        .get(script_name)
        .ok_or_else(|| anyhow!(error_fmt::script_not_found(&config, script_name)))?
        .clone();

    #[cfg(not(target_os = "windows"))]
    {
        let display = if extra_args.is_empty() {
            script_command.clone()
        } else {
            format!(
                "{} {}",
                script_command,
                extra_args
                    .iter()
                    .map(|a| shell_escape_posix(a))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        println!("{}", display.bold());

        let cmdline = if extra_args.is_empty() {
            script_command
        } else {
            format!(r#"{script_command} "$@""#)
        };

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(cmdline)
            .arg("--")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        for a in extra_args {
            cmd.arg(a);
        }

        let status = cmd
            .status()
            .map_err(|e| anyhow!("Failed to execute script '{script_name}': {e}"))?;

        if !status.success() {
            let code = status.code().unwrap_or(1);
            return Err(anyhow!(
                "Script '{script_name}' failed with exit code {code}"
            ));
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let display = if extra_args.is_empty() {
            script_command.clone()
        } else {
            format!(
                "{} {}",
                script_command,
                extra_args
                    .iter()
                    .map(|a| quote_cmd_arg(a))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        println!("{}", display.bold());

        let full_line = if extra_args.is_empty() {
            script_command
        } else {
            format!(
                "{} {}",
                script_command,
                extra_args
                    .iter()
                    .map(|a| quote_cmd_arg(a))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        let status = Command::new("cmd.exe")
            .arg("/D") // disable AutoRun
            .arg("/V:OFF") // disable delayed expansion (!VAR!)
            .arg("/E:OFF") // disable cmd extensions
            .arg("/S") // correctly process quotes
            .arg("/C")
            .arg(full_line)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| anyhow!("Failed to execute script '{}': {}", script_name, e))?;

        if !status.success() {
            let code = status.code().unwrap_or(1);
            return Err(anyhow!(
                "Script '{}' failed with exit code {}",
                script_name,
                code
            ));
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn shell_escape_posix(s: &str) -> String {
    if s.is_empty() {
        "''".to_string()
    } else if s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(target_os = "windows")]
fn quote_cmd_arg(s: &str) -> String {
    if s.is_empty() {
        return r#""""#.to_string();
    }

    let mut out = s.replace('%', "%%");
    out = out.replace('"', r#"^""#);

    let needs_quotes = out
        .chars()
        .any(|c| c.is_whitespace() || "&|<>^()".contains(c));

    if needs_quotes {
        format!(r#""{}""#, out)
    } else {
        out
    }
}
