//! Shutdown presentation observes Studio state without owning network cleanup.

use acton_config::color::OwoColorize;
use acton_studio::{EnvironmentRuntime, EnvironmentStatus, LocalProcessEnvironmentRuntime};
use indicatif::{ProgressBar, ProgressStyle};
use std::{io::IsTerminal, time::Duration};
use tokio::{sync::oneshot, time::Instant};

/// Keep polling while the server drains requests, closes environments and reaps tests.
/// Only the server future can confirm that the entire shutdown has completed.
pub(super) async fn wait<E: Into<anyhow::Error>>(
    runtime: &LocalProcessEnvironmentRuntime,
    work: impl Future<Output = Result<(), E>>,
    requested: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    tokio::pin!(work);
    tokio::select! {
        biased;
        _ = requested => {}
        result = &mut work => return result.map_err(Into::into),
    }

    // The terminal echoes ^C without a newline. Keep the twelve-column verb
    // aligned with other Acton commands even when shutdown starts from Ctrl-C.
    eprintln!(
        "\n{} Acton Studio gracefully (shutdown requested)",
        label("Stopping", true)
    );
    let started = Instant::now();
    let mut progress = Progress::new();
    let mut interval = tokio::time::interval(Duration::from_millis(250));

    let result = loop {
        tokio::select! {
            result = &mut work => break result.map_err(Into::into),
            _ = interval.tick() => {
                if let Ok(environments) = runtime.list().await {
                    let total = environments.len() as u64;
                    let stopped = environments.iter()
                        .filter(|environment| environment.status == EnvironmentStatus::Stopped)
                        .count() as u64;
                    let current = environments.iter()
                        .find(|environment| environment.status == EnvironmentStatus::Stopping)
                        .or_else(|| environments.iter().find(|environment| {
                            !matches!(environment.status, EnvironmentStatus::Stopped | EnvironmentStatus::Failed)
                        }));

                    if let Some(current) = current {
                        progress.update(
                            "Stopping",
                            &format!("environment \"{}\" — {stopped}/{total} environments stopped", current.name),
                            stopped,
                            total,
                        );
                    } else {
                        progress.update("Finishing", "Studio connections and test processes", stopped, 0);
                    }
                }
            }
        }
    };
    drop(progress);

    match &result {
        Ok(()) => eprintln!(
            "{} Acton Studio gracefully in {:.1}s",
            label("Stopped", false),
            started.elapsed().as_secs_f64()
        ),
        Err(_) => eprintln!(
            "{} to stop Acton Studio gracefully after {:.1}s",
            label("Failed", true),
            started.elapsed().as_secs_f64()
        ),
    }
    result
}

fn label(verb: &str, warning: bool) -> String {
    let padded = format!("{verb:>12}");
    if warning {
        padded.yellow().bold().to_string()
    } else {
        padded.green().bold().to_string()
    }
}

struct Progress {
    bar: ProgressBar,
    terminal: bool,
    last_message: String,
}

impl Progress {
    fn new() -> Self {
        let terminal = std::io::stderr().is_terminal();
        let bar = if terminal {
            ProgressBar::new_spinner()
        } else {
            ProgressBar::hidden()
        };
        bar.enable_steady_tick(Duration::from_millis(100));
        Self {
            bar,
            terminal,
            last_message: String::new(),
        }
    }

    fn update(&mut self, verb: &str, message: &str, stopped: u64, total: u64) {
        if self.last_message == message {
            return;
        }
        message.clone_into(&mut self.last_message);

        if !self.terminal {
            eprintln!("{} {message}", label(verb, true));
            return;
        }

        let template = if total > 0 {
            "{prefix} [{bar:16.green/white}] {wide_msg} ({elapsed})"
        } else {
            "{prefix} {spinner:.green} {wide_msg} ({elapsed})"
        };
        self.bar.set_style(
            ProgressStyle::with_template(template)
                .expect("static Studio progress template")
                .progress_chars("=> "),
        );
        self.bar.set_prefix(label(verb, true));
        self.bar.set_length(total);
        self.bar.set_position(stopped);
        self.bar.set_message(message.to_owned());
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.bar.finish_and_clear();
    }
}
