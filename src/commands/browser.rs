#[cfg(target_os = "macos")]
static OPEN_CHROME_SCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/open_chrome.applescript"
));

/// Opens a local web interface and reuses its existing Chromium tab on macOS.
///
/// Keeping this behavior in the CLI makes every embedded Acton interface use
/// the same tab discovery rules and preserves the current page when possible.
pub(crate) fn open_browser(url: &str) {
    if std::env::var_os("ACTON_INTERNAL_SKIP_BROWSER").is_some() {
        return;
    }

    #[cfg(target_os = "macos")]
    {
        let chromium_browsers = [
            "Google Chrome",
            "Arc",
            "Brave Browser",
            "Microsoft Edge",
            "Vivaldi",
        ];

        for browser in chromium_browsers {
            if !is_process_running(browser) {
                continue;
            }

            let child = std::process::Command::new("osascript")
                .arg("-")
                .arg(url)
                .arg(browser)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok();

            if let Some(mut child) = child {
                use std::io::Write;

                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(OPEN_CHROME_SCRIPT.as_bytes());
                }

                let status = child.wait().ok();
                if status.is_some_and(|status| status.success()) {
                    return;
                }
            }
        }
    }

    if let Err(error) = opener::open(url) {
        eprintln!("Warning: Failed to open browser: {error}");
    }
}

#[cfg(target_os = "macos")]
fn is_process_running(process_name: &str) -> bool {
    let output = std::process::Command::new("ps").arg("-cax").output().ok();

    let Some(output) = output else {
        return false;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().any(|line| line.contains(process_name))
}
