//! Terminal activity is transient; redirected output keeps concrete changes only.

use acton_config::color::OwoColorize;
use acton_localnet::{Operation, OperationProgress, OperationStep};
use indicatif::{ProgressBar, ProgressStyle};
use std::{io::IsTerminal, time::Duration};

pub(super) fn label(word: &str, warning: bool) -> String {
    // Pad before applying ANSI colors so the visible verb column is exactly 12.
    let padded = format!("{word:>12}");
    if warning {
        padded.yellow().bold().to_string()
    } else {
        padded.green().bold().to_string()
    }
}

/// Clearing on drop also handles Ctrl-C cancelling an in-flight polling future.
pub(super) struct Activity {
    bar: ProgressBar,
    json: bool,
    terminal: bool,
    last_line: String,
    determinate: Option<bool>,
}

impl Activity {
    pub(super) fn new(json: bool) -> Self {
        let terminal = std::io::stderr().is_terminal();
        let bar = if json || !terminal {
            ProgressBar::hidden()
        } else {
            ProgressBar::new_spinner()
        };
        bar.enable_steady_tick(Duration::from_millis(100));

        Self {
            bar,
            json,
            terminal,
            last_line: String::new(),
            determinate: None,
        }
    }

    pub(super) fn update(
        &mut self,
        verb: &str,
        message: &str,
        progress: Option<&OperationProgress>,
    ) {
        if self.json {
            return;
        }

        let warning = matches!(verb, "Stopping" | "Removing" | "Deleting");
        let prefix = format!("{} {message}", label(verb, warning));
        let detail = progress.map_or_else(String::new, |progress| {
            let count = progress.total.map_or_else(
                || progress.completed.to_string(),
                |total| format!("{}/{total}", progress.completed),
            );
            format!("{count} {} — {}", progress.unit, progress.detail)
        });

        if !self.terminal {
            let line = if detail.is_empty() {
                prefix
            } else {
                format!("{prefix}: {detail}")
            };
            if self.last_line != line {
                eprintln!("{line}");
                self.last_line = line;
            }
            return;
        }

        let total = progress
            .and_then(|progress| progress.total)
            .filter(|total| *total > 0);
        let determinate = total.is_some();
        if self.determinate != Some(determinate) {
            let template = if determinate {
                "{prefix} [{bar:20.green/white}] {wide_msg} ({elapsed})"
            } else {
                "{prefix} {spinner:.green} {wide_msg} ({elapsed})"
            };
            self.bar.set_style(
                ProgressStyle::with_template(template)
                    .expect("static progress template")
                    .progress_chars("=> "),
            );
            self.determinate = Some(determinate);
        }

        if let (Some(total), Some(progress)) = (total, progress) {
            self.bar.set_length(total);
            self.bar.set_position(progress.completed.min(total));
        }

        self.bar.set_prefix(prefix);
        self.bar.set_message(detail);
    }

    pub(super) fn operation(&mut self, operation: &Operation) {
        let (verb, message) = match operation.phase.as_str() {
            "checkingImage" => ("Checking", "Localton Docker image"),
            "pullingImage" => ("Pulling", "Localton Docker image"),
            "startingContainers" => ("Starting", "Docker services"),
            "waitingForApis" => ("Checking", "TON APIs and indexer readiness"),
            "stopping" => ("Stopping", "Docker services gracefully"),
            "creatingArchive" => ("Saving", "the network snapshot archive"),
            "restoringState" => ("Restoring", "blockchain state from the snapshot"),
            "resettingIndexer" => ("Resetting", "derived indexer data"),
            "deletingArchive" => ("Deleting", "the snapshot archive"),
            "joiningNode" => ("Joining", "a node to the network"),
            "removingNode" => ("Removing", "the node container and state"),
            "enteringElections" => ("Enabling", "validator participation in future elections"),
            "leavingElections" => ("Disabling", "validator participation in future elections"),
            _ => ("Preparing", action(&operation.kind)),
        };

        self.update(verb, message, operation.progress.as_ref());
    }

    pub(super) fn finish_step(&self, step: &OperationStep, kind: &str) {
        self.bar.finish_and_clear();
        if self.json {
            return;
        }

        let (verb, message) = match step.phase.as_str() {
            "checkingImage" => ("Checked", "Localton Docker image"),
            "pullingImage" => ("Pulled", "Localton Docker image"),
            "startingContainers" => ("Started", "Docker services"),
            "waitingForApis" => ("Checked", "TON APIs and indexer readiness"),
            "stopping" => ("Stopped", "Docker services"),
            "creatingArchive" => ("Saved", "the network snapshot archive"),
            "restoringState" => ("Restored", "blockchain state from the snapshot"),
            "resettingIndexer" => ("Reset", "derived indexer data"),
            "deletingArchive" => ("Deleted", "the snapshot archive"),
            "joiningNode" => ("Joined", "a node to the network"),
            "removingNode" => ("Removed", "the node container and state"),
            "enteringElections" => ("Enabled", "validator participation in future elections"),
            "leavingElections" => ("Disabled", "validator participation in future elections"),
            _ => ("Prepared", action(kind)),
        };
        eprintln!(
            "{} {message} ({:.1}s)",
            label(verb, false),
            step.duration_ms as f64 / 1000.0
        );
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.bar.finish_and_clear();
    }
}

pub(super) fn action(kind: &str) -> &str {
    match kind {
        "start" => "network startup",
        "stop" => "network shutdown",
        "delete" => "network deletion",
        "addNode" => "node creation",
        "removeNode" => "node removal",
        "enterValidation" => "validator election entry",
        "leaveValidation" => "validator election exit",
        "createSnapshot" => "snapshot creation",
        "restoreSnapshot" => "snapshot restoration",
        "deleteSnapshot" => "snapshot deletion",
        _ => kind,
    }
}
