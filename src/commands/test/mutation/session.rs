use anyhow::anyhow;
use chrono::Utc;
use rand::random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MutationStatus {
    Killed,
    Survived,
    CompileError,
}

impl MutationStatus {
    pub(crate) const fn is_killed(self) -> bool {
        matches!(self, Self::Killed)
    }

    pub(crate) const fn is_survived(self) -> bool {
        matches!(self, Self::Survived)
    }

    pub(crate) const fn is_compile_error(self) -> bool {
        matches!(self, Self::CompileError)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MutationRecord {
    pub(crate) id: usize,
    pub(crate) rule_name: String,
    pub(crate) rule_description: String,
    pub(crate) rule_level: String,
    pub(crate) rule_group: String,
    pub(crate) rule_explanation: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) source_path: String,
    pub(crate) code_context: String,
    pub(crate) status: MutationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum MutationSessionEvent {
    SessionStarted {
        session_id: String,
        contract_id: String,
        source_path: String,
        selected_ids: Vec<usize>,
        created_at: String,
    },
    MutationCompleted {
        session_id: String,
        record: MutationRecord,
        completed_at: String,
    },
    SessionFinished {
        session_id: String,
        total_mutants: usize,
        killed: usize,
        survived: usize,
        compile_errors: usize,
        mutation_score: f64,
        minimum_percent: Option<f64>,
        threshold_failed: bool,
        exit_code: i32,
        finished_at: String,
    },
}

pub(crate) struct MutationSessionState {
    pub(crate) session_id: String,
    pub(crate) progress_path: PathBuf,
    pub(crate) selected_ids: BTreeSet<usize>,
    pub(crate) completed_records: Vec<MutationRecord>,
    pub(crate) finished: bool,
    pub(crate) resumed: bool,
}

pub(crate) struct MutationSummary {
    pub(crate) total_mutants: usize,
    pub(crate) killed: usize,
    pub(crate) survived: usize,
    pub(crate) compile_errors: usize,
    pub(crate) mutation_score: f64,
}

pub(crate) fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn generate_mutation_session_id(
    project_root: &Path,
    mutate_contract: &str,
    source_path: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_root.to_string_lossy().as_bytes());
    hasher.update(mutate_contract.as_bytes());
    hasher.update(source_path.as_bytes());
    hasher.update(now_rfc3339().as_bytes());
    hasher.update(process::id().to_string().as_bytes());
    hasher.update(random::<u64>().to_le_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)[..16].to_owned()
}

fn mutation_session_progress_path(project_root: &Path, session_id: &str) -> PathBuf {
    project_root
        .join(".acton")
        .join("mutation-sessions")
        .join(format!("{session_id}.jsonl"))
}

pub(crate) fn append_mutation_session_event(
    progress_path: &Path,
    event: &MutationSessionEvent,
) -> anyhow::Result<()> {
    if let Some(parent) = progress_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(progress_path)?;
    serde_json::to_writer(&mut file, event)?;
    writeln!(file)?;
    Ok(())
}

fn event_session_id(event: &MutationSessionEvent) -> &str {
    match event {
        MutationSessionEvent::SessionStarted { session_id, .. }
        | MutationSessionEvent::MutationCompleted { session_id, .. }
        | MutationSessionEvent::SessionFinished { session_id, .. } => session_id,
    }
}

pub(crate) fn load_or_create_mutation_session(
    project_root: &Path,
    mutate_contract: &str,
    source_path: &str,
    selected_ids: &BTreeSet<usize>,
    requested_session_id: Option<&str>,
) -> anyhow::Result<MutationSessionState> {
    let session_id = requested_session_id.map(str::to_owned).unwrap_or_else(|| {
        generate_mutation_session_id(project_root, mutate_contract, source_path)
    });
    let progress_path = mutation_session_progress_path(project_root, &session_id);
    let selected_ids_sorted = selected_ids.iter().copied().collect::<Vec<_>>();

    if !progress_path.exists() {
        append_mutation_session_event(
            &progress_path,
            &MutationSessionEvent::SessionStarted {
                session_id: session_id.clone(),
                contract_id: mutate_contract.to_owned(),
                source_path: source_path.to_owned(),
                selected_ids: selected_ids_sorted,
                created_at: now_rfc3339(),
            },
        )?;
        return Ok(MutationSessionState {
            session_id,
            progress_path,
            selected_ids: selected_ids.clone(),
            completed_records: Vec::new(),
            finished: false,
            resumed: false,
        });
    }

    let file = fs::File::open(&progress_path)?;
    let reader = BufReader::new(file);
    let mut started_contract_id = None;
    let mut started_source_path = None;
    let mut started_selected_ids = None;
    let mut completed_records = Vec::new();
    let mut completed_ids = BTreeSet::new();
    let mut finished = false;

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: MutationSessionEvent = serde_json::from_str(&line).map_err(|err| {
            anyhow!(
                "Failed to parse mutation session file '{}' at line {}: {err}",
                progress_path.display(),
                line_idx + 1
            )
        })?;
        if event_session_id(&event) != session_id {
            anyhow::bail!(
                "Mutation session file '{}' contains an event for a different session ID",
                progress_path.display()
            );
        }
        match event {
            MutationSessionEvent::SessionStarted {
                contract_id,
                source_path,
                selected_ids,
                ..
            } => {
                if started_contract_id.is_some() {
                    anyhow::bail!(
                        "Mutation session '{}' contains multiple session_started events",
                        session_id
                    );
                }
                started_contract_id = Some(contract_id);
                started_source_path = Some(source_path);
                started_selected_ids = Some(selected_ids);
            }
            MutationSessionEvent::MutationCompleted { record, .. } => {
                if !completed_ids.insert(record.id) {
                    anyhow::bail!(
                        "Mutation session '{}' contains duplicate progress for mutation ID {}",
                        session_id,
                        record.id
                    );
                }
                completed_records.push(record);
            }
            MutationSessionEvent::SessionFinished { .. } => {
                if finished {
                    anyhow::bail!(
                        "Mutation session '{}' contains multiple session_finished events",
                        session_id
                    );
                }
                finished = true;
            }
        }
    }

    let Some(stored_contract_id) = started_contract_id else {
        anyhow::bail!(
            "Mutation session '{}' is missing a session_started event",
            session_id
        );
    };
    let Some(stored_source_path) = started_source_path else {
        anyhow::bail!(
            "Mutation session '{}' is missing source path metadata",
            session_id
        );
    };
    let Some(stored_selected_ids) = started_selected_ids else {
        anyhow::bail!(
            "Mutation session '{}' is missing selected mutation IDs",
            session_id
        );
    };

    if stored_contract_id != mutate_contract
        || stored_source_path != source_path
        || stored_selected_ids != selected_ids_sorted
    {
        anyhow::bail!(
            "Mutation session '{}' does not match the current mutation selection. Re-run with the same contract and mutation filters used to create the session",
            session_id
        );
    }

    let stored_selected_ids_set = stored_selected_ids.iter().copied().collect::<BTreeSet<_>>();
    if completed_records
        .iter()
        .any(|record| !stored_selected_ids_set.contains(&record.id))
    {
        anyhow::bail!(
            "Mutation session '{}' contains progress entries for mutation IDs outside the selected set",
            session_id
        );
    }

    Ok(MutationSessionState {
        session_id,
        progress_path,
        selected_ids: stored_selected_ids_set,
        completed_records,
        finished,
        resumed: true,
    })
}

pub(crate) fn mutation_summary(records: &[MutationRecord]) -> MutationSummary {
    let compile_errors = records
        .iter()
        .filter(|record| record.status.is_compile_error())
        .count();
    let killed = records
        .iter()
        .filter(|record| record.status.is_killed())
        .count();
    let survived = records
        .iter()
        .filter(|record| record.status.is_survived())
        .count();
    let scored_total = killed + survived;
    let mutation_score = if scored_total > 0 {
        (killed as f64 / scored_total as f64) * 100.0
    } else {
        0.0
    };

    MutationSummary {
        total_mutants: records.len(),
        killed,
        survived,
        compile_errors,
        mutation_score,
    }
}
